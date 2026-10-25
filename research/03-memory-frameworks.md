# 开源 Agent 记忆框架架构调研：Letta / Mem0 / Zep(Graphiti) / 补充

> 拆解各框架的**分层 → 每层存什么 → 写入/读取触发时机 → 数据结构**。
> **证据分级**：📄 论文｜📘 官方文档/博客/源码｜⚠️ **厂商自测 benchmark**（未经第三方复现，**不可跨厂商横比**）。
> 调研于 2026 年；此类框架迭代极快，请以引用链接当期版本为准。

---

## A. Letta（原 MemGPT）

### A.1 原始划分：main vs external context 📄

MemGPT 论文（arXiv:2310.08560）以 OS 虚拟内存为隐喻：上下文窗口当 RAM、外部存储当磁盘，agent 用函数调用换页。论文原文：prompt 分三段连续区。

| 层 | 子结构 | 存什么 |
|---|---|---|
| **main context**（prompt 内） | system instructions | 控制流、工具用法，**只读** |
| | working context（core memory） | 关键事实、偏好、persona，agent 可写 |
| | FIFO queue | 滚动消息历史 + 函数输入输出；**队首存被驱逐消息的递归摘要** |
| **external context**（prompt 外） | recall storage | 完整对话消息库，函数检索、分页 |
| | archival storage | 文档/上传文本库，函数检索、分页 |

触发时机：**用户消息、系统消息（容量告警）、定时事件**均触发一轮推理。**心跳**：工具带 `request_heartbeat: bool`（默认 False）——置 true 则把输出加入 main context 并立即再推理，不置则视为 **yield** 停下等下一事件，即**函数链**。⚠️ 该参数的**定义不在论文正文**，权威措辞来自作者随论文发布的函数规格（数据集 `MemGPT/function-call-traces`），且**各函数措辞不统一**（部分函数用短版"use to chain multiple functions"）。
⚠️ **论文版本陷阱**：v1 §2.1 三段名为 "system instructions / **conversational context** / working context"（明确 working context 可被 LLM 经函数写入，system instructions 为 pin 住的只读段）；v2 才改为 "system instructions / working context / **FIFO Queue**"。v1 图里的 `working_context_append` / `.replace` 亦在 v2 才改名 `core_memory_append` / `core_memory_replace`。另：2023 年召回函数名为 `recall_memory_search`，`conversation_search` 是当时多轮对话（MSC）变体，二者同在一份 schema 里按配置选用。
> **演进——已正式废弃** 📘：新架构 `letta_v1_agent`（2025-10 博客）明确"**heartbeats 与 `send_message` 已废弃，仅支持模型原生推理与直接助手消息生成**"。替代物是**三件套**（源码 SHA `67013ef1`）：① **"调用工具即继续"循环**——`_decide_continuation` 文档串："没调工具？循环结束。调了工具？循环继续"；② `DEFAULT_MAX_STEPS = 50` 兜底；③ ⭐ **ToolRule 系统**——`create_agent_async` 自动为 `send_message`/`memory_finish_edits` 挂 **`TerminalToolRule`**（终止），为 `BASE_TOOLS`/`BASE_MEMORY_TOOLS*`/`BASE_SLEEPTIME_TOOLS` 挂 **`ContinueToolRule`**（继续）。**`ContinueToolRule` 就是 `request_heartbeat=True` 的结构化等价物**，而 `TerminalToolRule` 取代了"send_message 后不再继续"。`_decide_continuation` 已**完全没有 `request_heartbeat` 形参**，三个 provider 都主动剥除该参数。

### A.2 现代 Letta 的分层 📘

⚠️ `docs.letta.com/v1-sdk/*` 各页 frontmatter 均为 **`status: legacy`**。当前权威是 **Context Constitution**：顶层**两层**——**上下文窗口**（含**四个 Context Primitive**：System Prompt / Tools / Messages / Skills）与 **external context**。**recall storage 已不再是独立层**，由 ① compaction → 递归摘要、② 工具检索取代。

legacy 选层表仍有工程参考价值：

| 抽象 | 可编辑 | 常驻上下文 | 工具 | 单条上限 | 数量上限 |
|---|---|---|---|---|---|
| **Memory Blocks** | ✅（可设只读） | ✅ | `memory_rethink` `memory_replace` `memory_insert` | 建议 <50k 字符 | 建议 <20 个/agent |
| **Files** | 只读 | 部分（可 open/close） | `open` `close` `semantic_search` `grep` | 5MB | 建议 <100 个 |
| **Archival** | 读写 | ❌ | `archival_memory_insert` `archival_memory_search` | 300 tokens | 无限 |
| **External RAG** | 读写 | ❌ | 自定义工具 / MCP | 无限 | 无限 |

**工具演进（源码 `letta/constants.py`）** 📘：V2 = `["memory_replace","memory_insert"]` **取代**论文的 `core_memory_append`/`core_memory_replace`；V3 收敛为单一 omni-tool **`memory`**（带 `command=`）。⚠️ **只有 archival 工具**带显式 `DEPRECATED_LETTA_TOOLS` 常量。`conversation_search` 属当前 `BASE_TOOLS`。

### A.3 memory blocks 作为一等抽象 📘

block = `label` + `description` + `value` + `limit`（字符数）。
- **`description` 是给 agent 看的用法说明**，是 agent 判断"该写什么"的主要依据。
- **常驻实现**：block 以 XML **prepend 进 prompt**；不变则前缀稳定，无检索开销。
- **可共享**：独立持久化（`block_id`），经 `agents.blocks.attach()` **挂到多个 agent**（"更新一次、处处可见"）；`detach` 只移出上下文、不删 block，`delete` 从所有 agent 移除。
- **只读 block**（`read_only=True`）：适合下发政策、跨 agent 共享规范。
- ⚠️ **并发**：`blocks.update(value=...)` 是**整体替换**（非 append），多写者 **last-write-wins**——官方明确提醒会丢数据。
- **archival 存储** 📘：论文期为 **PostgreSQL + pgvector + HNSW**（embedding `text-embedding-ada-002`）；现为 `NATIVE | TPUF | PINECONE`，**按 archive 配置**。`NATIVE` 在 Postgres 上解析为 pgvector、SQLite 上为 sqlite-vec，故**开箱默认实为 SQLite**；Turbopuffer 需三条件同时满足且是**双写迁移**；`PINECONE` 从 archive 创建路径**不可达**。默认 archive **惰性自动创建**；`ArchivalPassage` 按 **`archive_id`（非 `agent_id`）** 归属——MemGPT 的"每 agent 独立 archival"已不存在。attach/detach 存在（`PATCH /v1/agents/{id}/archives/attach/{archive_id}`），但每 agent 限一个 archive。
- ⚠️ **archival 插入不切块**（SHA `67013ef1`）：`insert_passage` 把 `embedding_chunk_size` 读进局部变量后**从未使用**，紧邻 `text_chunks = [text]` 与 TODO 注释——即**一次插入 = 一个 passage = 一次 embedding**。真正的切块器 `LineChunker` 只用于 **files**。`archival_memory_search` 实参比 legacy 文档更丰富：`tag_match_mode: "any"|"all"`、`top_k`（默认 10）、日期范围；`conversation_search` 是**混合检索**（"hybrid search (text + semantic similarity)"）带 role/日期过滤，**并非纯向量**。

### A.4 sleep-time compute → 后台"做梦" 📄📘

论文（arXiv:2504.13171）把推理由"用户等待的 test time"挪到"空闲 sleep time"。⚠️ 论文术语是 "**re-represented context**"（c′），"learned context" 是**博客用词**。摘要报告：等精度下 test-time 计算**降约 5×**；扩 sleep-time compute 带来 **+13% / +18%** 准确率；按每 context 10 次查询摊销，平均**每查询成本降 2.5×**；并注明 **o1 收益有限**。

实现 📘（0.7.0，博客原文）："Letta **实际在底层创建两个 agent：primary 与 sleep-time**……**primary 不获得编辑其 core memory 的工具**……**这些工具挂给 sleep-time agent**，后者可管理 primary 的 in-context memory **以及它自己的**。"
⭐ **结构性证明**（源码 SHA `67013ef1`）：`AgentManager.create_agent_async` 中 `elif agent_create.enable_sleeptime:` 分支位于 memgpt_v2 / letta_v1 / catch-all 分支**之上**，故该路径**根本不会调用 `calculate_base_tools`**——`enable_sleeptime=True` 的 primary agent 恰好只拿到 `["send_message","conversation_search","archival_memory_search"]`，**完全拿不到 `core_memory_*` 与 `memory_*` 工具**。能力是**转移（transfer），不是复制**；且无运行期"剥离"变更，纯静态 if/elif。memory blocks 在两者间**共享**（`attach_block_async` 注释："For sleeptime agents, also attaches to paired agents in the same group"），sleep-time agent 的 boot sequence 为空。`memory_finish_edits` 被设为 **terminal**，这正是 sleep-time 编辑会话的结束方式。
这解决论文三痛点：记忆操作不再阻塞对话；记忆/业务工具混用的可靠性问题；论文式**增量**记忆易长成"脏乱差"，后台 agent 可反复精炼。两者可用**不同模型**与**不同频率**。
⚠️ **归属纠正**：论文全文**从未出现 "sleep-time agent"**，也**未规定谁持有记忆编辑工具**（附录只说 "the sleep-time compute process" / "the answer agent"）——该"双 agent 工具切分"是**博客 + 代码的设计，不是论文结论**。⚠️ 工具名跨三份材料不一致：论文附录为 `rethink_memory`/`finish_rethinking`，实验代码为同名，**产品发布版**才是 `memory_rethink`/`memory_finish_edits`。附录另述：模型"可调用 `rethink_memory` **至多 10 次**"，c′ 存于**两个 agent 共享**的 `rethink_memory_block`（`limit=5000`）。
⚠️ **配置名纠正**：字段是 **`enable_sleeptime: Optional[bool]`**（**无 `sleeptime_agent` 字段**）；`sleeptime_agent_frequency` 挂在 **GROUP** 上，不在 create/update-agent 请求体。

> **2026 演进** 📘：更名 **"dreaming"**，配置 `dreaming: {trigger: "off"|"step-count"|"compaction-event", behavior: "reminder"|"auto-launch", stepCount: 25}`，并叠加 **MemFS**——**git-backed 记忆文件系统**。记忆不再是 block，而是 `$MEMORY_DIR` 下 Markdown 文件树：`system/` 下**每轮全量进 system prompt**，其外**按需读取**，**文件树本身常驻 prompt** 作导航路标（progressive disclosure）。每次编辑产生 git commit；后台记忆子 agent 用 **git worktree** 并发写入再合并；defrag 重组为 **15–25 个聚焦文件**。⚠️ MemFS **默认不含语义/向量索引**。

### A.5 prompt 缓存友好性 📘

**机制**（源码 SHA `67013ef1`，`letta/llm_api/anthropic_client.py`）：恰好 **3 个无条件** `{"type":"ephemeral"}` 断点——`tools[-1]`、system block、末条消息的最后一个内容块。**无 `ttl` 字段**，仅用 Anthropic 默认 5 分钟 TTL。另有一条**真正为缓存优化**的路径：Opus 4.5 + reasoning 时发送 `context_management.edits = [{"type":"clear_thinking_20251015","keep":"all"}]`，注释为 "**Preserve all thinking blocks for maximum cache performance**"。provider 矩阵：Bedrock 继承同样 3 个断点；**OpenAI 端零缓存代码**；Gemini 只做 `cached_content_token_count` 读回。
⚠️ **不存在 `enable_prompt_caching` / `prompt_cache` 开关**（已 grep，零匹配）；缓存**按 provider 隐式**生效。`tests/test_prompt_caching.py` 记录门槛：OpenAI ≥1024 tokens 且 `% 128 == 0`；Anthropic Sonnet 4.5 ≥1024、Opus/Haiku 4.5 ≥**4096**；Gemini 2.5 Flash ≥1024、Pro ≥4096；命中下限断言 `>= 0.15`（而非某处陈旧注释所称 ">50%"）。
**失效根因**（Letta 自家测试）：**system prompt 里的 "Memory blocks were last modified" 时间戳**——测试 diff 真实 trace 后记录 "⚠️ TIMESTAMP IS CHANGING IN ACTUAL REQUESTS! → This is the root cause of cache misses"。
**官方指导**（Context Constitution 原文）："**把频繁更新的记忆放在 system prompt 靠后的位置以最小化缓存失效**"，维持"**稳定、压缩、只在学习真正持久时才变化**的 system prompt"。

> ⚠️ **文档缺口**：流行的"**agent 状态即 prompt**"**不能归因于 Letta**——其五篇 memory/context 博客经两次独立抽取均为字节一致，其中 "prompt caching"/"cache"/"stable prefix" **零出现**；Letta **没有任何 prompt caching 官方文档**。上述证据仅为**源码 + 测试**，另有一条**非官方**论坛回复（@ezra，2025-12）。

---

## B. Mem0 / Mem0^g

### B.1 论文原设计：抽取 + 更新两阶段 📄

论文（arXiv:2504.19413）把记忆做成**逐消息对的增量流水线**：
- **写入**：新消息对 `(m_{t-1}, m_t)` + 上下文（异步刷新的摘要 `S` + 近邻窗口，论文 `m=10`）→ LLM **抽取**候选记忆 `Ω` → 对**每条候选**取 top-`s`（论文 `s=10`）语义相似历史记忆 → LLM 经 **function calling 选择 ADD / UPDATE / DELETE / NOOP**。**两趟 LLM 调用**。
- **读取**：检索相关记忆，用强调**时间戳、按新近性消解矛盾**的 prompt 作答。
- **Mem0^g**：单元为**实体节点 + 带时间戳与 embedding 的三元组**（Neo4j）；矛盾时**"失效"过期边而非删除**；检索走"实体锚定子图扩展 + 三元组语义匹配"。
- 📄 自陈短板：**DELETE 是硬删除**（丢历史）、无 decay/TTL、无写入期投毒威胁模型、竞品基线难复现。

### B.2 2026 重大转向：单趟 ADD-only 📘⚠️

**本调研最重要的时效性发现**：Mem0 已重写抽取层，架构假设与论文**相反**。

| 维度 | 旧（论文 / v2） | 新（v3，当前） |
|---|---|---|
| 抽取 | **两趟** LLM（抽取 + 合并） | **单趟 ADD-only**（一次调用） |
| 变更操作 | ADD / UPDATE / DELETE | **仅 ADD**，不覆盖、不删除 |
| 冲突处理 | 写入期合并/删除 | **移到检索期排序**——新旧并存，靠多信号把"当前正确"排前 |
| 图记忆 | 外部图库 + 手工配置 | **内置、自动、常开**；**OSS 已彻底移除**图存储（删约 4000 行驱动） |
| 检索 | 纯语义（向量） | **混合多信号** |

📘 新写入（6 阶段）：① 对话**异步**入库（在 agent 回复之后）→ ② **Context Lookup** 取 top-10 旧记忆作去重上下文 → ③ 单趟 LLM **蒸馏**出 ADD-only 事实 → ④ **MD5 哈希去重** + 批量 embed → ⑤ **实体链接**（抽专有名词、引号文本、复合名词短语，跨记忆连成图）→ ⑥ **时序推理**独立异步 pass（事件何时发生 / 进行中还是已完成 / 时间精度 / 记忆类型）。
📘 读取为**四信号并行打分再融合**：语义 + BM25（含词形还原）+ 实体匹配增益 + 时序匹配；融合分即 `score`。**关键约束：BM25 与实体只是 boost、不扩展候选集**——只有语义检索结果是候选，故**只改排序、不扩召回**。
OSS v3 默认值 📘：`top_k` 100→**20**；`threshold` 无→**0.1**；`rerank` True→**False**（开启 +200–400ms）。`user_id`/`agent_id`/`run_id` **移入 `filters` dict**（顶层传会报错）。

### B.3 存储分层与作用域 📘

| 存储 | 存什么 | 用途 |
|---|---|---|
| **SQL DB** | 事实 + 元数据；history log（ADD 事件）+ 滚动消息窗口 | 真值来源；审计追溯 + 抽取去重上下文 |
| **Vector DB** | 记忆文本、embedding、元数据（时间戳、hash、categories、attributed_to） | 主事实存储 + 语义检索 |
| **Graph / Entity Store** | 实体 + embedding + 关联 memory id | 跨记忆图连接 + 实体检索增益 |

作用域为 `user_id`/`agent_id`/`run_id` **多级过滤** + 自定义 `metadata`。官方强调**搜索必须带作用域过滤**，否则跨用户/agent/会话记忆互相污染。

### B.4 Benchmark（⚠️ 厂商自测，须区分论文版与平台版）

📄 **论文版**（LOCOMO）：LLM-as-a-Judge 相对 **OpenAI 提升 26%**（相对值）；Mem0^g 再高约 **2%**；相对 full-context **p95 延迟降 91%**、**token 省 >90%**。绝对分 `J` ≈ 66.9%（Mem0）/ 68.4%（Mem0^g）vs 52.9%（OpenAI）vs 66.0%（Zep）；total p95 ≈ 1.44s / 2.59s vs 17.12s（full-context）。

⚠️ **平台 v3 自测**（口径已变，**不可与论文数字续接**）：LoCoMo **71.4 → 91.6（+20.2）**、LongMemEval **67.8 → 93.4（+25.6）**，抽取延迟 p50 **2.0s → 1.0s**。
⚠️ **文档最新自测**（top_200 预算）：LoCoMo **92.5**、LongMemEval **94.4**、BEAM(1M) **64.1**、BEAM(10M) **48.6**；平均 **6.8–7.0K tokens/query**。
⚠️ 官方两项保留：① 分数反映**托管平台**，含 OSS 不具备的专有优化；② 带 **±1 分**置信区间。官方亦主动指出 BEAM@10M 上 temporal reasoning(16.3)、event ordering(20.2)、multi-session(26.1) 是**全领域未解问题**；ADD-only 下 knowledge-update(93.6) 最难，因旧事实不被覆盖。

### B.5 与 OpenAI / Anthropic 的差异

| 维度 | Mem0 | OpenAI（ChatGPT） | Anthropic（Claude 平台） |
|---|---|---|---|
| 定位 | **独立记忆层**，插在应用与模型之间 | **产品内建**、闭源 | **原语提供方**，不做托管记忆服务 |
| 记忆形态 | 抽取后的**事实**（非逐字稿），存向量/图/SQL | 「已保存记忆」显式列表 + 「引用历史对话」隐式检索；已引入后台 **dreaming** | **文件目录** `/memories`，agent 用 `view`/`create`/`str_replace` 读写 |
| 存储归属 | Mem0（云）或你的向量库（OSS） | OpenAI | **你的应用**——memory tool 是 **client-side**，Claude 只发请求、你的代码落盘 |
| 检索触发 | 应用显式 `search()` 后拼 prompt | 模型侧自动 | **just-in-time**：任务前查目录，需要时再读文件 |
| 上下文治理 | 靠检索排序（v3 无写入期删除） | 产品侧不透明 | `clear_tool_uses_20250919` 清旧工具结果、`clear_thinking_20251015` 清 thinking block + 服务端 compaction |
| 缓存意识 | 未列为一等公开议题 | — | **显式**：清工具结果会**使缓存前缀失效**，故提供 `clear_at_least` 保证单次清够 token 才划算；保留 thinking 则缓存可复用 |

一句话：**Mem0 卖"记忆流水线"，OpenAI 卖"开箱记忆产品"，Anthropic 卖"记忆与上下文的原语 + 缓存语义"**。

---

## C. Zep / Graphiti

### C.1 三层子图 📄

`G = (N, E, φ)`，三个层级子图：

| 子图 | 节点 → 边 | 存什么 |
|---|---|---|
| **Episode** `G_e` | episode → entity | **原始输入** message / text / JSON。**non-lossy** 真值流，是抽取来源与溯源依据 |
| **Semantic Entity** `G_s` | entity → entity | 抽取并与既有实体**消解**后的实体；边即**事实**，带有效期 |
| **Community** `G_c` | community → entity | **强连通实体聚类** + 该簇高层摘要；名字由关键词构成并 embed 供检索 |

物理 schema（代码）：`(:Episodic)-[:MENTIONS]->(:Entity)`、`(:Entity)-[:RELATES_TO]->(:Entity)`、`(:Community)-[:HAS_MEMBER]->(:Entity)`；另有**晚于论文新增**的 `Saga` 层。`group_id` 作分区。论文所称 hyper-edge 在代码里只是**多条两两边**。
渊源 📄：episode/semantic 对应**情景记忆 vs 语义记忆**；community 层借鉴 GraphRAG，但**用 label propagation 而非 Leiden**——因其有朴素的**动态扩展**（新节点按邻居社区**多数票**归入，平票归最大社区）。⚠️ **代码侧 `build_communities()` 先 `remove_communities()` 再重建——社区层刷新是"整组删除+重建"，本质非增量**；仅当 `update_communities=True`（**默认 False**）才做逐节点增量。⚠️ Graphiti 自家 communities 文档写 "Leiden"，与论文及代码**自相矛盾**（以代码为准）。

### C.2 bi-temporal 模型与边失效 📄

**两条时间轴、四个时间戳，都存在边上**：

| 时间轴 | 字段 | 含义 |
|---|---|---|
| **T（valid time，世界时间）** | `t_valid` | 该事实在现实中**开始为真**的时刻 |
| | `t_invalid` | 该事实**不再为真**的时刻 |
| **T′（ingestion，摄取时间）** | `t'_created` | Zep **学到**该事实的时刻 |
| | `t'_expired` | Zep **使其失效**的时刻 |

官方举例：`created_at` = "Zep 得知用户结婚的时间"；`valid_at` = "用户结婚的时间"；`invalid_at` = "用户离婚的时间"；`expired_at` = "Zep 得知用户离婚的时间"。
**边失效**：新边写入时 LLM 与**语义相关既有边**比对识别矛盾；发现**时间上重叠的矛盾**时，把旧边 `t_invalid` **设为新边的 `t_valid`**——"旧事实到该时点为止为真"，**而非删除**。按 T′ 轴**新信息永远优先**。⚠️ 代码侧还会同时设 `expired_at = now`，且反向也可**让新边失效**（论文只提到设 `t_invalid`）。仅 `valid_at`/`invalid_at` 渲染进上下文，格式 `FACT (Date range: from - to)`。支持时点过滤 `SearchFilters`（`valid_at`/`invalid_at`/`created_at`/`expired_at`，外层 OR、内层 AND，**仅作用于边**）。

### C.3 写入路径 📄

1. **取前序 episode 上下文** → 2. **建 `EpisodicNode`** → 3. **实体抽取**（LLM）：**说话者自动作第一个节点**，用 reflexion 式**反思**降幻觉、提覆盖，同时抽实体摘要 → 4. **实体消解**：实体名 embed（**1024 维**）→ **cosine 搜候选**（`NODE_DEDUP_COSINE_MIN_SCORE = 0.6`、上限 `15`），叠**确定性匹配**（归一化同名必试；MinHash/LSH 3-gram、32 置换、4 bands、Jaccard ≥ `0.9`；熵门 `1.5`），未决者交 LLM → 5. **事实抽取**（LLM）→ 6. **边去重**：**搜索空间约束为"同一实体对之间的边"**，既防跨实体误合并又大幅降计算量 → 7. 再做一次**无约束**混合检索取**矛盾候选** → 8. **边消解**（LLM 判重复/矛盾）→ 9. **时序抽取**（小模型，ISO-8601；现在时用 `t_ref`；"不要从相关事件推断日期"）→ 10. **失效**（见 C.2）→ 11. **节点属性 + 摘要**（30 个/批）→ 12. 落库（`resolved_edges + invalidated_edges`，**无硬删**）。
📄 落库用**预定义 Cypher** 而非 LLM 生成查询，以保证 schema 一致、减少幻觉。并发由 `SEMAPHORE_LIMIT` 控制（**默认 10**）。
⚠️ **源间冲突**：论文说上下文窗口取 `n=4` 条，`add_episode` 传 10，底层 `EPISODE_WINDOW_LEN = 3`；论文说消解候选 = cosine **+ 全文检索**，代码实为 **cosine only**；论文/文档说 3 种 episode 类型，代码枚举有 **4** 种（多 `fact_triple`）。

### C.4 检索路径 📄

形式化为 `f(α) = χ(ρ(φ(α))) = β`——**Search → Reranker → Constructor**：

| Search（φ） | 相似性类型 | 检索字段 |
|---|---|---|
| `φ_cos` cosine 语义 | 语义相似 | edge→fact；entity→name；community→name |
| `φ_bm25` Okapi BM25 全文 | 词面相似 | 同上 |
| `φ_bfs` 广度优先图遍历 | **上下文相似** | n-hop 内额外节点与边；**可接受节点作种子**——用最近 episode 作种子可把"刚提到的实体与关系"拉进上下文 |

**Reranker（ρ）**：论文支持 **RRF** 与 **MMR**；另有 **episode-mentions**（按提及频次排序，让高频信息更易获取）、**node distance**（按到**中心节点**的图距离排序，局部化上下文）与 **cross-encoder**（最贵）。各方法并行取 `2*limit` 候选，重排后截断到 `limit`；`node_distance`/`cross_encoder` **预置 RRF 种子**。RRF 代码实现为 `1/(rank+1)`。
**Constructor（χ）**：edge → fact + 有效期；entity → name + summary；community → summary。
📄 实验配置：**top 20** 边与实体节点；embedding 与 rerank 均为 **BAAI BGE-m3**；图构建 `gpt-4o-mini-2024-07-18`。

> **工程侧精确参数** 📘（**纠正一个常见误解**）：所谓 `search_type` **并非 API 参数**——`*_HYBRID_SEARCH_*` 是 **Python 常量（recipes）**，graphiti-core 无 `search_type` 形参；Zep 托管 API 用 **`scope`**（默认 `"edges"`）+ **`reranker`**（默认 `"rrf"`）。默认值：`sim_min_score = 0.6`、`mmr_lambda = 0.5`、**`bfs_max_depth = 3`**、`limit = 10`、`reranker_min_score = 0`。
> **入口默认**：`graphiti.search()` → `EDGE_HYBRID_SEARCH_RRF`（bm25 + cosine，**无 BFS、无 LLM 重排**）；`search_()` → `COMBINED_HYBRID_SEARCH_CROSS_ENCODER`（含 BFS）。**只有 3 个 recipe 含 BFS**，RRF 默认配方**不含**。⚠️ `bfs_max_depth` 仅存在于 graphiti-core，**托管 API 未公开**该参数。
> ⚠️ `OpenAIRerankerClient`（默认 `gpt-4.1-nano`，`max_tokens=1` + `logit_bias` + logprobs 做二分类）实为 **LLM-as-classifier**，并非论文 §3.2 所称 "cross-attention" 的真正 cross-encoder。⚠️ 官方文档自相矛盾：一面称混合检索 "no LLM-in-the-loop reranking"，一面把 cross-encoder 设为 `search_()` 默认。

### C.5 Benchmark（⚠️ 全部为厂商/作者自测）

📄 论文自报，实验期 2024-12 至 2025-01。

**DMR**——500 段多轮对话，每段仅 60 条消息：

| Memory | Model | Score |
|---|---|---|
| Recursive Summarization † | gpt-4-turbo | 35.3% |
| Conversation Summaries | gpt-4-turbo | 78.6% |
| **MemGPT †** | gpt-4-turbo | **93.4%** |
| Full-conversation | gpt-4-turbo | 94.4% |
| **Zep** | gpt-4-turbo | **94.8%** |
| Full-conversation | gpt-4o-mini | 98.0% |
| **Zep** | gpt-4o-mini | **98.2%** |

† 引自 MemGPT 论文。
> ⚠️ **作者的诚实提醒**：Zep(94.8%) 仅比 MemGPT(93.4%) 高 **1.4 pp**，比**平凡的 full-context 基线**(94.4%) 只高 **0.4 pp**（gpt-4o-mini 上仅 **+0.2 pp**）。论文直言该基准**设计有缺陷**——只有单轮事实检索题、措辞歧义、60 条消息塞得进现代窗口，"不足以评估记忆系统"。

**LongMemEval**——对话平均约 **115,000 tokens**：

| Memory | Model | Score | Latency | Latency IQR | Avg Context Tokens |
|---|---|---|---|---|---|
| Full-context | gpt-4o-mini | 55.4% | 31.3 s | 8.76 s | 115k |
| **Zep** | gpt-4o-mini | **63.8%** | **3.20 s** | 1.31 s | **1.6k** |
| Full-context | gpt-4o | 60.2% | 28.9 s | 6.01 s | 115k |
| **Zep** | gpt-4o | **71.2%** | **2.58 s** | 0.684 s | **1.6k** |

提升：**+15.2 pp**（mini）/ **+18.5 pp**（4o）；延迟约 **-90%**；上下文由 **115k 压到 1.6k**。增益集中在 `single-session-preference`（4o：20.0%→56.7%）、`temporal-reasoning`（45.1%→62.4%）、`multi-session`（44.3%→57.9%）。
⚠️ **但 `single-session-assistant` 两个模型上均下降**（4o 94.6%→80.4%，即 **-17.7%**），值得作为反面证据记录。⚠️ 论文自陈**未能复现 MemGPT 在 LME 上的结果**，故 LME 一节**无 MemGPT 对照**，只与 full-context 比。

**LOCOMO——⚠️ 一场未平息的争议，各数字不可比**：

| 数字 | 来源 |
|---|---|
| **84%**（后被本人在原文更正） | Zep，约 2025-04/05 |
| **58.44% ± 0.20**（10 runs），称原结果"虚高约 25.56 pp" | Mem0 CTO 在 Zep 仓 issue #5 的反驳 |
| **75.14% ± 0.17**（10 runs，Zep 承认并更正） | Zep，2025-05-12 |
| 80.32 ± 0.43（30/30）、80.06 ± 0.33（20/20）… | Zep，2025-12（gpt-4o-mini 作答+评分） |
| **94.7%**（1,459/1,540） | Zep 官方 research 页，2026（reader/judge = gpt-5.4） |

⚠️ 84 → 75.14 → 80.32 → 94.7 是**方法论变迁，不是单调改进**：各行的模型、检索预算、harness 全不同。⚠️ Zep 自己称"**并不十分喜欢这个 benchmark**"（题目质量与答案歧义问题），并指出 LoCoMo 平均仅 16k–26k tokens、**没有 knowledge-update 题**。⚠️ 该 issue 在 Zep 要求 Mem0 提供数据集后于 2025-05-19 **以"不活跃"关闭**，58.44% 与 75.14% **从未在帖内调和**。
⚠️ **另一处前提纠正**：**Graphiti README 从来没有 benchmark 数字表**（已核 `main`、commit `993e081a` 及 v0.5.0/v0.10.0/v0.13.0/v0.17.0 等 tag），其唯一性能表述是定性的 "sub-200ms performance at scale"。因此**不存在"论文 vs README"的数字冲突**；论文的 DMR/LongMemEval 表被 Zep 博客**逐字复现**——那是厂商回声，不是独立复现。

### C.6 与纯向量记忆的对比

官方主张 📘 + 论文支撑 📄：相对**纯向量 RAG / 静态 GraphRAG**，Zep 的差异是①**增量构建**（新数据即时并入，无需全量重算/重新摘要）；②**显式 bi-temporal + 自动事实失效**（而非"LLM 摘要判断"式矛盾处理，且保留完整时间史）；③**语义 + BM25 + 图遍历**混合检索；④**每个实体/关系可回溯到 episode**、可引用原文，即溯源而非"依赖 LLM 摘要"；⑤ Pydantic 定义本体或让本体从数据涌现。延迟上官方称典型**亚秒级**，而 GraphRAG 为"秒级到数十秒"。
**本质优势**：向量记忆只能回答"哪段文本最像我"，无法区分"**曾经为真**"与"**现在为真**"——这正是 temporal reasoning 与 knowledge-update 类问题的失败根因。Zep 用"**失效而非删除**"把时间维度变成图上**一等公民**；代价是写入路径严重依赖 LLM，成本远高于向量入库。⚠️ 但"增量"仅对 episode 层成立，**社区层仍是周期性全量重建**。

---

## D. 补充框架

### D.1 Cognee：图 + 向量 + 关系三存 📘

**三库并列，非层级**：关系库（文档/切块/provenance）+ 向量库 + 图库（实体/关系）。API 层分**永久记忆**（dataset 级图）与**会话记忆**（`(user_id, session_id)` 缓存，**原样写入、不切块不抽实体不 embed**，TTL 默认 7 天）。核心单元 **DataPoint**（Pydantic），身份 `uuid5(...)`；**节点身份由归一化 `name` 派生**——用**确定性 id** 代替 LLM 猜去重。默认图库 **Kuzu**、向量库 **LanceDB**（生产推荐 Neo4j + PGVector）；**支持 RDF/OWL 本体**（`ONTOLOGY_MODE=annotate` 默认 / `strict`，**无推理机**）。
**写入**：📘 ⚠️ "ECL（Extract-Cognify-Load）"**只是历史品牌名**（v0.1.27 README），当前文档已不用；v1.0 API 为 `remember()` / `recall()` / `improve()` / `forget()`。`cognify` **确为 LLM 图构建**，六步：分类文档 → 权限校验 → 切块 → **LLM 抽实体关系入图库** → 逐块摘要 → embed 入向量库，默认**每块 2 次 LLM 调用**。两个默认关闭项：`PROVENANCE_TRACKING=true`（SHA-256 链式账本，`verify_chain()` 可检测篡改）、`CONTRADICTION_DETECTION=true`（写 `contradicts` 边，阈值默认 0.5）——**非破坏性，只加边、不失效旧事实**。
**读取**：`recall()` 先查会话缓存、否则走永久图；自动路由**是规则式的（2 条 regex），不是 LLM 决策**；`top_k` 默认 15。**记忆演进**：当前为 **`improve()`**（九阶段，含会话蒸馏、偏好更新、triplet 富化）；legacy `memify()` 为抽取 + 富化两阶段。
⚠️ **Benchmark（厂商自测）**：BEAM **0.79（100K）/ 0.67（10M）**，可追溯到发布工件，但厂商自跑、judge 为 `gpt-5`、**每档仅 20 题**；10M 档官方自称 "exploratory" 且 "in-sample"，报告原话"**我们没做受控消融**"。另有 24 题 HotPotQA 对比（Cognee 调优 vs 竞品用默认），官方自陈"**这是 cognee 的 benchmark，不是独立研究**"。**不存在与 Zep 的对比**；论文为**预印本**（`is_accepted: false`）。

### D.2 MemoryScope：前台/后台解耦的"观察 → 洞察"分层 📘

独特之处在于**把记忆维护彻底移出用户请求路径**。**分层**（`MemoryTypeEnum`）：**`conversation` · `observation` · `insight` · `obs_customized`**；`insight` = **同主题 observations 聚合而成的高层信息**。⚠️ **不存在持久化 working memory 层**——`read_message` 只是"读短期记忆"（未记忆的滚动窗口）。存储**仅向量库（默认 ElasticSearch）**，`retrieve_mode: dense`，**无图库、无关系库**。
**架构 = Service → Frontend/BackendOperation → WorkerFlow → `*Worker` → `MemoryManager`**，两个**定时后台**工作流：`consolidate_memory`（间隔 1s 轮询：信息过滤 → 抽观察 → **查重与矛盾处理** → 落库）、`reflect_and_reconsolidate`（间隔 15s：反思 → 更新洞察 → 长期矛盾处理）。抽取时 LLM 返回四元组 `[index, time_inference, observation_content, keywords]`。⚠️ 不存在 `ExtractMemory`/`UpdateMemory` 这类算子名。⚠️ README 自称"**20+ workers**"，归档代码实际只有 **16 个**——README 夸大。
**读取**：`retrieve_memory` = 三路并发向量检索（observations + `obs_customized`、insights、expired，各 top-100）→ `semantic_rank`（默认 `gte-rerank`，`top_n 500`，**`enable_ranker` 默认 false**）→ `fuse_rerank`（权重 **`{conversation 0.5, observation 1, obs_customized 1.2, insight 2.0}`**）。用户查询同步触发。
**触发时机**：后台与前端检索解耦，故用户侧响应**只取决于前端检索**，官方称约 **~500ms**；检索与巩固均**时间敏感**，并过滤用户虚构内容。
⚠️ **MemoryScope 自身没有任何 benchmark，也没有论文**——仓库只有软件引用。⚠️ `modelscope/MemoryScope` 已**重定向**到 `agentscope-ai/ReMe`，原版为该仓的 `memoryscope_branch` 归档（v0.1.1.0，2024-09），其文档站已 **404**。后继 **ReMe** 才是经同行评审的那个（Findings of ACL 2026），LongMemEval 89.4%、BEAM 100K 66.1% 等数字**属于 ReMe，不属于 MemoryScope**。

### D.3 A-MEM：Zettelkasten 式自组织记忆网络 📄

A-MEM（arXiv:2502.12110，**NeurIPS 2025**，已从 arXiv Comments 字段核实）批评现有系统"只有基本存储与检索、缺少精细的记忆组织"。**无层级**，是**扁平但互联的笔记网络**，存于 **ChromaDB + 进程内 dict，无图库**——所谓"网络"就是每条笔记 metadata 里的 `links` 列表，用 Python 遍历。
数据模型：`m_i = {c_i, t_i, K_i, G_i, X_i, e_i, L_i}`（内容、时间戳、LLM 关键词、LLM 标签、LLM 上下文描述、embedding、链接）。
三机制：
1. **原子笔记 + 结构化属性**：新记忆生成含**上下文描述 / 关键词 / 标签**的综合笔记。
2. **链接生成**：先按 `cosine(e_n, e_j)` 取 top-k 近邻，**再由 LLM 决定建哪些链接**（不是 embedding 自动连边）。
3. **记忆演化（memory evolution）** ⭐ 最本质差异：`m_j* ← LLM(m_n ‖ M_near\{m_j} ‖ m_j ‖ P_s3)`，新笔记进网的同时**反向改写既有邻居笔记**。⚠️ 代码实际只改邻居的 **`context` 与 `tags`**——**`content` 从不改动，`keywords` 也未按论文所称更新**；且演化后的邻居**要等到每 100 次演化触发 consolidation 才重新入索引**。

📄 **这是三者中唯一有同行评审数字的**。LoCoMo（7,512 题）GPT-4o-mini：A-Mem MultiHop **27.02** / Temporal **45.85** / OpenDomain 12.14 / SingleHop 44.65 / Adversarial 50.03，**平均 rank 1.2、2,520 tokens**，对比 LoCoMo 基线（rank 2.4、16,910 tok）与 MemGPT（rank 2.4、16,977 tok）。DialSim：A-Mem F1 **3.45** vs LoCoMo 2.55、MemGPT 1.18。**消融（Table 3）直接量化了机制价值**：去掉链接生成与记忆演化后 Temporal 仅 24.55、SingleHop 13.28；只加链接生成升至 31.24 / 39.17；完整版 45.85 / 44.65。
⚠️ 常被误读两点：Table 1 **没有平均 F1/BLEU 列**（只有平均 rank 与 token 长度）；正文称 k=10，但**头条 GPT 结果实际用的是 k=40/50**。⚠️ 仓库名有三套：评测 `WujiangXu/A-mem`、系统 `WujiangXu/A-mem-sys`、README 指向的论文复现 `WujiangXu/AgenticMemory`。

### D.4 三者对照

| 框架 | 分层 | 数据结构 | 关键机制 | 证据强度 |
|---|---|---|---|---|
| **Cognee** | 永久记忆 / 会话记忆（+ 关系·向量·图三库） | 图 + 向量 + 关系 | 确定性 name 派生 id；可选 provenance 哈希链、`contradicts` 边（非破坏性） | 厂商自测 BEAM（官方承认非受控） |
| **MemoryScope** | `conversation` → `observation` → `insight`（+`obs_customized`） | **仅向量库**（ElasticSearch） | **前后台解耦**（~500ms）；定时查重与矛盾处理；时间感知 | **无任何 benchmark、无论文** |
| **A-MEM** | 无层级（扁平互联笔记网络） | **仅 ChromaDB** + 内存 dict | 结构化属性 + LLM 决定链接 + **记忆演化（反向改写邻居）** | **NeurIPS 2025** 论文 + 消融（唯一同行评审） |

---

## E. 横向对比总表

| 维度 | **Letta** | **Mem0** | **Zep / Graphiti** |
|---|---|---|---|
| 记忆隐喻 | OS 虚拟内存 / 文件系统 | 事实抽取流水线 | 时序知识图谱 |
| 分层 | 上下文窗口（System Prompt/Tools/Messages/Skills）+ external context；MemFS 文件树 | SQL（事实+审计）/ Vector / Graph+Entity | Episode / Semantic Entity / Community 三层子图 |
| 写入触发 | agent 主动调工具；或后台 dreaming agent | 应用调 `add()`，回复后**异步**入流水线 | 应用调 `add_episode()`，即时增量 |
| 写入决策者 | **LLM agent 自己**（自编辑） | LLM 抽取（v3 单趟 ADD-only） | LLM 抽取 + 消解 + 矛盾判定；Cypher 落库 |
| 会不会丢历史 | 覆盖写 block 会丢 | 论文版会（DELETE 硬删）；**v3 ADD-only 不丢** | **不丢**——失效而非删除，四条时间戳留痕 |
| 检索方式 | 向量（archival，`top_k`/`tag_match_mode`/日期过滤）+ **混合**检索（会话搜索 = 文本 + 语义）+ 文件 grep | 语义 + BM25 + 实体 + 时序**融合排序**（BM25/实体不扩候选） | cosine + BM25 + BFS，RRF/MMR/cross-encoder/episode-mentions/node-distance 重排 |
| 上下文占用 | blocks 常驻（建议 <50k 字符/块） | 按需注入（约 7K tokens/query） | 按需注入（论文实验约 1.6k tokens） |
| 缓存友好度 | **高**（block 在 prompt 前缀；编辑单轮失效；官方建议高频记忆放 prompt 尾部 + 后台写） | 高（不在 prompt 中） | 高（不在 prompt 中） |
| 多 agent 共享 | ✅ block attach/detach；MemFS 共享 Git 仓 | 靠 `agent_id`/`run_id` 隔离 | 靠图分组（group）隔离 |
| 最大亮点 | agent 自编辑记忆 + 后台做梦 + Git 版本化 | 工程化流水线 + 极低延迟/token | bi-temporal 图，可查"任意历史时点的真相" |
| 最大风险 | 覆盖写丢历史、block 并发 last-write-wins | v3 记忆**只增不减**，靠检索排序兜底 | 写入 LLM 依赖重、成本高；社区层全量重建 |
| 证据强度 | 论文 + 源码（**benchmark 少**） | 论文 + ⚠️厂商自测（口径多次变更） | 论文 + ⚠️厂商自测（LOCOMO 有争议） |

---

## F. 参考链接

**Letta / MemGPT**
- MemGPT 论文：https://arxiv.org/abs/2310.08560 ｜Sleep-time Compute：https://arxiv.org/abs/2504.13171
- Letta V1 agent（heartbeat 废弃）：https://www.letta.com/blog/letta-v1-agent ｜Sleep-time 博客：https://www.letta.com/blog/sleep-time-compute
- Memory Blocks 博客：https://www.letta.com/blog/memory-blocks ｜Context Repositories：https://www.letta.com/blog/context-repositories
- **Context Constitution（当前权威分层）**：https://github.com/letta-ai/context-constitution/blob/main/constitution/CONSTITUTION.md
- 文档（**均标 `status: legacy`**）·上下文选型表：https://docs.letta.com/v1-sdk/memory/context-hierarchy ｜memory blocks：https://docs.letta.com/v1-sdk/memory/memory-blocks ｜archival：https://docs.letta.com/v1-sdk/memory/archival-memory
- 文档（当前）·MemFS：https://docs.letta.com/concepts/memfs ｜Memory & dreaming：https://docs.letta.com/configuration/memory
- 源码（**引用请用 SHA `67013ef1`，`main` 已 404**，harness 拆至 `letta-ai/letta-code`）·缓存断点：https://github.com/letta-ai/letta/blob/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/llm_api/anthropic_client.py ｜工具签名与常量：https://github.com/letta-ai/letta/blob/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/functions/function_sets/base.py ｜https://github.com/letta-ai/letta/blob/67013ef1bb491ec8bdd629bfbeceb70227936afa/letta/constants.py ｜缓存测试：https://github.com/letta-ai/letta/blob/67013ef1bb491ec8bdd629bfbeceb70227936afa/tests/test_prompt_caching.py
- 论坛·prompt caching（**非官方**）：https://forum.letta.com/t/breaking-prompt-caching/149

**Mem0**
- 论文：https://arxiv.org/abs/2504.19413
- How Mem0 Works：https://docs.mem0.ai/core-concepts/how-it-works ｜Memory Evaluation：https://docs.mem0.ai/core-concepts/memory-evaluation
- Platform v2→v3（ADD-only 转向）：https://docs.mem0.ai/migration/platform-v2-to-v3 ｜OSS v2→v3（图记忆移出 OSS）：https://docs.mem0.ai/migration/oss-v2-to-v3
- 评测框架开源：https://github.com/mem0ai/memory-benchmarks

**Zep / Graphiti**
- 论文：https://arxiv.org/abs/2501.13956 ｜全文：https://arxiv.org/html/2501.13956v1
- Graphiti GitHub：https://github.com/getzep/graphiti ｜文档：https://help.getzep.com/
- LOCOMO 争议：https://github.com/getzep/zep-papers/issues/5 ｜Zep 回应：https://blog.getzep.com/lies-damn-lies-statistics-is-mem0-really-sota-in-agent-memory/
- 检索权衡（50 次实验）：https://blog.getzep.com/the-retrieval-tradeoff-what-50-experiments-taught-us-about-context-engineering/

**补充框架**
- Cognee·Cognify 管道：https://docs.cognee.ai/core-concepts/main-operations/legacy-operations/cognify ｜`improve()`：https://docs.cognee.ai/core-concepts/main-operations/improve ｜DataPoints：https://docs.cognee.ai/core-concepts/building-blocks/datapoints ｜GitHub：https://github.com/topoteretes/cognee
- MemoryScope（`memoryscope_branch` 归档）：https://github.com/agentscope-ai/ReMe/tree/memoryscope_branch ｜后继 ReMe（ACL 2026 Findings）：https://aclanthology.org/2026.findings-acl.829/
- A-MEM 论文：https://arxiv.org/abs/2502.12110 ｜系统：https://github.com/WujiangXu/A-mem-sys

**对比参照（OpenAI / Anthropic）**
- Anthropic Memory tool：https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool
- Anthropic Context editing（含与 prompt caching 的交互）：https://platform.claude.com/docs/en/build-with-claude/context-editing
- Anthropic Effective context engineering：https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
