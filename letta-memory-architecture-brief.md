# Letta (ex-MemGPT) Memory Architecture — Technical Brief

**Research date:** current as of the latest accessible Letta docs/blog (blog index runs to **AUG 2026**; site footer reads "© 2026 Letta").
**Method:** primary sources only — `docs.letta.com/*.md`, `letta.com/blog/*`, `arxiv.org`, `github.com/letta-ai/*`. Every claim carries a URL. Unverified items are flagged **[NOT VERIFIED]**.

---

## 0. CRITICAL FRAMING: Letta has been restructured *twice*. Read this before using any doc page.

There are **three distinct eras** in the sources, and conflating them is the single biggest error risk in this report:

| Era | Doc location today | Status label | Core memory abstraction |
|---|---|---|---|
| **A. MemGPT paper (2023)** | arXiv:2310.08560 | — | main context (system instructions + working context + FIFO) vs external context (recall + archival) |
| **B. Letta Platform / "V1 SDK" (2024–2025)** | `docs.letta.com/v1-sdk/...` | **literally `status: legacy` in frontmatter** | memory blocks ("core memory") + archival memory + files |
| **C. Letta Harness / MemFS era (2026)** | `docs.letta.com/concepts/memfs`, `/configuration/memory`, `/agent-sdk/memory` | current | **git-versioned memory filesystem (MemFS)** + "dreaming" |

Evidence for B being explicitly legacy — the fetched page returns this frontmatter verbatim:

```
---
title: Context hierarchy | Letta Docs
applies_to:
  backends:
    - cloud
  interfaces:
    - sdk
status: legacy
---
```
Source: https://docs.letta.com/v1-sdk/memory/context-hierarchy.md (and identically for `.../archival-memory.md` and `.../tools/builtin-tools.md`)

Evidence for C: the live docs nav has no `Guides` section at all; `v1-sdk` is nested under the heading **"V1 SDK (legacy)"**, and the top-level concepts are now Stateful agents / Conversations / **MemFS** / Shared memory.
Source: https://docs.letta.com/llms.txt

**Broken-URL warning:** the URL you supplied, `https://docs.letta.com/guides/agents/architectures.md`, now returns **HTTP 404**. Same for `/guides/agents/groups.md`, `/guides/agents/base-tools.md`, `/guides/agents/context-engineering.md`, `/guides/legacy/migration-guide/`. Those pages existed under a Fern-based docs site in 2025–early 2026 and are recoverable only via the Wayback Machine (I used it; snapshots cited below).

---

## 1. The context hierarchy — exact tiers, and how it maps to the paper

### 1.1 What the *current* (2026) Letta says — the authoritative source

The single best current primary source is the **Context Constitution**, which `llms.txt` describes as "the principles governing what a Letta agent places in its context window, in what order, at what level of detail, and for how long."
Source: https://docs.letta.com/llms.txt → https://github.com/letta-ai/context-constitution

Its own definition:

> "The Context Constitution is a set of rules and principles governing what a Letta agent places in its context window, in what order, at what level of detail, and for how long."
> — https://raw.githubusercontent.com/letta-ai/context-constitution/main/constitution/CONSTITUTION.md

**The exact tiers (verbatim section "Context Architecture"):**

> "Letta agents have context architecture consisting of simple primitives organized in a context hierarchy."

**Tier-defining sentence — note this is a TWO-tier split, not a four-tier one:**

> "A Letta agent's memory is split hierarchically. The top-level split is between two tiers: **the context window** (what the model sees on each inference cycle) and **external context** (everything else the Letta agent can access through taking action)."
> — https://raw.githubusercontent.com/letta-ai/context-constitution/main/constitution/CONSTITUTION.md

**The four "Context Primitives" of the context window (verbatim headings + definitions):**

1. **System Prompt** — "the fixed preamble prepended to every inference call. It defines the Letta agent's identity, behavioral guidelines, persistent goals, and background context that is always relevant. It sits at the front of the context window on every call…"
2. **Tools** — "functions the Letta agent invokes to act on the world or retrieve external information. Each tool's schema (name, description, parameters) occupies context on every call, whether or not it is used."
3. **Messages** — "a record of the conversation history, or 'events,' containing user or system/automated inputs, reasoning traces, agent outputs, tool calls, and tool responses." Critically: "Letta agents have their conversation history automatically managed by the system — old messages will be periodically evicted from the conversation history and replaced with a recursive summary ('compaction'), yet **all messages are persisted and remain retrievable through active tool calling**."
4. **Skills** — "folders of reusable, on-demand context… Skills follow the principle of progressive disclosure: the Letta agent loads only what is relevant to the current task."

**Answer to your specific question — is "recall storage" still a separate concept?**
In the **current** architecture, **no**. There is no recall-storage *tier*. The Constitution replaces it with two mechanisms:
- **Compaction**: evicted messages → recursive summary (exactly the paper's mechanism, but now framed as context management rather than a queue-manager tier).
- **Tool-based retrieval**: "all messages are persisted and remain retrievable through active tool calling." Also: "recall that Letta agents automatically store all prior experiences in a queryable database."

MemFS docs make the same point operationally, splitting memory from conversation history:
> "**Conversation-history search is separate from MemFS.** On Letta Cloud, `letta messages search` supports full-text, vector, and hybrid search over messages; local backends currently search conversation transcripts with full-text matching only."
> — https://docs.letta.com/concepts/memfs/index.md

### 1.2 What the page you asked for actually says (era B / legacy)

`https://docs.letta.com/v1-sdk/memory/context-hierarchy.md` **does not define ordered tiers.** It defines four *abstractions for supplying external context*, with this exact table:

| | Access | In-Context | Tools | Size Limit | Count Limit |
|---|---|---|---|---|---|
| **Memory Blocks** | Editable (optional read-only) | Yes | `memory_rethink` `memory_replace` `memory_insert` & custom tools | Recommended <50k characters | Recommended <20 blocks per agent |
| **Files** | Read-only | Partial (files can be opened/closed) | `open` `close` `semantic_search` `grep` | 5MB | Recommended <100 files per agent |
| **Archival Memory** | Read-write | No | `archival_memory_insert` `archival_memory_search` & custom tools | 300 tokens | Unlimited |
| **External RAG** | Read-write | No | Custom tools or MCP | Unlimited | Unlimited |

Source: https://docs.letta.com/v1-sdk/memory/context-hierarchy.md

### 1.3 The six named context-window components (era B, blog) — verbatim

Letta's own blog names the anatomy explicitly. Per the JUL 3 2025 post *"Anatomy of a Context Window"*:

> "An agent's context window consists of the following components:
> - **System prompt**: Defines the agent's architecture and control flow, providing high-level behavioral instructions and explaining core concepts like memory, blocks, and files.
> - **Tool schemas**: Specifications that define available tools and their interfaces, enabling the agent to understand what actions it can perform.
> - **System metadata:** Stores statistics and metadata about the agent's state (e.g. the size of the full message history).
> - **Memory blocks**: Persistent units of context (e.g. for long term memory or working context), managed by the agent itself or other background agents.
> - **Files & Artifacts**: Files (PDFs, source code, etc.) that the agent can access and manipulate.
> - **Message buffer**: The message stream containing messages (user, assistant, and system, and tool calls and tool returns)."
> — https://www.letta.com/blog/guide-to-context-engineering/

The "hierarchy" that post actually names is a **two-scope OS split**, not an ordered tier list:

> "Like resources in traditional operating systems, in an LLM OS, *context* is scoped to the application layer (i.e. user space) and the system layer (i.e. kernel space):
> - **Kernel Context** - Managed context that can be modified through APIs and tool calling (like kernel memory). This represents the underlying agent configuration (the system prompt and tools), as well as managed context such as memory blocks and files.
> - **User Context** - The message buffer containing conversations and external context (like user processes), as well as system calls (tools) to modify the kernel context."
> "Kernel context represents the managed, mutable state of an agent—analogous to kernel memory in an operating system… Crucially, the kernel context is rendered into the context window where the LLM can observe it, but modifications happen through controlled interfaces."
> — same URL

Message-buffer sub-types, verbatim: "**User messages**: Direct input from users" / "**Assistant messages**: The agent's responses" / "**System messages**: Framework or system-generated notifications (like system logs)" / "**Tool calls and responses**: The log of both system calls and other tools, as well as their results".

**[NOT VERIFIED: no post states the render order of these components into the prompt.]** Post 2 says only that "The prompt template (i.e. formatting in the context window) **can be customized with Jinja templating**." (https://www.letta.com/blog/memory-blocks/)

⚠️ **Retrieval limitation to note:** post 1's kernel/user-context figure is an image with **no alt text or caption**; labels drawn inside it are unrecoverable from the HTML. Only two text captions exist across posts 1 and 3.

### 1.3b The four named memory types (JUL 7 2025) — verbatim, and the recall tier is explicit

> "**The message buffer** stores the most recent messages in a conversation. In Letta, every agent maintains a single perpetual thread…"
> "**Core memory** consists of in-context memory blocks that can be managed by the agent itself or by other agents… these blocks are editable via APIs and remain pinned to the agent's context window…"
> "**Recall memory** preserves the complete history of interactions that can be searched and retrieved when needed, even when not in the active context window (i.e., in the message buffer). In Letta, **recall memory saves to disk automatically**, while other frameworks require developers to handle persistence manually."
> "**Archival memory** represents explicitly formulated knowledge stored in external databases. Unlike recall memory, which stores raw conversation history, archival memory contains processed and indexed information…"
> "Agents can move data between in-context core memory (analogous to RAM) and externally stored archival and recall memory (analogous to disk storage)…"
> Also: "Generally, you should evict only a portion (e.g., **70%**) of messages to ensure continuity" and "**Recursive Summarization:** Evicted messages undergo recursive summarization—they're summarized along with existing summaries from previously summarized messages."
> — https://www.letta.com/blog/agent-memory/

So as of **JUL 2025 recall was still a distinct, separately persisted tier** — it was collapsed only in the later MemFS rewrite (§1.4). The word **"compaction" does not appear** in that post; the concept appears as "Message Eviction & Summarization."

### 1.4 Mapping: 2023 paper → 2025 → 2026

**Paper (2023), verbatim:**
> "MemGPT's OS-inspired multi-level memory architecture delineates between two primary memory types: **main context** (analogous to main memory/physical memory/RAM) and **external context** (analogous to disk memory/disk storage). Main context consists of the LLM *prompt tokens*… External context refers to any information that is held outside of the LLMs fixed context window."
> — https://arxiv.org/html/2310.08560v2 (§2)

> "The prompt tokens in MemGPT are split into three contiguous sections: **the system instructions, working context, and FIFO Queue**. The system instructions are read-only (static)… Working context is a fixed-size read/write block of unstructured text, writeable only via MemGPT function calls… The FIFO queue stores a rolling history of messages… **The first index in the FIFO queue stores a system message containing a recursive summary of messages that have been evicted from the queue.**"
> — https://arxiv.org/html/2310.08560v2 (§2.1)

Figure 3 caption, verbatim:
> "The LLM's prompt tokens (inputs), or *main context*, consist of the system instructions, working context, and a FIFO queue. The LLM completion tokens (outputs) are interpreted as function calls by the function executor. MemGPT uses functions to move data between main context and *external context* (the archival and recall storage databases)."
> — https://arxiv.org/html/2310.08560v2

| Paper (2023) | Letta 2025 (era B) | Letta 2026 (era C, MemFS) |
|---|---|---|
| system instructions (read-only, static) | system prompt | **System Prompt** primitive (agent-rewritable — "system prompt learning") |
| working context (fixed-size read/write text block) | **memory blocks** ("core memory"), injected as XML `<memory_blocks>` | **files under `system/`** in MemFS, in the system prompt every turn |
| FIFO queue (rolling message history) | message buffer | **Messages** primitive + compaction |
| recursive summary at FIFO index 0 | compaction summary | **compaction** ("recursive summary") |
| recall storage (message DB, searchable) | **Recall Memory** tier (still named in JUL 2025) | **gone as a tier** → `letta messages search` / tool-based retrieval |
| archival storage (vector DB) | **Archival Memory** (`archival_memory_insert/search`) | **legacy**; default MemFS has **no** vector index — see §3 |

**The biggest single change:** working context → memory blocks → **a git repository projected as a filesystem**. Letta states it directly:
> "**MemFS** is how a Letta agent works with its long-term memory. The memory itself is part of the agent's state, held in a git repository that belongs to the agent… MemFS **projects** that repository onto whatever computer the agent is running on, as a real checkout the agent reads and edits with ordinary file tools."
> — https://docs.letta.com/concepts/memfs/index.md

And:
> "Files under `system/` are loaded into the agent's system prompt on every turn… Files outside `system/` stay out of context until they are needed. The file tree itself is always in the system prompt, so directory and file names act as signposts the agent follows to read the right file."
> — https://docs.letta.com/concepts/memfs/index.md

> "**MemFS does not include a semantic or vector index by default.** Agents find memory in its Markdown files with normal file-search and read tools."
> — https://docs.letta.com/concepts/memfs/index.md

Note the recursive-summary mechanism survived intact, but its *trigger* is now framed as compaction with explicit modes. The API exposes `compaction_settings` with `mode: "all" | "sliding_window" | "self_compact_all" | "self_compact_sliding_window"`, plus `clip_chars`, `model`, `prompt`, `prompt_acknowledgement`, `sliding_window_percentage`.
Source: https://docs.letta.com/api/resources/agents/methods/create/index.md

### 1.5 ⚠️ Version trap — cite the MemGPT paper BY VERSION

The two arXiv versions name the three parts of main context **differently**. This matters for exact quotes:

- **v1 (§2.1):** "we further divide main context into three components: **system instructions**… **conversational context**, which holds a first-in-first-out (FIFO) queue of recent event history…, and **working context**, which serves as a working memory scratchpad for the agent. System instructions are read-only and **pinned** to main context (they do not change during the lifetime of the MemGPT agent), conversational context is **read-only with a special eviction policy**…, and working context is **writeable by the LLM processor via function calls**."
- **v2 (§2.1 / Fig. 3):** "the system instructions, **working context, and a FIFO Queue**."

Quote **v1** for the RAM/disk analogy, the OS framing, and the external-context definition; quote **v2** for the FIFO naming and the queue manager. v1's external-context sentence is the most quotable:
> "We make a distinction between two types of external context: **recall storage**, which stores the entire history of events processed by the LLM processor (in essense the full uncompressed queue from active memory), and **archival storage**, which serves as a general **read-write datastore** that the agent can utilize as **overflow for the in-context read-write core memory**."

Also v1: "In MemGPT we refer to the LLM inputs (that are bound by the maximum number of input tokens) as the system's **main context**" — and "Combined, the three parts of main context **cannot exceed** the underlying LLM processors's maximum context size."

**Terminology trap — "core memory" is the agent-facing name.** v2 uses "working context" in §2.1/Fig. 3 but "**core memory**" in §2.3; the agent-facing system prompt is entirely "core memory". Safe formulation: main context = system instructions + working context (= "core memory" in the agent prompt, writable) + FIFO queue; external context = recall storage + archival storage.

**Function rename v1→v2:** v1 Figures 2 and 4 show **`working_context_append`** and **`working_context.replace`** (e.g. `working_context_append('Birthday: 11th October.\nFavorite cake: Chocolate Lava made by mom Brenda')`), which v2 renamed to **`core_memory_append`** / **`core_memory_replace`**. "Core memory" as a term postdates v1.

### 1.6 The 2023 paper's functions — verbatim descriptions and argument lists

Sourced from the function spec the MemGPT authors released with the paper (dataset `MemGPT/function-call-traces`; the card states the traces "were generated while evaluating MemGPT (arXiv:2310.08560)"). Required args in **bold**; `request_heartbeat` is `"required"` wherever present.

| Function | `description` (verbatim) | parameters (in order) |
|---|---|---|
| `send_message` | "Sends a message to the human user" | `message` (string) |
| `pause_heartbeats` | "Temporarily ignore timed heartbeats. You may still receive messages from manual heartbeats and other events." | `minutes` (integer) — "Number of minutes to ignore heartbeats for. **Max value of 360 minutes (6 hours).**" |
| `message_chatgpt` | "Send a message to a more basic AI, ChatGPT…" | `message`, `request_heartbeat` |
| `core_memory_append` | "Append to the contents of core memory." | `name` — "Section of the memory to be edited (persona or human)."; `content`; `request_heartbeat` |
| `core_memory_replace` | "Replace to the contents of core memory. To delete memories, use an empty string for new_content." | `name`; `old_content` — "String to replace. Must be an exact match."; `new_content`; `request_heartbeat` |
| `recall_memory_search` | "Search prior conversation history using a string." | `query`; `page` — "Allows you to page through results. Defaults to 0 (first page)."; `request_heartbeat` |
| `conversation_search` | "Search prior conversation history using **case-insensitive string matching**." | `query`; `page`; `request_heartbeat` |
| `recall_memory_search_date` | "Search prior conversation history using a date range." | `start_date` — "'YYYY-MM-DD'"; `end_date`; `page`; `request_heartbeat` |
| `conversation_search_date` | "Search prior conversation history using a date range." | `start_date`; `end_date`; `page`; `request_heartbeat` |
| `archival_memory_insert` | "Add to archival memory. Make sure to phrase the memory contents such that it can be easily queried later." | `content`; `request_heartbeat` |
| `archival_memory_search` | "Search archival memory using **semantic (embedding-based) search**." | `query`; `page`; `request_heartbeat` |

**Resolving the recall-search naming question:** in 2023 the recall-storage search function was called **`recall_memory_search`**, while **`conversation_search` was the Multi-Session-Chat variant** — both appear in the *same* schema, and the agent instructions select one per configuration (MSC prompt: "You can search your recall memory using the **'conversation_search'** function"; DocQA prompt: "…using the **'recall_memory_search'** function"). **`conversation_search_date` does exist** — not a hallucination — but Letta's code carries `# Note that we don't include "conversation_search_date" for now`, so it is **not** in the modern default toolset. There is no `recall_memory_insert`.

**"Inner monologue" — ⚠️ NOT in the papers.** The term appears only in the released system prompt: "When you send a message, the contents of your message are your **inner monologue (private to you only)**, this is how you think… Do not let your inner monologue exceed 50 words." The paper's nearest phrasing is "LLM is able to '**plan out loud**' when executing functions."

**Modern drift:** `request_heartbeat` is **absent** from the modern `core_memory_append`/`core_memory_replace` signatures in `letta/functions/function_sets/base.py`.

---

## 2. Memory-block tools — exact names, semantics, legacy vs current

### 2.1 The definitive source is the codebase, not the docs

The current docs **do not enumerate memory-edit tools** — `https://docs.letta.com/v1-sdk/tools/builtin-tools.md` covers only `web_search`, `fetch_webpage`, `run_code`. The tool registry lives in `letta/constants.py`. Pinned commit that resolves reliably: `67013ef1bb491ec8bdd629bfbeceb70227936afa` (the repo's `main` 404s on these paths; the tree was reorganized).

```
BASE_TOOLS = [SEND_MESSAGE_TOOL_NAME, "conversation_search",
              "archival_memory_insert", "archival_memory_search"]
DEPRECATED_LETTA_TOOLS = ["archival_memory_insert", "archival_memory_search"]
BASE_MEMORY_TOOLS      = ["core_memory_append", "core_memory_replace", "memory", "memory_apply_patch"]
BASE_MEMORY_TOOLS_V2   = ["memory_replace", "memory_insert"]
BASE_MEMORY_TOOLS_V3   = ["memory"]
BASE_SLEEPTIME_CHAT_TOOLS = [SEND_MESSAGE_TOOL_NAME, "conversation_search",
                             "archival_memory_search"]
BASE_SLEEPTIME_TOOLS   = ["memory_replace", "memory_insert", "memory_rethink",
                          "memory_finish_edits"]
REQUEST_HEARTBEAT_PARAM = "request_heartbeat"   # + full REQUEST_HEARTBEAT_DESCRIPTION string
DEFAULT_MAX_STEPS = 50
```
Source: https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/constants.py

### 2.2 What replaced `core_memory_append` / `core_memory_replace` — it is **both**, in two generations

**Generation 1 (v2 toolset) — the split pair:**
`core_memory_append` + `core_memory_replace` → **`memory_insert`** + **`memory_replace`** (`BASE_MEMORY_TOOLS_V2`). Semantics: `memory_insert` appends/inserts into a labelled block; `memory_replace` performs a targeted string replacement inside a labelled block. A third tool **`memory_rethink`** lets the agent rewrite an entire block, and **`memory_finish_edits`** terminates a sleep-time edit session.

**Generation 2 (v3 toolset) — the omni-tool:**
`BASE_MEMORY_TOOLS_V3 = ["memory"]` — a single tool named **`memory`** with a `command=...` parameter, i.e. a consolidated memory omni-tool. The code comment describes it as "v3 collection, currently just a omni memory tool for anthropic".

**Naming precision (important):** the *exposed tool name* is **`memory`** — that is Anthropic's tool name. **"Memory omni-tool" is Letta's descriptive label, not a literal tool name.** Letta's announcement:
> "You can now use Anthropic's `memory` tool with Letta agents, enabling Letta agents to take full advantage of Claude Sonnet 4.5's advanced memory capabilities to dynamically manage their own memory blocks. Agents in Letta can now not only modify existing memory blocks, but also create and delete memory blocks on-the-fly as they learn over time."
> "The Letta `memory` tool exposes the same filesystem-like API to the model as Anthropic's version (storing memories in specific paths), but **under the hood, the memory 'files' are actually stored inside of Letta memory blocks**."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/ (SEP 30 2025)

Note the *link targets* are https://docs.claude.com/en/docs/agents-and-tools/tool-use/memory-tool and https://www.anthropic.com/news/context-management. ⚠️ **[NOT VERIFIED: the strings `memory_20250818`, `clear_tool_uses`, `tool_use clearing`, and the unhyphenated phrase "context editing" do NOT appear in that post]** — the post only says "a **context-editing** memory tool" (hyphenated) and links out. Do not attribute Anthropic's context-editing/tool-use-clearing features to Letta on the basis of this post.

⚠️ **Documentation inconsistency worth flagging in your report:** the only built-in memory tool names Letta prints in the blog are **`memory_replace`, `memory_rethink`, and `memory_append`** (https://www.letta.com/blog/guide-to-context-engineering/). The codebase's V2 set is `memory_replace`/`memory_insert` — **`memory_append` appears in the blog but not in `BASE_MEMORY_TOOLS_V2`**, while `memory_insert` appears in code but not in the blog. Treat `memory_append` as either a typo or an unlisted alias. The blog posts also contain **no** deprecation statement for `core_memory_append`/`core_memory_replace` and never mention `memory_insert`.

**Also present: `memory_apply_patch`** (in `BASE_MEMORY_TOOLS`), a diff-style block editor.

**`ToolType` enum** (which toolset a tool belongs to) — exact values:
`"custom"`, `"letta_core"`, `"letta_memory_core"`, `"letta_multi_agent_core"`, `"letta_sleeptime_core"`, `"letta_voice_sleeptime_core"`, `"letta_builtin"`, `"letta_files_core"`, `"external_langchain"`, `"external_composio"`, `"external_mcp"`
Source: https://docs.letta.com/api/resources/agents/methods/create/index.md

### 2.3 Status of each tool you listed

| Tool | Status | Notes |
|---|---|---|
| `core_memory_append` | **LEGACY** (`BASE_MEMORY_TOOLS`, the original v1 set) | superseded by `memory_insert` / `memory` |
| `core_memory_replace` | **LEGACY** (same set) | superseded by `memory_replace` / `memory` |
| `memory_insert` | **Current** (V2) + assigned to sleep-time agent | |
| `memory_replace` | **Current** (V2) + assigned to sleep-time agent | |
| `memory_rethink` | **Current** (V2 / sleep-time set) | whole-block rewrite |
| `memory_finish_edits` | **Current** (sleep-time set only) | terminates an edit session |
| `memory` (omni) | **Current** (V3) | single tool, `command=` param |
| `memory_apply_patch` | present in code | **[NOT VERIFIED: full arg signature]** |
| `archival_memory_insert` | **EXPLICITLY DEPRECATED** — listed in `DEPRECATED_LETTA_TOOLS` | still in `BASE_TOOLS` |
| `archival_memory_search` | **EXPLICITLY DEPRECATED** — listed in `DEPRECATED_LETTA_TOOLS` | still in `BASE_TOOLS` |
| `conversation_search` | **Current**, in `BASE_TOOLS` | the recall-storage replacement |
| `send_message` | **DEPRECATED for `letta_v1_agent`** | "the `send_message` tool are deprecated. Only native reasoning and direct assistant message generations from the models are supported." |

Sources: constants URL above; https://www.letta.com/blog/letta-v1-agent/

> ⚠️ **Notable:** the *only* tools Letta explicitly marks with a `DEPRECATED_` constant are the two **archival** tools — not the core-memory pair. The core-memory pair is "legacy" by virtue of being in the older toolset list.

### 2.4 Argument signatures — RESOLVED (verbatim from source)

All from `letta/functions/function_sets/base.py` at the pinned SHA (https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/functions/function_sets/base.py). This closes what was gap #1 of this brief.

```python
def core_memory_append(agent_state: "AgentState", label: str, content: str) -> Optional[str]
def core_memory_replace(agent_state: "AgentState", label: str, old_content: str, new_content: str) -> Optional[str]
def memory_replace(agent_state: "AgentState", label: str, old_str: str, new_str: str) -> str
def memory_insert(agent_state: "AgentState", label: str, new_str: str, insert_line: int = -1) -> Optional[str]
def memory_apply_patch(agent_state: "AgentState", label: str, patch: str) -> str
def memory_rethink(agent_state: "AgentState", label: str, new_memory: str) -> None
def memory_finish_edits(agent_state: "AgentState") -> None
def rethink_memory(agent_state: "AgentState", new_memory: str, target_block_label: str) -> None
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

**Key semantics (docstrings, verbatim):**
- `memory_replace`: "allows you to replace a specific string in a memory block with a new string. This is used for making precise edits. **Do NOT attempt to replace long strings, e.g. do not attempt to replace the entire contents of a memory block with a new string.**" It rejects line-number-prefixed input (`re.search(r"\nLine \d+: ", old_str)`) and fails unless the match count is exactly 1.
- `memory_insert`: "allows you to insert text at a specific location in a memory block… `insert_line` (int): The line number after which to insert the text (0 for beginning of file). **Defaults to -1 (end of the file).**"
- `memory_rethink`: "allows you to **completely rewrite the contents of a memory block**. Use this tool to make large sweeping changes (e.g. when you want to condense or reorganize the memory blocks), **do NOT use this tool to make small precise edits**."
- `memory_finish_edits`: "Call the memory_finish_edits command when you are finished making edits… This function is called when the agent is done rethinking the memory."
- `conversation_search`: "Search prior conversation history using **hybrid search (text + semantic similarity)**." — i.e. the modern recall replacement is *hybrid*, not purely vector.
- `archival_memory_search`: "Search archival memory using semantic similarity… `tag_match_mode`: **"any" = match memories with ANY of the tags, "all" = match only memories with ALL tags**… `top_k`: Maximum number of results to return (**default: 10**)."
- `memory` documents sub-commands `"create" | "str_replace" | "insert" | "delete" | "rename"`.

⚠️ **Two rethink functions coexist** — `rethink_memory(agent_state, new_memory, target_block_label)` (older, matching the sleep-time paper's `rethink_memory`) **and** `memory_rethink(agent_state, label, new_memory)` (newer, in the shipped sleep-time tool set). Do not conflate them.

⚠️ **Stubs vs implemented:** real bodies exist for `core_memory_append`, `core_memory_replace`, `rethink_memory`, `memory_replace`, `memory_insert`, `memory_rethink`, `memory_finish_edits`, `send_message`, `conversation_search`. These four are **stubs** raising `NotImplementedError("This should never be invoked directly. Contact Letta if you see this error message.")`: **`memory`, `archival_memory_insert`, `archival_memory_search`, `memory_apply_patch`** — real work happens in the tool executor.

⚠️ **Correction:** `letta/functions/function_sets/extras.py` contains **no memory tool** — only `message_chatgpt`, `read_from_text_file`, `append_to_text_file`, `http_request`.

### 2.4b How the tool sets are actually selected — `calculate_base_tools`

`letta/services/helpers/agent_manager_helper.py`:
```python
def calculate_base_tools(is_v2: bool) -> Set[str]:
    if is_v2:
        return (set(BASE_TOOLS) - set(DEPRECATED_LETTA_TOOLS)) | set(BASE_MEMORY_TOOLS_V2)
    else:
        return (set(BASE_TOOLS) - set(DEPRECATED_LETTA_TOOLS)) | set(BASE_MEMORY_TOOLS)
```
`DEPRECATED_LETTA_TOOLS` is **subtracted in both branches** — so **`archival_memory_insert` and `archival_memory_search` are removed from the default toolset of every agent** built by this helper (both the `memgpt_agent` and v2 paths). They stay defined in `base.py` and stay in `BASE_SLEEPTIME_CHAT_TOOLS`.

`core_memory_append`/`core_memory_replace` remain reachable only via the `else` (v1/`memgpt_agent`) branch. Their replacement is structural, not flagged:
- **v2 path** (`memgpt_v2_agent`, and `letta_v1_agent`) → the **pair** `memory_replace` + `memory_insert`
- **v3 path** → the **single omni-tool** `memory`

`AgentType` default on create is `letta_v1_agent` (`Field(default_factory=lambda: AgentType.letta_v1_agent, ...)`).

⚠️ There is **no `LETTA_CORE_TOOL_NAMES` list**, **no `LETTA_SLEEPTIME_TOOL_MODULE_NAME`**, and **no `deprecated` decorator** on the memory functions. The `deprecated` decorator does exist (`letta/helpers/decorators.py`) but is used on `AnthropicClient.request`, not on `core_memory_*`.

### 2.5 Memory-block structure and prompt injection (era B, precise)

> "Under the hood, memory blocks are simply prepended to the agent's prompt in an XML-like format."
> — https://docs.letta.com/v1-sdk/memory/memory-blocks.md

Exact wire format shown in the docs:
```xml
<memory_blocks>
<persona>
<description>The persona block: Stores details about your current persona, guiding how you behave and respond.</description>
<metadata>
- chars_current=128
- chars_limit=5000
</metadata>
<value>I am a helpful assistant named Sam. I enjoy helping users solve problems.</value>
</persona>
...
</memory_blocks>
```

Block fields: **`label`** (unique identifier), **`description`** ("the main information used by the agent to determine how to read and write to that block"), **`value`**, **`limit`** (size limit in characters), **`read_only`** (bool; agent cannot update). Default descriptions are auto-generated for labels `persona` and `human`. Note: "**Setting `value` completely replaces the entire block content** — it is not an append operation… the last write wins."
Source: https://docs.letta.com/v1-sdk/memory/memory-blocks.md

Shared blocks: `block_ids=[...]` at agent creation, or `client.agents.blocks.attach(agent_id, block_id)` / `.detach(...)`.
Source: https://docs.letta.com/v1-sdk/memory/memory-blocks.md

---

## 3. Archival memory — storage, chunking, search, archives abstraction

### 3.1 What it is (era B docs)

> "Archival memory is a semantically searchable database where agents can store facts, knowledge, and information for long-term retrieval. Unlike memory blocks, archival memory fragments cannot be pinned to the context window, and must be queried on-demand via tools."
> Key characteristics: "**Agent-immutable** — Agents cannot easily modify or delete archival memories (though developers can via SDK)"; "Unlimited storage"; "Semantic search"; "**Tagged organization** — Agents can categorize memories with tags."
> — https://docs.letta.com/v1-sdk/memory/archival-memory.md

Limits from the context-hierarchy table: **300 tokens per memory, unlimited count.**
Source: https://docs.letta.com/v1-sdk/memory/context-hierarchy.md

### 3.2 The original (2023) storage engine — this one *is* precise

> "We use MemGPT's default storage settings which uses **PostgreSQL for archival memory storage with vector search enabled via the pgvector extention**. We pre-compute embeddings and load them into the database, which uses an **HNSW index** to enable approximate, sub-second query times."
> — https://arxiv.org/html/2310.08560v2 (§3.2.1)

Embeddings in the experiments: **OpenAI `text-embedding-ada-002`**, cosine similarity. Pagination is used to prevent retrieval calls from overflowing context (§2.3).

### 3.3 Current codebase — provider enum and the `Archive` model

```
VectorDBProvider enum = NATIVE | TPUF | PINECONE
```
- ORM table `archives`, column `vector_db_provider` defaulting to `VectorDBProvider.NATIVE` — i.e. **per-archive, not global**.
- Junction table `archives_agents` with `UniqueConstraint("agent_id", name="unique_agent_archive")` and the source comment: *"TODO: Remove this unique constraint when we support multiple archives per agent / For now, each agent can only have one archive"*.
- Turbopuffer client: `letta/helpers/tpuf_client.py`, gated by **`settings.use_tpuf` (default `False`)**, dual-written only when `archive.vector_db_provider == VectorDBProvider.TPUF`.
- Embeddings are **not** in a standalone `letta/embeddings.py` (that path 404s); they are inline in `letta/services/passage_manager.py` (`get_openai_embedding`, `get_openai_embedding_async`) and via `LLMClient.request_embeddings`.
Source: codebase at SHA `67013ef1bb491ec8bdd629bfbeceb70227936afa` (paths as listed).

**How `NATIVE` actually resolves — this is the answer to "what vector DB does Letta use":**

| Engine | Mechanism | Source |
|---|---|---|
| **Postgres + pgvector** | `from pgvector.sqlalchemy import Vector` → `mapped_column(Vector(MAX_EMBEDDING_DIM))`, order by `embedding.cosine_distance(...)` | `letta/orm/passage.py` |
| **SQLite + sqlite-vec** | `import sqlite_vec`, `sqlite_vec.serialize_float32(...)`, registered as a connect-event extension | `letta/orm/sqlite_functions.py`, `letta/orm/custom_columns.py` (`class CommonVector(TypeDecorator)`) |
| **Turbopuffer** | `class TurbopufferClient` / `AsyncTurbopuffer`, per-archive namespaces | `letta/helpers/tpuf_client.py` |
| **Pinecone** | `letta/helpers/pinecone_utils.py`, `settings.enable_pinecone` | — |

⭐ **SQLite (sqlite-vec) is the effective default out of the box.** `database_engine` returns `POSTGRES if self.letta_pg_uri_no_default else SQLITE`, and `letta_pg_uri_no_default` is `None` unless `pg_uri` or all five `pg_*` fields are set. `.env.example` contains only OpenAI/Ollama/vLLM sections — **no `LETTA_PG_*` and no `LETTA_USE_TPUF`**.

⚠️ **Turbopuffer requires THREE simultaneous conditions** — `should_use_tpuf()`:
```python
def should_use_tpuf() -> bool:
    # We need OpenAI since we default to their embedding model
    return bool(settings.use_tpuf) and bool(settings.tpuf_api_key) and bool(model_settings.openai_api_key)
```
Turbopuffer writes are **dual writes, not migrations** — SQL first, then TPUF with the *same IDs*, best-effort.

⚠️ **Turbopuffer hard-codes its own embedding config**, ignoring the archive's:
```python
default_embedding_config = EmbeddingConfig(embedding_model="text-embedding-3-small",
    embedding_endpoint_type="openai", embedding_endpoint="https://api.openai.com/v1",
    embedding_dim=1536, embedding_chunk_size=DEFAULT_EMBEDDING_CHUNK_SIZE)
```
So with TPUF both write and query vectors are OpenAI 1536-dim, unpadded — an asymmetry with the pgvector path (which pads to 4096 and uses `archive.embedding_config`).

⚠️ **`PINECONE` is unreachable for archive creation** — `create_archive_async` can only pick TPUF or NATIVE.

**[NOT VERIFIED: Letta Cloud's actual production backend.]** The repo exposes no cloud config for it. The strongest available *inference* (labelled as inference, not a verified cloud claim): TPUF + `tpuf_region = "gcp-us-central1"` + environment-suffixed multi-tenant namespaces (`f"archive_{archive_id}_{environment.lower()}"`, `f"messages_{organization_id}_{environment.lower()}"`, `f"tools_{organization_id}_..."`, `f"file_passages_{organization_id}_..."`) point to Turbopuffer being the managed path.

⚠️ **No `Chroma`, `Milvus`, `Qdrant`, `LanceDB`, `Elasticsearch`, or `Weaviate`** appears in any storage-relevant file read, and there is **no `VectorDBBase` / pluggable-provider ABC** — the "provider" is a per-archive enum plus branch points in `PassageManager`/`ArchiveManager`. Repo-wide absence: **[NOT VERIFIED]**.

`EMBEDDING_BATCH_SIZE` and other constants: see §3.5.

### 3.4 The `archives` abstraction and attach/detach — YES, it exists

**⚠️ Correct exact route shapes** (verified in `letta/server/rest_api/routers/v1/agents.py` — the verb is **PATCH** and `archive_id` comes **last**, not the `/archives/{archive_id}/attach` shape you might guess):
- **`PATCH /v1/agents/{agent_id}/archives/attach/{archive_id}`** — operationId `attach_archive_to_agent`
- **`PATCH /v1/agents/{agent_id}/archives/detach/{archive_id}`** — operationId `detach_archive_from_agent`
(Adjacent for comparison: `PATCH /{agent_id}/tools/attach/{tool_id}`, `PATCH /{agent_id}/tools/detach/{tool_id}`.)

Top-level archive CRUD lives in `letta/server/rest_api/routers/v1/archives.py`, `prefix="/archives"`: `POST /v1/archives/` (create), `GET /v1/archives/` (list; supports `agent_id` filter), `GET|PATCH|DELETE /v1/archives/{archive_id}`, `GET /v1/archives/{archive_id}/agents`, `POST /v1/archives/{archive_id}/passages`, `DELETE /v1/archives/{archive_id}/passages/{passage_id}`.

⚠️ **The vector DB provider is deliberately NOT client-settable** — `ArchiveCreateRequest` docstring: *"Intentionally excludes vector_db_provider. These are derived internally (vector DB provider from env)."* And in the docs nav, top-level **Archives is filed under a "Deprecated" section** (https://docs.letta.com/api/resources/archives).

⭐ **A default archive is created lazily** — you do *not* have to create one to use archival memory. `ArchiveManager.get_or_create_default_archive_for_agent_async` builds one named `f"{agent_state.name}'s Archive"` with `description="Default archive created automatically"` and attaches it with `is_owner=True`; `PassageManager.insert_passage` calls it automatically. `ArchivalPassage` is scoped by **`archive_id`, not `agent_id`** — MemGPT's per-agent archival scoping is gone; `agent_id` is only reachable through the `archives_agents` join.

So: **archives are a separate resource that can be attached/detached to agents**, but the current constraint is **one archive per agent**. That is a real, load-bearing limitation for anyone designing multi-corpus retrieval.

### 3.5 Chunking and embedding config

`embedding` (model handle, format `provider/model-name`) and `embedding_config { embedding_dim, embedding_endpoint_type, ... }` are still on agent create. **`embedding_chunk_size` is explicitly marked `Deprecated: No longer used.`**
Source: https://docs.letta.com/api/resources/agents/methods/create/index.md

**⭐ RESOLVED — archival-memory inserts are NOT chunked.** In `PassageManager.insert_passage`:
```python
embedding_chunk_size = agent_state.embedding_config.embedding_chunk_size
...
# TODO: check to make sure token count is okay for embedding model
text_chunks = [text]
```
`embedding_chunk_size` is read into a local and **never used**; `text_chunks` is a single-element list ⇒ **one `archival_memory_insert` payload becomes exactly one passage and one embedding.** This is corroborated by the schema: `embedding_chunk_size` carries `description="Deprecated: No longer used. The embedding chunk size used by the agent.", deprecated=True`.

**A real chunker exists — but for files, not archival memory:** `letta/services/file_processor/chunker/line_chunker.py` (`class LineChunker`), dispatching on `ChunkingStrategy`: `DOCUMENTATION` → `_chunk_by_sentences` (regex `(?<=[.!?])\s+(?=[A-Z])`), `CODE` → `_chunk_by_lines(preserve_indentation=True)`, else `_chunk_by_lines(preserve_indentation=False)`. It splits on **lines/sentences, not tokens**.

**Constants** (`letta/constants.py`): `MAX_EMBEDDING_DIM = 4096  # maximum supported embeding size - do NOT change or else DBs will need to be reset`; `DEFAULT_EMBEDDING_CHUNK_SIZE = 300`; `DEFAULT_EMBEDDING_DIM = 1024`; `EMBEDDING_BATCH_SIZE = 200` (⚠️ **NOT VERIFIED: any consumer of this** — the OpenAI client hard-codes `initial_batch_size = 2048` / `min_chunk_size = 128` instead). Default embedding model: `text-embedding-3-small` @ 1536 dims.

**Storage invariant:** embeddings are zero-padded to 4096 dims for Postgres (`# Always pad when writing to Postgres vector DB (don't pad for Turbopuffer/Pinecone)`), and null bytes are stripped from text (`text = text.replace("\x00", "")`).

### 3.6 Archival vs conversation search — Letta's own distinction (verbatim)

> "**Archival memory** is for **intentional** storage: Agents decide what's worth remembering long-term… **Conversation search** is for **historical** retrieval: Searches through actual past messages… **Automatic** — no agent curation needed."
> — https://docs.letta.com/v1-sdk/memory/archival-memory.md

That page also quotes the motivating example: "User says 'I prefer Python for data science projects' → **Archival:** Agent inserts 'User prefers Python for data science' as a fact → **Conversation search:** Agent can search for the original message later."

---

## 4. `request_heartbeat` — confirmed in the paper, **dead** in modern Letta

### 4.1 It existed. Exact paper semantics.

**Figure 3 caption, verbatim:**
> "The LLM can request immediate follow-up LLM inference to chain function calls together by generating a special keyword argument (**request_heartbeat=true**) in its output; function chaining is what allows MemGPT to perform multi-step retrieval to answer user queries."
> — https://arxiv.org/html/2310.08560v2

**Mechanism, §2.4 "Control flow and function chaining", verbatim:**
> "In MemGPT, *events* trigger LLM inference… Many practical tasks require calling multiple functions in sequence… **Function chaining** allows MemGPT to execute multiple function calls sequentially before returning control to the user. In MemGPT, functions can be called with a special flag that requests control be immediately returned to the processor after the requested function completes execution. If this flag is present, MemGPT will add the function output to main context and (as opposed to pausing processor execution). If this flag is not present (a ***yield***), MemGPT will not run the LLM processor until the next external event trigger (e.g. a user message or scheduled interrupt)."
> — https://arxiv.org/html/2310.08560v2

Note the paper's own term for the negative case: a **yield**.

**Default value:** Letta's engineering blog states > "By default, `request_heartbeat` is set to `False` in MemGPT, so agents terminate by default rather than continue by default as in ReAct."
> — https://www.letta.com/blog/letta-v1-agent/

**Tool-schema description, per Letta's legacy docs:** > "Every tool in Letta automatically receives an additional parameter called `request_heartbeat`, which defaults to `false`. When an agent sets this parameter to `true`, it signals to the Letta server that it wants to continue executing after the current tool call completes."
> Server behavior on `request_heartbeat=true`: "1. Completes the current tool execution 2. Restarts the agent loop with a system message acknowledging the heartbeat request 3. Allows the agent to continue with an additional tool calls"
> Also: "If a tool call fails at runtime, Letta **automatically generates a heartbeat**."
> — https://web.archive.org/web/20250914103956id_/https://docs.letta.com/guides/agents/heartbeats

### 4.2 What replaced it — you were right: standard multi-step tool calling

**The definitive statement, verbatim:**
> "At Letta, we're transitioning from the previous MemGPT-style architecture to a new Letta V1 architecture (`letta_v1_agent`) that follows these modern patterns. **In this architecture, `heartbeats` and the `send_message` tool are deprecated. Only native reasoning and direct assistant message generations from the models are supported.**"
> Drawbacks listed: "**No heartbeats:** Agents no longer understand the concept of heartbeats (unless implemented manually). For repeatedly triggering agents to run independently (e.g., processing time or sleep-time compute), you'll need custom prompting to explain this environment to the agent"
> — https://www.letta.com/blog/letta-v1-agent/ (OCT 14 2025)

So the replacement is: **the model's native reasoning + native tool-calling loop with self-directed termination**, no special keyword argument. The blog ties this to staying "in-distribution" — "Today's LLMs are also trained to be adept at agentic patterns such as multi-step tool calling, reasoning interleaved within tool calling, and **self-directed termination conditions**."

### 4.3 Code-level confirmation that it is legacy-scoped

- `AgentType.letta_v1_agent`, with the inline comment: `# simplification of the memgpt loop, no heartbeats or forced tool calls` — https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/schemas/enums.py
- `LettaAgentV3` docstring: *"No heartbeats (loops happen on tool calls)"*; `_get_valid_tools()` passes `request_heartbeat=False  # NOTE: difference for v3 (don't add request heartbeat)`; `_decide_continuation` takes **no** `request_heartbeat` argument and defaults `continue_stepping = True`.
- Legacy `LettaAgent` still does `continue_stepping = request_heartbeat` and passes `request_heartbeat=True`.
- `letta/llm_api/anthropic_client.py` actively **strips** the parameter for v1 agents.
- The constant `REQUEST_HEARTBEAT_PARAM = "request_heartbeat"` and its full description string **still exist** in `letta/constants.py`.
- `letta/agents/letta_agent.py` (legacy) does `continue_stepping = request_heartbeat` and injects the param with `runtime_override_tool_json_schema(..., request_heartbeat=True, ...)`. `LettaAgentV2` is identical here.
- `LettaAgentV3`'s `_decide_continuation` **has no `request_heartbeat` parameter at all** and defaults `continue_stepping = True`.

### 4.3b The precise replacement mechanism — loop-on-any-tool-call + **ToolRules**

The replacement is threefold, all verified in code:

**1. Loop-on-any-tool-call.** `LettaAgentV3._decide_continuation` docstring, verbatim:
> "In v3 loop, we apply the following rules: **1. Did not call a tool? Loop ends. 2. Called a tool? Loop continues.** This can be: 2a. Called tool, tool executed successfully 2b. Called tool, tool failed to execute 2c. Called tool + tool rule violation (did not execute)"

Terminating the turn now requires **not calling a tool** (`if tool_call_name is None: ... return False, None, LettaStopReason(stop_reason=StopReasonType.end_turn.value)`).

**2. `max_steps`.** `DEFAULT_MAX_STEPS = 50`; loop is `for i in range(max_steps)`; the last step sets `StopReasonType.max_steps.value`.

**3. ToolRules — the structural replacement for the heartbeat idiom.** `ToolRuleType` enum:
`run_first` | `exit_loop` | `continue_loop` | `conditional` | `constrain_child_tools` | `max_count_per_step` | `parent_last_tool` | `required_before_exit` | `requires_approval`

Auto-assigned in `AgentManager.create_agent_async`:
```python
if should_add_base_tool_rules:
    for tn in tool_names:
        if tn in {"send_message", "send_message_to_agent_async", "memory_finish_edits"}:
            tool_rules.append(TerminalToolRule(tool_name=tn))
        elif tn in (BASE_TOOLS + BASE_MEMORY_TOOLS + BASE_MEMORY_TOOLS_V2 + BASE_MEMORY_TOOLS_V3 + BASE_SLEEPTIME_TOOLS):
            tool_rules.append(ContinueToolRule(tool_name=tn))
```
So **`ContinueToolRule` replaces `request_heartbeat=True`** for memory tools, and **`TerminalToolRule` replaces "don't continue after `send_message`"** — and `memory_finish_edits` is terminal by construction, which is exactly how a sleep-time agent's edit session ends.

⚠️ Note the exception list for base tool rules: `EXCLUDE_MODEL_KEYWORDS_FROM_BASE_TOOL_RULES = ["claude-4-sonnet", "claude-3-5-sonnet", "gpt-5", "gemini-2.5-pro"]` with `INCLUDE_MODEL_KEYWORDS_BASE_TOOL_RULES = ["mini"]` — but `sleeptime_agent` is explicitly exempted (`and agent_create.agent_type != AgentType.sleeptime_agent`), so **sleep-time agents always get base tool rules**.

⚠️ **Legacy trace still in `letta/constants.py`** — a commented-out prompt line: `# "Remember to pass request_heartbeat = true if you would like to send a message immediately after."`, confirming the idiom's removal.

**Other providers also strip heartbeats for `letta_v1_agent`:**
- `anthropic_client.py`: `native_content=is_v1, strip_request_heartbeat=is_v1`, then `drop_heartbeats(data["messages"])` and a loop that pops `REQUEST_HEARTBEAT_PARAM` from each tool's `input_schema["properties"]` and removes it from `required`. Helpers present: `is_heartbeat()`, `drop_heartbeats()`, `merge_heartbeats_into_tool_responses()`.
- `openai_client.py`, with the candid comment: `# For some reason, request heartbeats are still leaking into here... # So strip them manually for v3`
- Gemini/Vertex: suppressed via `llm_config.put_inner_thoughts_in_kwargs = False` + `native_content=True` (no explicit `pop`).

**Verdict: `request_heartbeat` is not deleted, but it is confined to the deprecated MemGPT-style loop. It is not part of the `letta_v1_agent` architecture, and it is not used by the current MemFS/harness-era agents.** `DEFAULT_MAX_STEPS = 50` is the loop bound in the current code.

### 4.4 ⚠️ The exact wording is NOT in the paper — it is in the *released function schema*

**Important sourcing caveat:** the MemGPT paper's body and appendix **do not contain** the definition text of `request_heartbeat`, and neither paper version formally defines the terms **"heartbeat"** or **"step"**. The canonical wording comes from the function spec the MemGPT authors released alongside the paper.

**From the released experiment function spec** (dataset `MemGPT/function-call-traces`, whose card states these traces "were generated while evaluating MemGPT (arXiv:2310.08560)"), the exact JSON entry is:

```json
"request_heartbeat": {
  "type": "boolean",
  "description": "Request an immediate heartbeat after function execution. Set to 'true' if you want to send a follow-up message or run a follow-up function."
}
```

This wording is attached to `recall_memory_search`, `conversation_search`, `recall_memory_search_date`, `conversation_search_date`, `archival_memory_insert`, `archival_memory_search` — and it is **`"required"`** on every function that carries it.

**⚠️ Genuine inconsistency in the released schema:** `message_chatgpt`, `core_memory_append`, and `core_memory_replace` carry a **shorter** description — `"Request an immediate heartbeat after function execution, use to chain multiple functions."` Do not attribute one wording to all functions.

**Current canonical constant** in `letta/constants.py`:
> `REQUEST_HEARTBEAT_DESCRIPTION = "Request an immediate heartbeat after function execution. You MUST set this value to `True` if you want to send a follow-up message or run a follow-up tool call (chain multiple tools together). If set to `False` (the default), then the chain of execution will end immediately after this function call."`

Same file also holds `REQ_HEARTBEAT_MESSAGE = f"{NON_USER_MSG_PREFIX}Function called using request_heartbeat=true, returning control"` and `FUNC_FAILED_HEARTBEAT_MESSAGE = "…Function call failed, returning control"` — the latter being the implementation of the "automatic heartbeat on failure" behaviour.

**The mechanism distinction, from the released 2023 system prompt** (this is the clearest statement anywhere, and it is *not* in the paper):
> "Newer model DiJis like yourself utilize an event system that runs your brain at regular intervals… your brain is run **at regular intervals (timed heartbeat events)**… Furthermore, you can also **request heartbeat events when you run functions, which will run your program again after the function completes, allowing you to chain function calls** before your thinking is temporarly suspended."

So there are **two** heartbeat kinds: **timed** (scheduler-driven periodic wake-ups) vs **requested** (`request_heartbeat=true`). **[NOT VERIFIED: the timed-heartbeat interval value.]**

---

## 5. Sleep-time agents — official design

### 5.1 The design (blog, verbatim — this is the money quote for your tool-split question)

> "When you create agents with this type, Letta actually creates **two agents under the hood: a primary agent and a sleep-time agent**. The primary agent is similar to Letta agents today: it has the ability to send messages to the user, can call custom user tools, and can search its external memory (recall memory for conversation history, and archival memory for specifically saved memories). **However the primary agent is not provided with tools to edit its core memory**, which is the memory stored in-context composed of memory blocks. **These tools are attached to the sleep-time agent**, which has the ability to manage *both* the in-context memory of the primary agent as well as its own in-context memory."
> — https://www.letta.com/blog/sleep-time-compute (APR 21 2025)

**⇒ Your reading is CONFIRMED: the primary agent loses the core-memory edit tools; the sleep-time agent holds them.**

Motivation, verbatim:
> "In MemGPT, memory management, conversation, and other tasks are all bundled into a single agent, which means the agent may be slower… and potentially less reliable… **Offloading memory to a sleep-time agent allows memory management to happen asynchronously.** Memory formation in MemGPT is incremental, so memories may become messy and disorganized over time. Sleep-time agents on the other hand can continuously improve their learned context to generate clean, concise, and detailed memories."

Also: "Since Letta is model agnostic, the primary and sleeptime agents can be configured independently with different underlying models. We have found that it is useful to make the sleep-time agents stronger models since they are less latency constrained — for example, the conversational primary agent can use a fast model like gpt-4o-mini, while the sleep-time agent can use a larger and slower model like gpt-4.1 or Sonnet 3.7."
And: "the sleep-time agent modifies the memory in an 'anytime' fashion — so the primary agent can read from this memory whenever, without having to wait for the sleep-time agent to finish its reasoning."
Released "as part of **Letta 0.7.0**". Paper: https://arxiv.org/abs/2504.13171 (Lin, Snell, Wang, Packer, Wooders, Stoica, Gonzalez).

### 5.2 Exact configuration names

**Agent types** (`AgentType` enum, from the live API reference — exact strings):
`"memgpt_agent"`, `"memgpt_v2_agent"`, `"letta_v1_agent"`, `"react_agent"`, `"workflow_agent"`, `"split_thread_agent"`, `"sleeptime_agent"`, `"voice_convo_agent"`, `"voice_sleeptime_agent"`
Source: https://docs.letta.com/api/resources/agents/methods/create/index.md

**Group / manager types** (`ManagerType` enum): `round_robin` | `supervisor` | `dynamic` | `sleeptime` | `voice_sleeptime` | `swarm`
**[NOT VERIFIED: there is no `manager` manager_type]** — do not report one.

**Sleep-time config fields:**
- **`enable_sleeptime: Optional[bool]`** — on `AgentState` / create-agent. There is **no** `sleeptime_agent` field on the agent.
- **`sleeptime_agent_frequency`** lives on the **GROUP model**, not the agent. It is **not** present in the create-agent or update-agent body of the current REST API reference (grepped).
- Orchestration classes: **`SleeptimeMultiAgent`** (`letta/groups/sleeptime_multi_agent.py`) with v2/v3/v4 variants; v4 subclasses `LettaAgentV3` and injects a `<system-reminder>You are a sleeptime agent - a background agent...` transcript message.
Source: codebase at SHA `67013ef1`, files as listed; API reference URL above.

⚠️ **Important caveat on `sleeptime_agent_frequency`:** it is documented conceptually ("Sleeptime agents can also be configured to run at different frequencies. The higher the frequency setting, the more tokens your agent will use" — https://www.letta.com/blog/sleep-time-compute) and exists on the group model, but I could **not** find it in the current public create/update-agent API surface. Treat its exact spelling/placement as **[VERIFIED in the group model only]**.

### 5.2b ⭐ The primary agent's tool loss — exact code (strongest evidence available)

`AgentManager.create_agent_async` in `letta/services/agent_manager.py`:
```python
tool_names = set(agent_create.tools or [])
if agent_create.include_base_tools:
    if agent_create.agent_type == AgentType.voice_sleeptime_agent:
        tool_names |= set(BASE_VOICE_SLEEPTIME_TOOLS)
    elif agent_create.agent_type == AgentType.voice_convo_agent:
        tool_names |= set(BASE_VOICE_SLEEPTIME_CHAT_TOOLS)
    elif agent_create.agent_type == AgentType.sleeptime_agent:
        tool_names |= set(BASE_SLEEPTIME_TOOLS)
    elif agent_create.enable_sleeptime:
        tool_names |= set(BASE_SLEEPTIME_CHAT_TOOLS)
    elif agent_create.agent_type == AgentType.memgpt_v2_agent:
        tool_names |= calculate_base_tools(is_v2=True)
    elif agent_create.agent_type == AgentType.react_agent:
        pass  # no default tools
    elif agent_create.agent_type == AgentType.letta_v1_agent:
        tool_names |= calculate_base_tools(is_v2=True)
        tool_names.discard("send_message")
        agent_create.llm_config.put_inner_thoughts_in_kwargs = False
    elif agent_create.agent_type == AgentType.workflow_agent:
        pass  # no default tools
    else:
        tool_names |= calculate_base_tools(is_v2=False)
```

**This is the definitive proof.** The branch `elif agent_create.enable_sleeptime:` sits **above** the `memgpt_v2_agent` / `letta_v1_agent` / catch-all branches. So a **primary** agent created with `enable_sleeptime=True` gets **exactly** `BASE_SLEEPTIME_CHAT_TOOLS` — `calculate_base_tools` is **never called on that path**, so it receives **no `core_memory_*` and no `memory_*` tools at all**. The memory-edit capability is **transferred, not duplicated**. Note there is **no runtime "strip tools off the primary agent" mutation** — the split is expressed statically as list constants plus this `if/elif` chain.

**Supporting details (all verified):**
- Memory blocks are **shared** between the pair: `AgentManager.attach_block_async` docstring — *"Attaches a block to an agent. For sleeptime agents, also attaches to paired agents in the same group."* It walks `group.agent_ids` and appends the block to any `AgentType.sleeptime_agent`.
- Sleep-time agents get an **empty boot sequence**: `if agent_state.agent_type == AgentType.sleeptime_agent: initial_boot_messages = []`.
- `derive_system_message(...)`: `sleeptime_agent` → `gpt_system.get_system_text("sleeptime_v2")` (comment: *"v2 drops references to specific blocks, and instead relies on the block description injections"*); `letta_v1_agent` → `"letta_v1"`.
- Toggling `enable_sleeptime` on update **regenerates the system prompt**.
- `CreateAgent` validator: `AGENT_TYPES_REQUIRING_SLEEPTIME = {AgentType.voice_convo_agent}` — voice agents **must** have `enable_sleeptime=True`.
- Deleting the primary agent deletes the sleeptime agent + group (comment: *"TODO this is flimsy pls fix"*).
- Sleep-time agents run on **`LettaAgentV3`** — so they are heartbeat-free, loop via `ContinueToolRule`, and terminate on `memory_finish_edits` (a `TerminalToolRule`).
- `SleeptimeMultiAgentV4._participant_agent_step` contains the literal role definition:
  > "You are a sleeptime agent - a background agent that asynchronously processes conversations after they occur. **IMPORTANT: You are NOT the primary agent.** You are reviewing a conversation that already happened between a primary agent and its user: Messages labeled "assistant" are from the primary agent (not you)… **Your primary role is memory management.** Review the conversation and use your memory tools to update any relevant memory blocks…"
- `SleeptimeManager` config: `manager_type: Literal[ManagerType.sleeptime]`, `manager_agent_id: str`, `sleeptime_agent_frequency: Optional[int]`.
- Routing in `routers/v1/agents.py`: `agent_eligible = agent.multi_agent_group is None or agent.multi_agent_group.manager_type in ["sleeptime", "voice_sleeptime"]`.
- `SleeptimeMultiAgent.step_stream` raises: "Streaming is not supported for SleeptimeMultiAgent. Use the non-streaming step() method instead."
- ⚠️ ManagerType `swarm` is declared but its class is **commented out**.

### 5.3 Tool split — the exact tool-set constants

This is the cleanest evidence, and it matches the blog exactly:

```
BASE_SLEEPTIME_CHAT_TOOLS = [SEND_MESSAGE_TOOL_NAME, "conversation_search", "archival_memory_search"]
BASE_SLEEPTIME_TOOLS      = ["memory_replace", "memory_insert", "memory_rethink", "memory_finish_edits"]
```
Source: https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/constants.py

Read together with `BASE_MEMORY_TOOLS_V2 = ["memory_replace", "memory_insert"]`:

| Agent | Memory-edit tools | Read/search tools | Messaging |
|---|---|---|---|
| **Primary** | **none** | `conversation_search`, `archival_memory_search` | yes |
| **Sleep-time** | `memory_replace`, `memory_insert`, `memory_rethink`, `memory_finish_edits` | `conversation_search`, `archival_memory_search` | `send_message` (sleeptime-chat variant) |

The presence of **`memory_finish_edits` only in the sleep-time set** is strong corroboration: it exists to close out a background editing session that the primary agent never opens.

### 5.4 Era C (2026): sleep-time became "dreaming"

The concept was renamed and re-platformed in the MemFS era:

```js
dreaming: {
  trigger: "step-count",   // "off" | "step-count" | "compaction-event"
  behavior: "auto-launch", // "reminder" | "auto-launch"
  stepCount: 25,
}
```
> "`trigger` controls when dreaming runs: after a number of steps, on context compaction, or never. `behavior` controls what happens at the trigger: remind the agent to update memory, or automatically launch a background dreaming subagent. **It can only be set at agent creation.**"
> — https://docs.letta.com/agent-sdk/memory/index.md

> "Dreaming uses background subagents to review recent conversations, consolidate useful lessons, and update memory without interrupting your active work. Configure dreaming with **`/sleeptime`** in the CLI or **Dream settings** in the app… Select **Agent reviews before applying** to have your agent review and revise proposed memory updates in a second background conversation."
> — https://docs.letta.com/configuration/memory.md

Note the CLI command is still literally `/sleeptime`. And MemFS states: "Memory subagents such as dreaming and memory doctor **use git worktrees** so they can update memory concurrently without blocking the main agent." — https://docs.letta.com/concepts/memfs/index.md

**Status caveat:** the dedicated sleep-time docs page was filed under **"Experimental & legacy"** in the 2026 docs nav — https://web.archive.org/web/20260421133854id_/https://docs.letta.com/guides/agents/architectures/sleeptime/. And the architectures comparison table (Oct 2025 snapshot) listed six architectures: MemGPT agents, Sleep-time agents, Low-latency (voice) agents, ReAct agents, Workflows, Stateful workflows — https://web.archive.org/web/20251005195618id_/https://docs.letta.com/guides/agents/architectures

### 5.5 The paper's formal setup (arXiv:2504.13171, Lin et al., Letta + UC Berkeley)

The paper was written by Letta researchers (Kevin Lin, Yu Wang, Charles Packer, Sarah Wooders, Ion Stoica, Joseph E. Gonzalez — affiliation "Letta"), plus Charlie Snell (UC Berkeley).

**The formal definitions, verbatim (§3):**

> "In the standard paradigm of applying test-time compute, a user inputs a prompt *p* to the LLM... However, the *p* provided to the LLM can oftentimes be decomposed into a pre-existing **context c** (eg. a codebase) and a **user query q** (eg. a question about the codebase). When the LLM is not actively responding to the user, it typically still has access to the existing context c. During this time, the LLM is typically idling, missing the opportunity to reason about c offline: a process we term **sleep-time compute**."

> "**Test-time compute.** ... the user provides *q* along with some context *c* and the model outputs a reasoning trace followed by a final answer *a*. We denote this process, as: **T_B(q,c) → a**, where T is the method for using test-time compute with budget B..."

> "**Sleep-time compute.** During sleep-time we are given the context *c* but not the query *q*. Using just this context *c*, we can use the LLM to infer likely questions and reason about the context ultimately producing a more new **re-represented context c′**. We denote this process as: **S(c) → c′**... After pre-processing the context, we can provide the new context *c′* at test-time in place of *c*: **T_b(q, c′) → a**. Since much of the reasoning about *c* has been done ahead of time in this case, we can use a much smaller test-time budget **b ≪ B**. Moreover, *c′* can be shared across different queries *q_i* about the same context, effectively amortizing the compute required to arrive at *c′* across queries."
> — https://arxiv.org/html/2504.13171v1

**Headline quantitative results (from the abstract, verbatim):**
- "sleep-time compute can reduce the amount of test-time compute needed to achieve the same accuracy by **~5×** on Stateful GSM-Symbolic and Stateful AIME"
- "by scaling sleep-time compute we can further increase accuracy by up to **13%** on Stateful GSM-Symbolic and **18%** on Stateful AIME"
- "By amortizing sleep-time compute across related queries about the same context using Multi-Query GSM-Symbolic, we can decrease the average cost per query by **2.5×**"
- "the **predictability of the user query** [is] well correlated with the efficacy of sleep-time compute"

Also verbatim, on the OS lineage (this ties §5 back to §4): "sleep-time compute builds on the idea of **pre-fetching in traditional operating systems**, in the context of LLMs à la Packer et al. (2023)" — i.e. the paper explicitly cites MemGPT. And a framing worth quoting: "**Sleep-time compute as representation learning over tokens**… Unlike traditional representation learning, which typically operates in model parameter or activation space, we instead form representations in the **space of natural language**."

Benchmarks/models: Stateful GSM-Symbolic (GPT-4o-mini, GPT-4o), Stateful AIME (o1, o3-mini, Claude Sonnet 3.7 Extended Thinking, DeepSeek-R1), plus a new **SWE-Features** agentic benchmark (~1.5× decrease in test-time tokens at low budgets). Sleep-time compute "generally **pareto dominates** pass@k" parallel scaling. Scaling method: "for non-reasoning models, we run *k* parallel generations, given input *c*, resulting in *c₁,…,c_k*. At test-time, the model then receives the inputs **concatenated** *c₁,…,c_k*"; and notably "we find that **5 parallel generations generally outperforms 10**." A context-only baseline confirms "the questions in our datasets are **not trivially predictable** from the context."
> — https://arxiv.org/html/2504.13171v1

### 5.6 ⚠️ Paper-vs-implementation drift — THREE caveats you must carry into the report

**(1) The two-agent division of labour is NOT a paper claim.** The paper never uses the phrase **"sleep-time agent"** and makes **no explicit statement about which agent owns memory editing**. Appendix K says only "the sleep-time compute process"; Appendix F's docstring refers in passing to "**the answer agent**". The primary-agent-loses-its-memory-tools design exists **only in the Letta blog and in `letta/constants.py`** (§5.1, §5.3). Cite it as post-paper implementation, not as the paper's claim.

**(2) "Learned context" is the blog's term, not the paper's.** The blog: "Reasoning during sleep time transforms '**raw context**' into '**learned context**'." The paper says "**re-represented context**" / "the new context *c′*". Do not put "learned context" in the paper's mouth.

**(3) Tool names differ across three artifacts.** The paper's **Appendix K**, verbatim:
> "We implement sleep-time compute **via function calling**. When applying sleep-time compute, the model is given access to **two functions, `rethink_memory` and `finish_rethinking`**. The `rethink_memory` function takes as input a new string, and replaces the current context *c*… The `finish_rethinking` function **terminates the sleep-time compute process**. The model is allowed to call the function `rethink_memory` for **up to 10 times**."

| Artifact | Names |
|---|---|
| Paper Appendix K / F | `rethink_memory`, `finish_rethinking` (Listing 2 header reads `finish_rethinking_memory`) |
| Released experiment code (`letta-ai/sleep-time-compute`) | `rethink_memory(agent_state, new_memory: str, target_block_label: Optional[str], source_block_label: Optional[str])`, `finish_rethinking_memory(agent_state)` |
| Shipped Letta product (`letta/constants.py`) | **`memory_rethink`**, **`memory_finish_edits`** |

**Cite the shipped product names as authoritative.** Note also that the paper's Appendix F docstring for `rethink_memory` — "Re-evaluate the memory in block_name… Replace outdated information with the most likely truths, avoiding redundancy with original memories." — differs from the released experiment code's docstring, which is about "coming up with useful inferences and pre-computations that could be helpful for answering potential questions about the situation."

**Implementation detail from the released experiment code** (not in the paper's visible text; verify separately before publishing): the test-time "answer" agent uses `tools=["send_message"]` and `include_base_tools=False`; *c′* physically lives in a shared core-memory block created as `client.blocks.create(label="rethink_memory_block", value="[empty]", limit=5000)` attached to **both** agents; the sleep-time trigger message is literally `"[trigger_rethink_memory] New situation:" + context`. Cost model: "tokens generated at test-time are a factor **t** the cost of the tokens at sleep-time. In our analysis, we set **t=10**." One stated exception: "with the exception of **o1, which demonstrates limited gains**."

---

## 6. Prompt caching — concrete mechanism and config

### 6.1 Where the caching lives: three Anthropic breakpoints, no TTL

`letta/llm_api/anthropic_client.py` places **exactly three** `{"type": "ephemeral"}` cache breakpoints:

1. `data["tools"][-1]["cache_control"] = {"type": "ephemeral"}` — **the last tool definition**
2. `_add_cache_control_to_system_message()` — **the system block**
3. `_add_cache_control_to_messages()` — **the final content block of the final message**

There is **NO `ttl` field, NO `"1h"` value, and NO `PROMPT_CACHE_*` constant** anywhere in that file (explicitly grepped). So: **5-minute default Anthropic ephemeral TTL; the 1h TTL is NOT used.** Cache metrics are read from `response.usage.cache_read_input_tokens` / `cache_creation_input_tokens` and normalized into `UsageStatisticsPromptTokenDetails(cache_read_tokens=..., cache_creation_tokens=...)`.
Source: https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/anthropic_client.py

**Placement is UNCONDITIONAL** — no model check, no feature flag, no token-threshold check. `build_request_data` always sets the three breakpoints whenever tools/system/messages exist. There is **no `MAX_CACHE_BREAKPOINTS` constant** (Anthropic's limit is 4; Letta uses 3 without naming it).

**Config flags — answering your specific question:**
- **`enable_prompt_caching` — NOT FOUND.** Not in the create/update-agent API reference, not in the Anthropic client, and **not in `letta/settings.py`** (read in full).
- **`prompt_cache` — NOT FOUND.** Caching is **on by default per provider**; there is no user-facing toggle. The nearest flag is `model_settings.anthropic_sonnet_1m` (adds beta `context-1m-2025-08-07`) — a *context-window* flag, not a caching one.
- **`PROMPT_CACHE_*` — NOT FOUND** in `anthropic_client.py`, `constants.py`, `settings.py`, or `anthropic_constants.py` (all read in full). ⚠️ Repo-wide absence is **[NOT VERIFIED]** (GitHub code search needs auth; grep.app returned HTTP 429).
Sources: https://docs.letta.com/api/resources/agents/methods/create/index.md; `letta/settings.py`; `letta/llm_api/anthropic_client.py`.

**⭐ One genuine cache-*optimising* mechanism — Opus 4.5 context management:**
```python
# Context management for Opus 4.5 to preserve thinking blocks and improve cache hits
# See: https://docs.anthropic.com/en/docs/build-with-claude/context-editing
if llm_config.model.startswith("claude-opus-4-5") and llm_config.enable_reasoner:
    data["context_management"] = {
        "edits": [{"type": "clear_thinking_20251015", "keep": "all"}],  # Preserve all thinking blocks for maximum cache performance
    }
```
plus the beta `context-management-2025-06-27`. This is the concrete code answer to "thinking blocks break the cache" flagged by the tests.

**Provider-by-provider, verified:**
| Provider | Caching implementation |
|---|---|
| **Anthropic** | 3 explicit `{"type":"ephemeral"}` breakpoints; **no TTL** |
| **Bedrock** | `class BedrockClient(AnthropicClient)` → **inherits the same 3 breakpoints**. ⚠️ **No `cachePoint` / Bedrock-native caching block exists** in `bedrock_client.py` |
| **OpenAI** | ⚠️ **Zero caching code** — no `cache_control`, no breakpoint logic. Entirely provider-side/automatic |
| **Gemini / Vertex** | ⚠️ **No explicit cache creation** — no `cached_content`, no `client.caches.create(...)`. Only **readback**: `cached_content_token_count` → `UsageStatisticsPromptTokenDetails(cached_tokens=...)`. Relies on implicit caching |

**Cache accounting:** `cache_read_input_tokens` / `cache_creation_input_tokens` from the provider response are folded into `UsageStatisticsPromptTokenDetails(cache_read_tokens=, cache_creation_tokens=)`; `actual_input_tokens = prompt_tokens + cache_read_tokens + cache_creation_tokens` (per Anthropic's documented summation). Public field names surfaced to clients: **`cached_input_tokens`** and **`cache_write_tokens`** — exactly what the integration tests read.

### 6.2 The official test file — what it actually asserts

Full file retrieved at the pinned commit: https://raw.githubusercontent.com/letta-ai/letta/67013ef1bb491ec8bdd629bfbeceb70227936afa/tests/test_prompt_caching.py

Its module docstring states the provider model precisely:
> "These tests verify that our LLM clients properly enable caching for each provider:
> - **OpenAI: Automatic caching (≥1024 tokens)**
> - **Anthropic: Requires cache_control breakpoints (≥1024 tokens for Sonnet 4.5)**
> - **Gemini: Implicit caching on 2.5 models (≥1024 tokens for 2.5 Flash)**
> If these tests fail, it means: … For Anthropic: **We're not adding cache_control breakpoints**"

Test strategy: "1. Create agent with large memory block (>5000 tokens to exceed all thresholds) 2. Send message 1 → assert cache WRITE occurred 3. Send message 2 → assert cache HIT occurred."

**Exact provider thresholds asserted in `CACHING_TEST_CONFIGS`:**

| Provider / model | Threshold | Read field | Write field |
|---|---|---|---|
| `openai/gpt-4o` | 1,024 tokens, automatic ("caching is free") | `cached_tokens` | none |
| `anthropic/claude-sonnet-4-5-20250929` | 1,024 (Sonnet 4.5); **4,096 for Opus/Haiku 4.5** | `cache_read_tokens` | `cache_creation_tokens` |
| `google_ai/gemini-2.5-flash` | 1,024 (2.5 Flash); 4,096 (2.5 Pro); 2,048 (3 Pro Preview) | `cached_tokens` | none |
| `google_ai/gemini-3-pro-preview` | 2,048 | `cached_tokens` | none — marked `xfail`: "Gemini 3 Pro Preview doesn't have implicit caching (only 2.5 models do)" |

Other asserted invariants worth quoting:
- Anthropic must create **≥1024** cache tokens on the *first* message: assertion message — *"This means cache_control breakpoints are NOT being added!"*
- OpenAI: `assert cached_tokens_2 % 128 == 0, "OpenAI cached_input_tokens should be in 128-token increments"` — **128-token cache granularity**.
- Acceptable cache-hit ratio floor: `assert cache_hit_ratio >= 0.15`, with the comment *"With thinking mode enabled, Anthropic may have lower cache ratios due to thinking blocks changing between messages."*

### 6.3 Memory edits DO invalidate the cache — and the root cause is a timestamp

`test_prompt_caching_cache_invalidation_on_memory_update`:
> "Test that updating memory blocks invalidates the cache. When memory is modified, the prompt changes, so the cache should miss and a new cache should be created."
> It updates the `persona` block via `client.blocks.update(...)` then asserts `cache_ratio_after < cache_ratio_before`.

The smoking gun, from `test_anthropic_system_prompt_stability`, which diffs the **actual provider traces** (`client.telemetry.retrieve_provider_trace(step_id=...)` → `request_json["system"]`):
> `if "Memory blocks were last modified" in diff_output: logger.error("⚠️ TIMESTAMP IS CHANGING IN ACTUAL REQUESTS!")`
> `logger.error("→ This is the root cause of cache misses")`

So the concrete failure mode Letta's own tests guard against is a **"Memory blocks were last modified" timestamp embedded in the system prompt**, which changes on every request and destroys prefix stability. Also listed as candidate causes: *"1. Thinking blocks breaking cache 2. Tool definitions changing 3. Something else in the request changing."* `test_gemini_request_prefix_stability` checks the same three things for Gemini — system instruction identity, **tool order** ("Tool order must be identical for Gemini implicit caching"), and content-prefix identity.

### 6.4 "The agent state IS the prompt" — official statements

**Letta's own engineering guidance (Context Constitution, verbatim) — this is the single most quotable line on the caching design:**
> "Letta agents should minimize the total token count by evicting stale content, **placing frequently updated memories near the end of the system prompt to minimize cache invalidation**, compressing where possible, and avoiding redundant information across context blocks."
> — https://raw.githubusercontent.com/letta-ai/context-constitution/main/constitution/CONSTITUTION.md

> "A well-managed context window has a **stable, compressed system prompt that changes only when a learning is truly durable**."
> — same URL

**The closest official statement to "the agent state IS the prompt"** (present these as *mechanism* claims, not caching claims):
> "Unlike ephemeral memory in many LLM frameworks, Letta's memory blocks are individually persisted in the DB, with a unique `block_id` to access them via the API and Agent Development Environment (ADE). **When Letta makes a request to the LLM, the context window is 'compiled' from existing DB state, including current block values.**"
> — https://www.letta.com/blog/memory-blocks/

> "the **compiled kernel context** (blocks & files)"
> — https://www.letta.com/blog/guide-to-context-engineering/

The mechanism is independently confirmed at the wire level: memory blocks are literally serialized into the prompt as `<memory_blocks>` XML (§2.5), so editing a block necessarily edits the prompt prefix.

**Letta forum answer attributed to `@ezra` (Dec 23 2025)** — ⚠️ **forum-tier, not primary docs; treat as unofficial**:
> "Letta implements prompt caching, and **memory block edits only invalidate the cache for that single turn**. The invalidation is temporary and localized - the cache recovers on the next turn where no edits occur."
> Cost-mitigation advice given: "**Use sleeptime agents** - they do memory updates asynchronously during idle periods, so the foreground agent maintains cache hits during active conversation"; "**Use archival memory for less critical info** - archival writes don't affect the context cache since archival isn't in-context."
> — https://forum.letta.com/t/breaking-prompt-caching/149

⚠️ **[NOT VERIFIED] — the exact phrase "the agent state IS the prompt" does not appear in any primary source.** I had all five memory/context blog posts extracted twice through independent pipelines (byte-identical results), and they contain **zero** occurrences of "prompt caching", "cache", "cache invalidation", "stable prefix", or "byte-stable". **Do not attribute that exact wording to Letta.**

**[NOT VERIFIED]** — I found **no Letta blog post dedicated to prompt caching**. The blog index (https://www.letta.com/blog/) has no such post; the five memory/context posts I had independently extracted contain **zero** occurrences of "prompt caching", "cache", "cache invalidation", "stable prefix", or "byte-stable". The caching documentation exists only in **code + tests + one forum thread**. This is a genuine gap in Letta's public documentation and worth stating plainly in your report.

---

## 7. Summary of what changed, 2023 → 2026

| Concern | 2023 MemGPT paper | Letta 2025 (era B) | Letta 2026 (era C) |
|---|---|---|---|
| Memory primitive | working context (mutable text block) | memory blocks (`<memory_blocks>` XML in prompt) | MemFS files under `system/` in a git repo |
| Conversation history | FIFO queue + recall storage DB | message buffer + Recall Memory tier | `Messages` primitive; compaction; `letta messages search` |
| Long-term store | archival storage (Postgres + pgvector + HNSW) | archival memory, tagged, `archival_memory_*` | **MemFS (no vector index by default)**; archival tools marked `DEPRECATED_LETTA_TOOLS` |
| Control flow | `request_heartbeat=true` to chain; absent = *yield* | `request_heartbeat` on every tool, default false; auto-heartbeat on tool failure | **removed** — native reasoning + native multi-step tool calling; `letta_v1_agent` |
| Message output | `send_message` tool | `send_message` tool | **`send_message` deprecated**; direct assistant messages |
| Memory editing | self-directed, by the single agent | `core_memory_append`/`replace` → `memory_insert`/`replace`/`rethink` | `memory` omni-tool (V3); or filesystem edits via git |
| Background consolidation | none (queue eviction + recursive summary) | **sleep-time agent** holds the edit tools | **"dreaming"** subagents + git worktrees; `/sleeptime` CLI |
| Prompt caching | not discussed | 3 Anthropic ephemeral breakpoints; provider-native elsewhere | same; Constitution advises updated memories late in the system prompt |

---

## 8. Explicit gaps / things I could NOT verify

1. ~~Verbatim Python signatures~~ — **NOW RESOLVED** (§2.4). All signatures and docstrings captured from `letta/functions/function_sets/base.py` at the pinned SHA. Superseded gap: **the new `main`-branch path of `letta/functions/` is still NOT VERIFIED** (the tree was reorganized; a `.github/workflows/letta-code-sync.yml` exists, consistent with the split-out `letta-ai/letta-code` harness repo).
2. ~~Chunking algorithm~~ — **NOW RESOLVED** (§3.5): archival inserts are **not chunked** (`text_chunks = [text]`); a line/sentence `LineChunker` exists for files only.
3. **Letta Cloud's production vector backend** — repo exposes no cloud setting. Inference only (§3.3). Also **`PINECONE` is unreachable** from archive creation.
4. **`sleeptime_agent_frequency` location** — RESOLVED: it is a **group** field (`Group`, `SleeptimeManager`), not an agent field; the agent field is `enable_sleeptime: Optional[bool]` (§5.2, §5.2b).
5. **No `enable_prompt_caching` / `prompt_cache` flag exists** — RESOLVED across `settings.py`, `constants.py`, `anthropic_client.py`, `anthropic_constants.py` (all read in full) (§6.1). Repo-wide absence NOT VERIFIED (code search unavailable).
6. **No `ttl` / `"1h"` cache TTL** anywhere in Letta's Anthropic client — only default ephemeral (5m). Verified.
7. **No dedicated Letta blog post or docs page on prompt caching.** The phrase "the agent state IS the prompt" is **not** attributable to Letta.
8. **No `manager` manager_type** and **no `sleeptime_agent` agent field** — RESOLVED: `ManagerType` has 6 members; `swarm`'s class is commented out (§5.2).
9. **The four memory types (Message Buffer / Core Memory / Recall Memory / Archival Memory) are a JUL 2025 statement**, superseded by the MemFS rewrite (§1.3b).
10. ~~`conversation_search_date` / `pause_heartbeats`~~ — **NOW RESOLVED** (§1.6): both exist in the released 2023 schema; `conversation_search_date` is simply excluded from the modern default toolset.
11. **`memory_append` vs `memory_insert` discrepancy** — the blog prints `memory_append`; the code has `memory_insert` and no `memory_append`. Unresolved; treat `memory_append` as a blog typo.
12. **Prompt-caching documentation is absent from Letta's public docs and blog entirely** (§6). Primary evidence is code + tests (§6.1–6.3) plus one unofficial forum answer.
13. **Render order of prompt components is never stated** by Letta; post 2 says formatting is customizable "with Jinja templating".
14. **Post 1's kernel/user-context diagram is an image with no alt text or caption.**
15. **Sleep-time tool names differ across paper / experiment code / shipped product** (§5.6).
16. **Paper's Figures 12–18 sleep-time prompts are images** (not text-extractable); per-model AIME/GSM numbers are figures, not tables.
17. **"Heartbeat" and "step" are never formally defined in either MemGPT paper version**; timed-heartbeat interval NOT VERIFIED.
18. **`request_heartbeat`'s canonical description string is not in the paper** (§4.4), and two divergent description strings exist in the released schema.
19. **The paper never says "sleep-time agent" nor who owns memory editing** (§5.6).
20. **Original `docs.memgpt.ai` content not retrieved.**
21. **`EMBEDDING_BATCH_SIZE = 200` has no located consumer** (the OpenAI client hard-codes 2048/128) — [NOT VERIFIED].
22. **Full `letta/services/` and `tests/` directory listings not obtained** (GitHub REST API rate-limited, HTTP 403), so several "absent from every file read" claims are not repo-wide proofs.
23. **Latent code inconsistency:** the embedding-padding guard keys on `should_use_tpuf()` (a *global* setting) while the TPUF write path keys on `archive.vector_db_provider` (a *per-archive* field). They agree today only because `create_archive_async` derives one from the other.

---

## 9. Key URLs

- https://arxiv.org/html/2310.08560v2 — MemGPT paper, full HTML (§2, §2.1, §2.2, §2.4, §3.2.1 are the load-bearing sections)
- https://raw.githubusercontent.com/letta-ai/context-constitution/main/constitution/CONSTITUTION.md — **current** context hierarchy, primitives, progressive disclosure, caching guidance
- https://docs.letta.com/llms.txt — authoritative map of the current docs; shows `v1-sdk` as legacy
- https://docs.letta.com/concepts/memfs/index.md — MemFS
- https://docs.letta.com/configuration/memory.md + https://docs.letta.com/agent-sdk/memory/index.md — memory & dreaming
- https://docs.letta.com/v1-sdk/memory/context-hierarchy.md — legacy, the 4-abstraction table
- https://docs.letta.com/v1-sdk/memory/archival-memory.md — legacy, archival semantics + tags
- https://docs.letta.com/v1-sdk/memory/memory-blocks.md — `<memory_blocks>` wire format
- https://docs.letta.com/api/resources/agents/methods/create/index.md — `AgentType`, `ToolType`, `compaction_settings`, `embedding_chunk_size` (deprecated)
- https://www.letta.com/blog/letta-v1-agent/ — **heartbeats + send_message deprecated; `letta_v1_agent`**
- https://www.letta.com/blog/sleep-time-compute — **primary loses core-memory tools; sleep-time agent holds them**
- https://www.letta.com/blog/why-memory-isnt-a-plugin/ — "memory is a harness responsibility, not a plugin"
- https://www.letta.com/blog/guide-to-context-engineering/ — context-window component list
- https://www.letta.com/blog/agent-memory/ — the four memory types (JUL 2025)
- https://arxiv.org/abs/2504.13171 — Sleep-time Compute
- https://github.com/letta-ai/letta (pin `67013ef1bb491ec8bdd629bfbeceb70227936afa`) — `letta/constants.py`, `letta/schemas/enums.py`, `letta/llm_api/anthropic_client.py`, `letta/groups/sleeptime_multi_agent.py`, `tests/test_prompt_caching.py`
- https://web.archive.org/web/20250914103956id_/https://docs.letta.com/guides/agents/heartbeats — heartbeat implementation detail (Wayback; page is now 404)
- https://forum.letta.com/t/breaking-prompt-caching/149 — unofficial caching trade-off answer
