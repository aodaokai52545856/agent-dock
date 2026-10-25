# Letta Official Blog — Verbatim Fact Extraction (Memory Architecture)

Extraction method: all five posts fetched with `web_fetch` (HTTP 200, full-page markdown decode), then re-fetched through an
independent second extraction pipeline (`r.jina.ai`). Both passes returned **byte-identical article text** for all five posts,
so the "NOT FOUND" entries below are verified absences in the served pages, not artifacts of one decoder.
Quotes are verbatim (including Letta's own typography and its occasional typos). Nothing below is paraphrased from memory.

Limitation that must be stated in any report: figures/diagrams on these pages are images. Post 1's key anatomy diagram
(`guide-to-context-engineering-f4.webp`) carries **no alt text and no caption**, so any labels drawn inside it are NOT
recoverable from the HTML. Only the captions quoted below exist as text.

---

## Post 1 — "Anatomy of a Context Window: A Guide to Context Engineering"
URL: https://www.letta.com/blog/guide-to-context-engineering/ — `CONCEPTS — JUL 3, 2025`

### (A) Context-window anatomy — THE canonical list (verbatim)

> "An agent's context window consists of the following components:
> - **System prompt**: Defines the agent's architecture and control flow, providing high-level behavioral instructions and explaining core concepts like memory, blocks, and files.
> - **Tool schemas**: Specifications that define available tools and their interfaces, enabling the agent to understand what actions it can perform.
> - **System metadata:** Stores statistics and metadata about the agent's state (e.g. the size of the full message history) .
> - **Memory blocks**: Persistent units of context (e.g. for long term memory or working context), managed by the agent itself or other background agents.
> - **Files & Artifacts**: Files (PDFs, source code, etc.) that the agent can access and manipulate.
> - **Message buffer**: The message stream containing messages (user, assistant, and system, and tool calls and tool returns)."
> — https://www.letta.com/blog/guide-to-context-engineering/

So the named tiers are exactly six: **System prompt / Tool schemas / System metadata / Memory blocks / Files & Artifacts /
Message buffer**. The "Memory blocks" bullet is hyperlinked to https://www.letta.com/blog/memory-blocks/.

### (A) The "two-scope" hierarchy (this is what Letta actually calls a hierarchy here)

> "Like resources in traditional operating systems, in an LLM OS, _context_ is scoped to the application layer (i.e. user space) and the system layer (i.e. kernel space):
> - **Kernel Context** - Managed context that can be modified through APIs and tool calling (like kernel memory). This represents the underlying agent configuration (the system prompt and tools), as well as managed context such as memory blocks and files.
> - **User Context** - The message buffer containing conversations and external context (like user processes), as well as system calls (tools) to modify the kernel context."
> — https://www.letta.com/blog/guide-to-context-engineering/

> "Kernel context represents the managed, mutable state of an agent—analogous to kernel memory in an operating system. This layer enables agents to maintain memory and work with persistent data structures that evolve over time. Crucially, the kernel context is rendered into the context window where the LLM can observe it, but modifications happen through controlled interfaces."
> — https://www.letta.com/blog/guide-to-context-engineering/

**NOT FOUND (exact phrase):** "context hierarchy". Letta never uses those two words in this post.
**NOT FOUND:** any ordered template of the prompt. The six components are listed as *components*, not as a rendering order.
**NOT FOUND in this post:** summarization, compaction, eviction, FIFO, queue, archival memory as a context tier.
**NOT FOUND in this post:** prompt caching of any kind.

Diagram captions in this post (the only two text captions):
> "Typical operating system architecture (from linux-kernel-labs.github.io)"
> — https://www.letta.com/blog/guide-to-context-engineering/
(The figure at `guide-to-context-engineering-f3.png`; the f4 webp that depicts the kernel/user context split has no caption text.)

### (A) Message buffer sub-anatomy (verbatim)

> "The buffer contains several message types:
> - **User messages**: Direct input from users
> - **Assistant messages**: The agent's responses
> - **System messages**: Framework or system-generated notifications (like system logs)
> - **Tool calls and responses**: The log of both system calls and other tools, as well as their results"
> — https://www.letta.com/blog/guide-to-context-engineering/

### (A) Context management definition + external context

> "Context management refers to both how this context window is _designed_ (through configuration of files, blocks, tools, and prompts), as well as how this context window _evolves_ over time. The context window can be controlled directly by the underlying system (the "LLM OS" or "AI OS") or with agentic tool calling (which is executed by the OS)."
> — https://www.letta.com/blog/guide-to-context-engineering/

> "For example, external context can be pulled into the context window via tools, such as with MCP (Model Context Protocol) or built-in tools for conversational or file search"
> — https://www.letta.com/blog/guide-to-context-engineering/
(This is the nearest thing in all five posts to a "conversation search" tool — the phrase is "built-in tools for conversational or
file search", and it links to https://docs.letta.com/guides/core-concepts/filesystem/.)

### (B) Memory blocks in post 1 (verbatim)

> "Introduced by MemGPT, memory blocks are reserved portions of the context window designed for persistent memory. Each memory block has several key properties:
> - **Size limits**: Hard constraints preventing overflow
> - **Labels and descriptions**: Metadata that guides what information should be stored
> - **Access patterns**: Including read-only flags for protected memory"
> — https://www.letta.com/blog/guide-to-context-engineering/

> "Memory blocks enable agents to maintain state across conversations, learn from interactions, and build up knowledge over time, and can also be used as "working memory" to achieve complex tasks like deep research. Memory blocks can also be shared across multiple agents."
> — https://www.letta.com/blog/guide-to-context-engineering/

### (B) The exact built-in tool names in post 1 (verbatim — this is the only tool-name list in any of the five posts)

> "Just as operating systems provide system calls for user processes to interact with kernel space, the LLM OS provides built-in tools that act as the system call interface:
> - **Memory operations**: Tools like memory_replace, memory_rethink, and memory_append allow controlled modification of memory blocks
> - **File operations**: Tools like open, close, and grep manage file state in kernel space
> - **Custom operations**: Custom tools can call APIs to modify underlying state"
> — https://www.letta.com/blog/guide-to-context-engineering/

Printed names are exactly: `memory_replace`, `memory_rethink`, `memory_append` (memory); `open`, `close`, `grep` (files).
**NOT FOUND in any of the five posts:** `memory_insert`, `core_memory_append`, `core_memory_replace`, and therefore also
NOT FOUND: any statement that `core_memory_append`/`core_memory_replace` are deprecated or "replaced by" the newer names.
Do not attribute a deprecation claim to these pages.

### (B) Files/artifacts + system-vs-custom tools (verbatim)

> "Files can be "open" (loaded into context) or "closed" (stored with metadata only), and also searched with common operations like vector search or grep, which translates well to agent capabilities"
> — https://www.letta.com/blog/guide-to-context-engineering/

> "**Artifacts** represent a special category of editable files (often with extra dependencies or file groupings). These allow agents to iteratively modify content—whether code, documentation, or creative writing—through successive refinements."
> — https://www.letta.com/blog/guide-to-context-engineering/

> "**System Tools** (Built-in system calls) are provided by the LLM OS itself and have privileged access to modify agent state internals reflected in the compiled kernel context (blocks & files)."
> — https://www.letta.com/blog/guide-to-context-engineering/

> "**Custom Tools** (User programs) are defined by developers and run in user space with limited privileges."
> — https://www.letta.com/blog/guide-to-context-engineering/

### (D) Caching-related: NOT FOUND
Post 1 contains **no** occurrence of "cache", "caching", "prompt caching", "prefix", or "byte-stable".
The only adjacent (non-caching) statement is the phrase "the compiled kernel context (blocks & files)" quoted above.

---

## Post 2 — "Memory Blocks: The Key to Agentic Context Management"
URL: https://www.letta.com/blog/memory-blocks/ — `CONCEPTS — MAY 14, 2025`

### (B) Definition of a memory block with its fields (verbatim — the `label`/`value`/`limit`/`description` set)

> "Rather than treating the context window as a monolithic entity, memory blocks break the context window down into manageable, purposeful units that are also persisted. A memory block consists of:
> - A **label** that identifies its purpose (e.g., "human", "persona", "knowledge")
> - A **value** that is the string representation of the block data
> - A size **limit** (e.g. in characters or tokens) that dictates how much of the context window it can occupy
> - Optional **descriptions** that guide how the block should be used"
> — https://www.letta.com/blog/memory-blocks/

### (B) Read-only blocks (verbatim)

> "The block value can be edited by the agent via memory tools (or custom tools), unless they are **read-only**: in which case, only the developer can modify them."
> — https://www.letta.com/blog/memory-blocks/

### (B) Shared blocks (verbatim)

> "One of Letta's most powerful features is the ability for multiple agents to share memory blocks"
> — https://www.letta.com/blog/memory-blocks/

> "This enables sophisticated patterns like:
> - **Shared knowledge bases**: Multiple agents accessing the same reference information
> - **Sleep-time compute**: Background agents updating the memory of primary agents
> - **Collaborative memory**: Teams of agents maintaining a shared understanding"
> — https://www.letta.com/blog/memory-blocks/

### (B) Character limits — MemGPT origin (verbatim)

> "These blocks were editable by the agent, and also restricted to a certain character limit (to limit context window allocation to those memories)."
> — https://www.letta.com/blog/memory-blocks/
The two original MemGPT blocks are named as: "**"Human" memory block**: Storing information about the user, their preferences,
facts about them, and relevant context." and "**"Persona" memory block**: Containing the agent's own self-concept, personality
traits, and behavioral guidelines." (both at the same URL).

### (D-adjacent, NOT a caching claim) "Compiled from DB state" + Jinja templating (verbatim)

> "Unlike ephemeral memory in many LLM frameworks, Letta's memory blocks are individually persisted in the DB, with a unique `block_id` to access them via the API and Agent Development Environment (ADE). This provides a way for developers to directly modify parts of their agent's context window."
> — https://www.letta.com/blog/memory-blocks/

> "When Letta makes a request to the LLM, the context window is "compiled" from existing DB state, including current block values. The prompt template (i.e. formatting in the context window) can be customized with Jinja templating."
> — https://www.letta.com/blog/memory-blocks/
This is the closest any of the five posts comes to "prompt is rebuilt from state each call", and it also shows the rendering
order is template-defined. It says **nothing** about caching, cache invalidation, or prefix stability — those are NOT FOUND.

### (B) Tool-based memory editing in post 2 (verbatim; a *custom* tool, not a built-in name)

> "Memory blocks can be modified through a variety of tools to customize memory management. Below is an example of a custom tool to "rethink" (replace) an entire memory block's value by specifying the new block value and the target block label:"
> — https://www.letta.com/blog/memory-blocks/

```python
def rethink_memory(agent_state: "AgentState", new_memory: str, target_block_label: str):
    """Re-evaluate the memory in the block, integrating new information."""
    if agent_state.memory.get_block(target_block_label) is None:
        # Create the memory block if it doesn't exist yet
        agent_state.memory.create_block(
            label=target_block_label,
            value=new_memory,
        )

    # Update the memory block with the new information
    agent_state.memory.update_block_value(
        label=target_block_label,
        value=new_memory,
    )
```
— https://www.letta.com/blog/memory-blocks/

> "Because memory block values are just strings, you can store more complex data structures like lists or dictionaries in blocks, as long as they can be represented into a string format."
> — https://www.letta.com/blog/memory-blocks/

### (B/D) Sleep-time agents writing blocks (verbatim)

> "As we demonstrated in our Sleep-time Compute research, agents can process information during idle periods and update shared memory blocks. ... Learned context is written to a memory block in Letta, and can be shared across multiple agents."
> — https://www.letta.com/blog/memory-blocks/

**NOT FOUND in post 2:** any built-in tool name (`memory_insert` / `memory_replace` / `memory_rethink` / `core_memory_*`), any
deprecation statement, any mention of recall memory, archival memory, caching, or context-window ordering.

---

## Post 3 — "Agent Memory: How to Build Agents That Learn and Remember"
URL: https://www.letta.com/blog/agent-memory/ — `CONCEPTS — JUL 7, 2025`

### (A/B/C) The four named memory components (verbatim — this is the tier taxonomy)

> "Agent memory systems typically consist of several distinct components, each serving different purposes:"
> — https://www.letta.com/blog/agent-memory/

> "The message buffer stores the most recent messages in a conversation. In Letta, every agent maintains a single perpetual thread, which represents a continuous sequence of messages. This provides immediate conversational context and maintains dialogue flow."
> — https://www.letta.com/blog/agent-memory/

> "Core memory consists of in-context memory blocks that can be managed by the agent itself or by other agents. These blocks focus on specific topics such as memories about the user, organization, or the current task. For example, one block might contain user preferences, while another maintains the agent's persona or current objectives. The key feature is that these blocks are editable via APIs and remain pinned to the agent's context window, providing an abstraction for managed context units."
> — https://www.letta.com/blog/agent-memory/

### (C) RECALL vs ARCHIVAL — explicitly still two distinct things (verbatim)

> "Recall memory preserves the complete history of interactions that can be searched and retrieved when needed, even when not in the active context window (i.e., in the message buffer). In Letta, recall memory saves to disk automatically, while other frameworks require developers to handle persistence manually."
> — https://www.letta.com/blog/agent-memory/

> "Archival memory represents explicitly formulated knowledge stored in external databases. Unlike recall memory, which stores raw conversation history, archival memory contains processed and indexed information. It can utilize different storage formats, such as vector databases or graph databases, with specialized tools that query and retrieve data back into the context window."
> — https://www.letta.com/blog/agent-memory/

**Answer to (C) for this post:** yes — as of JUL 7, 2025 Letta still distinguishes recall memory (conversation history) from
archival memory, and recall is described as its own persisted tier ("saves to disk automatically", searchable "even when not in
the active context window").
**NOT FOUND in any of the five posts:** the tool name `conversation_search`; and NOT FOUND: any statement that recall storage
was removed/collapsed and that conversation history is now searched directly by a single tool.

### (B) Block fields as stated in post 3 (verbatim — note the wording differs from post 2)

> "Memory blocks provide structured, editable storage within the agent's context window. Each block contains:
> - A **label**
> - A **description** (explaining what's stored in the block)
> - A **value** (the actual tokens placed in context)
> - A **character** limit (defining how much context window space is allocated)"
> — https://www.letta.com/blog/agent-memory/

> "Memory blocks abstract the context window for automated management. Agents can update their own memory blocks based on new information, using tools to rewrite specific blocks. Other agents specialized in memory management (such as sleep-time agents) can also modify these blocks. This creates a mechanism for context rewriting, allowing agents to improve their context window over time by consolidating important information."
> — https://www.letta.com/blog/agent-memory/
**NOT FOUND in post 3:** any concrete tool name for block editing (it says only "using tools to rewrite specific blocks").

### (A) Eviction, summarization, "memory hierarchy", storage tiers (verbatim)

> "One fundamental challenge in agent memory is managing the limited context window. Summarization techniques help compress information while preserving essential details:"
> — https://www.letta.com/blog/agent-memory/

> "**Eviction Methods:** When the context window reaches capacity, intelligent eviction strategies determine what information to remove. This might involve summarizing and storing important details before removing them from active context. Generally, you should evict only a portion (e.g., 70%) of messages to ensure continuity."
> — https://www.letta.com/blog/agent-memory/

> "**Recursive Summarization:** Evicted messages undergo recursive summarization—they're summarized along with existing summaries from previously summarized messages. As conversations grow longer, older messages have progressively less influence on the summary than recent messages."
> — https://www.letta.com/blog/agent-memory/

> "MemGPT (MemoryGPT) is a system that intelligently manages different storage tiers to effectively provide extended context within the LLM's limited context window. MemGPT treats context windows as a constrained memory resource and implements a memory hierarchy similar to operating systems."
> — https://www.letta.com/blog/agent-memory/
Diagram caption in this post: "System figure from the original MemGPT research paper" (same URL).

> "Agents can move data between in-context core memory (analogous to RAM) and externally stored archival and recall memory (analogous to disk storage), creating an illusion of unlimited memory while working within fixed context limits."
> — https://www.letta.com/blog/agent-memory/

> "An agent's "short-term" memory consists of whatever resides in the message buffer, as this content will eventually be evicted. All other memory types qualify as "long-term." ... Ultimately, memory is about choosing which tokens to place in your context window at any given moment."
> — https://www.letta.com/blog/agent-memory/

### (C) RAG-vs-memory in post 3 + vector/graph DBs (verbatim)

> "Memory can also be stored in external databases and retrieved via tool calling. Different storage and retrieval mechanisms suit different applications:
> - **Vector DBs:** Memories are saved, embedded, and queried via vector search
> - **Graph DBs:** Memories form graph structures where agents can traverse relationships between concepts, enabling sophisticated reasoning about connected information"
> — https://www.letta.com/blog/agent-memory/

> "While retrieval (or RAG) is a tool for agent memory, it is not "memory" in of itself."
> — https://www.letta.com/blog/agent-memory/

### (A) Memory as context management (verbatim)

> "Therefore, designing an agent's memory is essentially context engineering: determining which tokens enter the context window and how they're organized. Memory systems compose multiple techniques (such as summarization, context rewriting, and retrieval) to manage various memory components (messages, memory blocks, and external databases)."
> — https://www.letta.com/blog/agent-memory/

### (D) Caching: NOT FOUND. "the agent state IS the prompt": NOT FOUND.

---

## Post 4 — "Introducing Claude Sonnet 4.5 and the Memory Omni-Tool in Letta"
URL: https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/ — `PRODUCT — SEP 30, 2025`

### (B) The omni-tool: exact tool name and what it does (verbatim)

> "You can now use Anthropic's [`memory` tool](https://docs.claude.com/en/docs/agents-and-tools/tool-use/memory-tool) with Letta agents, enabling Letta agents to take full advantage of Claude Sonnet 4.5's advanced memory capabilities to dynamically manage their own memory blocks. Agents in Letta can now not only modify existing memory blocks, but also create and delete memory blocks on-the-fly as they learn over time."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

> "Previously, enabling an agent to create and delete memory blocks required developers to write custom tools or scripts building on the Letta API - but now, using the new built-in memory omni-tool, agents can easily handle it themselves, reorganizing their in-context memory as needed."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

> "While Claude Sonnet 4.5 is specifically post-trained for the new `memory` tool and excels at using it, the tool works with any model available on Letta. And because Letta manages the underlying memory state, all memories remain model agnostic, API-accessible, and shareable between agents for collaborative learning."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

> "Letta provides Anthropic's `memory` tool as a new built-in tool that manages persistence of memories through memory blocks."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

> "The Letta `memory` tool exposes the same filesystem-like API to the model as Anthropic's version (storing memories in specific paths), but under the hood, the memory "files" are actually stored inside of Letta memory blocks."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

Precise naming to use in the report: the **exposed tool name is `memory`** (Anthropic's tool); "memory omni-tool" is Letta's
descriptive label for it, not a literal tool-name string. Section heading is verbatim "Using the `memory` tool in Letta".

### (B) Relationship to Anthropic's memory tool / context management (verbatim)

> "[Claude 4.5 Sonnet](https://www.anthropic.com/news/context-management) is the first frontier model trained (publically) to be [context-aware](https://cognition.ai/blog/devin-sonnet-4-5-lessons-and-challenges), released alongside a new [`memory` tool](https://docs.claude.com/en/docs/agents-and-tools/tool-use/memory-tool) for context management."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

> "Because the model is post-trained to understand context limitations and use a context-editing memory tool, it is able to manage its own context window more efficiently with less in-context prompting."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

> "Memory tools in MemGPT and Letta have historically relied on the generality of LLM's tool use capabilities. Frontier models can follow instructions from prompts to understand how to use memory tools to manage their memory and context."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

Exact model/API feature names that DO appear: "Claude Sonnet 4.5" / "Claude 4.5 Sonnet"; the `memory` tool; "context
management"; "context-aware"; "a context-editing memory tool". Outbound authoritative links printed by the post:
https://docs.claude.com/en/docs/agents-and-tools/tool-use/memory-tool and https://www.anthropic.com/news/context-management.

**NOT FOUND in post 4 (or anywhere in the five posts):** "memory_20250818" (the Anthropic tool-version string);
"clear_tool_uses"; "tool_use clearing"; the unhyphenated phrase "context editing". Do **not** claim Letta documents
Anthropic's context-editing / tool-use-clearing features on the basis of this post — it only links to Anthropic's pages.

### (B) Dynamic block creation + manual management (verbatim)

> "The memory tool enables agents to dynamically restructure their memory as they learn by creating new memory blocks. An agent might start with basic memory blocks but evolve to create specialized sections for different types of information. This allows your agent to restructure its in-context memory over time."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/
Caption for the accompanying figure: "Example of an agent creating a memory block focused on learning how to use tools."
The other figure caption reads: "The memory tool being called inside of Letta, which updates the human memory block stored on
Letta." (both at the same URL).

> "Since memories are stored as Letta memory blocks, you can:
> - Query and edit memories through the Letta API or Agent Development Environment (ADE), using the associated `block_id`
> - Share memory between agents by attaching existing memory blocks to new agents - enabling collaborative learning or knowledge transfer from experienced agents to new ones"
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

### (C/D) Storage abstractions + context pollution (verbatim)

> "This phenomenon, known as "context pollution," means that simply cramming more information into the context window isn't a viable long-term solution for agent memory."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

> "Early memory systems primarily focused on retrieval of prior conversations (RAG). MemGPT introduced the idea of agentic memory management through using tools to read/write to an external vector DB as well as rewrite segments of the agent's context window - enabling the agent to actually _learn_ over time."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

> "Many models have also been explicitly post-trained at software engineering tasks, making them especially adept at filesystem operations - meaning that a simple filesystem can outperform purpose-built memory tools. Letta enables support for a variety of memory management tools by providing underlying storage abstractions (such as memory blocks, filesystem, and archives) that can be used to implement specialized memory tools."
> — https://www.letta.com/blog/introducing-sonnet-4-5-and-the-memory-omni-tool-in-letta/

**NOT FOUND in post 4:** "recall memory" as a named tier (the word used is "archives"); caching of any kind.

---

## Post 5 — "RAG Is Not Agent Memory"
URL: https://www.letta.com/blog/rag-vs-agent-memory/ — `CONCEPTS — FEB 13, 2025`

### (C) RAG vs agent memory — the core thesis (verbatim)

> "RAG ("retrieval augmented generation") is currently the go-to solution whenever we need to connect LLMs to external sources of information. RAG is even used as a form of memory for conversational agents, by retrieving old messages that are semantically similar to the most recent message."
> — https://www.letta.com/blog/rag-vs-agent-memory/

> "Although RAG provides a way to connect LLMs and agents to more data than what can fit into context, it doesn't come without its limitations and risks. RAG often places irrelevant data into the context window, resulting in context pollution and degraded performance (especially for newer reasoning models). Although RAG is an important tool in the agents stack, it is far from a complete solution."
> — https://www.letta.com/blog/rag-vs-agent-memory/
(The two in-text links behind that sentence point to https://arxiv.org/pdf/2302.00093 and https://arxiv.org/abs/2307.03172.)

### (C) Explicit vector-DB / embedding / similarity statements (verbatim)

> "RAG rose in popularity because of its simplicity: you embed a document in your LLM workflow, use an algorithm to find the top-K-related snippets of that document, and then deposit those snippets into the context window. This can also be used to provide a rudimentary form of memory, by searching over old messages."
> — https://www.letta.com/blog/rag-vs-agent-memory/

> "Typically, RAG sorts the shreds from Step 2 by cosine similarity, a methodology rooted in correlation and notoriously bad at finding relevant snippets. Putting the top 10 only possibly applicable excerpts into a context window will likely lead to irrelevant results."
> — https://www.letta.com/blog/rag-vs-agent-memory/

> "If a user says, "Today is my birthday," a RAG agent will search for the word "birthday" in the vector database. The problem with searching for the word "birthday" is that the agent relies on RAG to mimic having memory by retrieving potentially relevant previous messages."
> — https://www.letta.com/blog/rag-vs-agent-memory/

> "But RAG isn't going to retrieve personalization information that isn't semantically similar (i.e., according to an embedding model) to the searched word."
> — https://www.letta.com/blog/rag-vs-agent-memory/

> "In fact, you'll never get that level of personalization by embedding search (how RAG is usually implemented) because that personal information will never be retrieved."
> — https://www.letta.com/blog/rag-vs-agent-memory/

Headings that carry the argument (verbatim): "1. RAG is single step" and "2. RAG is purely reactive".

### (C) The proposed alternative: "agentic RAG" (verbatim)

> "The approach described above, multi-step reasoning with tools, is "agentic RAG," and it's the underlying foundation of Letta. Letta's design takes all the work that's gone into developing the search and doc retrieval tools we use today and embeds those tools into LLMs, preparing the context window by summarizing and organizing its "memory.""
> — https://www.letta.com/blog/rag-vs-agent-memory/

> "With agentic RAG, LLMs can paginate through multiple pages of results, potentially even traversing an entire dataset, while also maintaining state."
> — https://www.letta.com/blog/rag-vs-agent-memory/

> "With agentic RAG, an AI agent isn't doing a top-K match and dump. It's already distilled important data it's received in the past (a customer's favorite movie or favorite color) and organized it in such a way that the model can proactively relate it to the user's prompt and curate a response"
> — https://www.letta.com/blog/rag-vs-agent-memory/

> "For companies that want to build robust AI agents, traditional RAG is insufficient. The key to developing AI agents that are precise, interpretable, proactive, and deeply aligned with an organization's unique goals and data environments is to give your LLMs memory."
> — https://www.letta.com/blog/rag-vs-agent-memory/

**NOT FOUND in post 5:** the terms "recall memory", "archival memory", "memory block" (plural or singular), "core memory" — post 5
never uses Letta's own tier vocabulary; it argues only RAG-vs-memory. **NOT FOUND in post 5:** prompt caching.

---

## Cross-post "NOT FOUND" ledger (verified absences — do not assert these from these pages)

| Claim searched for | Posts checked | Result |
| --- | --- | --- |
| "prompt caching", "cache", "cache invalidation", "stable prefix", "byte-stable" | all 5 | NOT FOUND |
| "the agent state IS the prompt" (or any equivalent slogan) | all 5 | NOT FOUND |
| "context hierarchy" (exact phrase) | all 5 | NOT FOUND (post 1 uses "Kernel Context"/"User Context"; post 3 uses "memory hierarchy" for MemGPT) |
| Rendering order of prompt components | all 5 | NOT FOUND (post 2 says formatting "can be customized with Jinja templating") |
| FIFO queue / message queue wording | all 5 | NOT FOUND |
| `conversation_search` | all 5 | NOT FOUND |
| `memory_insert` | all 5 | NOT FOUND |
| `core_memory_append`, `core_memory_replace` + deprecation claim | all 5 | NOT FOUND (only named memory tools: `memory_replace`, `memory_rethink`, `memory_append` in post 1) |
| `memory_20250818` | all 5 | NOT FOUND |
| "context editing", `clear_tool_uses`, "tool_use clearing" | all 5 | NOT FOUND (post 4 says "a context-editing memory tool" and links to anthropic.com/news/context-management) |
| Compaction (as a named operation) | all 5 | NOT FOUND as a term; post 3 covers the concept as "Message Eviction & Summarization" |

## Retrieval notes
- All five pages returned HTTP 200 and decoded to complete articles (intro through footer).
- Observed footer on all five pages: "© 2026 Letta" (site-rendered copyright), alongside GitHub / Discord / X links.
- Post dates as printed by the pages: JUL 3 2025 (context engineering), MAY 14 2025 (memory blocks), JUL 7 2025 (agent memory),
  SEP 30 2025 (Sonnet 4.5 / memory omni-tool), FEB 13 2025 (RAG is not agent memory).
