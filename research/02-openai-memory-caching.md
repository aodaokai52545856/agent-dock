# OpenAI 生态的记忆系统与 Prompt Caching 调研

> 调研范围：ChatGPT 产品记忆、Responses API 会话状态、Agents SDK Session、Codex CLI、Prompt Caching，并与 Anthropic 缓存机制对比。
> 标注约定：**【官方】**＝官方文档/官方 Cookbook 原文明确说明；**【推断】**＝由官方机制推导或来自第三方逆向/社区观测，未经 OpenAI 明说。

---

## 1. ChatGPT 的记忆体系

### 1.1 两套机制：saved memories vs. reference chat history

| 维度 | Saved memories（显式记忆条目） | Reference chat history（引用历史对话） |
| --- | --- | --- |
| 本质 | 结构化事实条目，带时间戳逐条存储 | 从历史对话中蒸馏出的用户画像/最近对话摘要 |
| 用户可见可编辑 | 可查看、逐条删除、可让模型"忘掉" | 官方 FAQ 明说用户无法直接查看或编辑 |
| 生效方式 | 作为 `bio` 工具（工具名也叫 `Model Set Context`）由模型显式写入 | 后台周期性生成，写入系统提示的多个分段 |
| 存储上限 | 条目数有限 | 官方称"开启后没有存储上限" |

**【官方】** 帮助中心《How does "Reference saved memories" work?》《Memory FAQ》定义了这两个开关，并说明两者可独立开关。**【推断】** 二者是独立管线：saved memories 是"写入—注入"的显式事实表；reference chat history 是"离线摘要—注入"的画像层。

### 1.2 注入方式：官方未明说，逆向证据指向 system 层注入

**【官方】** OpenAI 从未公开说明记忆是 system 层注入还是 RAG 检索注入。

**【推断/第三方逆向】** Michael Bargury 的逆向分析（2025-05）显示，o3 的系统提示在 `Model Set Context` 之后依次存在：`ASSISTANT RESPONSE PREFERENCES`（约 15 条，带 `Confidence=high`）、`NOTABLE PAST CONVERSATION TOPICS HIGHLIGHTS`、`HELPFUL USER INSIGHTS`、`RECENT CONVERSATION CONTENT`（约 38–40 条最近会话，含时间戳、标题、用户消息原文，用 `||||` 分隔，不含助手回复）、`USER INTERACTION METADATA`（17 项，含模型分布、账号年龄、时区、窗口尺寸等）。作者明确做了 3 次针对性检索测试，ChatGPT **无法**召回一年前的特定历史对话，因此认为这不是向量检索 RAG，而是"近期历史 + 长期画像摘要"的**系统提示静态拼接**。

结论：**【推断】** ChatGPT 记忆＝system 层注入（前缀拼接），而非按需检索注入。这也解释了它为何与 Prompt Caching 强耦合（见 §5）。

### 1.3 管理、Projects 与 Temporary Chat

- **【官方】** 管理入口为 **Settings > Personalization**；可逐条删除 saved memory，也可整体清空。Codex 本地侧另有 `/memories` 命令与 `[features] memories = true` 开关（**本地 Codex 记忆是独立存储**，位于 `~/.codex/memories/`，默认关闭）。
- **【官方】** Projects：项目的 chats 共享同一套 **project instructions + Sources（上传文件/连接源）**。项目可切换为"仅项目记忆"（project-only memory），此时项目内信息不会在项目外被使用；共享项目会自动设为仅项目记忆。
- **【官方】** Temporary Chat（临时对话）不写入记忆、不出现在历史中、不用于训练。

### 1.4 2025 年的关键演进：从"保存事实"到"引用全部历史"

- 2024 年：只有 saved memories，模型通过 `bio` 工具保存零散事实。
- **2025-04**：OpenAI 上线 reference chat history，"ChatGPT can now remember and reference all your previous chats"（Ars Technica 2025-04-10 报道；官方 FAQ 同步更新）。
- **2025-05**：逆向研究确认其代价是系统提示中新增 5 个画像分段，且**不可查看、不可删除**（第三方作者的隐私批评）。
- **【官方】** 现行文档进一步引入 Computer History（把本机活动转成记忆与时间线）、Codex 本地记忆存储与 `memories.*` 细粒度配置（`use_memories` / `generate_memories` / `disable_on_external_context` / `min_rate_limit_remaining_percent`）。

---

## 2. Responses API 的会话状态

### 2.1 三种状态管理方式

| 方式 | 机制 | 官方要点 |
| --- | --- | --- |
| `store=true` + `previous_response_id` | 服务端按链保存 response，下轮只传新输入 | **【官方】** Response 默认保存 **30 天**；`store=false` 可关闭。Conversation 对象及其 items **不受 30 天 TTL 约束** |
| Conversations API（`conversation` 参数） | 持久 `conv_xxx` 对象，items 会前置于 `input_items`，请求结束后输入/输出自动写回 | **【官方】** 可跨设备、跨 job 复用 |
| 手动 `input` 数组 | 自己保存 `response.output`（含 reasoning items 与 `phase`） | **【官方】** 无状态时必须保留每个 output item（含加密 reasoning） |

**【官方】** 关键计费提醒：**即使使用 `previous_response_id`，链上所有历史输入 token 仍按 input token 计费**——服务端保存状态≠免费上下文。

### 2.2 `truncation` 参数：只有 `auto` 与 `disabled`

**【官方】（SDK/API 参考原文）**
- `auto`：若本次与历史上下文超出模型上下文窗口，模型会**丢弃对话中间的 input items** 以适配窗口。
- `disabled`（**默认**）：超窗时请求直接以 **400 错误**失败。

**【官方】** Response 对象里可见 `"truncation": "disabled"` 字段。

**【官方+社区】** Assistants API 时代的 `truncation_strategy: {type: "last_messages", last_messages: N}` **在 Responses API 中没有对应物**；开发者社区（2025-10）确认 `responses` 仅支持 `auto`/`disabled`，想按条数截断只能自行管理 `input`。这是 Assistants → Responses 迁移中的一处能力回退。

### 2.3 `instructions` 与 `input` 的区别

**【官方】** `instructions` 是顶层系统级引导（等价于 system/developer 层），`input` 是 string 或 items 数组。迁移文档明确：**`previous_response_id` 不会携带上一轮的顶层 `instructions`**，多轮中必须每轮重发稳定 instructions——否则指令会"掉线"。

### 2.4 上下文窗口管理与 400 错误

**【官方】** 上下文窗口＝input + output（部分模型还含 reasoning tokens），超出会被截断或报错。官方现推荐用 **Compaction** 而非裸截断：
- 服务端压缩：`context_management: [{type: "compaction", compact_threshold: 200000}]`，越阈值时服务端自动压缩并在同一流里发出 opaque 的 compaction item；
- 独立端点 `POST /responses/compact`，完全无状态、ZDR 友好；**其输出不可再裁剪**，必须原样作为下一轮 input。

---

## 3. OpenAI Agents SDK

- **【官方】** `Session` 抽象：run 前自动 `session.get_items()` 并把历史前置到输入，run 后自动把新 items 写回，**免去手动 `to_input_list()`**。内置实现含 `SQLiteSession`、`AsyncSQLiteSession`、`SQLAlchemySession`、`RedisSession`、`DaprSession`、`MongoDBSession`、`EncryptedSession`、`AdvancedSQLiteSession`。
- **【官方】** 同一 run 中 Session 不能与 `conversation_id` / `previous_response_id` / `auto_previous_response_id` 混用——客户端记忆与服务端记忆二选一。
- **【官方】** `RunConfig.session_input_callback(history, new_input)` 自定义合并（可做剪枝/重排）；`SessionSettings(limit=N)` 只取最近 N 条；更靠后的最后一道闸是 `call_model_input_filter`。
- **【官方】** `RunContextWrapper[T]` 是**本地上下文**（`wrapper.context` / `.usage` / `.tool_input` / `.approve_tool()`），**不会发送给 LLM**；要影响模型必须写入 `instructions`、`input` 或工具返回值。会话状态是另一回事（`to_input_list()` / `session` / `conversation_id` / `previous_response_id` 四选一）。
- **【官方】** Guardrails：**input guardrails 只作用于链上第一个 agent，output guardrails 只作用于产出最终输出的 agent**；tool guardrails 才逐次函数调用生效。handoff 不触发 tool input guardrails。
- **【官方】** Handoff 历史传递：**默认接收方看到完整先前对话**。可用 `input_filter`（接收 `HandoffInputData`，含 `input_history` / `pre_handoff_items` / `new_items` / `input_items`）改写；`RunConfig.nest_handoff_history`（默认关闭的 beta）会把历史压成 `<CONVERSATION HISTORY>` 摘要段。**服务端会话（`conversation_id` / `previous_response_id`）不支持 handoff input filter**。
- **【官方】** `result.to_input_list()` 用于手动聊天循环；`mode="normalized"` 给出 handoff 改写后的规范下一轮输入；`new_items` 提供带 agent/tool/handoff/approval 元数据的富视图。

---

## 4. Codex CLI / Codex 云端

### 4.1 AGENTS.md 的层级与合并规则（**【官方】**）

发现顺序（每次 run 构建一次指令链）：
1. **全局**：`$CODEX_HOME`（默认 `~/.codex`）下优先 `AGENTS.override.md`，否则 `AGENTS.md`，**只取第一个非空文件**。
2. **项目**：从项目根（通常 Git root）逐级下走到 cwd，每级依次找 `AGENTS.override.md` → `AGENTS.md` → `project_doc_fallback_filenames` 中的名字，**每级最多一个文件**。
3. **合并**：从根向下用空行拼接；**越靠近 cwd 的文件越靠后、优先级越高**（覆盖而非合并字段）。
4. **截断**：跳过空文件，累计大小达到 `project_doc_max_bytes`（**默认 32 KiB**）即停止追加。

相关配置：`project_doc_max_bytes`、`project_doc_fallback_filenames`、`project_root_markers`、`CODEX_HOME`。CLI 可用 `codex debug prompt-input` 打印模型可见的 prompt input JSON 来核对指令发现结果。

### 4.2 `codex.md` 的历史

**【官方】** 现行官方文档**只**以 `AGENTS.md` + fallback 列表描述指令文件，未再提 `codex.md`。
**【推断/社区】** `codex.md` 是 Codex CLI 早期的项目说明文件名，后统一到跨厂商的 `AGENTS.md` 约定（agents.md）；旧仓库可用 `project_doc_fallback_filenames = ["codex.md"]` 兼容。此项我未能取得官方迁移声明原文，请按社区共识对待。

### 4.3 自动 compaction 与 `/compact`

**【官方】** `config.toml`：
- `model_auto_compact_token_limit`：触发自动历史压缩的 token 阈值（未设置则用模型默认值）；
- `model_auto_compact_token_limit_scope`：`total`（默认，按完整活跃上下文计数）或 `body_after_prefix`（只算压缩窗口前缀之后的增量）；
- `compact_prompt` / `experimental_compact_prompt_file`：覆盖压缩提示；
- 生命周期钩子 `PreCompact` / `PostCompact` 可在压缩前后执行脚本。

**【官方】** `/compact` 手动触发："Codex replaces earlier turns with a concise summary, freeing context while keeping critical details." IDE 扩展同样有 `/compact`。**【推断】** 自动压缩的默认阈值偏保守：社区 issue #40095 指出默认使用原始窗口计算，导致要等到可用上下文约 94.7% 才触发，长任务中容易先撞上 400 或质量下滑。

### 4.4 会话 resume 与 "context window left"

**【官方】** `codex resume`（`--last` 默认限当前目录，`--all` 跨全部）、TUI 内 `/resume`、`/fork`（克隆为新 chat）、`codex fork`；`tui.resume_cwd = "current" | "session"` 决定 cwd 不一致时的默认选择。会话可 `/archive`、`codex unarchive`、`/delete`。

**【官方】** 剩余上下文提示：`/status` 显示 "session configuration and token usage … remaining context capacity"；`/statusline` 可把 `context`（context stats）与 token 计数放入 TUI footer；IDE 的 `/status` 显示 "chat ID, context usage, and rate limits"。

---

## 5. Prompt Caching 细节

### 5.1 机制（**【官方】** Prompt caching 指南 + Prompt Caching 201 Cookbook）

- **自动开启**，无需显式声明；对 gpt-4o 及更新模型生效，无额外费用。
- **最小可缓存前缀 1024 token**（GPT-5.6+ 明确为 1,024 visible input tokens；此前模型随 tools/images/schema/reasoning effort/verbosity 变化）。隐藏系统内容不计入该最小值。
- **命中要求前缀完全一致**（entire rendered prefix must match）：`model`、`tools`（名称/描述/schema/**顺序**）、`parallel_tool_calls`、`text.format`、`reasoning.effort`、`text.verbosity`、`context_management` 变更都会破坏其后缀的匹配。
- **精确度**：更早模型的 `cached_tokens` = 最后一个匹配断点减去隐藏 system token 后**向下取整到 128 的倍数**（即 128 token 增量的来源）；GPT-5.6+ 按**精确断点**报告，不再取整。
- **缓存范围**：整个渲染后的上下文——OpenAI 提供的指令、developer messages、**tool definitions**、对话历史（文本/图像/文档/受支持音频）全部可缓存。缓存存的是 **KV 张量**，不存原文。

### 5.2 生命周期与价格

| 项 | GPT-5.6+ | 更早模型 |
| --- | --- | --- |
| 生命周期控制 | `prompt_cache_options.ttl`，唯一值 `30m`（也是默认），到期前复用会刷新 | `prompt_cache_retention`：`in_memory`（**约 5–10 分钟不活跃**，最多 1 小时）或 `24h`（通常约 30 分钟，最多 24 小时） |
| 默认 | 30m | 非 ZDR 组织默认 `24h`；**ZDR 组织默认 `in_memory`** |
| 缓存读取 | **0.1×** 未缓存输入价 | **按模型不同**：gpt-4o 0.5×、gpt-4.1 0.25×、gpt-5-nano / gpt-5.2 0.1×、gpt-realtime(audio) 0.0125× |
| 缓存写入 | **1.25×**（新增费用项） | 无额外写入费用 |

**【官方】** Cookbook 原文：Prompt Caching 可把 **TTFT 延迟降低最多 80%**、**输入 token 成本降低最多 90%**（"reduce time-to-first-token latency by up to 80% and input token costs by up to 90%"）。实测：1024 token 提示仅快 7%，150k+ token 时 TTFT 快 **67%**。注意：用户提到的"75% 成本下降"实为 gpt-4.1 的缓存折扣，非通用上限。

### 5.3 `prompt_cache_key` 与路由亲和性

**【官方】** 缓存存在于**单机 GPU-local 存储**；每台机器对"同一前缀 + 同一 key"约承受 **~15 RPM**，超出会溢出到其他机器并造成一次性 miss。路由依据是隐藏内容之后**前 ~256 token 的哈希**（含 tools），`prompt_cache_key` 与该哈希组合以提升粘性——但它**只影响路由，不保证命中**。一个编码类客户命中率从 60% 提升到 87%。**GPT-5.6+ 上 key 不再用于优化路由**，仅用于"按客户/用户隔离缓存计费"（同时可防 cache-hit probing）。缓存不跨组织、不跨区域处理边界。

### 5.4 GPT-5.6 引入的显式断点

**【官方】** `prompt_cache_options.mode: "explicit"` + 在内容块上打 `prompt_cache_breakpoint: {mode: "explicit"}`；每请求最多 **4 个 cache write**；顶层 `instructions` **不能**打断点（要缓存的 developer 指令必须放进 `input_text` 块）。隐式模式（`implicit`）会在最新 eligible message 处自动打断点。另有 `prompt_cache_options.prewarm: true` 预热缓存。

### 5.5 与 compaction 的天然冲突

**【官方】** 官方明说："compaction replaces earlier conversation context with a shorter representation. That can change the prefix, so the first request after compaction may reuse less of the previous cache"，并给出原则："context engineering and prompt caching are inherently at odds — one prioritizes dynamism, the other stability." Codex 的做法是保持 system instructions、tool definitions、sandbox 配置与环境上下文**完全一致且顺序稳定**，运行期配置变更时**追加新消息而非改写旧消息**。

---

## 6. 与 Anthropic Prompt Caching 的关键差异

| 维度 | OpenAI | Anthropic |
| --- | --- | --- |
| 启用方式 | **自动**（默认开启，无需声明）；GPT-5.6+ 可选显式 | **显式** `cache_control: {type:"ephemeral"}`；**也支持**顶层 `cache_control` 自动缓存（断点自动前移） |
| 断点数量 | 隐式自动；显式模式每请求最多 **4 次 cache write** | 最多 **4 个 breakpoint**（自动缓存占用其中一个槽位） |
| 最小可缓存长度 | **1024 token**（GPT-5.6+）；更早模型随设置变化 | 512–4096 token，**随模型不同** |
| 粒度 | 更早模型按 128 token 倍数报告；5.6+ 精确 | 按 block 边界 |
| 默认 TTL | 5–10 分钟（in_memory）／30 分钟–24 小时（24h） | **5 分钟**；可显式设 `ttl: "1h"` |
| TTL 计时起点 | 最近一次写入或复用后刷新 | 从写入/读取该条目的**请求开始**计时（生成耗时也计入 TTL） |
| 缓存写入价格 | 5.6+ 为 **1.25×**；更早模型无写入费 | **1.25×**（5m）／**2×**（1h） |
| 缓存读取价格 | **0.1×**（5.6+）；更早按模型（0.5×/0.25×/0.1×） | **0.1×**（少数模型例外：Opus 5.5 为 0.05×，Fable/Mythos 5.1 为 0.025×） |
| 可缓存内容范围 | 渲染后全部：隐藏指令、developer messages、**tools**、对话历史（文本/图像/文档/音频） | 明确为 `tools` → `system` → `messages` **按此顺序的整个前缀**，up to 指定 block；tool 定义通过给 `tools` 数组**最后一个 tool** 打 `cache_control` 来缓存 |
| 路由控制 | `prompt_cache_key`（~15 RPM/前缀+key） | 无等价参数，由服务端处理 |
| 预热 | `prompt_cache_options.prewarm: true` | `max_tokens: 0` 预热请求 |
| 手动清缓存 | 不支持 | 不支持 |

**核心差异一句话**：OpenAI 走"**默认自动 + 前缀全量可缓存 + 按机器路由**"路线，代价是缓存不可精确控制、受 compaction/截断影响大；Anthropic 走"**显式断点 + 严格的 tools→system→messages 顺序 + 明确 TTL 与写入费**"路线，控制力更强但需要在前缀顺序上做设计。

---

## 7. 官方明说 vs. 我的推断（汇总）

| 结论 | 性质 |
| --- | --- |
| ChatGPT saved memories 与 reference chat history 是两个独立可开关的功能 | 官方 |
| ChatGPT 记忆以系统提示分段（Model Set Context / Response Preferences / User Insights / Recent Conversation Content / Interaction Metadata）注入 | **推断**（第三方逆向，官方从未说明） |
| 记忆不是向量检索，而是近期历史 + 长期画像摘要 | **推断** |
| Responses API `truncation` 只有 `auto` / `disabled`，默认 `disabled` 且超窗报 400 | 官方（API 参考）；`last_messages` 缺失为社区确认 |
| `previous_response_id` 不携带顶层 `instructions`，且历史 token 仍计费 | 官方 |
| AGENTS.md 全局→项目逐级拼接、越近优先级越高、默认上限 32 KiB | 官方 |
| `codex.md` → `AGENTS.md` 的更名历史 | **推断/社区**，未取得官方原文 |
| Codex 自动压缩默认阈值实际约在可用上下文 94.7% 才触发 | **推断**（社区 issue，未获官方确认） |
| Prompt caching 自动生效、最小 1024 token、前缀必须完全一致、TTFT 最多降 80%、成本最多降 90% | 官方 |
| 更早模型以 128 token 为增量报告 `cached_tokens` | 官方（Cookbook 原文） |
| GPT-5.6+ 缓存读取 0.1×、写入 1.25×、TTL 默认 30m | 官方 |
| 缓存与 compaction 相互冲突 | 官方（明确写在 Prompt caching 指南） |

---

## 参考来源

- [Prompt caching（OpenAI API 指南）](https://developers.openai.com/api/docs/guides/prompt-caching)
- [Prompt Caching 201（OpenAI Cookbook）](https://developers.openai.com/cookbook/examples/prompt_caching_201)
- [Conversation state（Responses API）](https://developers.openai.com/api/docs/guides/conversation-state)
- [Migrate to the Responses API](https://developers.openai.com/api/docs/guides/migrate-to-responses)
- [Compaction](https://developers.openai.com/api/docs/guides/compaction)
- [Assistants migration guide](https://developers.openai.com/api/docs/assistants/migration)
- [Responses create（API 参考）](https://developers.openai.com/api/reference/resources/responses/methods/create)
- [Error codes](https://developers.openai.com/api/docs/guides/error-codes)
- [How to replicate last_messages truncation from Assistants in Responses API?（开发者社区）](https://community.openai.com/t/how-to-replicate-last-messages-truncation-from-assistants-in-responses-api/1361074)
- [Agents SDK — Sessions](https://openai.github.io/openai-agents-python/sessions/)
- [Agents SDK — Context management](https://openai.github.io/openai-agents-python/context/)
- [Agents SDK — Handoffs](https://openai.github.io/openai-agents-python/handoffs/)
- [Agents SDK — Results](https://openai.github.io/openai-agents-python/results/)
- [Codex — Custom instructions with AGENTS.md](https://learn.chatgpt.com/docs/agent-configuration/agents-md)
- [Codex — Configuration Reference](https://learn.chatgpt.com/docs/config-file/config-reference)
- [Codex — Developer commands / Slash commands](https://learn.chatgpt.com/docs/developer-commands)
- [Codex — Memories](https://learn.chatgpt.com/docs/customization/memories)
- [Codex — Projects and chats](https://learn.chatgpt.com/docs/projects)
- [Default auto-compaction waits until 94.7%（openai/codex issue #40095）](https://github.com/openai/codex/issues/40095)
- [How ChatGPT Remembers You（Embrace The Red 逆向分析）](https://www.mbgsec.com/archive/2025-05-05-how-chatgpt-remembers-you-a-deep-dive-into-its-memory-and-chat-history-features-embrace-the-red/)
- [Anthropic — Prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)
- [ChatGPT can now remember and reference all your previous chats（Ars Technica, 2025-04-10）](https://arstechnica.com/ai/2025/04/chatgpt-can-now-remember-and-reference-all-your-previous-chats/)
