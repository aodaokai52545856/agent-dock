# Letta (formerly MemGPT) — Technical Research Report

**Scope:** memory-block edit tools · `request_heartbeat` · sleep-time agents · prompt caching · archival memory storage
**Primary source pinned commit:** `67013ef1bb491ec8bdd629bfbeceb70227936afa`
**Pinned-commit raw URL prefix:** `https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/`
**Repository:** https://github.com/letta-ai/letta

## 0. Method & reliability notes

- All `letta/**` paths below were fetched successfully as **raw** files at the pinned SHA. Unless a URL says otherwise, it is a pinned-SHA `raw.githubusercontent.com` URL.
- **`main` has moved `letta/functions/`.** `https://raw.githubusercontent.com/letta-ai/letta/main/letta/functions/function_sets/base.py` returns **HTTP 404**, while the pinned-SHA equivalent returns HTTP 200. The pinned-SHA tree does contain `.github/workflows/letta-code-sync.yml`, consistent with a split-out harness repo (`letta-ai/letta-code`). **NOT VERIFIED:** I did not determine the new `main`-branch path for `base.py`.
- Claims are labelled **[VERIFIED]** (read directly in source, quoted), **[PARTIAL]**, or **NOT VERIFIED**.
- No source line numbers are given: raw fetches do not carry them and I did not use the blob view. Function/attribute names are given instead so every quote is greppable.
- `letta/embeddings.py` **does not exist** at this SHA (HTTP 404).
- The GitHub REST API hit a rate limit mid-research (`HTTP 403 API rate limit exceeded`), so one directory listing (`letta/services`) was never obtained.

---

## 1. Memory-block edit tools

**File:** `letta/functions/function_sets/base.py`
**URL:** https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/functions/function_sets/base.py **[VERIFIED]**

Module-level import in that file:
```python
from letta.constants import CORE_MEMORY_LINE_NUMBER_WARNING
```

### 1.1 Exact signatures **[VERIFIED]**

All of the following are defined **in `base.py`** (not split across files):

```python
def core_memory_append(agent_state: "AgentState", label: str, content: str) -> Optional[str]:  # type: ignore
def core_memory_replace(agent_state: "AgentState", label: str, old_content: str, new_content: str) -> Optional[str]:  # type: ignore
def rethink_memory(agent_state: "AgentState", new_memory: str, target_block_label: str) -> None
def memory_replace(agent_state: "AgentState", label: str, old_str: str, new_str: str) -> str:  # type: ignore
def memory_insert(agent_state: "AgentState", label: str, new_str: str, insert_line: int = -1) -> Optional[str]:  # type: ignore
def memory_apply_patch(agent_state: "AgentState", label: str, patch: str) -> str:  # type: ignore
def memory_rethink(agent_state: "AgentState", label: str, new_memory: str) -> None
def memory_finish_edits(agent_state: "AgentState") -> None:  # type: ignore
def memory(agent_state: "AgentState", command: str, path: Optional[str] = None, file_text: Optional[str] = None,
           description: Optional[str] = None, old_str: Optional[str] = None, new_str: Optional[str] = None,
           insert_line: Optional[int] = None, insert_text: Optional[str] = None,
           old_path: Optional[str] = None, new_path: Optional[str] = None) -> Optional[str]
def send_message(self: "Agent", message: str) -> Optional[str]
def conversation_search(self: "Agent", query: str,
                        roles: Optional[List[Literal["assistant", "user", "tool"]]] = None,
                        limit: Optional[int] = None, start_date: Optional[str] = None,
                        end_date: Optional[str] = None) -> Optional[str]
async def archival_memory_insert(self: "Agent", content: str, tags: Optional[list[str]] = None) -> Optional[str]
async def archival_memory_search(self: "Agent", query: str, tags: Optional[list[str]] = None,
                                 tag_match_mode: Literal["any", "all"] = "any",
                                 top_k: Optional[int] = None, start_datetime: Optional[str] = None,
                                 end_datetime: Optional[str] = None) -> Optional[str]
```

Note the naming trap: there are **two** "rethink" functions — `rethink_memory(agent_state, new_memory, target_block_label)` (older, uses `target_block_label`) and `memory_rethink(agent_state, label, new_memory)` (newer, uses `label`).

### 1.2 Which are stubs vs. implemented **[VERIFIED]**

Implemented with real bodies: `core_memory_append`, `core_memory_replace`, `rethink_memory`, `memory_replace`, `memory_insert`, `memory_rethink`, `memory_finish_edits` (returns `None`), `send_message`, `conversation_search`.

Stubs that raise at call time (the real work happens in the tool executor / agent loop, not the function body):
```python
raise NotImplementedError("This should never be invoked directly. Contact Letta if you see this error message.")
```
used by: `memory`, `archival_memory_insert`, `archival_memory_search`, `memory_apply_patch`.

### 1.3 Docstrings (verbatim) **[VERIFIED]**

`core_memory_append`:
```
Append to the contents of core memory.

Args:
    label (str): Section of the memory to be edited.
    content (str): Content to write to the memory. All unicode (including emojis) are supported.

Returns:
    Optional[str]: None is always returned as this function does not produce a response.
```

`core_memory_replace`:
```
Replace the contents of core memory. To delete memories, use an empty string for new_content.

Args:
    label (str): Section of the memory to be edited.
    old_content (str): String to replace. Must be an exact match.
    new_content (str): Content to write to the memory. All unicode (including emojis) are supported.

Returns:
    Optional[str]: None is always returned as this function does not produce a response.
```
(Body raises `ValueError(f"Old content '{old_content}' not found in memory block '{label}'")` if `old_content not in current_value`.)

`memory_replace`:
```
The memory_replace command allows you to replace a specific string in a memory block with a new string. This is used for making precise edits.
Do NOT attempt to replace long strings, e.g. do not attempt to replace the entire contents of a memory block with a new string.

Args:
    label (str): Section of the memory to be edited, identified by its label.
    old_str (str): The text to replace (must match exactly, including whitespace and indentation).
    new_str (str): The new text to insert in place of the old text. Do not include line number prefixes.
```
Its head comment names the inspiration:
```python
# Based off of: https://github.com/anthropics/anthropic-quickstarts/blob/main/computer-use-demo/computer_use_demo/tools/edit.py?ref=musings.yasyf.com#L154
```
It rejects line-number prefixes via `re.search(r"\nLine \d+: ", old_str)` and `CORE_MEMORY_LINE_NUMBER_WARNING`, expands tabs, and fails on 0 or >1 occurrences.

`memory_insert`:
```
The memory_insert command allows you to insert text at a specific location in a memory block.

Args:
    label (str): Section of the memory to be edited, identified by its label.
    new_str (str): The text to insert. Do not include line number prefixes.
    insert_line (int): The line number after which to insert the text (0 for beginning of file). Defaults to -1 (end of the file).
```
Module-level helper used by both: `SNIPPET_LINES: int = 4`.

`memory_rethink`:
```
The memory_rethink command allows you to completely rewrite the contents of a memory block. Use this tool to make large sweeping changes (e.g. when you want to condense or reorganize the memory blocks), do NOT use this tool to make small precise edits (e.g. add or remove a line, replace a specific string, etc).

Args:
    label (str): The memory block to be rewritten, identified by its label.
    new_memory (str): The new memory contents with information integrated from existing memory blocks and the conversation context.
```

`memory_finish_edits`:
```
Call the memory_finish_edits command when you are finished making edits (integrating all new information) into the memory blocks. This function is called when the agent is done rethinking the memory.
```

`send_message`:
```
Sends a message to the human user.

Args:
    message (str): Message contents. All unicode (including emojis) are supported.

Returns:
    Optional[str]: None is always returned as this function does not produce a response.
```

`conversation_search`:
```
Search prior conversation history using hybrid search (text + semantic similarity).

Args:
    query (str): String to search for using both text matching and semantic similarity.
    roles (Optional[List[Literal["assistant", "user", "tool"]]]): Optional list of message roles to filter by.
    limit (Optional[int]): Maximum number of results to return. Uses system default if not specified.
    start_date (Optional[str]): Filter results to messages created on or after this date (INCLUSIVE). ...
    end_date (Optional[str]): Filter results to messages created on or before this date (INCLUSIVE). ...
```
It defaults `limit` to `RETRIEVAL_QUERY_DEFAULT_PAGE_SIZE` and returns `"No results found."` or `f"Found {len(messages)} results: {json_dumps(results_formatted)}"` where each entry is `{"role": ..., "content": ...}`.

`archival_memory_insert`:
```
Add information to long-term archival memory for later retrieval.

Use this tool to store facts, knowledge, or context that you want to remember
across all future conversations. Archival memory is permanent and searchable by
semantic similarity.
...
Args:
    content: The information to store. Should be clear and self-contained.
    tags: Optional list of category tags (e.g., ["meetings", "project-updates"])
```

`archival_memory_search`:
```
Search archival memory using semantic similarity to find relevant information.
...
Args:
    query: What you're looking for, described naturally (e.g., "meetings about API redesign")
    tags: Filter to memories with these tags. Use tag_match_mode to control matching.
    tag_match_mode: "any" = match memories with ANY of the tags, "all" = match only memories with ALL tags
    start_datetime: Only return memories created after this time (ISO 8601: "2024-01-15" or "2024-01-15T14:30")
    end_datetime: Only return memories created before this time (ISO 8601 format)
    top_k: Maximum number of results to return (default: 10)
```

`memory` (omni-tool) documents sub-commands `"create" | "str_replace" | "insert" | "delete" | "rename"`.

**Correction to the brief:** `extras.py` (`letta/functions/function_sets/extras.py`, https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/functions/function_sets/extras.py) does **not** contain any memory tool. It contains only `message_chatgpt`, `read_from_text_file`, `append_to_text_file`, `http_request` **[VERIFIED]**.

### 1.4 The legacy/deprecated mechanism — `letta/constants.py` **[VERIFIED]**

**URL:** https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/constants.py

```python
SEND_MESSAGE_TOOL_NAME = "send_message"
# Base tools that cannot be edited, as they access agent state directly
# Note that we don't include "conversation_search_date" for now
BASE_TOOLS = [SEND_MESSAGE_TOOL_NAME, "conversation_search", "archival_memory_insert", "archival_memory_search"]
DEPRECATED_LETTA_TOOLS = ["archival_memory_insert", "archival_memory_search"]
# Base memory tools CAN be edited, and are added by default by the server
BASE_MEMORY_TOOLS = ["core_memory_append", "core_memory_replace", "memory", "memory_apply_patch"]
# New v2 collection of the base memory tools (effecitvely same as sleeptime set), to pair with memgpt_v2 prompt
BASE_MEMORY_TOOLS_V2 = [
    "memory_replace",
    "memory_insert",
    # NOTE: leaving these ones out to simply the set? Can have these reserved for sleep-time
    # "memory_rethink",
    # "memory_finish_edits",
]

# v3 collection, currently just a omni memory tool for anthropic
BASE_MEMORY_TOOLS_V3 = [
    "memory",
]
# Base tools if the memgpt agent has enable_sleeptime on
BASE_SLEEPTIME_CHAT_TOOLS = [SEND_MESSAGE_TOOL_NAME, "conversation_search", "archival_memory_search"]
# Base memory tools for sleeptime agent
BASE_SLEEPTIME_TOOLS = [
    "memory_replace",
    "memory_insert",
    "memory_rethink",
    "memory_finish_edits",
    # "archival_memory_insert",
    # "archival_memory_search",
    # "conversation_search",
]
# Base tools for the voice agent
BASE_VOICE_SLEEPTIME_CHAT_TOOLS = [SEND_MESSAGE_TOOL_NAME, "search_memory"]
# Base memory tools for sleeptime agent
BASE_VOICE_SLEEPTIME_TOOLS = [
    "store_memories",
    "rethink_user_memory",
    "finish_rethinking_memory",
]
```

Module-to-tool-type mapping **[VERIFIED]**:
```python
LETTA_CORE_TOOL_MODULE_NAME = "letta.functions.function_sets.base"
LETTA_MULTI_AGENT_TOOL_MODULE_NAME = "letta.functions.function_sets.multi_agent"
LETTA_VOICE_TOOL_MODULE_NAME = "letta.functions.function_sets.voice"
LETTA_BUILTIN_TOOL_MODULE_NAME = "letta.functions.function_sets.builtin"
LETTA_FILES_TOOL_MODULE_NAME = "letta.functions.function_sets.files"
```

### 1.5 The replacement relationship — **both** an omni-tool and a v2 pair **[VERIFIED]**

The exact function that computes the default toolset:
**File:** `letta/services/helpers/agent_manager_helper.py`
**URL:** https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/services/helpers/agent_manager_helper.py

```python
def calculate_base_tools(is_v2: bool) -> Set[str]:
    if is_v2:
        return (set(BASE_TOOLS) - set(DEPRECATED_LETTA_TOOLS)) | set(BASE_MEMORY_TOOLS_V2)
    else:
        return (set(BASE_TOOLS) - set(DEPRECATED_LETTA_TOOLS)) | set(BASE_MEMORY_TOOLS)
```

So `DEPRECATED_LETTA_TOOLS` is **subtracted** from `BASE_TOOLS` in *both* branches. This is the only place `DEPRECATED_LETTA_TOOLS` is consumed (it is imported by `letta/services/agent_manager.py` and by `agent_manager_helper.py`; the grep of both files shows no other use) **[VERIFIED for those two files]**.

Consequences, stated exactly:
- `archival_memory_insert` and `archival_memory_search` **are the tools explicitly tagged as deprecated** by `DEPRECATED_LETTA_TOOLS`, and they are removed from the default toolset of *every* agent created via `calculate_base_tools` (both v1/`memgpt_agent` and v2 paths). They remain defined in `base.py` and remain in the `BASE_SLEEPTIME_CHAT_TOOLS` list, so the *sleeptime-primary* configuration still gets `archival_memory_search`.
- `core_memory_append` + `core_memory_replace` are **not** listed in any `DEPRECATED_*` constant. Their replacement is structural:
  - **v2 path** (`AgentType.memgpt_v2_agent`, and also `AgentType.letta_v1_agent`): replaced by the **pair** `memory_replace` + `memory_insert` (`BASE_MEMORY_TOOLS_V2`).
  - **v3 path**: replaced by the **single omni-tool** `memory` (`BASE_MEMORY_TOOLS_V3`, comment: *"v3 collection, currently just a omni memory tool for anthropic"*).
- `core_memory_append`/`core_memory_replace` remain reachable only through the catch-all `else` branch (`tool_names |= calculate_base_tools(is_v2=False)`, i.e. the default `memgpt_agent` type) **[VERIFIED]**.

`ToolType` enum — **`letta/schemas/enums.py`**, https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/schemas/enums.py **[VERIFIED]**:
```python
class ToolType(str, Enum):
    CUSTOM = "custom"
    LETTA_CORE = "letta_core"
    LETTA_MEMORY_CORE = "letta_memory_core"
    LETTA_MULTI_AGENT_CORE = "letta_multi_agent_core"
    LETTA_SLEEPTIME_CORE = "letta_sleeptime_core"
    LETTA_VOICE_SLEEPTIME_CORE = "letta_voice_sleeptime_core"
    LETTA_BUILTIN = "letta_builtin"
    LETTA_FILES_CORE = "letta_files_core"
    EXTERNAL_LANGCHAIN = "external_langchain"  # DEPRECATED
    EXTERNAL_COMPOSIO = "external_composio"  # DEPRECATED
    # TODO is "external" the right name here? Since as of now, MCP is local / doesn't support remote?
    EXTERNAL_MCP = "external_mcp"
```
In `letta/schemas/tool.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/schemas/tool.py) the schema for `LETTA_CORE`, `LETTA_MEMORY_CORE` **and** `LETTA_SLEEPTIME_CORE` is all generated from the *same* module:
```python
elif self.tool_type in {ToolType.LETTA_CORE, ToolType.LETTA_MEMORY_CORE, ToolType.LETTA_SLEEPTIME_CORE}:
    # If it's letta core tool, we generate the json_schema on the fly here
    self.json_schema = get_json_schema_from_module(module_name=LETTA_CORE_TOOL_MODULE_NAME, function_name=self.name)
```
**[VERIFIED]** — there is no separate `LETTA_SLEEPTIME_TOOL_MODULE_NAME`; sleeptime memory tools live in `base.py`.

There is **no `LETTA_CORE_TOOL_NAMES` list** and **no `deprecated` Python decorator applied to these memory functions** in `base.py`. The `deprecated` decorator does exist in `letta/helpers/decorators.py` and is applied to `AnthropicClient.request`:
```python
@trace_method
@deprecated("Synchronous version of this is no longer valid. Will result in model_dump of coroutine")
def request(self, request_data: dict, llm_config: LLMConfig) -> dict:
```
**[VERIFIED]** — i.e. `deprecated` exists as a mechanism but is **not** used to mark `core_memory_append`/`core_memory_replace`.

⚠️ **NOT VERIFIED:** whether any *other* repo file (e.g. docs, generated SDK, or a migration) declares `core_memory_append`/`core_memory_replace` formally deprecated. Repo-wide code search was not available (GitHub code-search API needs auth; grep.app returned HTTP 429).

---

## 2. `request_heartbeat`

### 2.1 It still exists as a constant and in the legacy loop **[VERIFIED]**

`letta/constants.py`, https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/constants.py:
```python
PRE_EXECUTION_MESSAGE_ARG = "pre_exec_msg"

REQUEST_HEARTBEAT_PARAM = "request_heartbeat"
REQUEST_HEARTBEAT_DESCRIPTION = "Request an immediate heartbeat after function execution. You MUST set this value to `True` if you want to send a follow-up message or run a follow-up tool call (chain multiple tools together). If set to `False` (the default), then the chain of execution will end immediately after this function call."
```
Also in the same file:
```python
# REQ_HEARTBEAT_MESSAGE = f"{NON_USER_MSG_PREFIX}request_heartbeat == true"
REQ_HEARTBEAT_MESSAGE = f"{NON_USER_MSG_PREFIX}Function called using request_heartbeat=true, returning control"
FUNC_FAILED_HEARTBEAT_MESSAGE = f"{NON_USER_MSG_PREFIX}Function call failed, returning control"
```
and `MAX_PAUSE_HEARTBEATS = 360  # in min`.

There is no `ToolType` member for heartbeats and no `LEGACY` tool tag for it.

### 2.2 Where it is still honoured: the legacy loops **[VERIFIED]**

`letta/agents/letta_agent.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/agents/letta_agent.py) — used by the older `LettaAgent` path:
```python
def _decide_continuation(
    self,
    agent_state: AgentState,
    request_heartbeat: bool,
    tool_call_name: str,
    tool_rule_violated: bool,
    tool_rules_solver: ToolRulesSolver,
    is_final_step: bool | None,
) -> tuple[bool, str | None, LettaStopReason | None]:
    continue_stepping = request_heartbeat
```
and it injects the param into tool schemas:
```python
allowed_tools = runtime_override_tool_json_schema(
    tool_list=allowed_tools, response_format=agent_state.response_format, request_heartbeat=True, terminal_tools=terminal_tool_names
)
```
`_handle_ai_response` reads it with `request_heartbeat: bool = _pop_heartbeat(tool_args)`.

`letta/agents/letta_agent_v2.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/agents/letta_agent_v2.py) — class `LettaAgentV2` — is identical in this respect.

### 2.3 The current mechanism that replaced it **[VERIFIED]**

**`AgentType` comment — `letta/schemas/enums.py` and duplicated in `letta/schemas/agent.py`:**
```python
letta_v1_agent = "letta_v1_agent"  # simplification of the memgpt loop, no heartbeats or forced tool calls
```
(Note the in-file comment above the duplicated enum in `schemas/agent.py`: *"TODO: Remove this upon next OSS release, there's a duplicate AgentType in enums"* — https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/schemas/agent.py)

**`letta/agents/letta_agent_v3.py`** (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/agents/letta_agent_v3.py) — class `LettaAgentV3(LettaAgentV2)`, docstring:
```
Similar to V2, but stripped down / simplified, while also generalized:
* Supports non-tool returns
* No inner thoughts in kwargs
* No heartbeats (loops happen on tool calls)
```
Its `_get_valid_tools()`:
```python
allowed_tools = runtime_override_tool_json_schema(
    tool_list=allowed_tools,
    response_format=self.agent_state.response_format,
    request_heartbeat=False,  # NOTE: difference for v3 (don't add request heartbeat)
    terminal_tools=terminal_tool_names,
)
```
Its `_handle_ai_response` strips the param rather than reading it:
```python
args.pop(REQUEST_HEARTBEAT_PARAM, None)
args.pop(INNER_THOUGHTS_KWARG, None)
```
and its `_decide_continuation` **has no `request_heartbeat` parameter at all**:
```python
def _decide_continuation(
    self,
    agent_state: AgentState,
    tool_call_name: Optional[str],
    tool_rule_violated: bool,
    tool_rules_solver: ToolRulesSolver,
    is_final_step: bool | None,
    finish_reason: str | None = None,
) -> tuple[bool, str | None, LettaStopReason | None]:
    """
    In v3 loop, we apply the following rules:

    1. Did not call a tool? Loop ends

    2. Called a tool? Loop continues. This can be:
       2a. Called tool, tool executed successfully
       2b. Called tool, tool failed to execute
       2c. Called tool + tool rule violation (did not execute)
    """
    continue_stepping = True  # Default continue
```

**The replacement is therefore threefold, all verified:**
1. **Loop-on-any-tool-call**: in v3, calling any tool continues the loop; ending the turn requires *not* calling a tool (`if tool_call_name is None: ... return False, None, LettaStopReason(stop_reason=StopReasonType.end_turn.value)`).
2. **`max_steps`**: `DEFAULT_MAX_STEPS = 50` in `letta/constants.py`; loop is `for i in range(max_steps)`; hitting the last step sets `StopReasonType.max_steps.value`.
3. **Tool rules** (the `ToolRule` system) — `letta/schemas/enums.py`:
```python
class ToolRuleType(str, Enum):
    run_first = "run_first"
    exit_loop = "exit_loop"  # reasoning loop should exit
    continue_loop = "continue_loop"
    conditional = "conditional"
    constrain_child_tools = "constrain_child_tools"
    max_count_per_step = "max_count_per_step"
    parent_last_tool = "parent_last_tool"
    required_before_exit = "required_before_exit"  # tool must be called before loop can exit
    requires_approval = "requires_approval"
```
and in `AgentManager.create_agent_async`:
```python
if should_add_base_tool_rules:
    for tn in tool_names:
        if tn in {"send_message", "send_message_to_agent_async", "memory_finish_edits"}:
            tool_rules.append(TerminalToolRule(tool_name=tn))
        elif tn in (BASE_TOOLS + BASE_MEMORY_TOOLS + BASE_MEMORY_TOOLS_V2 + BASE_MEMORY_TOOLS_V3 + BASE_SLEEPTIME_TOOLS):
            tool_rules.append(ContinueToolRule(tool_name=tn))
```
So `ContinueToolRule` is what replaces the old "`request_heartbeat=True`" idiom for memory tools, and `TerminalToolRule` replaces "don't continue after `send_message`".

### 2.4 Heartbeats are actively stripped for `letta_v1_agent` **[VERIFIED]**

`letta/llm_api/anthropic_client.py`, https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/anthropic_client.py:
```python
        data["messages"] = PydanticMessage.to_anthropic_dicts_from_list(
            messages=messages[1:],
            current_model=llm_config.model,
            inner_thoughts_xml_tag=inner_thoughts_xml_tag,
            put_inner_thoughts_in_kwargs=put_kwargs,
            # if react, use native content + strip heartbeats
            native_content=is_v1,
            strip_request_heartbeat=is_v1,
            tool_return_truncation_chars=tool_return_truncation_chars,
        )
        ...
        if agent_type == AgentType.letta_v1_agent:
            # Both drop heartbeats in the payload
            data["messages"] = drop_heartbeats(data["messages"])
            # And drop heartbeats in the tools
            if "tools" in data:
                for tool in data["tools"]:
                    tool["input_schema"]["properties"].pop(REQUEST_HEARTBEAT_PARAM, None)
                    if "required" in tool["input_schema"] and REQUEST_HEARTBEAT_PARAM in tool["input_schema"]["required"]:
                        # NOTE: required is not always present
                        tool["input_schema"]["required"].remove(REQUEST_HEARTBEAT_PARAM)
```
Helpers in the same file: `is_heartbeat(message, is_ping=False)`, `drop_heartbeats(messages)`, `merge_heartbeats_into_tool_responses(messages)` ("*For extended thinking mode, we don't want anything other than tool responses in-between assistant actions*").

`letta/llm_api/openai_client.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/openai_client.py) does the same strip, with the comment:
```python
        # For some reason, request heartbeats are still leaking into here...
        # So strip them manually for v3
        if agent_type == AgentType.letta_v1_agent:
            new_tools = []
            if data.tools:
                for tool in data.tools:
                    # Remove request_heartbeat from the properties if it exists
                    if tool.function.parameters and "properties" in tool.function.parameters:
                        tool.function.parameters["properties"].pop(REQUEST_HEARTBEAT_PARAM, None)
```
It is also stripped for Gemini/Vertex (via `llm_config.put_inner_thoughts_in_kwargs = False` and `native_content=True` in `letta/llm_api/google_vertex_client.py`, https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/google_vertex_client.py) **[VERIFIED]**, though no `pop(REQUEST_HEARTBEAT_PARAM)` call appears there.

Residual legacy traces **[VERIFIED]**: `letta/constants.py` still contains the (now commented-out) heartbeat reference in `MESSAGE_SUMMARY_WARNING_STR`:
```python
        "If there is any important new information or general memories about you or the user that you would like to save, you should save that information immediately by calling function core_memory_append, core_memory_replace, or archival_memory_insert.",
        # "Remember to pass request_heartbeat = true if you would like to send a message immediately after.",
```

**Doc reference:** `https://docs.letta.com/guides/legacy/architectures-overview` and `https://docs.letta.com/guides/legacy/migration-guide/` were returned by search as the legacy-architecture pages for this topic. I fetched neither page body, so I do **not** quote them — **NOT VERIFIED at doc level** (code-level evidence above is complete and unambiguous).

---

## 3. Sleep-time agents

### 3.1 Agent types **[VERIFIED]**

`letta/schemas/enums.py` **and** duplicated in `letta/schemas/agent.py`:
```python
class AgentType(str, Enum):
    """
    Enum to represent the type of agent.
    """

    memgpt_agent = "memgpt_agent"  # the OG set of memgpt tools
    memgpt_v2_agent = "memgpt_v2_agent"  # memgpt style tools, but refreshed
    letta_v1_agent = "letta_v1_agent"  # simplification of the memgpt loop, no heartbeats or forced tool calls
    react_agent = "react_agent"  # basic react agent, no memory tools
    workflow_agent = "workflow_agent"  # workflow with auto-clearing message buffer
    split_thread_agent = "split_thread_agent"
    sleeptime_agent = "sleeptime_agent"
    voice_convo_agent = "voice_convo_agent"
    voice_sleeptime_agent = "voice_sleeptime_agent"
```
URLs: https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/schemas/enums.py and https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/schemas/agent.py

For the agent types in the brief: `sleeptime_agent` ✔, `letta_v1_agent` ✔, `memgpt_v2_agent` ✔, `react_agent` ✔, `voice_convo_agent` ✔, `workflow_agent` ✔. Additionally present: `memgpt_agent`, `split_thread_agent`, `voice_sleeptime_agent`.

Default agent type on creation: `agent_type: AgentType = Field(default_factory=lambda: AgentType.letta_v1_agent, description="The type of agent.")` **[VERIFIED]**.

### 3.2 Config fields: `enable_sleeptime` (agent) vs `sleeptime_agent_frequency` (group) **[VERIFIED]**

`letta/schemas/agent.py` — on `AgentState`, `CreateAgent`, `UpdateAgent`:
```python
    enable_sleeptime: Optional[bool] = Field(
        None,
        description="If set to True, memory management will move to a background agent thread.",
    )
```
There is **no `sleeptime_agent_frequency` field on the agent** and **no `sleeptime_agent` field on the agent**. ⚠️ **Correction to the brief:** `sleeptime_agent_frequency` is a *group* field.

Validation on `CreateAgent`:
```python
    @model_validator(mode="after")
    def validate_sleeptime_for_agent_type(self) -> "CreateAgent":
        """Validate that enable_sleeptime is True when agent_type is a specific value"""
        AGENT_TYPES_REQUIRING_SLEEPTIME = {AgentType.voice_convo_agent}

        if self.agent_type in AGENT_TYPES_REQUIRING_SLEEPTIME:
            if not self.enable_sleeptime:
                raise ValueError(f"Agent type {self.agent_type} requires enable_sleeptime to be True")

        return self
```
On update:
```python
            if agent_update.enable_sleeptime and agent_update.system is None:
                ...
                    enable_sleeptime=agent_update.enable_sleeptime,
```
(i.e. toggling `enable_sleeptime` regenerates the system prompt via `derive_system_message`.)

`letta/schemas/group.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/schemas/group.py):
```python
class Group(GroupBase):
    ...
    sleeptime_agent_frequency: Optional[int] = Field(None, description="")
    turns_counter: Optional[int] = Field(None, description="")
    last_processed_message_id: Optional[str] = Field(None, description="")
    max_message_buffer_length: Optional[int] = Field(...)
    min_message_buffer_length: Optional[int] = Field(...)
```
and the discriminated config:
```python
class SleeptimeManager(ManagerConfig):
    manager_type: Literal[ManagerType.sleeptime] = Field(ManagerType.sleeptime, description="")
    manager_agent_id: str = Field(..., description="")
    sleeptime_agent_frequency: Optional[int] = Field(None, description="")
```

### 3.3 Group manager types **[VERIFIED]** — no `manager` member

`letta/schemas/group.py`:
```python
class ManagerType(str, Enum):
    round_robin = "round_robin"
    supervisor = "supervisor"
    dynamic = "dynamic"
    sleeptime = "sleeptime"
    voice_sleeptime = "voice_sleeptime"
    swarm = "swarm"
```
**NOT VERIFIED / does not exist:** there is no `ManagerType.manager` and no `manager` group type. There is no `SleeptimeAgent` group type. `swarm` is declared in the enum but its class is commented out:
```python
# class SwarmGroup(ManagerConfig):
#   manager_type: Literal[ManagerType.swarm] = Field(ManagerType.swarm, description="")
```
The dispatch property is `Group.manager_config`, a `match` on `self.manager_type` returning `RoundRobinManager` / `SupervisorManager` / `DynamicManager` / `SleeptimeManager` / `VoiceSleeptimeManager`. `ManagerConfigUnion` and `ManagerConfigUpdateUnion` are `Annotated[Union[...], Field(discriminator="manager_type")]` over exactly those five.

Also note `shared_block_ids: List[str] = Field([], description="", deprecated=True)` on `Group`/`GroupCreate`/`GroupUpdate`. **[VERIFIED]**

### 3.4 The sleeptime orchestration classes **[VERIFIED]**

Files present (from the pinned-SHA git tree listing):
- `letta/groups/sleeptime_multi_agent.py` — class `SleeptimeMultiAgent(BaseAgent)`
- `letta/groups/sleeptime_multi_agent_v2.py`
- `letta/groups/sleeptime_multi_agent_v3.py`
- `letta/groups/sleeptime_multi_agent_v4.py` — class `SleeptimeMultiAgentV4(LettaAgentV3)`
- also `letta/groups/dynamic_multi_agent.py`, `round_robin_multi_agent.py`, `supervisor_multi_agent.py`, `helpers.py`

`SleeptimeMultiAgent` constructor (v1):
```python
class SleeptimeMultiAgent(BaseAgent):
    def __init__(
        self,
        interface: AgentInterface,
        agent_state: AgentState,
        user: User,
        # custom
        group_id: str = "",
        agent_ids: List[str] = [],
        description: str = "",
        sleeptime_agent_frequency: Optional[int] = None,
    ):
```
Its `step()` runs the main agent, bumps the turn counter, and fires background tasks:
```python
            # Update turns counter
            turns_counter = None
            if self.sleeptime_agent_frequency is not None and self.sleeptime_agent_frequency > 0:
                turns_counter = self.group_manager.bump_turns_counter(group_id=self.group_id, actor=self.user)

            # Perform participant steps
            if self.sleeptime_agent_frequency is None or (
                turns_counter is not None and turns_counter % self.sleeptime_agent_frequency == 0
            ):
                ...
                for participant_agent_id in self.agent_ids:
                    try:
                        run_id = self._issue_background_task(...)
```
and it uses a raw thread (`_run_async_in_new_thread`) plus `Run`/`JobManager` records. `step_stream` raises:
```python
raise NotImplementedError("Streaming is not supported for SleeptimeMultiAgent. Use the non-streaming step() method instead.")
```
URL: https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/groups/sleeptime_multi_agent.py

`SleeptimeMultiAgentV4`:
```python
class SleeptimeMultiAgentV4(LettaAgentV3):
    def __init__(self, agent_state: AgentState, actor: User, group: Group):
        super().__init__(agent_state, actor)
        assert group.manager_type == ManagerType.sleeptime, f"Expected group type to be 'sleeptime', got {group.manager_type}"
```
Its `_participant_agent_step` builds the transcript prompt — this is the exact textual definition of the sleep-time agent's role:
```python
            message_text = (
                "<system-reminder>\n"
                "You are a sleeptime agent - a background agent that asynchronously processes conversations after they occur.\n\n"
                "IMPORTANT: You are NOT the primary agent. You are reviewing a conversation that already happened between a primary agent and its user:\n"
                '- Messages labeled "assistant" are from the primary agent (not you)\n'
                '- Messages labeled "user" are from the primary agent\'s user\n\n'
                "Your primary role is memory management. Review the conversation and use your memory tools to update any relevant memory blocks with information worth preserving. "
                "Check your memory_persona block for any additional instructions or policies.\n"
                "</system-reminder>\n\n"
                f"Messages:\n{messages_text}"
            )
```
and it instantiates the sleep-time agent explicitly as V3:
```python
            sleeptime_agent = LettaAgentV3(
                agent_state=sleeptime_agent_state,
                actor=self.actor,
            )
```
URL: https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/groups/sleeptime_multi_agent_v4.py

Routing to v4 is in the REST layer — `letta/server/rest_api/routers/v1/agents.py`:
```python
    agent_eligible = agent.multi_agent_group is None or agent.multi_agent_group.manager_type in ["sleeptime", "voice_sleeptime"]
    ...
    if agent_eligible and model_compatible:
        agent_loop = AgentLoop.load(agent_state=agent, actor=actor)
```
and the `capture_messages` endpoint:
```python
    sleeptime_group = agent.multi_agent_group if agent.multi_agent_group and agent.multi_agent_group.manager_type == "sleeptime" else None
    if sleeptime_group:
        sleeptime_agent_loop = SleeptimeMultiAgentV4(agent_state=agent, actor=actor, group=sleeptime_group)
        sleeptime_agent_loop.response_messages = response_messages
        run_ids = await sleeptime_agent_loop.run_sleeptime_agents()
```
URL: https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/server/rest_api/routers/v1/agents.py

### 3.5 ⭐ The primary agent LOSES the memory-edit tools — exact code **[VERIFIED]**

**File:** `letta/services/agent_manager.py` — `AgentManager.create_agent_async`
**URL:** https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/services/agent_manager.py

```python
        # tools
        tool_names = set(agent_create.tools or [])
        if agent_create.include_base_tools:
            if agent_create.agent_type == AgentType.voice_sleeptime_agent:
                tool_names |= set(BASE_VOICE_SLEEPTIME_TOOLS)
                # NOTE: also overwrite initial message sequence to empty by default
                if agent_create.initial_message_sequence is None:
                    agent_create.initial_message_sequence = []
            elif agent_create.agent_type == AgentType.voice_convo_agent:
                tool_names |= set(BASE_VOICE_SLEEPTIME_CHAT_TOOLS)
            elif agent_create.agent_type == AgentType.sleeptime_agent:
                tool_names |= set(BASE_SLEEPTIME_TOOLS)
                # NOTE: also overwrite initial message sequence to empty by default
                if agent_create.initial_message_sequence is None:
                    agent_create.initial_message_sequence = []
            elif agent_create.enable_sleeptime:
                tool_names |= set(BASE_SLEEPTIME_CHAT_TOOLS)
            elif agent_create.agent_type == AgentType.memgpt_v2_agent:
                tool_names |= calculate_base_tools(is_v2=True)
            elif agent_create.agent_type == AgentType.react_agent:
                pass  # no default tools
            elif agent_create.agent_type == AgentType.letta_v1_agent:
                tool_names |= calculate_base_tools(is_v2=True)
                # Remove `send_message` if it exists
                tool_names.discard("send_message")
                # NOTE: also overwriting inner_thoughts_in_kwargs to force False
                agent_create.llm_config.put_inner_thoughts_in_kwargs = False
                # NOTE: also overwrite initial message sequence to empty by default
                if agent_create.initial_message_sequence is None:
                    agent_create.initial_message_sequence = []
                # NOTE: default to no base tool rules unless explicitly provided
                if not agent_create.tool_rules and agent_create.include_base_tool_rules is None:
                    agent_create.include_base_tool_rules = False
            elif agent_create.agent_type == AgentType.workflow_agent:
                pass  # no default tools
            else:
                tool_names |= calculate_base_tools(is_v2=False)
```

**Answer to the question, precisely:** Yes. The branch `elif agent_create.enable_sleeptime: tool_names |= set(BASE_SLEEPTIME_CHAT_TOOLS)` sits **above** the `memgpt_v2_agent` / `letta_v1_agent` / catch-all branches, so when a *primary* agent is created with `enable_sleeptime=True`:
- its base toolset is **exactly** `BASE_SLEEPTIME_CHAT_TOOLS = ["send_message", "conversation_search", "archival_memory_search"]` — **no `core_memory_*` and no `memory_*` tools at all** (not `calculate_base_tools`, which is never called on this path);
- the paired agent created with `AgentType.sleeptime_agent` gets **exactly** `BASE_SLEEPTIME_TOOLS = ["memory_replace", "memory_insert", "memory_rethink", "memory_finish_edits"]`.

So the memory-edit capability is *transferred*, not duplicated, and the transfer is expressed as list constants in `letta/constants.py` plus this `if/elif` chain — there is no runtime "strip tools off the primary agent" mutation. **[VERIFIED]**

Supporting details **[VERIFIED]**:
- Sleep-time agents get an empty boot sequence: `if agent_state.agent_type == AgentType.sleeptime_agent: initial_boot_messages = []` in `initialize_message_sequence`, and in `initialize_message_sequence_async`: `if agent_state.agent_type == AgentType.sleeptime_agent or agent_state.agent_type == AgentType.letta_v1_agent: initial_boot_messages = []`. Also `letta_v1_agent` short-circuits entirely: `if agent_state.agent_type == AgentType.letta_v1_agent: return [{"role": "system", "content": full_system_message}]`.
  URL: https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/services/helpers/agent_manager_helper.py
- `derive_system_message(agent_type, enable_sleeptime=None, system=None)` picks the prompt; for sleeptime: `system = gpt_system.get_system_text("sleeptime_v2")` with comment *"v2 drops references to specific blocks, and instead relies on the block description injections"*; for memgpt/memgpt_v2 it picks `memgpt_v2_chat` in both the sleeptime and non-sleeptime cases, with the note *"same as the chat one, since the chat one says that you 'may' have the tools"*; `letta_v1_agent` → `"letta_v1"`. (Same file as above.)
- Memory **blocks are shared** between primary and sleep-time agent: `AgentManager.attach_block_async` docstring is *"Attaches a block to an agent. For sleeptime agents, also attaches to paired agents in the same group."* and the body:
```python
            # If agent is part of a sleeptime group, attach block to the sleeptime_agent
            if agent.multi_agent_group and agent.multi_agent_group.manager_type == ManagerType.sleeptime:
                group = agent.multi_agent_group
                # Find the sleeptime_agent in the group
                for other_agent_id in group.agent_ids or []:
                    if other_agent_id != agent_id:
                        try:
                            other_agent = await AgentModel.read_async(db_session=session, identifier=other_agent_id, actor=actor)
                            if other_agent.agent_type == AgentType.sleeptime_agent and block not in other_agent.core_memory:
                                other_agent.core_memory.append(block)
                                await other_agent.update_async(session, actor=actor)
                        except NoResultFound:
                            continue
```
- Deleting the primary agent also deletes the sleeptime agent + group (comment: *"Delete sleeptime agent and group (TODO this is flimsy pls fix)"*), matching `ManagerType.sleeptime, ManagerType.voice_sleeptime`.
- `AgentManager._should_exclude_model_from_base_tool_rules` override is skipped for sleeptime agents: `and agent_create.agent_type != AgentType.sleeptime_agent` — i.e. **sleep-time agents always get base tool rules (ContinueToolRule) even on models that are normally excluded** (`EXCLUDE_MODEL_KEYWORDS_FROM_BASE_TOOL_RULES = ["claude-4-sonnet", "claude-3-5-sonnet", "gpt-5", "gemini-2.5-pro"]`, `INCLUDE_MODEL_KEYWORDS_BASE_TOOL_RULES = ["mini"]`).
- Sleep-time agents are run with `LettaAgentV3`, so they are **heartbeat-free** and rely on `ContinueToolRule` for multi-step memory edits, terminating on `memory_finish_edits` (`TerminalToolRule`).

---

## 4. Prompt caching

### 4.1 `tests/test_prompt_caching.py` — what it actually asserts **[VERIFIED]**

**URL (raw):** https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/tests/test_prompt_caching.py
**URL (blob):** https://github.com/letta-ai/letta/blob/67013ef1bb491ec8bdd629bfbeceb70227936afa/tests/test_prompt_caching.py

Module docstring (verbatim):
```
Integration tests for prompt caching validation.

These tests verify that our LLM clients properly enable caching for each provider:
- OpenAI: Automatic caching (≥1024 tokens)
- Anthropic: Requires cache_control breakpoints (≥1024 tokens for Sonnet 4.5)
- Gemini: Implicit caching on 2.5 models (≥1024 tokens for 2.5 Flash)

Test strategy:
1. Create agent with large memory block (>5000 tokens to exceed all thresholds)
2. Send message 1 → assert cache WRITE occurred
3. Send message 2 → assert cache HIT occurred

If these tests fail, it means:
- For OpenAI: Something is broken (caching is automatic)
- For Anthropic: We're not adding cache_control breakpoints
- For Gemini: Implicit caching isn't working (or we're below threshold)
```

Key structural facts:
- These are **integration tests against a running Letta server**, not unit tests: they use `letta_client.AsyncLetta`, base URL `os.getenv("LETTA_SERVER_URL", "http://localhost:8283")`.
- Fixture `create_agent_with_large_memory` creates an agent with a single `persona` block set to `LARGE_MEMORY_BLOCK` (a deliberately huge repeated string, "designed to exceed 2,048 tokens"), `embedding="openai/text-embedding-3-small"`, and *"Use default settings - include_base_tools defaults to True, thinking enabled by default / This tests REAL production behavior, not simplified scenarios"*.
- Parametrization table `CACHING_TEST_CONFIGS`, filtered by env var `PROMPT_CACHE_TEST_MODEL`:
```python
CACHING_TEST_CONFIGS = [
    # OpenAI: Automatic caching, ≥1024 tokens
    pytest.param("openai/gpt-4o", {}, 1024, "cached_tokens", None, id="openai-gpt4o-auto"),
    # Anthropic: Requires cache_control, ≥1024 tokens for Sonnet 4.5
    pytest.param("anthropic/claude-sonnet-4-5-20250929", {}, 1024, "cache_read_tokens", "cache_creation_tokens",
                 id="anthropic-sonnet-4.5-explicit"),
    # Gemini: Implicit caching on 2.5 models, ≥1024 tokens for 2.5 Flash
    pytest.param("google_ai/gemini-2.5-flash", {}, 1024, "cached_tokens", None, id="gemini-2.5-flash-implicit"),
    # Gemini 3 Pro Preview: NOTE - Implicit caching seems to NOT work for 3 Pro Preview
    pytest.param("google_ai/gemini-3-pro-preview", {}, 2048, "cached_tokens", None, id="gemini-3-pro-preview-implicit",
                 marks=pytest.mark.xfail(reason="Gemini 3 Pro Preview doesn't have implicit caching (only 2.5 models do)")),
]
```
- The tests (10 total):
  `test_prompt_caching_cache_write_then_read`, `test_prompt_caching_multiple_messages`, `test_prompt_caching_cache_invalidation_on_memory_update`, `test_anthropic_system_prompt_stability`, `test_anthropic_inspect_raw_request`, `test_anthropic_cache_control_breakpoints`, `test_openai_automatic_caching`, `test_gemini_2_5_flash_implicit_caching`, `test_gemini_3_pro_preview_implicit_caching`, `test_gemini_request_prefix_stability`.

**Specific assertions (grouped):**

*(a) Cache write on turn 1, cache read on turn 2.* `test_prompt_caching_cache_write_then_read`: sends "Hello! Please introduce yourself." then "What are your main areas of expertise?" and asserts
```python
        total_input_tokens = (
            response1.usage.prompt_tokens + (response1.usage.cache_write_tokens or 0) + (response1.usage.cached_input_tokens or 0)
        )
        assert total_input_tokens >= min_tokens, f"Total input must be ≥{min_tokens} tokens for caching, got {total_input_tokens}"
```
then, for Anthropic only:
```python
            if "anthropic" in model:
                assert write_tokens is not None and write_tokens > 0, (
                    f"Anthropic should create cache on first message, got cache_write_tokens={write_tokens}"
                )
```
and the critical read assertion + ratio floor:
```python
        assert read_tokens is not None and read_tokens > 0, (
            f"Provider {model} should have cache hit on message 2, got cached_input_tokens={read_tokens}. This means caching is NOT working!"
        )
        ...
        cache_hit_ratio = read_tokens / total_input_msg2 if total_input_msg2 > 0 else 0
        ...
        # Note: With thinking mode enabled, Anthropic may have lower cache ratios due to
        # thinking blocks changing between messages. The key assertion is that SOME caching occurs.
        assert cache_hit_ratio >= 0.15, (
            f"Expected >15% cache hit ratio, got {cache_hit_ratio:.2%}. Some portion of prompt should be cached!"
        )
```
So the concrete cache-hit floor asserted is **≥15 % of total input tokens**, not "most of the prompt" (an earlier comment says "expect >50% cache hit" but the actual `assert` is `>= 0.15`).

*(b) Cache persists over multiple turns.* `test_prompt_caching_multiple_messages`: 3 messages; asserts every response after the first has `cached_input_tokens > 0`:
```python
        for i in range(1, len(responses)):
            read_tokens = responses[i].usage.cached_input_tokens
            assert read_tokens is not None and read_tokens > 0, (
                f"Message {i + 1} should have cache hit, got cached_input_tokens={read_tokens}"
            )
```

*(c) A memory-block edit invalidates the cache.* `test_prompt_caching_cache_invalidation_on_memory_update` — this is the test that speaks to the "are memory edits placed so as to preserve the cache prefix?" question, and the answer is **no, the test asserts the opposite**:
```python
        # Update memory block (this should invalidate cache)
        agent = await async_client.agents.get(agent_id=agent.id)
        persona_block = next((b for b in agent.memory_blocks if b.label == "persona"), None)
        ...
        await async_client.blocks.update(
            block_id=persona_block.id,
            label="persona",
            value=LARGE_MEMORY_BLOCK + "\n\nADDITIONAL NOTE: You are now extra helpful!",
        )
        ...
        # After update, cache hit ratio should drop significantly (or be zero)
        assert cache_ratio_after < cache_ratio_before, "Cache hit ratio should drop after memory update"
```
i.e. the test **expects and requires** that editing a memory block (which lives in the system prompt) *reduces* cache-hit ratio. There is no assertion that memory edits are placed in a cache-preserving position.

*(d) System-prompt stability is compared from real provider traces.* `test_anthropic_system_prompt_stability` fetches `async_client.telemetry.retrieve_provider_trace(step_id=...)` for two turns and diffs `request_json["system"]`:
```python
        system1 = trace1.request_json.get("system", [])
        system2 = trace2.request_json.get("system", [])
        system1_str = json.dumps(system1, sort_keys=True)
        system2_str = json.dumps(system2, sort_keys=True)

        if system1_str == system2_str:
            logger.info("✅ ACTUAL SYSTEM PROMPTS SENT TO ANTHROPIC ARE IDENTICAL")
            logger.info("   → Cache SHOULD work, but isn't. Issue is likely:")
            logger.info("   → 1. Thinking blocks breaking cache")
            logger.info("   → 2. Tool definitions changing")
            logger.info("   → 3. Something else in the request changing")
        else:
            logger.error("❌ ACTUAL SYSTEM PROMPTS SENT TO ANTHROPIC DIFFER!")
            ...
            if "Memory blocks were last modified" in diff_output:
                logger.error("⚠️  TIMESTAMP IS CHANGING IN ACTUAL REQUESTS!")
                logger.error("   → This is the root cause of cache misses")
```
Note these are `logger` calls, not `assert`s — this test is diagnostic, and it `return`s early if `step_id`s or traces are unavailable. The concrete cache-timing hypothesis being hunted is a changing **"Memory blocks were last modified"** timestamp inside the system prompt.

*(e) Breakpoint presence is asserted via cache-creation tokens.* `test_anthropic_cache_control_breakpoints`:
```python
        assert cache_creation is not None and cache_creation >= 1024, (
            f"Anthropic should create cache ≥1024 tokens on first message. Got {cache_creation}. This means cache_control breakpoints are NOT being added!"
        )
        ...
        assert max_cached > 0, (
            f"Anthropic should read from cache in at least one of {len(follow_up_messages)} follow-up messages. Got max={max_cached}. Cache reads are NOT working!"
        )
```

*(f) Raw-request inspection of breakpoint placement.* `test_anthropic_inspect_raw_request` logs:
```python
                logger.info(f"System has cache_control: {'cache_control' in provider_trace_1.request_json.get('system', [{}])[-1]}")
                ...
                for i, msg in enumerate(provider_trace_2.request_json.get("messages", [])):
                    logger.info(f"  Message {i}: role={msg.get('role')}")
                    content = msg.get("content")
                    if isinstance(content, list):
                        for j, block in enumerate(content):
                            logger.info(f"    Block {j}: type={block.get('type')}, has_cache_control={'cache_control' in block}")
```
i.e. it explicitly checks `cache_control` on the **last system block** and enumerates `cache_control` presence per content block. Again `logger`, not `assert`.

*(g) OpenAI.* `test_openai_automatic_caching`:
```python
        assert cached_tokens_2 is not None and cached_tokens_2 >= 1024, (
            f"OpenAI should cache ≥1024 tokens automatically on second message. Got {cached_tokens_2}. Automatic caching is NOT working!"
        )
        # Cached tokens should be in 128-token increments
        assert cached_tokens_2 % 128 == 0, f"OpenAI cached_input_tokens should be in 128-token increments, got {cached_tokens_2}"
```

*(h) Gemini.* `test_gemini_2_5_flash_implicit_caching` asserts `cached_input_tokens >= 1024`; `test_gemini_3_pro_preview_implicit_caching` asserts `max_cached >= 2048` across up to 3 follow-ups (and the parametrized case is marked `xfail`). `test_gemini_request_prefix_stability` compares `systemInstruction`, tool **names and order**, tool *definitions*, and the overlapping `contents` prefix between two real requests, logging `❌ SYSTEM INSTRUCTIONS DIFFER!`, `❌ TOOL ORDER/NAMES DIFFER!`, `✅ Content prefix matches between requests` — again logging, not asserting.

*(i) Usage sanity helper* `assert_usage_sanity(usage, context)` is called for every response and asserts `prompt_tokens > 0`, `prompt_tokens < 500000`, `completion_tokens > 1`, `completion_tokens < 50000`, and that `cache_write_tokens` / `cached_input_tokens` don't exceed total input.

### 4.2 `letta/llm_api/anthropic_client.py` — `cache_control` placement **[VERIFIED]**

**URL:** https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/anthropic_client.py

There are **exactly three** `cache_control` breakpoints, all `{"type": "ephemeral"}`:

1. **Last tool definition** (so tool schemas are cached), inside `build_request_data`:
```python
        if tools_for_request and len(tools_for_request) > 0:
            # TODO eventually enable parallel tool use
            # DISABLED: use_strict=False to disable structured outputs (TTFT latency impact)
            # See PR #7495 for original implementation
            data["tools"] = convert_tools_to_anthropic_format(
                tools_for_request,
                use_strict=False,  # Was: _supports_structured_outputs(llm_config.model)
            )
            # Add cache control to the last tool for caching tool definitions
            if len(data["tools"]) > 0:
                data["tools"][-1]["cache_control"] = {"type": "ephemeral"}
```

2. **System prompt** (last text block), via `_add_cache_control_to_system_message`:
```python
        data["system"] = self._add_cache_control_to_system_message(system_content)
```
```python
    def _add_cache_control_to_system_message(self, system_content):
        """Add cache control to system message content."""
        if isinstance(system_content, str):
            # For string content, convert to list format with cache control
            return [{"type": "text", "text": system_content, "cache_control": {"type": "ephemeral"}}]
        elif isinstance(system_content, list):
            # For list content, add cache control to the last text block
            cached_content = system_content.copy()
            for i in range(len(cached_content) - 1, -1, -1):
                if cached_content[i].get("type") == "text":
                    cached_content[i]["cache_control"] = {"type": "ephemeral"}
                    break
            return cached_content

        return system_content
```

3. **Final content block of the final message** (incremental conversation caching), via `_add_cache_control_to_messages`:
```python
        # Add cache control to final message for incremental conversation caching
        # Per Anthropic docs: "During each turn, we mark the final block of the final message with
        # cache_control so the conversation can be incrementally cached."
        data["messages"] = self._add_cache_control_to_messages(data["messages"])
```
```python
    def _add_cache_control_to_messages(self, messages):
        """
        Add cache control to the final content block of the final message.

        This enables incremental conversation caching per Anthropic docs:
        "During each turn, we mark the final block of the final message with cache_control
        so the conversation can be incrementally cached."

        Args:
            messages: List of Anthropic-formatted message dicts

        Returns:
            Modified messages list with cache_control on final block
        """
        if not messages or len(messages) == 0:
            return messages

        # Work backwards to find the last message with content
        for i in range(len(messages) - 1, -1, -1):
            message = messages[i]
            content = message.get("content")

            if not content:
                continue

            # Handle string content
            if isinstance(content, str):
                messages[i]["content"] = [{"type": "text", "text": content, "cache_control": {"type": "ephemeral"}}]
                return messages

            # Handle list content - add cache_control to the last block
            if isinstance(content, list) and len(content) > 0:
                # Add cache_control to the last content block
                messages[i]["content"][-1]["cache_control"] = {"type": "ephemeral"}
                return messages

        return messages
```

There is also a debug-only logging block after this:
```python
        # Debug: Log cache control placement
        logger.debug(f"Anthropic request has {len(data.get('messages', []))} messages")
        if data.get("messages") and len(data["messages"]) > 0:
            last_msg = data["messages"][-1]
            logger.debug(f"Last message role: {last_msg.get('role')}, content type: {type(last_msg.get('content'))}")
            ...
                logger.debug(f"Last content block type: {last_block.get('type')}, has cache_control: {'cache_control' in last_block}")
                if "cache_control" in last_block:
                    logger.debug(f"Cache control value: {last_block['cache_control']}")
```

**Negative findings (important, all via grep of the full file):**
- ⚠️ **No `ttl` and no `"1h"` / `"5m"` anywhere in `anthropic_client.py`.** Every breakpoint is plain `{"type": "ephemeral"}`. **[VERIFIED — full-file grep]**
- ⚠️ **No `PROMPT_CACHE_*` constant in `anthropic_client.py`.** Also absent from `letta/constants.py` and `letta/settings.py` (both read in full). ⚠️ **NOT VERIFIED repo-wide** — I could not run a repo-wide code search (GitHub code search needs auth; grep.app returned HTTP 429). Stated as: *no `PROMPT_CACHE_*` constant was found in the four files most likely to hold it.*
- ⚠️ **No `enable_prompt_caching` / `prompt_cache` config flag** in `letta/settings.py` **[VERIFIED — full file read]**. The only token-caching-related setting is `upsert_pinecone_indices` / `enable_pinecone` (unrelated) and the prompt-*token* fields. The closest flag is `model_settings.anthropic_sonnet_1m` (adds beta `context-1m-2025-08-07`), which is a context-window flag, not a caching flag.
- ⚠️ **`cache_control` is unconditional** for Anthropic — there is no model check, no feature flag, and no token-threshold check around breakpoint placement. `build_request_data` always sets the three breakpoints when tools/system/messages are present. **[VERIFIED]**
- There is **no `MAX_CACHE_BREAKPOINTS`** constant (Anthropic's limit is 4; Letta uses 3 without naming the limit). The nearest named limit is unrelated: `ANTHROPIC_MAX_STRICT_TOOLS = 15` in `letta/llm_api/anthropic_constants.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/anthropic_constants.py), whose header comment says *"Max 15 tools can use strict mode per request"*. That file's allowlist is `{"Write","Read","Edit","Glob","KillBash","fetch_webpage","EnterPlanMode","ExitPlanMode","Skill","conversation_search"}`.

**Cache accounting — same file:**
```python
        # Build prompt tokens details with cache data if available
        prompt_tokens_details = None
        cache_read_tokens = 0
        cache_creation_tokens = 0
        if hasattr(response.usage, "cache_read_input_tokens") or hasattr(response.usage, "cache_creation_input_tokens"):
            from letta.schemas.openai.chat_completion_response import UsageStatisticsPromptTokenDetails

            cache_read_tokens = getattr(response.usage, "cache_read_input_tokens", 0) or 0
            cache_creation_tokens = getattr(response.usage, "cache_creation_input_tokens", 0) or 0
            prompt_tokens_details = UsageStatisticsPromptTokenDetails(
                cache_read_tokens=cache_read_tokens,
                cache_creation_tokens=cache_creation_tokens,
            )

        # Per Anthropic docs: "Total input tokens in a request is the summation of
        # input_tokens, cache_creation_input_tokens, and cache_read_input_tokens."
        actual_input_tokens = prompt_tokens + cache_read_tokens + cache_creation_tokens
```
with `prompt_tokens=actual_input_tokens` on the emitted `UsageStatistics`.

One additional cache-related lever, in `request_async` / `stream_async` / `request`:
```python
        # Context management for Opus 4.5 to preserve thinking blocks (improves cache hits)
        if llm_config.model.startswith("claude-opus-4-5") and llm_config.enable_reasoner:
            betas.append("context-management-2025-06-27")
```
and in `build_request_data`:
```python
        # Context management for Opus 4.5 to preserve thinking blocks and improve cache hits
        # See: https://docs.anthropic.com/en/docs/build-with-claude/context-editing
        if llm_config.model.startswith("claude-opus-4-5") and llm_config.enable_reasoner:
            data["context_management"] = {
                "edits": [
                    {
                        "type": "clear_thinking_20251015",
                        "keep": "all",  # Preserve all thinking blocks for maximum cache performance
                    }
                ]
            }
```
This is the only place in the file where the code explicitly optimizes *for* cache hits.

### 4.3 Bedrock **[VERIFIED]**

**URL:** https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/bedrock_client.py

```python
class BedrockClient(AnthropicClient):
```
Its `build_request_data` only removes a field and otherwise delegates:
```python
    def build_request_data(self, agent_type, messages, llm_config, tools=None, force_tool_call=None,
                           requires_subsequent_tool_call=False, tool_return_truncation_chars=None) -> dict:
        data = super().build_request_data(agent_type, messages, llm_config, tools, force_tool_call, requires_subsequent_tool_call)
        # remove disallowed fields
        if "tool_choice" in data:
            del data["tool_choice"]["disable_parallel_tool_use"]
        return data
```
So Bedrock **inherits the same three `{"type": "ephemeral"}` breakpoints**. ⚠️ **No `cachePoint` / `cachePoint`-style Bedrock-native caching block exists in `bedrock_client.py`** — grepped, absent. `bedrock_anthropic_version: Optional[str] = "bedrock-2023-05-31"` is the only Bedrock-specific model setting (`letta/settings.py`).

### 4.4 OpenAI **[VERIFIED]**

**URL:** https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/openai_client.py

⚠️ **`openai_client.py` contains no caching code at all** — no `cache_control`, no cache-breakpoint helper, nothing read or set for prompt caching except the passthrough of the provider's own usage. This matches the test docstring's claim *"OpenAI: Automatic caching (≥1024 tokens)"* and the test comment *"OpenAI doesn't charge for cache writes, so cached_input_tokens should be 0 or None on first message"*. Caching for OpenAI is entirely server-side/implicit.

### 4.5 Gemini / Vertex **[VERIFIED]**

**URLs:** https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/google_vertex_client.py and https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/google_ai_client.py (`class GoogleAIClient(GoogleVertexClient)`).

- ⚠️ **No explicit cache creation.** There is no `cached_content`, no `CachedContent`, no `client.caches.create(...)`, and no `cache_control` anywhere in either file. Gemini relies on **implicit** caching, matching the test docstring.
- Cache usage is only **read back** from the response:
```python
            if response.usage_metadata:
                # Extract cache token data if available (Gemini uses cached_content_token_count)
                # Use `is not None` to capture 0 values (meaning "provider reported 0 cached tokens")
                prompt_tokens_details = None
                if (
                    hasattr(response.usage_metadata, "cached_content_token_count")
                    and response.usage_metadata.cached_content_token_count is not None
                ):
                    from letta.schemas.openai.chat_completion_response import UsageStatisticsPromptTokenDetails

                    prompt_tokens_details = UsageStatisticsPromptTokenDetails(
                        cached_tokens=response.usage_metadata.cached_content_token_count,
                    )
```
This is exactly the "normalized from `cached_content_token_count`" the test file refers to.
- Cache-prefix-relevant behaviours in `google_vertex_client.py`: tool schema sanitization `_clean_google_ai_schema_properties` (removes `default`, `exclusiveMaximum`, `exclusiveMinimum`, `additionalProperties`, `$schema`), and `add_dummy_model_messages` which injects a synthetic `{"role": "model", "parts": [{"text": f"{NON_USER_MSG_PREFIX}Function call returned, waiting for user response."}]}` — a stable, deterministic filler, which matters for prefix stability.
- `build_request_data` does **not** accept or forward a `cache_ttl`-like parameter.

### 4.6 Where cache metrics surface **[VERIFIED]**

`letta/agents/letta_agent_v2.py` — per-step persistence of cache fields into `Step` metrics:
```python
            prompt_details = None
            if step_usage.cached_input_tokens is not None or step_usage.cache_write_tokens is not None:
                prompt_details = UsageStatisticsPromptTokenDetails(
                    cached_tokens=step_usage.cached_input_tokens if step_usage.cached_input_tokens is not None else None,
                    cache_read_tokens=step_usage.cached_input_tokens if step_usage.cached_input_tokens is not None else None,
                    cache_creation_tokens=step_usage.cache_write_tokens if step_usage.cache_write_tokens is not None else None,
                )
```
and accumulation in `_update_global_usage_stats`:
```python
        if step_usage_stats.cached_input_tokens is not None:
            self.usage.cached_input_tokens = (self.usage.cached_input_tokens or 0) + step_usage_stats.cached_input_tokens
        if step_usage_stats.cache_write_tokens is not None:
            self.usage.cache_write_tokens = (self.usage.cache_write_tokens or 0) + step_usage_stats.cache_write_tokens
```
Public field names on usage objects: `cached_input_tokens`, `cache_write_tokens` (and `prompt_tokens_details.cache_read_tokens` / `cache_creation_tokens` at the OpenAI-compat layer). This is the mapping the integration tests read.

---

## 5. Archival memory storage

### 5.1 Vector DB providers **[VERIFIED]**

`letta/schemas/enums.py`:
```python
class VectorDBProvider(str, Enum):
    """Supported vector database providers for archival memory"""

    NATIVE = "native"
    TPUF = "tpuf"
    PINECONE = "pinecone"
```

**Provider classes found:**
| Backend | Where | Class / mechanism |
|---|---|---|
| Postgres + **pgvector** | `letta/orm/passage.py` | `from pgvector.sqlalchemy import Vector` → `embedding = mapped_column(Vector(MAX_EMBEDDING_DIM))` |
| **sqlite-vec** | `letta/orm/sqlite_functions.py` | `import sqlite_vec`, `sqlite_vec.serialize_float32(...)`, plus a Python `cosine_distance` UDF registered on connect |
| **Turbopuffer** | `letta/helpers/tpuf_client.py` | `class TurbopufferClient`, `AsyncTurbopuffer`, namespaces |
| **Pinecone** | `letta/helpers/pinecone_utils.py` (14.8 KB) | file exists in the tree; `settings.enable_pinecone` / `pinecone_*` settings; `embedding_endpoint_type="pinecone"` is a valid `EmbeddingConfig` literal |

`letta/orm/passage.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/orm/passage.py):
```python
    # Vector embedding field based on database type
    if settings.database_engine is DatabaseChoice.POSTGRES:
        from pgvector.sqlalchemy import Vector

        embedding = mapped_column(Vector(MAX_EMBEDDING_DIM))
    else:
        embedding = Column(CommonVector)
```
`letta/orm/sqlite_functions.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/orm/sqlite_functions.py):
```python
if settings.database_engine == DatabaseChoice.SQLITE:
    import sqlite_vec
...
def adapt_array(arr):
    """
    Converts numpy array to binary for SQLite storage using sqlite-vec
    """
    ...
    return sqlite_vec.serialize_float32(arr.tolist())
...
# Note: sqlite-vec provides native SQL functions for vector operations
# We don't need custom Python distance functions since sqlite-vec handles this at the SQL level
@event.listens_for(Engine, "connect")
def register_functions(dbapi_connection, connection_record):
    """Register SQLite functions and enable sqlite-vec extension"""
```
`CommonVector` is a `TypeDecorator` over `BINARY` in `letta/orm/custom_columns.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/orm/custom_columns.py):
```python
class CommonVector(TypeDecorator):
    """Custom SQLAlchemy column type for storing vectors in SQLite."""

    impl = BINARY
    cache_ok = True
```

⚠️ **NOT FOUND** (searched by name in `letta/schemas/enums.py`, `letta/orm/passage.py`, `letta/orm/custom_columns.py`, `letta/orm/sqlite_functions.py`, `letta/settings.py`, `letta/services/archive_manager.py`, `letta/services/passage_manager.py`, `letta/helpers/tpuf_client.py`): **Chroma / chromadb, Milvus, Qdrant, LanceDB, Elasticsearch, Weaviate**. There is **no `VectorDBBase` / `StorageConnectorBase` ABC** — there is no pluggable provider class hierarchy; the "provider" is a per-archive enum plus branch points in `PassageManager`/`ArchiveManager`. ⚠️ Repo-wide absence of those libraries is **NOT VERIFIED** (no repo-wide search available), but they are absent from every storage-relevant file I read.

Also note: `letta/schemas/enums.py` `VectorDBProvider` is the **only** provider enum; but `create_archive_async` can only ever pick TPUF or NATIVE — **`PINECONE` is not reachable from archive creation** at this SHA (see 5.3).

### 5.2 Settings and defaults **[VERIFIED]**

`letta/settings.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/settings.py):
```python
class DatabaseChoice(str, Enum):
    POSTGRES = "postgres"
    SQLITE = "sqlite"
```
```python
    # For embeddings
    enable_pinecone: bool = False
    pinecone_api_key: Optional[str] = None
    pinecone_source_index: Optional[str] = "sources"
    pinecone_agent_index: Optional[str] = "recall"
    upsert_pinecone_indices: bool = False

    # For tpuf - currently only for archival memories
    use_tpuf: bool = False
    tpuf_api_key: Optional[str] = None
    tpuf_region: str = "gcp-us-central1"
    embed_all_messages: bool = False
    embed_tools: bool = False
```
```python
    # DB engine selection
    @property
    def letta_pg_uri(self) -> str:
        if self.pg_uri:
            return self.pg_uri
        elif self.pg_db and self.pg_user and self.pg_password and self.pg_host and self.pg_port:
            return f"postgresql+pg8000://{self.pg_user}:{self.pg_password}@{self.pg_host}:{self.pg_port}/{self.pg_db}"
        else:
            return "postgresql+pg8000://letta:letta@localhost:5432/letta"

    @property
    def database_engine(self) -> DatabaseChoice:
        return DatabaseChoice.POSTGRES if self.letta_pg_uri_no_default else DatabaseChoice.SQLITE
```
Note `letta_pg_uri_no_default` returns `None` unless `pg_uri` or all five `pg_*` fields are set — so **SQLite is the effective default engine out of the box**, and Postgres/pgvector requires explicit configuration.

Also `archival_memory_token_limit: int = 8192`, and `use_letta_v1_agent: bool = False`.

`.env.example` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/.env.example) contains **only** OpenAI / Ollama / vLLM sections — **no `LETTA_USE_TPUF`, no `LETTA_PG_*`**. **[VERIFIED — full file read]**

### 5.3 Self-hosted vs Letta Cloud — the actual answer **[VERIFIED for self-hosted; NOT VERIFIED for cloud]**

`letta/services/archive_manager.py`, `create_archive_async` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/services/archive_manager.py):
```python
        """Create a new archive."""
        try:
            async with db_registry.async_session() as session:
                # determine vector db provider based on settings
                vector_db_provider = VectorDBProvider.TPUF if should_use_tpuf() else VectorDBProvider.NATIVE
```
`letta/helpers/tpuf_client.py`, `should_use_tpuf` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/helpers/tpuf_client.py):
```python
def should_use_tpuf() -> bool:
    # We need OpenAI since we default to their embedding model
    return bool(settings.use_tpuf) and bool(settings.tpuf_api_key) and bool(model_settings.openai_api_key)
```

**Therefore, for self-hosted / local [VERIFIED]:**
- Default is `VectorDBProvider.NATIVE`, gated on `settings.use_tpuf` whose default is `False`. `NATIVE` resolves at the ORM level to **pgvector `Vector(4096)` when Postgres is configured** (i.e. `pg_uri` or all `pg_*` set), and to a **`BINARY`/sqlite-vec column when the engine is SQLite** — which is the out-of-the-box state.
- Turbopuffer is **opt-in only** and requires three conditions simultaneously: `LETTA_USE_TPUF=true` (env prefix is `letta_`, so `settings.use_tpuf` ← `LETTA_USE_TPUF`), a Turbopuffer API key, **and** an OpenAI API key (because the TPUF client hard-codes OpenAI `text-embedding-3-small` as its default embedding config — see below). Without all three, TPUF is silently not used even if the flag is set.
- `PINECONE` exists in the enum and there are Pinecone settings/helpers, but `create_archive_async` never selects it — **dead path for archive creation at this SHA**.
- Turbopuffer writes are **dual writes**, not migrations: SQL is always written first, and Turbopuffer gets the same IDs afterwards, best-effort:
```python
            # Always write to SQL database first
            ...
            # If archive uses Turbopuffer, also write to Turbopuffer (dual-write)
            if archive.vector_db_provider == VectorDBProvider.TPUF:
                try:
                    from letta.helpers.tpuf_client import TurbopufferClient
                    ...
                    await tpuf_client.insert_archival_memories(
                        archive_id=archive.id,
                        text_chunks=passage_texts,
                        passage_ids=passage_ids,  # Use same IDs as SQL
                        ...
                    )
                except Exception as e:
                    logger.error(f"Failed to insert passages to Turbopuffer: {e}")
                    if strict_mode:
                        raise  # Re-raise the exception in strict mode
```
(`letta/services/passage_manager.py`, `insert_passage`.) Same dual-write/dual-delete pattern in `delete_agent_passage_by_id_async`, `delete_agent_passages_async`, and `ArchiveManager.create_passage_in_archive_async` / `delete_passage_from_archive_async`.

**For Letta Cloud:** ⚠️ **NOT VERIFIED.** The repo contains no cloud environment configuration exposing `LETTA_USE_TPUF`, and I could not query `docs.letta.com` successfully for a page stating the cloud storage backend (the docs pages I fetched returned navigation chrome only). The strongest *inference* available is: (a) `VectorDBProvider.TPUF` + `tpuf_region = "gcp-us-central1"` + namespace suffixes controlled by `settings.environment` (with `environment` documented as *"Application environment (prod, dev, canary, etc.)"*) indicate Turbopuffer is the production/managed path; and (b) the Turbopuffer namespace scheme is designed for multi-tenant isolation:
```python
            # generate namespace name using same logic as tpuf_client
            environment = settings.environment
            if environment:
                namespace_name = f"archive_{archive_id}_{environment.lower()}"
            else:
                namespace_name = f"archive_{archive_id}"
```
and in `tpuf_client.py`, per-archive namespaces plus org-scoped namespaces for other data: `f"messages_{organization_id}_{environment.lower()}"`, `f"tools_{organization_id}_..."`, `f"file_passages_{organization_id}_..."`. I present this as **inference, explicitly not a verified claim about Letta Cloud.**

### 5.4 Passage / ArchivalPassage **[VERIFIED]**

**Files:** `letta/orm/passage.py`, `letta/schemas/passage.py`, `letta/orm/passage_tag.py`, `letta/services/passage_manager.py`.

`letta/orm/passage.py` — three classes, one abstract base:
```python
class BasePassage(SqlalchemyBase, OrganizationMixin):
    """Base class for all passage types with common fields"""

    __abstract__ = True
    __pydantic_model__ = PydanticPassage

    id: Mapped[str] = mapped_column(primary_key=True, doc="Unique passage identifier")
    text: Mapped[str] = mapped_column(doc="Passage text content")
    embedding_config: Mapped[dict] = mapped_column(EmbeddingConfigColumn, doc="Embedding configuration")
    metadata_: Mapped[dict] = mapped_column(JSON, doc="Additional metadata")
    # dual storage: json column for fast retrieval, junction table for efficient queries
    tags: Mapped[Optional[List[str]]] = mapped_column(JSON, nullable=True, doc="Tags associated with this passage")

    # Vector embedding field based on database type
    if settings.database_engine is DatabaseChoice.POSTGRES:
        from pgvector.sqlalchemy import Vector
        embedding = mapped_column(Vector(MAX_EMBEDDING_DIM))
    else:
        embedding = Column(CommonVector)


class SourcePassage(BasePassage, FileMixin, SourceMixin):
    """Passages derived from external files/sources"""
    __tablename__ = "source_passages"
    file_name: Mapped[str] = mapped_column(doc="The name of the file that this passage was derived from")


class ArchivalPassage(BasePassage, ArchiveMixin):
    """Passages stored in archives as archival memories"""
    __tablename__ = "archival_passages"

    # junction table for efficient tag queries (complements json column above)
    passage_tags: Mapped[List["PassageTag"]] = relationship(
        "PassageTag", back_populates="passage", cascade="all, delete-orphan", lazy="noload"
    )
```
Table names: **`archival_passages`** and **`source_passages`**. Indexes: `ix_archival_passages_org_archive (organization_id, archive_id)`, `archival_passages_created_at_id_idx (created_at, id)`, `ix_archival_passages_archive_id`. `ArchiveMixin` / `SourceMixin` / `FileMixin` live in `letta/orm/mixins.py`.

So the MemGPT-per-agent scoping is **gone**: `ArchivalPassage` is scoped by **`archive_id`**, not `agent_id`. `agent_id` is only reachable via the `archives_agents` join. `PassageManager.agent_passage_size_async` makes this explicit:
```python
                result = await session.execute(
                    select(func.count(ArchivalPassage.id))
                    .join(ArchivesAgents, ArchivalPassage.archive_id == ArchivesAgents.archive_id)
                    .where(
                        ArchivesAgents.agent_id == agent_id,
                        ArchivalPassage.organization_id == actor.organization_id,
                        ArchivalPassage.is_deleted == False,
                    )
                )
```
⚠️ **Naming caution:** there is **no class named `Passage` in `letta/orm/passage.py`**. `PydanticPassage` is `letta/schemas/passage.py::Passage`. The ORM/base classes are `BasePassage`, `SourcePassage`, `ArchivalPassage`.

**Write/read API surface on `PassageManager`** (all `@enforce_types` + `@trace_method`) **[VERIFIED]**:
- `get_agent_passage_by_id_async(passage_id, actor)`
- `get_source_passage_by_id_async(passage_id, actor)`
- `create_agent_passage_async(pydantic_passage, actor)` — raises `ValueError("Agent passage must have archive_id")` / `"Agent passage cannot have source_id"`
- `create_source_passage_async(pydantic_passage, file_metadata, actor)`
- `create_many_archival_passages_async(passages, actor)` / `create_many_source_passages_async(...)`
- `insert_passage(agent_state, text, actor, tags=None, created_at=None, strict_mode=False) -> List[PydanticPassage]`
- `update_agent_passage_by_id_async`, `update_source_passage_by_id_async`
- `delete_agent_passage_by_id_async`, `delete_agent_passages_async`, `delete_source_passages_async`, `delete_agent_passages`
- `agent_passage_size_async(actor, agent_id=None)`, `source_passage_size_async`, `estimate_embeddings_size_async(actor, agent_id=None, storage_unit="GB")`
- `get_unique_tags_for_archive_async(archive_id, actor)`, `get_tag_counts_for_archive_async(archive_id, actor)`
- **Explicitly deprecated** (with `logger.warning(...)`) and kept for compat: `get_passage_by_id_async`, `create_passage_async`, `create_many_passages`, `create_many_passages_async`, `delete_passage_by_id_async`, `delete_passages`, `size`. E.g.
```python
    async def get_passage_by_id_async(self, passage_id: str, actor: PydanticUser) -> Optional[PydanticPassage]:
        """DEPRECATED: Use get_agent_passage_by_id_async() or get_source_passage_by_id_async() instead."""
        logger.warning(
            "get_passage_by_id_async is deprecated. Use get_agent_passage_by_id_async() or get_source_passage_by_id_async() instead."
        )
```
No embedding is returned to callers doing vector search over Turbopuffer: `_process_file_query_results` sets `embedding=[],  # empty embedding since we don't return it from Turbopuffer` **[VERIFIED]**.

Also note the **embedding padding** invariant, repeated in ~6 places in `passage_manager.py`:
```python
                # Always pad when writing to Postgres vector DB (don't pad for Turbopuffer/Pinecone)
                if not should_use_tpuf():
                    np_embedding = np.array(embedding)
                    if np_embedding.shape[0] != MAX_EMBEDDING_DIM:
                        embedding = np.pad(np_embedding, (0, MAX_EMBEDDING_DIM - np_embedding.shape[0]), mode="constant").tolist()
```
with `MAX_EMBEDDING_DIM = 4096  # maximum supported embeding size - do NOT change or else DBs will need to be reset` (`letta/constants.py`). And null-byte sanitisation: `text = text.replace("\x00", "")` with `logger.warning("Removed null bytes from passage text ...")`.

⚠️ Note a **latent inconsistency**: the padding guard is keyed on `should_use_tpuf()` (a *global* setting) while the write path is keyed on `archive.vector_db_provider == VectorDBProvider.TPUF` (a *per-archive* field). Since `create_archive_async` sets the per-archive field from the same global, they agree in practice — but they are not the same predicate.

### 5.5 The `archives` abstraction **[VERIFIED]**

**ORM model** — `letta/orm/archive.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/orm/archive.py):
```python
class Archive(SqlalchemyBase, OrganizationMixin):
    """An archive represents a collection of archival passages that can be shared between agents"""

    __tablename__ = "archives"
    __pydantic_model__ = PydanticArchive
    ...
    id: Mapped[str] = mapped_column(String, primary_key=True, default=lambda: f"archive-{uuid.uuid4()}")

    # archive-specific fields
    name: Mapped[str] = mapped_column(String, nullable=False, doc="The name of the archive")
    description: Mapped[Optional[str]] = mapped_column(String, nullable=True, doc="A description of the archive")
    vector_db_provider: Mapped[VectorDBProvider] = mapped_column(
        Enum(VectorDBProvider),
        nullable=False,
        default=VectorDBProvider.NATIVE,
        doc="The vector database provider used for this archive's passages",
    )
    embedding_config: Mapped[dict] = mapped_column(
        EmbeddingConfigColumn, nullable=False, doc="Embedding configuration for passages in this archive"
    )
    metadata_: Mapped[Optional[dict]] = mapped_column(JSON, nullable=True, doc="Additional metadata for the archive")
    _vector_db_namespace: Mapped[Optional[str]] = mapped_column(String, nullable=True, doc="Private field for vector database namespace")

    # relationships
    archives_agents: Mapped[List["ArchivesAgents"]] = relationship(
        "ArchivesAgents",
        back_populates="archive",
        cascade="all, delete-orphan",  # this will delete junction entries when archive is deleted
        lazy="noload",
    )
```
DB table **`archives`**; ID prefix `archive-` (`PrimitiveType.ARCHIVE = "archive"`). Indexes `ix_archives_created_at (created_at, id)`, `ix_archives_organization_id`. `Archive` also overrides `create`/`create_async` to set timestamps explicitly when `settings.database_engine == DatabaseChoice.SQLITE`.

**Pydantic schema** — `letta/schemas/archive.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/schemas/archive.py):
```python
class ArchiveBase(OrmMetadataBase):
    __id_prefix__ = PrimitiveType.ARCHIVE.value

    name: str = Field(..., description="The name of the archive")
    description: Optional[str] = Field(None, description="A description of the archive")
    organization_id: str = Field(..., description="The organization this archive belongs to")
    vector_db_provider: VectorDBProvider = Field(
        default=VectorDBProvider.NATIVE, description="The vector database provider used for this archive's passages"
    )
    embedding_config: EmbeddingConfig = Field(..., description="Embedding configuration for passages in this archive")
    metadata: Optional[Dict] = Field(default_factory=dict, validation_alias="metadata_", description="Additional metadata")


class Archive(ArchiveBase):
    """Representation of an archive - a collection of archival passages that can be shared between agents."""

    id: str = ArchiveBase.generate_id_field()
    created_at: datetime = Field(..., description="The creation date of the archive")


class ArchiveCreate(ArchiveBase):
    """Create a new archive"""


class ArchiveUpdate(ArchiveBase):
    """Update an existing archive"""
```

**Junction table** — `letta/orm/archives_agents.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/orm/archives_agents.py):
```python
class ArchivesAgents(Base):
    """Many-to-many relationship between agents and archives"""

    __tablename__ = "archives_agents"

    # TODO: Remove this unique constraint when we support multiple archives per agent
    # For now, each agent can only have one archive
    __table_args__ = (UniqueConstraint("agent_id", name="unique_agent_archive"),)

    agent_id: Mapped[str] = mapped_column(String, ForeignKey("agents.id", ondelete="CASCADE"), primary_key=True)
    archive_id: Mapped[str] = mapped_column(String, ForeignKey("archives.id", ondelete="CASCADE"), primary_key=True)

    # track when the relationship was created and if agent is owner
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default="now()")
    is_owner: Mapped[bool] = mapped_column(Boolean, default=False, doc="Whether this agent created/owns the archive")

    # relationships
    agent: Mapped["Agent"] = relationship("Agent", back_populates="archives_agents")
    archive: Mapped["Archive"] = relationship("Archive", back_populates="archives_agents")
```
⚠️ **Despite the name "many-to-many", the schema today enforces at most ONE archive per agent** (`UniqueConstraint("agent_id", ...)`, `agent_id` is the primary key). "Attach an archive to many agents" is supported; "attach many archives to one agent" is not. Enforced again at runtime in `ArchiveManager.get_default_archive_for_agent_async` and `get_or_create_default_archive_for_agent_async`:
```python
        if archive_ids:
            # TODO: Remove this check once we support multiple archives per agent
            if len(archive_ids) > 1:
                raise ValueError(f"Agent {agent_id} has multiple archives, which is not yet supported")
```

**Attach / detach service methods** — `letta/services/archive_manager.py`:
```python
    async def attach_agent_to_archive_async(self, agent_id: str, archive_id: str, is_owner: bool = False, actor: PydanticUser = None) -> None:
        """Attach an agent to an archive."""
        ...
    async def detach_agent_from_archive_async(self, agent_id: str, archive_id: str, actor: PydanticUser = None) -> None:
        """Detach an agent from an archive."""
        ...
    async def get_or_create_default_archive_for_agent_async(self, agent_state: PydanticAgentState, actor: PydanticUser = None) -> PydanticArchive:
        """Get the agent's default archive, creating one if it doesn't exist."""
        ...
        archive_name = f"{agent_state.name}'s Archive"
        archive = await self.create_archive_async(
            name=archive_name,
            embedding_config=agent_state.embedding_config,
            description="Default archive created automatically",
            actor=actor,
        )
        try:
            # Attach the agent to the archive as owner
            await self.attach_agent_to_archive_async(
                agent_id=agent_state.id, archive_id=archive.id, is_owner=True, actor=actor,
            )
```
(plus an `IntegrityError` race-condition handler that deletes the orphaned archive and refetches.)

`insert_passage` calls this automatically:
```python
        # Get or create the default archive for the agent
        archive = await self.archive_manager.get_or_create_default_archive_for_agent_async(agent_state=agent_state, actor=actor)
```
So **archival memory "just works" per-agent without the caller creating an archive** — a default archive named `"<agent name>'s Archive"` is lazily created and attached with `is_owner=True`.

### 5.6 REST endpoints **[VERIFIED]**

**Archive CRUD + passages + reverse lookup** — `letta/server/rest_api/routers/v1/archives.py`, `router = APIRouter(prefix="/archives", tags=["archives"])` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/server/rest_api/routers/v1/archives.py):

| Method + path | `operation_id` | Function |
|---|---|---|
| `POST /v1/archives/` | `create_archive` | `create_archive` |
| `GET /v1/archives/` | `list_archives` | `list_archives` |
| `GET /v1/archives/{archive_id}` | `retrieve_archive` | `retrieve_archive` |
| `PATCH /v1/archives/{archive_id}` | `modify_archive` | `modify_archive` |
| `DELETE /v1/archives/{archive_id}` | `delete_archive` | `delete_archive` (204) |
| `GET /v1/archives/{archive_id}/agents` | `list_agents_for_archive` | `list_agents_for_archive` |
| `POST /v1/archives/{archive_id}/passages` | `create_passage_in_archive` | `create_passage_in_archive` |
| `DELETE /v1/archives/{archive_id}/passages/{passage_id}` | `delete_passage_from_archive` | `delete_passage_from_archive` (204) |

Notable in that file:
```python
class ArchiveCreateRequest(BaseModel):
    """Request model for creating an archive.

    Intentionally excludes vector_db_provider. These are derived internally (vector DB provider from env).
    """

    name: str
    embedding_config: Optional[EmbeddingConfig] = Field(
        None, description="Deprecated: Use `embedding` field instead. Embedding configuration for the archive", deprecated=True
    )
    embedding: Optional[str] = Field(None, description="Embedding model handle for the archive")
    description: Optional[str] = None
```
i.e. **the vector DB provider is deliberately not client-settable.** And `list_archives` supports `agent_id: Optional[str] = Query(None, description="Only archives attached to this agent ID")`.

**Attach / detach** — in `letta/server/rest_api/routers/v1/agents.py`, `router = APIRouter(prefix="/agents", tags=["agents"])`:
```python
@router.patch("/{agent_id}/archives/attach/{archive_id}", response_model=None, operation_id="attach_archive_to_agent")
async def attach_archive_to_agent(
    agent_id: str,
    archive_id: str,
    ...
):
    ...
    await server.archive_manager.attach_agent_to_archive_async(
        agent_id=agent_id,
        archive_id=archive_id,
        ...
    )


@router.patch("/{agent_id}/archives/detach/{archive_id}", response_model=None, operation_id="detach_archive_from_agent")
async def detach_archive_from_agent(
    agent_id: str,
    archive_id: str,
    ...
):
    ...
    await server.archive_manager.detach_agent_from_archive_async(
        agent_id=agent_id,
        archive_id=archive_id,
        ...
    )
```

⚠️ **Correction to the brief's guessed shape.** The real paths are:
- **`PATCH /v1/agents/{agent_id}/archives/attach/{archive_id}`** — operationId `attach_archive_to_agent`
- **`PATCH /v1/agents/{agent_id}/archives/detach/{archive_id}`** — operationId `detach_archive_from_agent`

The verb is **PATCH, not POST**, and `archive_id` comes **after** the literal `attach`/`detach` segment — *not* `/agents/{agent_id}/archives/{archive_id}/attach`. This matches the docs' generated SDK nav, which lists **Agents → Archives → Attach / Detach** (https://docs.letta.com/api/resources/agents/subresources/archives/methods/attach, https://docs.letta.com/api/resources/agents/subresources/archives/methods/detach) and lists top-level **Archives** under a **"Deprecated"** section (https://docs.letta.com/api/resources/archives).

Adjacent endpoints in the same router for comparison: `PATCH /{agent_id}/tools/attach/{tool_id}` (`attach_tool_to_agent`), `PATCH /{agent_id}/tools/detach/{tool_id}`, and a `/{agent_id}/passages` family. **[VERIFIED]**

### 5.7 Embedding and chunking **[VERIFIED]**

**`letta/embeddings.py` does not exist** (HTTP 404 at the pinned SHA). There is no `EMBEDDING_*` constants module; embedding constants live in `letta/constants.py`:
```python
# number of concurrent embedding requests to sent
EMBEDDING_BATCH_SIZE = 200
...
# embeddings
MAX_EMBEDDING_DIM = 4096  # maximum supported embeding size - do NOT change or else DBs will need to be reset
DEFAULT_EMBEDDING_CHUNK_SIZE = 300
DEFAULT_EMBEDDING_DIM = 1024

# tokenizers
EMBEDDING_TO_TOKENIZER_MAP = {
    "text-embedding-3-small": "cl100k_base",
}
EMBEDDING_TO_TOKENIZER_DEFAULT = "cl100k_base"
```

**Embedding entry points:**
- `letta/services/passage_manager.py` defines the legacy direct helpers:
```python
# TODO: Add redis-backed caching for backend
@lru_cache(maxsize=8192)
def get_openai_embedding(text: str, model: str, endpoint: str) -> List[float]:
    from letta.settings import model_settings
    client = OpenAI(api_key=model_settings.openai_api_key, base_url=endpoint, max_retries=0)
    response = client.embeddings.create(input=text, model=model)
    return response.data[0].embedding


@async_redis_cache(key_func=lambda text, model, endpoint: f"{model}:{endpoint}:{text}")
async def get_openai_embedding_async(text: str, model: str, endpoint: str) -> list[float]:
    ...
```
- The modern path is provider-agnostic and goes through `LLMClient`:
```python
        embedding_client = LLMClient.create(
            provider_type=agent_state.embedding_config.embedding_endpoint_type,
            actor=actor,
        )
        ...
        embeddings = await embedding_client.request_embeddings(text_chunks, agent_state.embedding_config)
```
- The OpenAI implementation of `request_embeddings` (`letta/llm_api/openai_client.py`) does **batching, not chunking**, with an escalating-split retry:
```python
        """Request embeddings given texts and embedding config with chunking and retry logic

        Retry strategy prioritizes reducing batch size before chunk size to maintain retrieval quality:
        1. Start with batch_size=2048 (texts per request)
        2. On failure, halve batch_size until it reaches 1
        3. Only then start reducing chunk_size (for very large individual texts)
        """
        ...
        initial_batch_size = 2048
        chunks_to_process = [(i, inputs[i : i + initial_batch_size], initial_batch_size) for i in range(0, len(inputs), initial_batch_size)]
        min_chunk_size = 128
```
  Note `EMBEDDING_BATCH_SIZE = 200` in `constants.py` is **not** what this function uses (it hard-codes 2048/128). ⚠️ I did not locate a consumer of `EMBEDDING_BATCH_SIZE` — **NOT VERIFIED**.
- Empty/whitespace inputs are replaced with `" "` to avoid API rejection:
```python
            if not inp or not inp.strip():
                logger.warning(f"Empty or whitespace-only input at index {idx}, replacing with placeholder")
                valid_inputs.append(" ")
```

**Embedding config** — `letta/schemas/embedding_config.py` (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/schemas/embedding_config.py):
```python
class EmbeddingConfig(BaseModel):
    """Configuration for embedding model connection and processing parameters."""

    embedding_endpoint_type: Literal[
        "openai", "anthropic", "bedrock", "google_ai", "google_vertex", "azure", "groq", "ollama",
        "webui", "webui-legacy", "lmstudio", "lmstudio-legacy", "llamacpp", "koboldcpp", "vllm",
        "hugging-face", "mistral", "together",  # completions endpoint
        "pinecone",
    ] = Field(..., description="The endpoint type for the model.")
    embedding_endpoint: Optional[str] = Field(None, description="The endpoint for the model (`None` if local).")
    embedding_model: str = Field(..., description="The model for the embedding.")
    embedding_dim: int = Field(..., description="The dimension of the embedding.")
    embedding_chunk_size: Optional[int] = Field(300, description="The chunk size of the embedding.")
    handle: Optional[str] = Field(None, description="The handle for this config, in the format provider/model-name.")
    batch_size: int = Field(32, description="The maximum batch size for processing embeddings.")
```
with `default_config` returning `text-embedding-3-small` / `embedding_dim=1536` for OpenAI, `letta-free` / `1536` for `model_name == "letta"`, and `llama-text-embed-v2` / `1536` for `provider == "pinecone"`.

**Chunking — the key finding: archival-memory inserts are NOT chunked.** In `PassageManager.insert_passage`:
```python
        embedding_chunk_size = agent_state.embedding_config.embedding_chunk_size
        embedding_client = LLMClient.create(
            provider_type=agent_state.embedding_config.embedding_endpoint_type,
            actor=actor,
        )

        # Get or create the default archive for the agent
        archive = await self.archive_manager.get_or_create_default_archive_for_agent_async(agent_state=agent_state, actor=actor)

        # TODO: check to make sure token count is okay for embedding model
        text_chunks = [text]
```
`embedding_chunk_size` is read into a local and then **never used**; `text_chunks` is a single-element list. So a whole `archival_memory_insert` payload becomes **one** passage and **one** embedding. This is corroborated by `CreateAgent`:
```python
    embedding_chunk_size: Optional[int] = Field(
        DEFAULT_EMBEDDING_CHUNK_SIZE, description="Deprecated: No longer used. The embedding chunk size used by the agent.", deprecated=True
    )
```
(`letta/schemas/agent.py`.)

Where a real chunker **does** exist: **files/folders**, not archival memory — `letta/services/file_processor/chunker/line_chunker.py` (`class LineChunker`, `chunk_text(file_metadata, start=None, end=None, add_metadata=True, validate_range=False)`), dispatching on a `ChunkingStrategy` from `letta/services/file_processor/file_types.py`:
```python
        if strategy == ChunkingStrategy.DOCUMENTATION:
            content_lines = self._chunk_by_sentences(text)
        elif strategy == ChunkingStrategy.CODE:
            content_lines = self._chunk_by_lines(text, preserve_indentation=True)
        else:  # STRUCTURED_DATA or LINE_BASED
            content_lines = self._chunk_by_lines(text, preserve_indentation=False)
```
It splits on **lines**, or on sentence boundaries via `re.split(r"(?<=[.!?])\s+(?=[A-Z])", ...)` for documentation; it does **not** tokenize. `AgentManager` imports it as `from letta.services.file_processor.chunker.line_chunker import LineChunker`.

**Turbopuffer's embedding config is hard-coded, independent of the archive's:**
```python
class TurbopufferClient:
    """Client for managing archival memory with Turbopuffer vector database."""

    default_embedding_config = EmbeddingConfig(
        embedding_model="text-embedding-3-small",
        embedding_endpoint_type="openai",
        embedding_endpoint="https://api.openai.com/v1",
        embedding_dim=1536,
        embedding_chunk_size=DEFAULT_EMBEDDING_CHUNK_SIZE,
    )
```
and in `insert_archival_memories`: `# generate embeddings using the default config`. So with TPUF the query vector and the write vector are both OpenAI `text-embedding-3-small`@1536 — **not** the archive's `embedding_config`. ⚠️ This is a real asymmetry with the pgvector/sqlite path, which uses `agent_state.embedding_config` / `archive.embedding_config`. Also note the semantic conflict with `MAX_EMBEDDING_DIM = 4096`: TPUF vectors are 1536-dim and unpadded (`# Always pad when writing to Postgres vector DB (don't pad for Turbopuffer/Pinecone)`).

Turbopuffer write mechanics (quotable):
```python
        _GLOBAL_TURBOPUFFER_SEMAPHORE = asyncio.Semaphore(5)
        ...
                    await namespace.write(
                        upsert_columns=upsert_columns,
                        distance_metric="cosine_distance",
                        schema={"text": {"type": "string", "full_text_search": True}},
                    )
```
`upsert_columns` for archival writes = `{"id", "vector", "text", "organization_id", "archive_id", "created_at", "tags"}`. Query modes supported by `_execute_query`: `"vector"` (ANN), `"fts"` (BM25), `"hybrid"` (both + reciprocal-rank fusion), `"timestamp"`. RRF: `combined_score = vector_weight / (k + vector_rank) + fts_weight / (k + fts_rank)` with defaults `vector_weight = fts_weight = 0.5`. Deletion uses `await namespace.write(deletes=[...])` and `delete_all()`.

**pgvector/sqlite vector search** lives in SQL, not the client — `letta/services/helpers/agent_manager_helper.py`:
```python
    # Vector search
    if embedded_text:
        if settings.database_engine is DatabaseChoice.POSTGRES:
            # PostgreSQL with pgvector
            main_query = main_query.order_by(combined_query.c.embedding.cosine_distance(embedded_text).asc())
        else:
            # SQLite with custom vector type
            from letta.orm.sqlite_functions import adapt_array
            query_embedding_binary = adapt_array(embedded_text)
            main_query = main_query.order_by(
                func.cosine_distance(combined_query.c.embedding, query_embedding_binary).asc(),
                ...
            )
```
Query embeddings are padded to `MAX_EMBEDDING_DIM` before searching:
```python
        embedded_text = np.array(embeddings[0])
        embedded_text = np.pad(embedded_text, (0, MAX_EMBEDDING_DIM - embedded_text.shape[0]), mode="constant").tolist()
```
Relevant query builders: `build_passage_query` (union of source + archival passages via `union_all(...).cte("combined_passages")`), `build_source_passage_query`, `build_agent_passage_query(actor, agent_id=None, archive_id=None, ...)` — note `"If both are provided, agent_id takes precedence."`

---

## 6. Summary of corrections & explicit non-findings

**Corrections to premises in the brief:**
1. `letta/embeddings.py` **does not exist** at the pinned SHA (HTTP 404). Embedding helpers are in `letta/services/passage_manager.py` + `LLMClient.request_embeddings`.
2. `archival_memory_insert`, `archival_memory_search`, `conversation_search`, `send_message` are in **`base.py`**, not `extras.py`.
3. There is **no `manager` group type**. `ManagerType` = `round_robin | supervisor | dynamic | sleeptime | voice_sleeptime | swarm`.
4. `sleeptime_agent_frequency` is a **group** field (`Group`, `SleeptimeManager`), not an agent field. The agent field is `enable_sleeptime: Optional[bool]`. There is no `sleeptime_agent` boolean field.
5. Archive attach/detach is **`PATCH /v1/agents/{agent_id}/archives/attach/{archive_id}`** and **`.../detach/{archive_id}`** — not `POST .../archives/{archive_id}/attach`.
6. `core_memory_append` / `core_memory_replace` are **not** in any `DEPRECATED_*` list; the tools carrying the explicit deprecation tag are **`archival_memory_insert` and `archival_memory_search`**.
7. `archives_agents` is described as many-to-many but is constrained to **one archive per agent** today.

**Explicitly NOT VERIFIED:**
- New path of `letta/functions/function_sets/base.py` (and the rest of `letta/functions/`) on the current `main` branch.
- Any `PROMPT_CACHE_*` constant anywhere in the repo (absent from `anthropic_client.py`, `constants.py`, `settings.py`, `anthropic_constants.py` — all read in full; repo-wide search unavailable).
- Absence of Chroma / Milvus / Qdrant / LanceDB / Elasticsearch anywhere in the repo.
- Letta Cloud's actual production vector backend (inference only, clearly labelled in §5.3).
- Doc-level statements about deprecated tools or `request_heartbeat` (the docs pages for legacy architectures were not successfully fetched).
- A consumer of `EMBEDDING_BATCH_SIZE = 200`.
- The complete `letta/services/` and `tests/` directory listings (GitHub REST API rate-limited at `HTTP 403`).
