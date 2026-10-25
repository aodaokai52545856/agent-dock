# 企业级 / 云厂商 Agent 记忆平台与框架记忆抽象调研

> 约定：**GA** = 官方已作为正式能力提供；**Preview** = 官方标注 preview/beta/experimental；**未确认** = 官方文档未找到该说法（含与常见二手说法冲突之处）。

## 1. LangGraph / LangChain / LangMem

### 1.1 两层持久化模型

官方 Persistence 文档把记忆切成两个互补系统（[persistence](https://docs.langchain.com/oss/python/langgraph/persistence)）：

| 维度 | Checkpointer | Store |
| --- | --- | --- |
| 持久化内容 | 图状态快照 | 应用自定义 KV 数据 |
| 作用域 | 单个 thread | 跨 thread |
| 记忆类型 | 短期、线程级记忆 | 长期、跨线程记忆 |
| 典型用途 | 会话连续性、human-in-the-loop、时间旅行、容错 | 用户偏好、事实、共享知识 |
| 访问方式 | config 中传 `thread_id` | 节点或代码中读写 item |

多数应用两者并用；LangSmith Agent Server 自动托管 checkpointer 与 store。

### 1.2 短期记忆 = thread-scoped checkpointer

- **粒度与快照**：每个 super-step 边界写一个 checkpoint；节点级中间结果写 `checkpoint_writes`（pending writes），使同一步骤中失败节点的兄弟节点无需重跑。`StateSnapshot` 含 `values`、`next`、`config`（`thread_id`/`checkpoint_ns`/`checkpoint_id`）、`metadata`（`source ∈ input|loop|update`、`writes`、`step`）、`created_at`、`parent_config`、`tasks`。
- **`checkpoint_ns`**：根图 `""`，子图 `"node_name:uuid"`，嵌套用 `|` 连接；节点内用 `config["configurable"]["checkpoint_ns"]` 读取——这正是"父图看不到子图状态更新"的根因。
- **实现**：基类 `BaseCheckpointSaver`（`put`/`put_writes`/`get_tuple`/`list`，异步加 `a`，另有 `delete_thread`）；`InMemorySaver`（旧名 `MemorySaver`，重启即丢）、`SqliteSaver`/`AsyncSqliteSaver`、`PostgresSaver`/`AsyncPostgresSaver`、`MongoDBSaver`/`AsyncMongoDBSaver`、`CosmosDBSaverSync`/`CosmosDBSaver`；可选扩展 `adelete_for_runs`/`acopy_thread`/`aprune`/`aget_delta_channel_history`。
- **时间旅行与持久化强度**：`get_state(config)`、`get_state_history(config)`（时间倒序）、`update_state(...)`（**不改原 checkpoint**，生成新的，`as_node` 决定下一步）、以旧 `checkpoint_id` 重跑即 replay；`durability="exit"|"async"|"sync"`。`DeltaChannel` 只存增量（`langgraph>=1.2`，**beta**）（[checkpointers](https://docs.langchain.com/oss/python/langgraph/checkpointers)）。

### 1.3 长期记忆 = BaseStore

（[stores](https://docs.langchain.com/oss/python/langgraph/stores)；生产推荐 `PostgresStore`/`MongoDBStore`/`RedisStore`/`UpstashStore`，均继承 `BaseStore`）

- **命名空间是字符串元组且按前缀匹配**：`("user_id","memories")` 长度任意；`asearch(("alice",))` 会命中 `("alice","memories")`、`("alice","preferences")`；`aget(ns,key)` 须近似 O(1)。条目 `Item` 含 `value`（dict）、`key`（`memory_id`）、`namespace`、`created_at`、`updated_at`，语义检索时带 `score`。
- **API**：`aput(namespace,key,value,index=None)`、`aget`、`adelete`、`asearch(namespace_prefix,*,query=None,filter=None,limit=10,offset=0)`、`alist_namespaces(*,prefix,suffix,max_depth,limit=100,offset=0)`。
- **语义检索索引**：构造时传 `index={"embed": init_embeddings("openai:text-embedding-3-small"), "dims": 1536, "fields": ["$"]}`；`put(..., index=["food_preference"])` 只嵌该字段、`index=False` 不嵌（仍可 `get`）；`query` 非空时结果带 `score`。LangSmith 部署改在 `langgraph.json` 配 `store.index`。
- **两个坑（官方明确写出）**：超过 `limit` **静默截断**、无溢出信号；**默认排序依赖后端**（`PostgresStore` 按 `updated_at` 倒序，`InMemoryStore` 按插入序），需要顺序时客户端按 `item.updated_at` 排序。

### 1.4 语义 / 情景 / 程序记忆的落地

| 类型 | 存什么 | 落地形态 |
| --- | --- | --- |
| 语义 Semantic | 事实、偏好 | **Profile**（单文档持续更新，`enable_inserts=False`；可读易改但变大后易出错）或 **Collection**（多文档追加，召回率高但删除/更新难，模型易过度插入或过度更新） |
| 情景 Episodic | 过往经历 | Collection：few-shot 示例，如 `Episode(observation, thoughts, action, result)` |
| 程序 Procedural | 系统行为 | prompt 规则；官方范式是用 store 存 `("agent_instructions",) → key="agent_a"`，由 `update_instructions` 节点改写 |

### 1.5 hot path 与 background：官方对比

| 维度 | hot path | background |
| --- | --- | --- |
| 机制 | Agent 回复前调用记忆工具决定存什么 | 独立任务在会话后/静默后反思提取 |
| 优点 | 实时可用、可对用户透明 | 主链路零延迟、应用逻辑与记忆管理解耦 |
| 代价 | 增加延迟；需多挂工具；记忆数量/质量可能下降 | 需定频率与触发策略（定时重排 / cron / 手动） |

官方参考实现：hot path = [memory-agent](https://github.com/langchain-ai/memory-agent)（对应 ChatGPT 的 `save_memories` 工具范式）；background = [memory-template](https://github.com/langchain-ai/memory-template)。

### 1.6 LangMem SDK

- **工具（hot path）**：`create_manage_memory_tool(namespace, *, instructions, schema=str, actions_permitted=("create","update","delete"), store=None, name="manage_memory")` → `manage_memory(content=None, id=None, action="create"|"update"|"delete")`；`create_search_memory_tool(namespace, *, instructions, store=None, response_format="content"|"content_and_artifact", name="search_memory")` → `search_memory(query, limit=10, offset=0, filter=None)`。
- **管理器**：`create_memory_manager(model, *, schemas=(Memory,), instructions, enable_inserts=True, enable_updates=True, enable_deletes=False)`；`create_memory_store_manager(model, *, schemas, instructions, default, default_factory, enable_inserts=True, enable_deletes=False, query_model, query_limit=5, namespace=("memories","{langgraph_user_id}"), store, phases)`——数据流为"检索相似记忆 → LLM 抽取合并 → 写回 store"，`query_model` 可用更小模型降本。
- **`ReflectionExecutor`（去抖后台）**：`ReflectionExecutor(reflector, namespace, /, *, url, client, sync_client, store)` + `executor.submit(to_process, after_seconds=delay)`。**同会话线程维护待处理队列，新消息到达会取消旧挂起任务并重排**，静默 `after_seconds` 后才处理（实践建议 30–60 分钟）；serverless 下须传 `url` 改用远程执行器。
- **prompt optimizer（程序记忆）**：`create_prompt_optimizer(model, /, *, kind="gradient"|"prompt_memory"|"metaprompt", config=None)`。`gradient` 反思循环（4–10 次调用，`min/max_reflection_steps` 默认 1/3）；`prompt_memory` 单次调用；`metaprompt` 2–5 次。`create_multi_prompt_optimizer` 先用 **credit assignment** 判断哪个 prompt 需改，再逐个优化。

### 1.7 消息裁剪 / 摘要，以及对 prompt 缓存的影响

- **`trim_messages(messages, *, max_tokens, token_counter, strategy="last"|"first", allow_partial=False, end_on=None, start_on=None, include_system=False, text_splitter=None)`**：`start_on='human'` 保证裁剪后以人类消息开头，`include_system=True` 保留索引 0 的 `SystemMessage`——**二者仅对 `strategy="last"` 有效**（传错直接报错）。官方提醒裁剪后仍须是合法消息序列（部分厂商要求以 user 开头；带 tool calls 的 assistant 消息必须紧跟对应 tool 结果）。
- **永久删除**：`RemoveMessage(id=…)` 删指定消息；`RemoveMessage(id=REMOVE_ALL_MESSAGES)`（`langgraph.graph.message`）清空。前提是 messages 键使用 `add_messages` reducer（`AgentState` 默认满足）；在 LangChain Agent 中由 `@before_model` / `@after_model` 中间件执行。
- **摘要**：LangMem `SummarizationNode(model, *, max_tokens, max_tokens_before_summary=None, max_summary_tokens=256, token_counter, initial_summary_prompt, existing_summary_prompt, final_prompt, input_messages_key="messages", output_messages_key="summarized_messages", name="summarization")`，输出 `{summarized_messages, context.running_summary}`；`RunningSummary(summary, summarized_message_ids, last_summarized_message_id)` 用于**避免重复摘要**；`output_messages_key` 默认**不同于**输入键，以免污染主消息列表。LangChain `SummarizationMiddleware(model, trigger=("tokens",4000)|("messages",n)|("fraction",f)|dict|list, keep=("messages",20), token_counter, summary_prompt, trim_tokens_to_summarize=4000)`——`max_tokens_before_summary`/`messages_to_keep`/`summary_prefix` **已废弃**；另有 `ContextEditingMiddleware` + `ClearToolUsesEdit(trigger, clear_at_least, keep, clear_tool_inputs, exclude_tools, placeholder="[cleared]")` 只清旧工具输出（[built-in middleware](https://docs.langchain.com/oss/python/langchain/middleware/built-in)）。
- **对 prompt 缓存的影响（关键工程约束）**：Anthropic 官方明确**缓存的是完整前缀**，命中要求前缀逐字节一致（[prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)）。因此 `RemoveMessage` 删早期消息、`trim_messages` 滑动窗口、摘要"用一条 summary 替换 N 条旧消息"都会改变前缀 → 下一轮**整段前缀重新计费与重新处理**。LangChain 侧对应实现为 `AnthropicPromptCachingMiddleware(type="ephemeral", ttl="5m"|"1h", min_messages_to_cache=0, unsupported_model_behavior="warn")`，它显式标注 system prompt 与工具定义这类稳定内容，并"缓存到每条请求的最新消息"（[Anthropic middleware](https://docs.langchain.com/oss/python/integrations/middleware/anthropic)）。实践：稳定前缀单独设断点；历史尽量 append-only；把裁剪/摘要安排在不频繁的时机，或接受一次缓存冷启动。

---

## 2. AWS Bedrock AgentCore Memory

**状态：GA（非 Preview）**。控制面 SDK `bedrock-agentcore-control`（`CreateMemory`/`UpdateMemory`/`GetMemory`/`ListMemories`/`DeleteMemory`），数据面 `bedrock-agentcore`（[AgentCore Memory](https://docs.aws.amazon.com/bedrock-agentcore/latest/devguide/memory.html)）。

### 2.1 短期记忆：immutable raw events

`CreateEvent`（`POST /memories/{memoryId}/events`）参数：`memoryId`（路径）、`actorId`（≤255）、`sessionId`（≤100，可空）、`eventTimestamp`、`payload`（0–100 项）、`branch{name, rootEventId}`（消息编辑/多分支）、`clientToken`、`extractionMode="SKIP"`（入库但不提取）、`extractionConfig.namespaceVariables`。

`PayloadType` 是 **union**：**`conversational`**（`role ∈ ASSISTANT|USER|TOOL|OTHER` + `content`）、**`blob`**（二进制）、**`json`**（`MemoryJsonData`，≤100KB）——**只有 `conversational` 与 `json` 参与长期记忆提取，`blob` 仅存短期**。事件**不可变**；用 `ListEvents`（`filter.branch`/`filter.eventMetadata`/`includePayloads`/`maxResults` 默认 20）与 `GetEvent` 读取（`eventId` 形如 `<数字>#<hex>`）。保留期由 `CreateMemory.eventExpiryDuration` 决定（ISO 天数；API 约束 3–365、配额表写最小 7 天，**文档内部不一致**），**写入时按事件固化**。

### 2.2 长期记忆：策略（strategies）

`CreateMemory` 中策略为 **union key，只能指定其一**：`semanticMemoryStrategy`/`summaryMemoryStrategy`/`userPreferenceMemoryStrategy`/`customMemoryStrategy`/`episodicMemoryStrategy`；读取时 `MemoryStrategy.type ∈ SEMANTIC|SUMMARIZATION|USER_PREFERENCE|CUSTOM|EPISODIC`。

| 策略 | 处理步骤 | 默认命名空间 |
| --- | --- | --- |
| Semantic | Extraction + Consolidation（仅 USER/ASSISTANT） | `/strategy/{memoryStrategyId}/actors/{actorId}/` |
| User Preference | Extraction + Consolidation（仅 USER/ASSISTANT） | `/strategy/{memoryStrategyId}/actors/{actorId}/` |
| Summary | 仅 Consolidation，会话级切片 | `/strategy/{memoryStrategyId}/actor/{actorId}/session/{sessionId}/`（`sessionId` 必填） |
| Episodic | Extraction + Consolidation + **Reflection** | 会话/episode 级 |
| Custom | 自管管线（`appendToPrompt` ≤30KB） | 自定义 |

Episodic 仅在系统判定"episode 完成"后才产出记录，**延迟高于其它策略**，且 reflection 可跨 actor（官方提示隐私风险）。

### 2.3 命名空间模板与多租户隔离

- 正式参数是 **`namespaceTemplates`**；`namespaces` 在 `MemoryStrategy` 中**已 deprecated**。占位符 `{actorId}`、`{sessionId}`、**`{memoryStrategyId}`**（不是 `{strategyId}`）；粒度可到策略级 `/strategy/{memoryStrategyId}/`、actor 级、会话级或全局 `/`。官方示例中 `/actor/` 与 `/actors/` 混用，属文档内部不一致。
- **自定义变量是隔离核心**：`CreateMemory.namespaceKeys` 最多 5 个（键名**必须全小写**、≤32 字符、不得与内置同名），每模板最多引用 5 个变量，值在 `CreateEvent.extractionConfig.namespaceVariables` 运行时注入；支持 `allowedValues`（≤10，区分大小写）与 `regexPattern`（≤64，AND）。
- **失败模式（重要）**：模板引用的变量未提供时，`CreateEvent` **仍返回成功、事件落短期记忆，但该策略静默不触发长期提取**，仅靠 `NamespaceResolutionFailure` 指标暴露。
- IAM 条件键：写路径 `bedrock-agentcore:namespaceVariable/<key>`，读路径 `bedrock-agentcore:namespace` / `namespacePath`。

### 2.4 检索

`RetrieveMemoryRecords`（`POST /memories/{memoryId}/retrieve`）：`searchCriteria.searchQuery` 必填（1–10000 字符）、`memoryStrategyId` 可选、`topK`（1–100，**默认 10**，用于语义排序）、`metadataFilters`（≤5）；`namespace` / `namespacePath` **二选一必填**（前缀匹配 / 层级匹配）；顶层 `maxResults`（1–100，**默认 20**，是分页大小，与 `topK` 语义不同）。

**只做语义检索**（余弦相似度，`score` **不是**百分比匹配度），**官方未提供 lexical/keyword 检索参数**；非语义浏览需用 `ListMemoryRecords`（`namespace`/`namespacePath` + `memoryStrategyId` + `metadataFilters`，10 个 operator：`EQUALS_TO`/`EXISTS`/`NOT_EXISTS`/`BEFORE`/`AFTER`/`CONTAINS`/`GREATER_THAN`/`GREATER_THAN_OR_EQUALS`/`LESS_THAN`/`LESS_THAN_OR_EQUALS`）。单条用 `GetMemoryRecord`/`DeleteMemoryRecord`（需 `memoryRecordId` + `namespace` 供 IAM 鉴权）。

### 2.5 写入路径、异步延迟与配额

- 长期记忆是**异步后台流程**：Extraction → Consolidation（Episodic 再加 Reflection）。
- **官方对延迟的唯一承诺**："memory records typically appear within **seconds to minutes** depending on content size and strategy configuration"——**无 SLA**；官方示例用 `time.sleep(60)` 等待。
- 绕过抽取可直接写：`BatchCreateMemoryRecords`（每条含 `requestIdentifier`/`namespaces`/`content.text`/`timestamp`，返回 `successfulRecords`/`failedRecords`）；`IngestData` 可绕过短期事件直灌长期记忆（≤100 项、≤100KB，**不作用于 self-managed 策略**）。失败用 `ListMemoryExtractionJobs` + `StartMemoryExtractionJob` 重驱动；通知走 Kinesis（`MemoryRecordCreated/Updated/Deleted`）。
- **工程结论：同一轮对话内刚写入的偏好检索不到**，读路径必须容忍秒~分钟级最终一致。
- **配额**：每 memory ≤6 个策略；`CreateEvent` 200 TPS 但**每 actor/session 含负载仅 5 TPS**；`RetrieveMemoryRecords`/`ListMemoryRecords` 各 30 TPS；`IngestData` 10 TPS；长记忆提取 150,000 TPM。
- **定价（部分确认）**：$0.25 / 1,000 新事件；内置策略 **$0.75 / 1,000 条记忆记录/月**；检索与短期存储单价未确认。
- **区域**：us-east-1/2、us-west-2、eu-central-1、eu-west-1/2/3、eu-north-1、ap-south-1、ap-southeast-1/2、ap-northeast-1/2、ca-central-1、sa-east-1、us-gov-west-1；**不支持** us-west-1、eu-south-1/2、ap-south-2、ap-southeast-5/7。
- **框架集成**：Strands `AgentCoreMemorySessionManager`、LangGraph/LangChain `AgentCoreMemorySaver`/`AgentCoreMemoryStore`（`langgraph-checkpoint-aws`）；**CrewAI 未确认**。

---

## 3. Google Vertex AI Agent Engine / Memory Bank

> **命名更正**：Google 已于 **2026-04-22** 将 Vertex AI 并入 **Gemini Enterprise Agent Platform**（`Agent Engine` → **Agent Runtime**），文档迁至 `docs.cloud.google.com/gemini-enterprise-agent-platform/...`。因此"必须 us-central1"**已过时**——现支持单区域、`us`/`eu` 多区域与 `global`（global **不支持 CMEK**）（[release notes](https://docs.cloud.google.com/gemini-enterprise-agent-platform/release-notes)）。

### 3.1 读写 API

| 项 | 值 |
| --- | --- |
| SDK（新） | `client.memory_banks.memories.generate()` / `.retrieve()` / `.get()` / `.list()` |
| SDK（旧 aiplatform / ADK 内部） | `client.agent_engines.memories.generate` / `.retrieve` / `.create` / `.ingest_events` |
| REST | `memories.generate`、`memories.retrieve`、`memories.create`、`memories.ingest_events`、`memories.rollback`、`memories.revisions.list/get` |
| REST 路径 | `projects/{project}/locations/{location}/reasoningEngines/{id}:generateMemories` 与 `.../memories:retrieve`（另有 `projects.locations.memoryBanks.memories/*` 新路径，`reasoningEngines` 为兼容保留） |

**`GenerateMemories` 请求体**（三种数据源互斥）：`direct_contents_source.events[]`（每项含 `content`，即 `genai.types.Content`：`role` + `parts[]`）、`vertex_session_source{session, start_time, end_time}`、`direct_memories_source.direct_memories[]`（`[{fact}]`，**单请求上限 5 条**，ADK 自动分批）。公共字段 `name`、`scope`（dict）、`config`；`config` 含 `wait_for_completion`、`allowed_topics[]`、`disable_consolidation`、`disable_memory_revisions`、`metadata` + `metadata_merge_strategy`（`MERGE` 默认 / `OVERWRITE` / `REQUIRE_EXACT_MATCH`）、`revision_ttl`、`revision_expire_time`、`revision_labels`、`ttl`。响应为 `GenerateMemoriesOperation{name, done, response.generatedMemories[]}`，每项 `{memory, action ∈ CREATED|UPDATED|DELETED}`，附 `previous_revision` 供回滚。

**`RetrieveMemories` 请求体**：`name`、**`scope`（必填，精确匹配）**、`similarity_search_params{ search_query, top_k }`——**`top_k` 默认 3**，返回 `distance` 并按**欧氏距离**升序；**省略该参数即返回 scope 下全部记忆**（100 条/页）。过滤两套：`config.filter_groups[]`（元数据，DNF：组间 OR、组内 AND，元素 `MemoryFilter{key, value}`）与 `config.filter`（系统字段 EBNF 字符串，可过滤 `create_time`/`update_time`/`fact`/`topics`）。旧 REST 面的 `simple_retrieval_params` 已被"省略参数即全量"取代，**替代语义未确认**。

### 3.2 作用域（scope）与 topics

- `scope` 是**任意 KV dict**（`{"user_id":"123"}`、`{"session_id":"456"}` 或组合），**隔离语义 = 精确匹配**（key 集合与值都相同，顺序无关）；retrieve 只返回 scope 完全一致者，consolidation 只在同 scope 内发生；**scope 一经生成/创建即 immutable**。
- 以 Sessions 为源且未给 `scope` 时默认 `{"user_id": session.user_id}`；**ADK 的 `VertexAiMemoryBankService` 自动传 `{"app_name": app_name, "user_id": user_id}`**——即 **project/assistant 维度用 `app_name`、user 维度用 `user_id`**。可按 scope key 组合配不同提取策略：`unstructured_memory_configs[].scope_keys`。
- **Memory topics**：4 个托管 topic 默认全开——`USER_PERSONAL_INFO`、`USER_PREFERENCES`、`KEY_CONVERSATION_DETAILS`、`EXPLICIT_INSTRUCTIONS`（写法 `{"managed_memory_topic": {"managed_topic_enum": "USER_PREFERENCES"}}`）；自定义为 `{"custom_memory_topic": {"label": …, "description": …}}`。二者都进 extraction prompt，建议配 few-shot（`generate_memories_examples[].conversation_source.events[]` + `generated_memories[].fact`）；请求级收敛子集用 `config.allowed_topics[]`。
- **Consolidation**：提取 → 与同 scope 现有记忆比对 → 决定 ADD/UPDATE/DELETE，**矛盾信息会删除旧记忆**；默认只用候选记忆的最新 revision 比对，可用 `consolidation_config.revisions_per_candidate_count`（默认 1）调稳；`disable_consolidation=True` 则只提取不合并。

### 3.3 版本化、TTL 与成熟度

| 能力 | 细节 |
| --- | --- |
| 相似度检索 | 默认 embedding **`text-embedding-005`**；默认生成模型 **`gemini-3.5-flash`**（2026-06-29 起，此前实例仍为 `gemini-2.5-flash`） |
| Revisions | 默认开启，每次变更存**不可变 revision**（含中间产物 `extracted_memories` 与最终 `fact`），可用 `revision_labels` 打标过滤 |
| TTL | revision 用 `revision_ttl`（如 `"2592000s"`）或 `revision_expire_time`，**默认 365 天**；记忆用 `ttl_config.default_ttl` 或 `granular_ttl_config{create_ttl, generate_created_ttl, generate_updated_ttl}`，默认**永不过期**；父记忆删除后子 revision 仅留 **48 小时**可 `rollback` |

**成熟度**：Memory Bank 与 `GenerateMemories`/`RetrieveMemories` 主流程 **GA**（分批 GA：`IngestEvents` 2026-07-08、Memory profiles 2026-07-15、global/多区域端点 2026-06-17），但**REST 参考当前仍挂 `v1beta1`**。权限：`roles/aiplatform.memoryUser`。

### 3.4 ADK 集成

| 组件 | 精确名称 |
| --- | --- |
| Memory 服务类 | `VertexAiMemoryBankService(project=, location=, agent_engine_id=)`；`InMemoryMemoryService()`；`VertexAiRagMemoryService(rag_corpus=, similarity_top_k=, vector_distance_threshold=)` |
| 写记忆 | `add_session_to_memory(session)`；`add_events_to_memory(app_name, user_id, events, session_id, custom_metadata)`；`add_memory(app_name, user_id, memories=[MemoryEntry(...)], custom_metadata=)` |
| 默认写入路径 | `add_events_to_memory` **默认走 `memories.ingest_events`**（fire-and-forget，约 800ms 触发）；仅当 `custom_metadata` 含 GenerateMemories 专有键（`ttl`/`revision_ttl`/`metadata`/`wait_for_completion`）才切 `memories.generate`。`add_memory` 默认走 `memories.create`，置 `{"enable_consolidation": True}` 时改走 `generate` + `direct_memories_source` |
| 读记忆 | `search_memory(app_name=, user_id=, query=)` → 内部 `memories.retrieve(similarity_search_params={"search_query": query})`，返回 `SearchMemoryResponse{memories:[MemoryEntry(content, id, author, timestamp, custom_metadata)]}` |
| 工具/回调/装配 | `from google.adk.tools import preload_memory`（每轮自动预取）与 `load_memory`（模型决定调用）；`after_agent_callback=auto_save_session_to_memory_callback`；`Runner(agent=, app_name=, session_service=, memory_service=...)`、`adk web --memory_service_uri="agentengine://<id>"`（部署到 Agent Runtime 时 ADK 模板默认用 `VertexAiMemoryBankService`） |

框架级**只支持 1 个 memory service**；多路复用需自行实例化第二个并在自定义 tool 中查询（[ADK Memory](https://adk.dev/sessions/memory/)）。

---

## 4. Azure AI Foundry Agent Service 的记忆

**状态：Memory 与 Memory Store API 均为 Preview**；Foundry Agent Service 本体 GA，仅子能力 preview——工具名的 `_preview` 后缀正源于此。

### 4.1 `memory` 工具：类型名与参数

| 项 | 精确值 |
| --- | --- |
| 工具 type（REST / JS） | **`memory_search_preview`** |
| 工具类（Python / C# / Java） | **`MemorySearchPreviewTool`** |
| 工具参数 | `memory_store_name`、`scope`、`update_delay`（Python/JS/REST）/ `UpdateDelayInSecs`（C#/Java）。**单位是秒**（默认 300 = 5 分钟），官方示例 `update_delay: 1`；**不是 `01:00:00` 这类 time 字面量** |
| `scope` 特殊值 | `"{{$userId}}"` 启用自动身份解析 |

需要更正的任务前提：`chat_summary_memory` / `user_profile_memory` **不是两个独立工具名**，也未找到 `max_chat_summary_memories` / `user_profile_fields`（**未确认存在**）。现行设计是"**store 级开关 + 统一 `memory_search_preview` 工具**"：

```text
MemoryStoreDefaultDefinition{ kind: "default", chat_model, embedding_model, options }
MemoryStoreDefaultOptions{ chat_summary_enabled, user_profile_enabled,
                           procedural_memory_enabled, default_ttl_seconds, user_profile_details }
MemorySearchOptions{ max_memories }        # 检索条数在此控制
```

`MemoryItem{ memory_id, scope, content, kind }`，`kind ∈ {user_profile, chat_summary, procedural}`；三类记忆默认全开，均为 LLM 提取 + LLM 合并。定制化通过 **`user_profile_details`**（自然语言包含/排除清单）而非字段白名单。

### 4.2 store 模型与 REST 端点

`MemoryStore` 是**项目级资源**，不属于 agent 也不属于 thread；store 内按 **scope 分区**，官方建议**每个 agent 建一个专属 store**。**"默认 store 名 `{agent_id}_memory`"未在官方文档中找到（未确认）**——store 名由开发者显式指定。

| 操作 | 方法与路径（`api-version=2025-11-15-preview`） |
| --- | --- |
| 创建/更新/列表/删除 | `POST /memory_stores`、`POST /memory_stores/{name}`、`GET /memory_stores`、`DELETE /memory_stores/{name}` |
| 追加记忆 | `POST /memory_stores/{name}:update_memories`（`scope` / `items` / `update_delay` / `previous_update_id`） |
| 轮询更新 | `GET /memory_stores/{name}/updates/{update_id}` |
| 检索 | `POST /memory_stores/{name}:search_memories`（`scope` / `items` / `previous_search_id` / `options.max_memories`） |
| 删 scope | `POST /memory_stores/{name}:delete_scope` |
| Item CRUD | `POST .../items`、`GET/POST/DELETE .../items/{memory_id}`、`GET .../items:list?scope=` |

**静态检索 vs 上下文检索**：只给 `scope`（不给 `items`/`previous_search_id`）→ 只返回该 scope 的 **user profile** 记忆（适合会话开始注入）；给 `items`（最新消息）→ 返回 **user profile + chat summary** 中最相关者。

### 4.3 版本、延迟、区域与限制

- **API 版本**：Memory Store REST 用 **`2025-11-15-preview`**；文档明确指出 **agents API 用 `api-version=v1`，与 memory store API 不同**。`2025-05-15-preview` 未在当前文档出现，**是否仍受支持未确认**。
- **模型要求**：必须同时部署兼容的 Azure OpenAI **chat 模型**（示例 `gpt-5.2`）与 **embedding 模型**（示例 `text-embedding-3-small`），store 创建时即绑定。
- **`update_delay` 延迟语义**：按"**用户不活动（inactivity）**"计时，默认 **300 秒**；要立即处理须显式传 `update_delay: 0`。`begin_update_memories` 是**长时运行操作，约需 1 分钟**，可用 `previous_update_id` 链式接续。工具路径下记忆更新被**去抖动（debounced）**——官方 troubleshooting 把它列为"Memory 没出现"的头号原因。
- **区域（19 个）**：Australia East、Brazil South、Canada East、East US 2、France Central、Italy North、Japan East、Korea Central、North Central US、Norway East、South Africa North、South India、Sweden Central、Switzerland North、UAE North、UK South、West US、West US 2、West US 3。
- **限制与配额**：**不支持 VNet 集成**；低层 Memory API **必须显式传 `scope`**。每 store ≤**100 个 scope**、每 scope ≤**10,000 条记忆**，`search_memories`/`update_memories` 各 **1,000 请求/分钟**。身份解析：`x-memory-user-id` 请求头 → 否则回退 Entra token 的 **`{tid}_{oid}`**。
- **TTL**：`default_ttl_seconds`（`0` = 永不过期）；**仅对启用该能力后新建的 store 生效**；记忆被更新/合并后**重置 last-updated 时间**；procedural memory 与 default TTL **只能在 store 创建时设置**。
- **定价**：Memory 处 public preview，**定价与计费可能变化**；官方口径是**按底层 chat 与 embedding 模型用量计费**，API 本身**无独立单价**。

（来源：[what-is-memory](https://learn.microsoft.com/en-us/azure/foundry/agents/concepts/what-is-memory)、[memory-usage](https://learn.microsoft.com/en-us/azure/foundry/agents/how-to/memory-usage)）

---

## 5. 其他开源 / 厂商方案：记忆层次如何划分

### 5.1 Redis Agent Memory Server

官方仓库已**一分为二**：主分支现指向 Redis Iris 的托管产品 **Redis Agent Memory（官方标注 Preview）**；原开源实现移入 `V0/`，README 明确称其 *"no longer actively maintained and is not the supported production path"*（Apache-2.0，API v0.14.0）。

| 层 | 存什么 | 机制 |
| --- | --- | --- |
| Working memory（会话级） | `messages`（role/content）、`context`（旧消息摘要）、`memories`（待晋升）、`data`（会话 KV） | **Redis JSON**；`ttl_seconds`（文档一说默认持久、一说默认 1 小时，**内部不一致**）；超过 `SUMMARIZATION_THRESHOLD=0.7` 触发渐进摘要 |
| Long-term memory（跨会话） | 提取出的 memories，类型精确为 **`semantic` / `episodic` / `message`** | RediSearch + RedisVL 的向量/全文/元数据索引（`REDISVL_INDEX_NAME=memory_records`、`REDISVL_DISTANCE_METRIC=COSINE`、`REDISVL_VECTOR_DIMENSIONS=1536`、`REDISVL_INDEXING_ALGORITHM=HNSW`） |

REST 真实端点：`GET /v1/working-memory/`、`GET|PUT|DELETE /v1/working-memory/{session_id}`、`POST /v1/long-term-memory/`、`POST /v1/long-term-memory/search`、`PATCH /v1/long-term-memory/{memory_id}`、`POST /v1/memory/prompt`。搜索参数是 **`distance_threshold`**（不是 `similarity_threshold`）、`limit`、`search_mode ∈ semantic|keyword|hybrid` + `hybrid_alpha=0.7`，以及一组 `recency_*` 加权（`recency_boost`、`recency_semantic_weight`、`recency_half_life_last_access_days` 等）；过滤器为 `namespace`/`user_id`/`session_id`/`topics`/`entities`/`memory_type`，操作符 `eq|ne|any|all`。MCP 工具为 `set_working_memory`、`create_long_term_memories`、`search_long_term_memory`、`edit_long_term_memory`、`delete_long_term_memories`、`get_long_term_memory`、`memory_prompt`（**没有 `get_working_memory`**）。

**三处常见错误说法须纠正**：**不存在 `window_size` 滑动窗口、"按时间新旧自动晋升"、`EmbeddedJSON`**。晋升实由 `memories` 中 `persisted_at=null` 的记录，或 `long_term_memory_strategy ∈ discrete(默认)|summary|preferences|custom` 驱动。托管新产品改为 `POST /v1/stores/{storeId}/session-memory/events`、`POST /v1/stores/{storeId}/long-term-memory/search`，标识符 `sessionId`/`actorId`/`ownerId`，类型增加 **`session_summary_view`**。V0 可 Docker 自托管（`redislabs/agent-memory-server`），新产品托管于 Redis Cloud、Redis Software 自管为 **private preview**。

### 5.2 其余四种方案的记忆层次

| 系统 | 记忆层次（精确名） | 关键参数 | 自托管 / 成熟度 |
| --- | --- | --- | --- |
| **MongoDB Atlas** | 短时 `MongoDBSaver`（写 `checkpoints`/`checkpoint_writes`）；长时 `MongoDBStore` | `MongoDBStore(collection, ttl_config, index_config, auto_index_timeout=15, query_model, rerank_config)`；`ttl_config={"default_ttl": 秒, "refresh_on_read": True}`（索引建在 `updated_at`）；`create_vector_index_config(dims, embed, fields, relevance_score_fn='cosine')`；`$vectorSearch` 必填 `index`/`path`/`queryVector`/`limit`，另有 `numCandidates`（建议 ≥20×`limit`）、`filter`、`exact` | ✅ 自建或 Atlas；官方称 "production-ready" 但**无 GA 字样**，Automated Embedding 为 **Preview** |
| **Memobase** | **Profile**（`topic`/`sub_topic`/`content`）+ **Event 时间线**（`event_tags`/`profile_delta`）+ blob buffer | `POST /api/v1/users`、`POST /api/v1/blobs/insert/{user_id}`、`POST /api/v1/users/buffer/{user_id}/{buffer_type}`、`GET|POST /api/v1/users/profile/{user_id}`、`GET /api/v1/users/event/{user_id}`；YAML `additional_user_profiles`、`buffer_flush_interval:3600`、`max_chat_blob_buffer_token_size:1024` | ✅ Docker + Postgres + Redis；版本仅 **0.0.x** |
| **Supermemory** | 同一 `containerTag` 下三层：Document chunks（SuperRAG）/ Memories（图）/ Profile（`static`+`dynamic`） | `POST /v4/search` 的 `searchMode ∈ memories|documents|hybrid`、`limit`、`threshold=0.5`、`rerank`、`rewriteQuery`、`include.{documents,summaries,relatedMemories}`；写入 `POST /v3/documents`、`/v4/memories`、`dreaming ∈ dynamic|instant`、`taskType ∈ memory|superrag` | ⚠️ `supermemory local` 开源单二进制（端口 6767）；连接器与 MCP 仅托管版 |
| **MemU** | **旧三层 `Resource`/`MemoryItem`/`MemoryCategory` 已废弃**（ADR 0006：`MemoryItem→RecallEntry`、`MemoryCategory→RecallFile`），现为 `Resource`/`RecallFile`/`RecallFileSegment` | `RecallFile.track ∈ memory|skill`、`progressive_retrieve`、`commit_results`、`MEMU_DB`、`MEMU_MEMORY_MODE` | ✅ SQLite / Postgres+pgvector；**Beta**（0.11.0b3） |

补充否证：MongoDB 官方只有**设计蓝图**（文档 `## Agent Memory` 节区分 short/long-term）而非规定性 schema，**官方 MCP Server 没有任何记忆工具**，`mongodb-memory-provider` 在 PyPI **不存在**；Memobase 聊天写入端点是 `/api/v1/blobs/insert/{user_id}`（**不是** `/api/v1/users/{id}/chats`），配置键也不是 `slots`；Supermemory 现用 Bearer + `containerTag` 单数字符串（≤100 字符），**`x-sm-user-id` 与 `includeSummary` 均未确认**；MemU 的"24/7 proactive memory"与旧 LoCoMo 92.09% 成绩已被官方标为历史。

---

## 6. 记忆即服务：Mem0 Platform 与 Zep Cloud 的企业级设计

### 6.1 Mem0 Platform

- **多租户**：org → project → `user_id`；org/project 由 API key 经 `GET /v1/ping/` 解析，构造函数**不再接受** `org_id`/`project_id`。作用域 ID 为 `user_id`/`agent_id`/`run_id`/`app_id`（`app_id` 仅 Platform）。
- **API 演进**：主端点是 `/v3/memories/*`（`POST /v3/memories/add/` 返回 `event_id` 异步、`POST /v3/memories/search/`、分页 `POST /v3/memories/`）；`/v1`、`/v2` 仍可用。**已从 SDK 移除**的参数包括 `output_format`、`api_version`、`async_mode`、`enable_graph`、`immutable`、`filter_memories`、`includes/excludes`（REST schema 仍列出部分字段，**实际生效状态未确认**）。
- **检索**：`top_k`（1–1000，默认 10）、`threshold`（默认 0.1，`0.0` 关闭）、`rerank`（默认 false，开启约 +200–400ms）、`reference_date`、`show_expired`；返回含 `score_breakdown{semantic, bm25, entity}`。实体 ID 在 `search`/`get_all` 中**必须放在 `filters` 内**（顶层传参报 400）；`filters` 支持 `AND/OR/NOT` 与 `in/gte/lte/gt/lt/ne/contains/icontains/*`。
- **抽取与图谱**：抽取改为**单遍 ADD-only**（`event` 只返回 `ADD`，冲突交给检索排序而非覆盖）。**OSS 图记忆已在 v3 删除**（`enable_graph`、`graph_store` 及各图数据库驱动移除）；**Platform 图记忆原生内置且永远开启**——实体成为节点、共享实体的记忆互连，**只影响排序、不改变响应结构、不建立类型化谓词边**；Graph view 仅 Pro/Enterprise。
- **企业能力**：项目级角色 `READER`/`OWNER`（`client.project.add_member(email, role)`）；审计可用项目级事件流 `GET /v1/events/`（**字段级 schema 未确认**）；BYOK 由组织级 `byok_enabled` 控制，Platform **自带 LLM/嵌入器（BYOI 不适用）**。**SOC 2 / HIPAA / 数据驻留主张未确认**。Webhook 事件名为 `memory_add`/`memory_update`/`memory_delete`/`memory_categorize`/`ingest_job_*`（**不是 `MEMORY_CREATED`**）。OSS `mem0ai/mem0` 为 **Apache-2.0** 可 Docker Compose 自建，但**缺**图记忆、webhook、自定义分类、Memory Decay、Temporal Reasoning、批量与导出；默认用 `gpt-5-mini` 抽取、`text-embedding-3-small`（1536 维）、Qdrant 落盘、SQLite 历史。

### 6.2 Zep Cloud

- **架构**：以开源 **Graphiti**（Apache-2.0）从 episode 派生图产物，跑在自研图数据库 Konig 上。三种存储类型：**entity edges（边=事实）**、**entity nodes（实体+摘要）**、**episodic nodes（来源 episode）**。
- **双时序四时间戳**：`created_at`（得知）、`valid_at`（开始为真）、`invalid_at`（不再为真）、`expired_at`（得知失效）。矛盾到来时**作废旧边而非删除**，保留有效性历史以支持时点查询——这是它区别于 Mem0 的核心。边带 SCREAMING_SNAKE_CASE 的 `name`（如 `WORKS_AT`）；`hyperedge_uuid` 对多元事实投影出的成对边分组。
- **API 表面**（SDK 称 v3，**REST 路径仍是 `/api/v2/...`**）：`client.graph.search(user_id=` 或 `graph_id`（二选一）`, query(≤400 字符), scope, reranker, limit(默认 10/上限 50), max_characters(默认 2500/上限 50000), return_raw_results, mmr_lambda, search_filters, bfs_origin_node_uuids)`。`scope` 默认 `edges`，可选 `edges|nodes|episodes|thread_summaries|observations|auto`；`reranker` 默认 `rrf`，可选 `rrf|mmr|node_distance|episode_mentions|cross_encoder`（`scope="auto"` 时被忽略）。`search_filters`（`graphiti.SearchFilters`）含 `node_labels`、`edge_types`、`property_filters`、`episode_metadata_filters`（**Experimental**），以及 `valid_at`/`invalid_at` 等二维 `DateFilter`（外 OR 内 AND，**仅 edges scope 生效**）。写入 `graph.add(user_id=|graph_id=, type="text"|"json"|"message", data, source_description, metadata, created_at, document_id)`，**≤10,000 字符**。
- **记忆层级与 v2→v3 迁移**：`thread`（消息 + `thread_summaries`）→ `user`（自动 user graph + user summary）→ `graph`（`graph_id` 共享上下文；Groups 现称 **standalone graphs**）。`memory.get(session_id)` → `thread.get_user_context(thread_id)`；`memory.add` → `thread.add_messages`；`memory.search`/`sessions.search`/`memory.search_sessions` **已移除**；`session`→`thread`、`group`→`graph`。参数名是 **`return_raw_results`（不是 `return_raw_facts`）**，仅 `scope="auto"` 可用。
- **企业能力**：account + project 隔离图数据；**RBAC 仅 Enterprise**（账户级 Account Owner/Admin/Billing Admin/Viewer/Project Creator，项目级 Project Admin/Editor/Viewer）；**ABAC** 绑定 API key 与 UserGroups；**SSO 为 OIDC**（文档未见 SAML）；**审计日志仅 Enterprise 且只记仪表盘成员操作**（`actor_email`/`actor_role`/`request_id`/`client_ip`/`user_agent`），API 侧另有 API Logs（Flex 1 天 / Flex Plus 7 天 / Enterprise 1 年）；保留策略 + legal hold。合规：**SOC 2 Type II**、**HIPAA BAA 仅 Enterprise**、BYOK/BYOM/**BYOC（部署进自有 VPC）**。
- **自托管与延迟**：**Community Edition 已废弃且不再支持**（2025-04-02 公告），`getzep/zep` 虽仍 Apache-2.0 但已转为示例用途；可自托管路径只剩 **Graphiti OSS**（无治理）或企业 **BYOC**。博客给出重构后图检索 150ms P95、上下文检索 200ms P95，但**现行文档已不承诺 P95**；**写入异步**（"Episode ingestion is asynchronous"，可能"需要几分钟"），同一图的 add 请求串行。
- **成熟度**：facts / entities / episodes / thread summaries / user summary / observations 为 **Stable**（observations 仅 Flex Plus/Enterprise）；`scope="auto"` 系列为官方推荐入口；**`episode_metadata_filters`、`detect-patterns-experimental`、`relevance` 字段为 Experimental**；文档中**已无 community 摘要类型**（疑被 observations 取代，**未确认**）。

**架构差异一句话对照**：Mem0 走"向量库 + 事实表 + 实体共现图（仅排序加权）"，**冲突时 ADD-only 并存**；Zep 走"事实即带 4 时间戳与类型名的边"的**时序知识图谱**，**冲突时作废旧边**以支持时点查询。检索上 Mem0 把语义 + BM25 + 实体三信号融合为单一 `score`；Zep 是语义 + BM25 + 图遍历，并支持 RRF/MMR/节点距离/交叉编码器重排。

---

## 7. 横向对比表

| 平台 / 框架 | 短期记忆单位 | 长期记忆类型 | 写入方式 | 检索方式 | 作用域模型 | 可自托管 |
| --- | --- | --- | --- | --- | --- | --- |
| **LangGraph + LangChain** | thread（`thread_id` + `checkpoint_ns`），super-step 粒度 | 无内置分类，`BaseStore` 中 JSON 文档自定义 | 皆可：hot path 调工具；background 独立任务 | `asearch(namespace_prefix, query, filter, limit, offset)`，语义需配 `index.embed/dims/fields` | 元组**前缀匹配**，如 `("user_id","memories")` | ✅ 完全可 |
| **LangMem SDK** | 复用 checkpointer；`SummarizationNode`+`RunningSummary` | 语义（Profile/Collection）、情景（Episode）、程序（prompt optimizer） | 皆可：`create_manage_memory_tool` / `create_memory_store_manager`+`ReflectionExecutor.submit(after_seconds=…)` | `create_search_memory_tool(query, limit, offset, filter)`、`manager.asearch(query_limit=5)` | `NamespaceTemplate`，如 `("memories","{langgraph_user_id}")` | ✅ 完全可 |
| **AWS AgentCore** | event（`memoryId`+`actorId`+`sessionId`），**不可变** | 5 策略：`SEMANTIC`/`SUMMARIZATION`/`USER_PREFERENCE`/`CUSTOM`/`EPISODIC` | **异步** Extraction→Consolidation(→Reflection)，"seconds to minutes"；可 `BatchCreateMemoryRecords`/`IngestData` 直写 | `RetrieveMemoryRecords`：`searchQuery`+`topK`(10)+`namespace(Path)`，**仅语义**；浏览用 `ListMemoryRecords` | `namespaceTemplates`（`{actorId}`/`{sessionId}`/`{memoryStrategyId}`）+ ≤5 个自定义 `namespaceKeys` | ❌ 仅托管（GA） |
| **Vertex Memory Bank** | Session（`vertex_session_source`） | LLM 抽取 + 4 托管 topic + 自定义 topic + 不可变 revision | `GenerateMemories` 可 `wait_for_completion`；`IngestEvents` 异步（约 800ms 触发） | `RetrieveMemories`+`similarity_search_params{search_query, top_k=3}`（欧氏）+`filter_groups`；省略即全量 | `scope` 任意 KV **精确匹配**且不可变；ADK 自动传 `{"app_name","user_id"}` | ❌ 仅托管（GA） |
| **Azure AI Foundry** | thread + store 内 scope 分区 | `MemoryItem.kind ∈ {user_profile, chat_summary, procedural}` | **异步/去抖动**：`update_delay` 按不活动计时（默认 300 秒），`0` 才即时 | `POST ...:search_memories`（只给 `scope`→profile；给 `items`→profile+summary），`options.max_memories` | store（项目级）→ `scope`，`"{{$userId}}"` 自动解析 | ❌ 仅托管（**Preview**） |
| **Redis Agent Memory** | session（Redis JSON：`messages`/`context`/`memories`/`data`） | `semantic`/`episodic`/`message`（托管版加 `session_summary_view`） | 会话层同步；长期记忆由后台 task-worker 抽取 | `POST /v1/long-term-memory/search`：`distance_threshold`、`search_mode`、`recency_*` | `namespace`/`user_id`/`session_id`/`topics`/`entities`/`memory_type`；托管版 `sessionId`/`actorId`/`ownerId` | ⚠️ V0 可 Docker，**已停维护** |
| **MongoDB Atlas** | `MongoDBSaver` | `MongoDBStore` 内 JSON + 向量索引 | 同步 | `$vectorSearch`：`index`/`path`/`queryVector`/`limit`+`numCandidates`+`filter` | 元组命名空间 + `key`（同 LangGraph） | ✅ 完全可 |
| **Memobase** | blob buffer（flush 间隔 3600 秒） | **Profile**（`topic`/`sub_topic`）+ **Event 时间线** | **异步**：blob → buffer flush 时同批产出 | `GET /api/v1/users/profile|event/{user_id}`；`enable_event_embedding` | `user_id` + profile 槽位 | ✅ Docker |
| **Supermemory** | 无明显短期层（`containerTag` 的 space） | chunks / Memories（图）/ Profile（`static`+`dynamic`） | 同步写入 + 后台 `dreaming` | `POST /v4/search`：`searchMode`、`threshold`、`rerank`、`include.*` | 单数 `containerTag` | ⚠️ `supermemory local` 开源（本地） |
| **MemU** | 无 | `Resource`/`RecallFile`/`RecallFileSegment`（`track ∈ memory|skill`） | 同步（`commit_results`） | `progressive_retrieve` + 向量 | 本地 workspace / RecallFile 路径 | ✅ SQLite/PG |
| **Mem0 Platform** | `run_id` | 事实记忆（实体图**仅用于排序**）；OSS 无图记忆 | **异步**：`infer=True` 返回 `{event_id, status:"PENDING"}` 需轮询；`infer=False` 同步 | `POST /v3/memories/search/`：语义+BM25+实体融合为单一 `score` | `user_id`/`agent_id`/`run_id`/`app_id`，**必须在 `filters` 内** | ⚠️ OSS 子集 |
| **Zep Cloud** | thread（消息 + `thread_summaries`） | 时序知识图谱：edges（事实 + 4 时间戳）/ nodes / episodes / observations / user summary | **异步**：摄取可能"需几分钟"，同图串行 | `graph.search(user_id=\|graph_id=, query, scope, reranker, limit, max_characters, search_filters)` | `user_id`（user graph）/`graph_id`/`thread_id`；account+project 隔离 | ⚠️ CE 已废弃；仅 Graphiti OSS 或 BYOC |

### 7.1 选型要点

1. **"同一轮对话就能用上"→ 选同步或可等待的写路径**：AgentCore 与 Zep 的异步抽取有秒~分钟窗口；Mem0 需轮询 `event_id`；Azure 需 `update_delay: 0`；Vertex 可 `wait_for_completion`。
2. **多租户隔离能力差距极大**：AgentCore 用 IAM 条件键 + 命名空间变量最严谨（但**变量缺失会静默不提取**）；Zep 有 Enterprise RBAC/ABAC/OIDC/审计；Mem0 只有项目级两角色。
3. **冲突策略是哲学分歧**：Vertex 与 Zep **作废/删除旧记忆**并保留 revision 或时间戳历史；Mem0 改为 **ADD-only**，把"哪个是当前事实"交给检索排序。
4. **框架自由度 vs 托管能力**：可完全自托管且不牺牲语义检索的组合只有 **LangGraph/LangMem + Postgres/MongoDB/Redis**；云厂商方案全部不可自托管，其中 Azure **不支持 VNet**、Vertex `global` 端点**不支持 CMEK**。

---

## 8. 未确认事项汇总

- **AWS**：`eventExpiryDuration` 下限（API 3 天 vs 配额表 7 天）；检索与短期存储单价；CrewAI 集成。
- **Vertex**：`simple_retrieval_params` 的替代语义；community 摘要是否仍生成；REST 参考仍挂 `v1beta1` 却宣传 GA。
- **Azure**：`chat_summary_memory`/`user_profile_memory` 工具变体、`max_chat_summary_memories`、`user_profile_fields`、默认 store 命名、`2025-05-15-preview` 支持状态、单价。
- **Redis**：`window_size`、自动晋升、`EmbeddedJSON`、`similarity_threshold`、`POST /v1/working-memory/{session_id}/memory`、MCP `get_working_memory` —— 官方文档中**均不存在**。
- **其他**：MongoDB 是否可称 GA；Memobase `slots` 配置键；Supermemory `x-sm-user-id`/`includeSummary`；Mem0 的 SOC 2/HIPAA/数据驻留主张、审计字段 schema、月费与 credit 定义；Zep SAML（文档仅 OIDC）。

> 所有非显然结论均附官方链接（见各节内联 Markdown 链接）；凡与常见二手说法冲突处均以官方文档为准并标注"未确认"。厂商文档变动频繁（Google 2026-04-22 品牌改名、Mem0 v2→v3、Supermemory v3→v4、Zep v2→v3、Redis 仓库拆分），落地前请复核当前 SDK 版本。

### 参考来源（官方）

- LangChain / LangGraph：[persistence](https://docs.langchain.com/oss/python/langgraph/persistence)、[checkpointers](https://docs.langchain.com/oss/python/langgraph/checkpointers)、[stores](https://docs.langchain.com/oss/python/langgraph/stores)、[memory concepts](https://docs.langchain.com/oss/python/concepts/memory)、[short-term memory](https://docs.langchain.com/oss/python/langchain/short-term-memory)、[built-in middleware](https://docs.langchain.com/oss/python/langchain/middleware/built-in)、[Anthropic middleware](https://docs.langchain.com/oss/python/integrations/middleware/anthropic)、[Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)、[trim_messages](https://reference.langchain.com/python/langchain-core/messages/utils/trim_messages)
- LangMem：[reference](https://langchain-ai.github.io/langmem/reference/)、[memory](https://langchain-ai.github.io/langmem/reference/memory/)、[tools](https://langchain-ai.github.io/langmem/reference/tools/)、[prompt optimization](https://langchain-ai.github.io/langmem/reference/prompt_optimization/)、[short term](https://langchain-ai.github.io/langmem/reference/short_term/)、[utils](https://langchain-ai.github.io/langmem/reference/utils/)、[conceptual guide](https://langchain-ai.github.io/langmem/concepts/conceptual_guide/)、[delayed processing](https://langchain-ai.github.io/langmem/guides/delayed_processing/)
- AWS：[AgentCore Memory](https://docs.aws.amazon.com/bedrock-agentcore/latest/devguide/memory.html)、[CreateEvent](https://docs.aws.amazon.com/bedrock-agentcore/latest/APIReference/API_CreateEvent.html)、[RetrieveMemoryRecords](https://docs.aws.amazon.com/bedrock-agentcore/latest/APIReference/API_RetrieveMemoryRecords.html)、[limits](https://docs.aws.amazon.com/bedrock-agentcore/latest/devguide/bedrock-agentcore-limits.html)、[regions](https://docs.aws.amazon.com/bedrock-agentcore/latest/devguide/agentcore-regions.html)、[pricing](https://aws.amazon.com/bedrock/agentcore/pricing/)
- Google：[release notes](https://docs.cloud.google.com/gemini-enterprise-agent-platform/release-notes)、[generate memories](https://docs.cloud.google.com/gemini-enterprise-agent-platform/scale/memory-bank/generate-memories)、[fetch memories](https://docs.cloud.google.com/gemini-enterprise-agent-platform/scale/memory-bank/fetch-memories)、[ADK memory](https://adk.dev/sessions/memory/)
- Azure：[what is memory](https://learn.microsoft.com/en-us/azure/foundry/agents/concepts/what-is-memory)、[memory usage](https://learn.microsoft.com/en-us/azure/foundry/agents/how-to/memory-usage)
- Redis：[agent-memory-server](https://github.com/redis/agent-memory-server)、[V0 docs](https://github.com/redis/agent-memory-server/tree/main/V0/docs)、[托管 overview](https://redis.io/docs/latest/develop/ai/context-engine/agent-memory/overview/)
- MongoDB：[AI agents 蓝图](https://www.mongodb.com/docs/vector-search/about/ai-agents/)、[vector search index](https://www.mongodb.com/docs/vector-search/indexes/vector-search-type/)、[$vectorSearch](https://www.mongodb.com/docs/vector-search/query/aggregation-stages/vector-search-stage/)
- Memobase：[docs](https://docs.memobase.io/llms.txt)、[local config](https://docs.memobase.io/references/local_config)
- Supermemory：[how it works](https://supermemory.ai/docs/concepts/how-it-works.md)、[graph memory](https://supermemory.ai/docs/concepts/graph-memory.md)、[self-hosting](https://supermemory.ai/docs/self-hosting/overview.md)
- MemU：[ADR 0006](https://github.com/NevaMind-AI/memU/blob/main/docs/adr/0006-from-memory-item-category-to-tracked-workspace-memorization.md)
- Mem0：[quickstart](https://docs.mem0.ai/open-source/python-quickstart)、[graph memory](https://docs.mem0.ai/platform/features/graph-memory)、[memory filters](https://docs.mem0.ai/platform/features/v2-memory-filters)、[platform vs OSS](https://docs.mem0.ai/platform/platform-vs-oss)、[migration](https://docs.mem0.ai/migration/oss-v2-to-v3)
- Zep：[graph overview](https://help.getzep.com/graph-overview)、[searching the graph](https://help.getzep.com/searching-the-graph)、[facts](https://help.getzep.com/v3/facts)、[context types](https://help.getzep.com/context-types)、[RBAC](https://help.getzep.com/role-based-access-control)、[audit logging](https://help.getzep.com/audit-logging)、[deprecation wave](https://help.getzep.com/february-2026-deprecation-wave)、[pricing](https://www.getzep.com/pricing)
