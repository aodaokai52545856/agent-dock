# Anthropic Claude 生态：记忆系统与 Prompt Caching 深度调研

> 范围：Claude Code、Claude Agent SDK、Claude Developer Platform（Messages API）。
> 数字与结论均出自官方文档/工程博客，URL 就近标注。凡属笔者推断，一律标 **【推断】**。
> 快照：Claude Code 版本线 v2.1.28x。

## 0. 结论速览

1. **Claude Code 的整个 harness 是围绕 prompt caching 设计的**——官方原话（[claude.dev 博客](https://claude.dev/blog/lessons-from-building-claude-code-prompt-caching-is-everything/)）。
2. API 层 context editing **官方明确承认会失效缓存**，并把"值不值得破缓存"显式化为一个参数 `clear_at_least`。
3. Claude Code 记忆是**双轨**：你写的 `CLAUDE.md`（可提交/可企业下发）+ Claude 自写的 **auto memory**（机器本地），二者每次会话开始都注入。
4. 上下文管理是**三级瀑布**：先清旧工具输出 → 再摘要压缩 → 长任务靠 note-taking / subagent 外置。

## 1. Claude Code 的记忆分层

### 1.1 四层级与精确路径（[memory 页](https://code.claude.com/docs/en/memory)）

| 层级 | 路径 | 共享范围 |
|---|---|---|
| **Managed policy**（企业） | macOS `/Library/Application Support/ClaudeCode/CLAUDE.md`；Linux/WSL `/etc/claude-code/CLAUDE.md`；Windows `C:\Program Files\ClaudeCode\CLAUDE.md` | 组织内全员 |
| **User** | `~/.claude/CLAUDE.md` | 仅你（所有项目） |
| **Project** | `./CLAUDE.md` 或 `./.claude/CLAUDE.md` | 团队（随源码） |
| **Local** | `./CLAUDE.local.md` | 仅你（当前项目，建议 gitignore） |

- **企业可内联**：`managed-settings.json` 的 `claudeMd` 字段等价于下发 managed CLAUDE.md，"Loads before user and project CLAUDE.md"，且仅在 managed/policy 设置中生效：`{ "claudeMd": "Always run `make lint` before committing." }`。
- **`.claude/rules/`**：`.md` 递归发现；无 `paths` frontmatter 的规则在启动时以与 `.claude/CLAUDE.md` **相同优先级**加载；带 `paths` 的按 glob 条件加载（"trigger when Claude reads files matching the pattern, not on every tool use"）。用户级 `~/.claude/rules/` 在项目规则**之前**加载。规则唯一被识别的 frontmatter 字段是 `paths`。
- **拼接而非覆盖**：目录树"从文件系统根向下到工作目录"排列，故 `foo/CLAUDE.md` 在 `foo/bar/CLAUDE.md` **之前**；同目录内 `CLAUDE.local.md` 追加在 `CLAUDE.md` **之后**。冲突时 Claude 可能任选其一。块级 HTML 注释注入前被剥离（代码块内的保留）。
- **投递方式**："CLAUDE.md content is delivered as a **user message after the system prompt, not as part of the system prompt itself**"——因此官方明说没有严格遵从保证。要 system prompt 级指令用 `--append-system-prompt`。

### 1.2 子目录 CLAUDE.md 的加载时机

工作目录**及其之上**的 `CLAUDE.md` / `CLAUDE.local.md` 启动时加载；**其下**子目录的**按需加载**——"they are included when Claude reads files in those subdirectories"。嵌套 `.claude/rules/` 与 `paths` 规则同理。

### 1.3 `@import` 与递归深度

- 语法 `@path/to/import`；相对路径**相对包含该 import 的文件**解析，也支持绝对路径与 `~`。
- **官方原文："Imported files can recursively import other files, with a maximum depth of four hops."** —— 是 **4 跳**，"5 层"的说法与当前文档不符。
- 解析**跳过行内代码与围栏代码块**：`` `@README` `` 是字面量，`@README` 才导入。
- "Imported files are expanded and loaded into context **at launch**"——故**`@import` 只改善组织性、不减少 context token**。CLAUDE.md 建议 <200 行；**单文件超 4 MiB 被跳过**。
- 项目级文件的 `@` 若解析到工作目录之外，首次触发审批对话框；拒绝后保持禁用。用户级文件的导入受信任、不弹窗。
- 核销说明：网上流传的"导入文件不会被重新求值"在当前官方文档中**查无此句**。

### 1.4 `/memory`、`#`、`--add-dir`

- **`/memory`**："Edit `CLAUDE.md` files, enable or disable auto memory, and view auto memory entries"——列出各 scope 位置（含尚不存在的条目，选中即创建并打开）。
- **`#` 快捷记忆：当前官方文档中不存在。** 交互模式"Quick commands"只列 5 个前缀：`/`（命令/skill）、`!`（shell 模式）、`@`（文件路径补全）、`:`（emoji 短码）、`?`（空输入开帮助）。官方现有路径是 `/memory` 打开编辑、直接说 "add this to CLAUDE.md"、或交给 auto memory。**注意：说 "remember this" 默认写进 auto memory，不会写进 CLAUDE.md**——这是最常见的误用。
- **`--add-dir`**：默认**不加载**附加目录的 CLAUDE.md。需显式开启：
  ```bash
  CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD=1 claude --add-dir ../shared-config
  ```
  开启后加载该目录的 `CLAUDE.md`、`.claude/CLAUDE.md`、`.claude/rules/*.md`、`CLAUDE.local.md`。注意它同时会加载该目录的 `.claude/skills|commands|agents/`；而 `settings.json` 的 `permissions.additionalDirectories` **只授权文件访问，不加载配置**。
- **`/init`**（`CLAUDE_CODE_NEW_INIT=1` 进入交互流程）、**`/import`**（v2.1.213+ 导入其他 agent 配置）。

## 2. Claude Code 的上下文管理

### 2.1 auto-compact 阈值与行为（[model-config](https://code.claude.com/docs/en/model-config)、[context-window](https://code.claude.com/docs/en/context-window)）

- 默认 **= 模型 context limit**。例外：云会话"接近"上限时压缩；Sonnet 4.6 / Opus 4.6（未开 extended）在 **200K** 边界；原生 1M 窗口模型（Sonnet 5、Fable 系列、Anthropic API 的 Opus 4.7+）默认在**约 967K**。
- **不存在"百分比阈值"这种官方说法**——所有触发条件都是 token 数。"到 95% 触发"在任何官方页面都查不到。核销：`DISABLE_COMPACT` 关闭全部压缩（含手动 `/compact`）。
- **可调**：`/autocompact 500k`（写入 `autoCompactWindow`）、`--autocompact`、`CLAUDE_CODE_AUTO_COMPACT_WINDOW`（优先级最高）。窗口 **100K–1M**，支持 `200000` / `500k` / `1M` / 裸数字（100–1000 表示千），上限受模型窗口封顶。
- **行为原文**："It **clears older tool outputs first**, then summarizes the conversation if needed. Your requests and key code snippets are preserved; detailed instructions from early in the conversation may be lost."
- **防抖**：若单个文件/工具输出大到每次摘要后立刻填满，"Claude Code stops auto-compacting after a few attempts and shows an error instead of looping."
- **`/compact [instructions]`**："Free up context by summarizing the conversation so far. Optionally pass focus instructions"，如 `/compact Focus on code samples and API usage`。全新会话中会打印 `Not enough messages to compact.`。局部压缩用 `/rewind` → **Summarize from here / up to here**。
- **microcompact = 非官方概念**。commands / context-window / model-config / best-practices / permission-modes / interactive-mode / docs changelog / GitHub CHANGELOG **全部零命中**。只有第三方逆向（issue [#42542](https://github.com/anthropics/claude-code/issues/42542)、[ArkNill 分析](https://github.com/ArkNill/claude-code-hidden-problem-analysis/blob/main/05_MICROCOMPACT.md)）指认 `microCompact.ts` / `sessionMemoryCompact.ts` 与 GrowthBook flag（`tengu_slate_heron` / `tengu_session_memory` / `tengu_sm_compact`），效果是把旧工具结果替换为 `[Old tool result content cleared]`；其自测称**主会话命中率仍 99%+**（替换稳定、前缀不变），**代价是上下文质量**，子 agent 冷启动掉到 0–39%。官方最近似的表述只有上面那句 "clears older tool outputs first"。

### 2.2 上下文可视化与缓存可见性

- **`/context [all]`**："Visualize current context usage as a **colored grid**. Shows optimization suggestions for context-heavy tools, memory bloat, and capacity warnings." 它也列出实际加载的 CLI 文件，"including which CLAUDE.md and auto memory files loaded"（"Memory files" 列表是确认文件生效的官方手段）。核销：**官方从未公开 `/context` 的类别清单或百分比口径**。
- 官方上下文页给出代表性启动占用模型（200K 窗口）：System prompt ≈4,200 tokens、Auto memory(`MEMORY.md`) ≈680、Environment ≈280、MCP tools(deferred) ≈120、Skill descriptions ≈450、`~/.claude/CLAUDE.md` ≈320、Project CLAUDE.md ≈1,800——**首字未输入已约 7.8k**。
- `/usage` 输出 **`Prompt cache (main)`** 行（v2.1.251+）：命中率、miss 次数、是否 warm，并给出上次 miss 的推测原因（如 `likely cause: tool definitions changed`，v2.1.260+）。

### 2.3 压缩后什么活下来（官方表）

| 机制 | 压缩后 |
|---|---|
| System prompt / output style | 保留 |
| 项目根 CLAUDE.md + 无 scope 规则 | **从磁盘重新注入** |
| Auto memory | 从磁盘重新注入 |
| Git status 快照 | 重新读取仓库 |
| Plan mode 写的 plan | 从磁盘重新注入 |
| 带 `paths:` 的规则 / 子目录嵌套 CLAUDE.md | 读到匹配文件时重新加载 |
| Claude 读过/改过的文件 | 重读**最多 5 个、最近修改优先**；单文件 >5,000 tokens 只回填路径引用（显示 `Referenced file`） |
| 已调用 skill 正文 | 重新注入，**每 skill 上限 5,000 tokens、合计 25,000**，超预算**最旧的先丢** |
| **Skill 描述清单** | **不重新注入** |
| Hook 先前注入的内容 | 随对话一起摘要 |
| 后台命令 / 后台 subagent | 继续运行，并提醒 Claude 别重复启动 |

### 2.4 subagent 的独立上下文

- subagent 有**自己的 system prompt 与工具集**："Each subagent starts with a fresh, isolated context window. It doesn't see your conversation history, the skills you've already invoked, or the files Claude has already read."
- **回传**："Only the subagent's final text response comes back to your context, plus a small metadata trailer with token counts and duration." 官方模拟例：**subagent 读了 6,100 tokens 文件，主会话只增加 420 tokens**。工程博客给出量级 "often 1,000-2,000 tokens"（该数字在 Claude Code 文档中未记载）。
- 内置 **Explore / Plan** agent 跳过 CLAUDE.md 与 git status；**主会话 auto memory 不注入 subagent**（例外：fork 当前会话者继承父对话与 system prompt）。
- **上限**：同会话 **20 个 subagent** 并发后再派发会报 `Concurrent subagent limit reached`；嵌套默认 **3 层**。subagent transcript 落在 `projects/<project>/<session>/subagents/`。
- **缓存视角**：subagent 首次请求**读不到**父会话 cache（前缀不同），自暖一份；父会话 cache 不受影响。**fork 继承父的 system prompt/tools/history，首次请求即命中父缓存**。workflow fan-out 中同前缀 agent 被**默认最多 hold 5 秒**，让后续 agent 读到第一个 agent 写的前缀。

### 2.5 Task 工具与 plan 文件

- **重要更正：`TodoWrite` 现已默认关闭**。"`TodoWrite` | Manages the session task checklist. **Disabled by default** in favor of `TaskCreate`, `TaskGet`, `TaskList`, and `TaskUpdate`. Set `CLAUDE_CODE_ENABLE_TASKS=0` to re-enable it." 所以 2.x 的任务清单主角是 **Task\* 工具族**。
- 持久性官方表述："**Tasks persist across context compactions**"。核销：**"待办清单被重新读回上下文"查无实据**。
- 工程博客把 "Claude Code creating a to-do list" 列为 **structured note-taking** 范式。
- **plan 文件路径官方未公开**（`.claude/plans/` 等无记载）；可确认的是 plan 在压缩后 "Re-injected from disk"。进入 plan mode：`Shift+Tab` 或单条 prompt 前缀 `/plan`。

### 2.6 `/clear` 与会话续接

- **`/clear [name]`**："Start a new conversation with empty context. Pass a name to label the previous conversation in the `/resume` picker."
- **续接**：`--continue` / `-c`（当前目录最近一次对话，跳过 `-p`/SDK 会话与首条为 `/loop` 的会话）；`--resume`（选择器）；`--resume <name|session-id|transcript-path>`；`/resume`（会话内切换）；`--fork-session` 或 `/branch` 复制出新 session id（原会话不变）。失败文案 `No conversation found with session ID: <session-id>`。`--session-id` 可指定合法 UUID。
- **历史文件路径（精确）**：`~/.claude/projects/<project>/<session-id>.jsonl`，`<project>` = 工作目录路径中**非字母数字字符替换为 `-`**（`/Users/me/proj` → `-Users-me-proj`）；超 200 字符则截断到 200 并追加整路径 hash。
- 行格式：官方只保证"每行是一个 message / tool use / metadata 的 JSON 对象"，并警告 "**The entry format is internal to Claude Code and changes between versions**"。有记载的字段名仅 `parentUuid`、`requestId`。脚本应改用 `/export` 或 `claude -p --output-format json`。
- **保留期**：`cleanupPeriodDays`，**默认 30 天、最小 1**（`0` 校验失败）。auto memory 文件**被排除**在清扫之外。相关开关：`CLAUDE_CONFIG_DIR`、`CLAUDE_CODE_PROJECT_DIR_NAME`（v2.1.234+，须同设 `CLAUDE_CONFIG_DIR`）、`CLAUDE_CODE_SKIP_PROMPT_HISTORY`、`--no-session-persistence`。
- **从摘要续接**：Pro/Max 下续接**闲置 >约 1 小时且 >100,000 tokens** 的会话时弹三选一：**Resume from summary**（立刻 `/compact`）、**Resume full session as-is**、**Don't ask me again**。官方理由：此时 cache 已过期，无论选哪个下一次请求都要整段重算一次。

## 3. Anthropic API 的 Context Management

（[context-editing](https://platform.claude.com/docs/en/build-with-claude/context-editing)、[memory-tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)）

### 3.1 memory tool（`memory_20250818`）

- 配置即整个 tools 条目，`name` 必须为 `memory`，**不需要 beta header**：
  ```json
  { "tools": [{ "type": "memory_20250818", "name": "memory" }] }
  ```
- **完全客户端执行**：Claude 只发 file 操作请求，你的应用在 `/memories` 前缀映射的真实存储上执行并回 `tool_result`。支持全部 Claude 4 及更新模型。
- **须实现 6 命令**：`view`（目录/带行号文件视图，支持 `view_range: [start, -1]`）、`create`、`str_replace`、`insert`、`delete`、`rename`。
- **API 自动注入系统提示**（无需自己发）：
  ```text
  IMPORTANT: ALWAYS VIEW YOUR MEMORY DIRECTORY BEFORE DOING ANYTHING ELSE.
  MEMORY PROTOCOL:
  1. Use the `view` command of your `memory` tool to check for earlier progress.
  2. ... (work on the task) ...
     - As you make progress, record status / progress / thoughts etc in your memory.
  ASSUME INTERRUPTION: Your context window might be reset at any moment, so you risk
  losing any progress that is not recorded in your memory directory.
  ```
- 安全必须自理：**path traversal 防护**（`/memories/../../secrets.env`）、文件大小上限、记忆过期清理。
- 与 compaction 的官方分工："compaction keeps the active context small without client-side bookkeeping, and **memory preserves the information that must survive summarization**."

### 3.2 `clear_tool_uses_20250919`：参数与默认值

beta header：`anthropic-beta: context-management-2025-06-27`。官方配置（精简）：

```json
"context_management": {
  "edits": [{
    "type": "clear_tool_uses_20250919",
    "trigger":        { "type": "input_tokens", "value": 30000 },
    "keep":           { "type": "tool_uses",   "value": 3 },
    "clear_at_least": { "type": "input_tokens", "value": 5000 },
    "exclude_tools": ["web_search"]
  }]
}
```

| 参数 | 默认值 | 含义 |
|---|---|---|
| `trigger` | **100,000 input tokens** | 超阈值开始清理；可写 `input_tokens` 或 `tool_uses` |
| `keep` | **3 tool uses** | 清理后保留最近几组 tool use/result；**从最旧开始清** |
| `clear_at_least` | 无 | 保证每次至少清掉多少 token，**达不到就不执行本次清理** |
| `exclude_tools` | 无 | 永不清理的工具名单 |
| `clear_tool_inputs` | `false` | 是否连 tool_use 参数一起清（默认只清结果、保留调用记录） |

另有 **`clear_thinking_20251015`**（thinking block 清理），`keep` 默认按模型类：

| 模型类 | 保留全部历史 thinking | 只保留最后一轮 |
|---|---|---|
| Opus | 4.5 及以后 | 4.1 及以前 |
| Sonnet | 4.6 及以后 | 4.5 及以前 |
| Haiku | （无） | 直至 4.5 全部 |
| Fable / Mythos | 全部模型 | （无） |

**定位变化**：官方现在写 "For most use cases, **server-side compaction is the primary strategy**"，推荐用 `context_management` 里的 **`compact_20260112`**；SDK 侧 `compaction_control` **已废弃**（Python SDK v1.0 移除）。context editing 只留给"需要更细粒度控制"的场景。执行位置："applied **server-side** before the prompt reaches Claude"，客户端保留完整历史、无需同步。

### 3.3 与 prompt caching 的相互作用 —— 官方明说

> **Tool result clearing**："**Invalidates cached prompt prefixes when content is cleared.** To account for this, clear enough tokens to make the cache invalidation worthwhile. Use the `clear_at_least` parameter to ensure a minimum number of tokens is cleared each time. **You'll incur cache write costs each time content is cleared**, but subsequent requests can reuse the newly cached prefix."

> **Thinking block clearing**："When thinking blocks are **kept** in context, **the prompt cache is preserved** ... When thinking blocks are **cleared**, the cache is invalidated at the point where clearing occurs. Configure the `keep` parameter based on whether you want to **prioritize cache performance or context window availability**."

**【推断】权衡模型**：失效代价 ≈ 被清前缀的 **1.25x 重写**；收益 ≈ 被清 token 在之后**每次请求**省下的 1x（原本按 uncached 计）。故决策近似于"清理量 × 剩余请求数 vs 清理量 × (1.25 − 0.1)"——这正是 `clear_at_least` 的工程意义：把"这次清理值不值得破缓存"显式化。**短会话很可能净亏**；真正划算是长 agentic 循环 + 大体积工具结果，即官方说的 "best for agentic workflows with heavy tool use"。

## 4. Prompt Caching 细节

（[prompt-caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)）

### 4.1 两种启用方式与断点数量

- **Automatic caching**：请求体**顶层**一个 `cache_control`，系统把断点打在"最后一个可缓存 block"并随对话推进前移。
- **Explicit breakpoints**：`cache_control` 打在单个 content block 上。
- **最多 4 个断点**；混用时 automatic **占用 4 槽之一**；若已有 4 个显式断点，带顶层 `cache_control` 返回 **400**。末 block 带**相同 TTL** 的显式断点时 automatic 是 no-op，**TTL 不同则 400**。末 block 不适合作断点时系统**静默向前回退**，找不到就跳过缓存。

### 4.2 TTL 与价格

- 默认 **5 分钟**，命中即免费刷新。**TTL 从该次请求的"开始"计时，不是响应结束**——官方例：响应流式输出 4 分钟，后续请求须在响应结束后约 1 分钟内发出。
- 1 小时：`{ "cache_control": { "type": "ephemeral", "ttl": "1h" } }`（Claude API / Bedrock / Claude Platform on AWS / Google Cloud / Microsoft Foundry）。
- **乘数（官方）**：5 分钟写入 **1.25×** base input；1 小时写入 **2×**；**读取 0.1×**（例外：Fable 5.1 与 Mythos 5.1 为 **0.025×**，Opus 5.5 为 **0.05×**）。**断点本身不收费**。
- 数字示例（$/MTok，base → 5m write / 1h write / hit）：Opus 4.5 `5 → 6.25 / 10 / 0.50`；Sonnet 4.5 `3 → 3.75 / 6 / 0.30`；Haiku 4.5 `1 → 1.25 / 2 / 0.10`。响应 `cache_creation` 拆出 `ephemeral_5m_input_tokens` 与 `ephemeral_1h_input_tokens`，其和等于 `cache_creation_input_tokens`。

### 4.3 最小可缓存 token 数（分模型）

| 最小值 | 模型 |
|---|---|
| **512** | Fable 5.1、Mythos 5.1、Opus 5.5、Opus 5、Fable 5、Mythos 5 |
| **1,024** | Opus 4.8、Sonnet 5、Sonnet 4.6、Sonnet 4.5、Opus 4.1、Opus 4、Sonnet 4 |
| **2,048** | Opus 4.7、Mythos Preview、Haiku 3.5 |
| **4,096** | Opus 4.6、Opus 4.5、**Haiku 4.5** |

低于阈值**静默不缓存、不报错**（判断：`cache_creation_input_tokens` 与 `cache_read_input_tokens` 是否都为 0）。官方建议："If your prompt falls just short of the minimum ... **expanding the cached content to reach the threshold is often worthwhile**."

### 4.4 前缀匹配与 20-block 回溯

- **官方原文**："Prompt caching references the entire prompt: **`tools`, `system`, and `messages` (in that order)**, up to and including the block designated with `cache_control`." 这是层级结构，**任一层改动失效该层及其之后所有层**。
- 三条机制：① **写入只发生在断点处**——一个断点只写一条 entry，hash 覆盖到该 block 为止的全部前缀（累积 hash），故断点处或之前的任何改动都改变 hash。② **读取向后回溯**——先在断点算 hash 找 entry，找不到就**一次一个 block 往前找"先前请求写过的 entry"**；它找的是已写过的 entry，**不是"稳定内容"**。③ **回溯窗口 = 20 个 block**（含断点自身算第一个）；窗口内找不到就停止，或从下一个显式断点重新开始。**在 Claude API 上，连续的 `tool_use` block 串算 1 个位置，连续 `tool_result` 串也算 1 个位置**——故一轮大量并行工具调用不会把上次 entry 挤出窗口。
- **官方经典陷阱**：把断点打在"每次请求都变"的 block（带时间戳的 per-request block）上 → 每次都全量 write、永远没有 read。**回溯不会发现断点之前的稳定内容并替你缓存它。** 正确做法是把断点放在静态前缀末尾。只要每轮新增 <20 个 block，打在最后一块就够；增长极快的会话应**从早期就多加一个靠近上次写入点的断点**，让写入提前积累。

### 4.5 失效规则、隔离与 miss 门槛

| 改动 | tools | system | messages | 说明 |
|---|---|---|---|---|
| Tool 定义（名称/描述/参数） | ✘ | ✘ | ✘ | **整条缓存全失效** |
| web search / citations 开关、speed 设置 | ✓ | ✘ | ✘ | 改的是 system prompt |
| `tool_choice` | ✓ | ✓ | ✘ | 只影响 message block |
| 图片增删（任意位置） | ✓ | ✓ | ✘ | |
| thinking 配置 / `output_config.effort` | 视模型 | 视模型 | ✘ | 配置被渲染进 prompt；显式设为模型默认值等价于省略、不失效 |

- **隔离与匹配**：缓存按 **organization** 隔离，在 Claude API / Claude Platform on AWS / Microsoft Foundry 上**还按 workspace 隔离**（Bedrock 与 Google Cloud 只到 organization 级）；**命中要求 100% 完全一致**。
- **自动压缩一定失效缓存（官方明说）**：Claude Code 把 "Compacting the conversation" 列入 invalidating actions——"**By design, this invalidates the conversation layer**, since the next request has a new, shorter history that doesn't share a prefix with the old one." 关键省钱细节：产生摘要的那次请求**复用同一 system prompt、tools 与 history**，只在末尾追加摘要指令，所以**缓存还热时 `/compact` 读的是缓存前缀，成本远低于上下文规模所暗示的量**；冷启动（闲置超 TTL）压缩才按未缓存全价重算——这就是官方解释"为何 resume 老会话时压缩最贵"。
- **Claude Code 对 miss 的操作性定义**："counts a request as a **miss when the request re-processed more than 5% and at least 2,000 tokens** of what it could have read from cache."——小规模前缀扰动不计 miss，这给了"哪些改动值得在意"的量化门槛。
- **并发**：一条 entry **只在第一个响应开始后才可用**；要并行命中，必须先等第一个响应。
- **CLI 侧致命项**：`tool_use` block 的 JSON key 顺序若不稳定（Swift/Go 的 map 序列化会随机化）会直接打破缓存。
- **Claude Code 的 TTL 分桶**：主对话（交互、`-p`、SDK）在订阅额度内请求 **1h**，其余（subagent、workflow、teammate、fork、compaction、会话标题）**5m**；超额或 API key/云厂商一律 **5m**。控制项：`promptCacheTtl` / `CLAUDE_CODE_PROMPT_CACHE_TTL`、`subagentPromptCacheTtl` / `CLAUDE_CODE_SUBAGENT_PROMPT_CACHE_TTL`（v2.1.242+）、`ENABLE_PROMPT_CACHING_1H`、`FORCE_PROMPT_CACHING_5M`、`DISABLE_PROMPT_CACHING{,_HAIKU,_SONNET,_OPUS,_FABLE}`。

## 5. 官方"上下文工程 / 三件套"原始表述

### 5.1 [Effective context engineering for AI agents](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)（2025-09-29）

- 定义："the set of strategies for curating and maintaining the optimal set of tokens (information) during LLM inference"；目标 "find the **smallest possible set of high-signal tokens**"。
- 依据：context rot、**attention budget**、transformer **n² pairwise**、位置编码插值退化 → "**a performance gradient rather than a hard cliff**"。
- **Compaction**："summarizing its contents, and reinitiating a new context window with the summary ... The model preserves **architectural decisions, unresolved bugs, and implementation details** while discarding redundant tool outputs or messages. The agent can then continue with this compressed context **plus the five most recently accessed files**."
- **Structured note-taking**："the agent regularly writes notes persisted to memory outside of the context window"；Pokémon 例："For the last 1,234 steps I've been training my Pokémon in Route 1, Pikachu has gained 8 levels toward the target of 10."
- **Sub-agent**："Each subagent might explore extensively, using tens of thousands of tokens or more, but **returns only a condensed, distilled summary of its work (often 1,000-2,000 tokens)**."
- **点名 Claude Code 的混合策略**："**CLAUDE.md files are naively dropped into context up front**, while primitives like glob and grep allow it to navigate its environment and retrieve files just-in-time."
- 工具结果清理是"最安全的一级"："One of the safest lightest touch forms of compaction is **tool result clearing**."
- 三者分工：Compaction 保对话流；**Note-taking 适合有明确里程碑的迭代开发**；多 agent 适合并行探索的研究分析。

### 5.2 [Prompt caching is everything](https://claude.dev/blog/lessons-from-building-claude-code-prompt-caching-is-everything/)（2026-04-30）

- **"At Claude Code, we build our entire harness around prompt caching."** 及 **"we run alerts on our prompt cache hit rate and declare SEVs if they're too low."**
- 官方四层布局（静态在前）：① 静态 system prompt & tools（**全局缓存**）→ ② CLAUDE.md（**按项目缓存**）→ ③ session context（env / MCP / output style，**按会话缓存**）→ ④ conversation messages。
- **官方承认曾打破该顺序**："putting an in-depth timestamp in the static system prompt, shuffling tool order definitions non-deterministically, and updating parameters of tools."
- **Cache-safe forking（压缩）**：用**与父会话完全相同的 system prompt、user context、system context 和 tool definitions**，前置父会话消息，再把压缩 prompt 作为**新的最后一条 user message** 追加，则整段前缀完全相同、只需为压缩 prompt 付新 token；代价是**必须预留 "compaction buffer"**。反例（官方点名的成本陷阱）：用独立 system prompt、不带 tools 的 "summarize this" 调用，前缀从**第一个 token** 就分叉，**整段对话按未缓存全价计费**——对话越长（越需要压缩）这一发越贵。
- 五条 lessons：① 前缀匹配，围绕它设计整个系统；② 用 message 而非改 system prompt；③ 不中途换工具或模型（**用工具建模状态转换，用 `defer_loading` 延迟加载而非删除工具**）；④ **像监控 uptime 一样监控 cache 命中率**；⑤ fork 类操作必须共享父前缀。

### 5.3 [Best practices for Claude Code](https://code.claude.com/docs/en/best-practices)

全篇前提："Most best practices are based on one constraint: **Claude's context window fills up fast, and performance degrades as it fills.**" 记忆相关：逐行自问 "Would removing this cause Claude to make mistakes?"；"**Bloated CLAUDE.md files cause Claude to ignore your actual instructions!**"；跨任务 `/clear`；**同一问题纠正超过两次就 `/clear` 重开**；不必留上下文的问题用 `/btw`。

## 6. Claude Code 2.x 的 memory 相关能力

### 6.1 Auto memory（社区常称 "session memory"，官方名为 auto memory）

- **默认开启**。Claude 自写四类笔记，记在文件 frontmatter 的 `type`：`user`（角色/偏好）、`feedback`（你的纠正与被确认做法）、`project`（代码里看不出的进行中工作与决策）、`reference`（外部信息源）。它**跳过能从代码库推导的内容**，也跳过 CLAUDE.md 已写的。
- **存储**：`~/.claude/projects/<project>/memory/`，含 `MEMORY.md` 索引 + 每主题一文件；`<project>` 由 git 仓库推导，**同仓库所有 worktree 与子目录共用同一个 auto memory 目录**；非 git 仓库用项目根。
- **注入**："The **first 200 lines of `MEMORY.md`, or the first 25KB**, whichever comes first, are loaded at the start of every conversation." 超出部分不加载；主题文件**不在启动时加载**，Claude 按需用普通文件工具读。写入后接近/超出限制时会被提醒压缩索引，超限写入仍成功但返回错误要求重写索引。
- **机器本地**，不跨机器/云环境共享。开关与路径：`autoMemoryEnabled`、`CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`、`autoMemoryDirectory`。UI 会显示 **"Saved 2 memories" / "Recalled 2 memories"**；带 frontmatter 的文件写入时记录 `modified` ISO 8601 时间戳（v2.1.214+）。
- **subagent 记忆**：主会话 auto memory **不注入 subagent**；subagent 用 frontmatter 的 `memory` 开启独立目录——`user` → `~/.claude/agent-memory/<name>/`，`project` → `.claude/agent-memory/<name>/`，`local` → `.claude/agent-memory-local/<name>/`；开启后 **Read/Write/Edit 自动启用**。
- **重要更正：Claude Code 内部并没有名为 `memory` 的工具。** 官方 tools-reference 完整清单（Agent、Artifact、AskUserQuestion、Bash、Cron\*、Edit、Read、Skill、Task\*、WebFetch、WebSearch、Workflow、Write…）中**无 Memory 条目**——auto memory 是**纯 Markdown + 标准 Read/Write/Edit** 实现的。`memory_20250818` 是 **API 侧工具**（见 3.1），两者不同层。changelog 唯一的实现痕迹是一条修复："Fixed a stray `</ccmemory>`-style closing tag occasionally appearing in responses"。

### 6.2 Skills 与渐进披露

- 位置与优先级：Enterprise > Personal（`~/.claude/skills/`）> Project（`.claude/skills/`）；同名时**企业 > 个人 > 项目**。`.claude/commands/*.md` 已并入 skills 体系。
- **渐进披露**："a skill's body loads only when it's used"；默认 **description 常驻、正文调用时加载**。

  | frontmatter | 你 | Claude | 何时进上下文 |
  |---|---|---|---|
  | （默认） | 可 | 可 | description 常驻，正文调用时加载 |
  | `disable-model-invocation: true` | 可 | 否 | **description 也不进**，纯手动 `/name` |
  | `user-invocable: false` | 否 | 可 | description 常驻，正文调用时加载 |

- 调用后正文作为**单条 message 留在对话中**；正文相同只追加"已加载"提示；参数或动态注入结果变化则重新追加全文。
- **与压缩的交互**：压缩时对每个 skill 的**最近一次调用**取**前 5,000 tokens** 重附加，合计 **25,000 tokens** 预算，**从最近调用的开始填**，故旧的 skill 可能被整体丢弃——官方因此要求把最重要的指令写在 `SKILL.md` **靠前位置**。
- 动态注入：`` !`git diff HEAD` `` 在内容送模型**之前**执行并内联（`disableSkillShellExecution: true` 可禁）。
- 相邻能力：`skillOverrides`（`on`/`name-only`/`user-invocable-only`/`off`）、claude.ai 账号同步到 `~/.claude/skills/synced/`（约每 10 分钟检查，v2.1.273+）、`AGENTS.md` 原生读取（v2.1.277+，由 `/config` 的 **Project instructions** 在 `claude-md-or-agents-md` / `claude-md-and-agents-md` / `claude-md` / `managed-only` 间切换）。

## 7. "哪些设计是为了命中缓存"：官方明说 vs 我的推断

**官方明说**

| 设计 | 官方表述 |
|---|---|
| 四层 prompt 布局 | "**static content first, dynamic content last**"，目的是 "maximize how many sessions share cache hits" |
| `<system-reminder>` 追加而非改 system prompt | "It may be tempting to update the prompt, but that would result in a **cache miss** ... we add a `<system-reminder>` tag in the next user message or tool result" |
| Plan mode **保留全部工具**，用 `EnterPlanMode`/`ExitPlanMode` **工具本身**做状态转换 | "The intuitive approach would be ... swap out the tool set to only include read-only tools, **but that would break the cache**. Instead, we keep all tools in the request at all times" |
| MCP 工具用 `defer_loading: true` 发 **stub** 而非删除 | "removing them mid-conversation would break the cache ... keeps the cached prefix stable because the same stubs are always present in the same order" |
| 压缩用 **cache-safe forking** | 见 5.2；官方 API 亦据此内置 compaction |
| 换模型用 subagent 而非 `/model` | "it would actually be more expensive to switch to Haiku than to have Opus answer, because we would need to rebuild the prompt cache for Haiku" |
| CLAUDE.md 中途编辑**不生效** | 官方归入 "Actions that keep the cache"：不破缓存，但也不生效，下次 `/clear`/`/compact`/重启才加载 |
| 输出风格切换、权限模式切换、skill 调用、`/recap`、`/rewind` | 官方明确列入 "Actions that keep the cache"——都**以 message 形式 append**，不改前缀 |
| Workflow fan-out 默认 hold 5 秒 | 让后续同前缀 agent 读到第一个 agent 写下的前缀 |

**我的推断**

- **【推断】** auto memory 的 200 行/25KB 上限 + `modified` 时间戳，双重目的是**把"会话开头那段前缀"的体积固定住**，使其尽可能稳定、跨会话可共享缓存。记忆若无上限增长，启动前缀天天变，跨会话命中率会被系统性侵蚀。
- **【推断】** auto memory 放在 `~/.claude/projects/<project>/memory/`（仓库级、共享 worktree）而非仓库内，是为**不把易变内容混进可提交、跨机器共享的 CLAUDE.md**，避免同团队不同机器上前缀不同而互不命中。（但官方明说 cache scope 实际已按"机器 + 目录"隔离，故此条主因更可能是隔离性与隐私。）
- **【推断】** `clear_at_least` 的动机被官方话术直接支持（"helps determine if context clearing is worth breaking your prompt cache"），但**把决策写成"剩余请求数 × 清理量 > 清理量 × (1.25 − 0.1)"是我的建模**，官方未给公式。
- **【推断】** 20-block 回溯把"连续 `tool_use` 串算 1 个位置"，是为了**奖励高并行工具调用**的 agent 形态：否则一次并行 10 个工具调用就会吃掉一半回溯预算。
- **【推断】** 压缩后"重读最多 5 个最近修改文件、单文件 >5,000 tokens 退化为路径引用"，是为**避免压缩后立刻二次膨胀**；5 与工程博客 "plus the five most recently accessed files" 一致，属长期调优的经验值。

## 8. 参考来源

**Claude Code 官方文档**：[memory](https://code.claude.com/docs/en/memory) · [context-window](https://code.claude.com/docs/en/context-window) · [sessions](https://code.claude.com/docs/en/sessions) · [prompt-caching](https://code.claude.com/docs/en/prompt-caching) · [model-config](https://code.claude.com/docs/en/model-config) · [best-practices](https://code.claude.com/docs/en/best-practices) · [skills](https://code.claude.com/docs/en/skills) · [sub-agents](https://code.claude.com/docs/en/sub-agents) · [tools-reference](https://code.claude.com/docs/en/tools-reference) · [how-claude-code-works](https://code.claude.com/docs/en/how-claude-code-works) · [costs](https://code.claude.com/docs/en/costs) · [cli-reference](https://code.claude.com/docs/en/cli-reference)

**Claude Developer Platform**：[prompt-caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) · [context-editing](https://platform.claude.com/docs/en/build-with-claude/context-editing) · [memory-tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool) · [compaction](https://platform.claude.com/docs/en/build-with-claude/compaction) · [tool-use-with-prompt-caching](https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-use-with-prompt-caching)

**工程博客 / 发布**：[Effective context engineering for AI agents](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)（2025-09-29）· [Lessons from building Claude Code: Prompt caching is everything](https://claude.dev/blog/lessons-from-building-claude-code-prompt-caching-is-everything/)（2026-04-30）· [Managing context on the Claude Developer Platform](https://claude.com/blog/context-management)

**第三方（仅交叉验证）**：[microcompact 逆向分析](https://github.com/ArkNill/claude-code-hidden-problem-analysis/blob/main/05_MICROCOMPACT.md) · [#42542](https://github.com/anthropics/claude-code/issues/42542) · [`#` 快捷记忆现状核实](https://dev.to/aicoding-guide/does-add-to-claudemd-in-claude-code-what-the-current-docs-say-5325)
