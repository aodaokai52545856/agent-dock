# AI Agent 记忆系统分层、分类与上下文缓存编排 —— 综合调研报告

> **调研时点**：2026-09 ｜ **覆盖范围**：Claude Code / Anthropic API、ChatGPT / OpenAI Responses & Codex CLI、Cursor、Manus、Aider、Cline、Gemini CLI、Devin(Cognition)、Letta(MemGPT)、Mem0、Zep/Graphiti、LangGraph/LangMem、AWS AgentCore Memory、Vertex Memory Bank、Azure AI Foundry、阿里云百炼、智谱 GLM、DeepSeek、Kimi、MiniMax、vLLM/SGLang/TensorRT-LLM/LMCache/Mooncake
> **证据分级**：📄 论文 / 🏢 官方文档原文 / ⚠️ 厂商自测 benchmark / 🔧 本报告工程推断。凡二手资料与一手来源冲突处，均已显式标注。
> **分册**：本报告为综合结论；7 份原始调研底稿见 `research/01~07*.md`（含逐条 URL 与原文引文）。

---

## 0. 结论速览（TL;DR）

**关于分层**

1. **"记忆分几层"是伪命题，"按什么维度切"才是真问题。** 同一批系统，按**认知功能**切是 4~5 层，按**存储介质与生命周期**切是 6 层，按**组织作用域**切又是另一套。数量之争的根源是混淆了切分维度。
2. **学术界已收敛到 CoALA 骨架**：working / episodic / semantic / procedural 是事实标准；2025 年后的三篇综述（Zhang 2024、Wu 2025 3D-8Q、Du 2025 六算子）补上了**表征维度**与**算子维度**。
3. **两个最常见的溯源错误**（二手资料 90% 会犯）：① CoALA 的人类记忆映射来自 **Soar + Baddeley & Hitch(1974) + Atkinson & Shiffrin(1968)**，**不是 Tulving**；② "DMR 基准太简单"的批评出自 **Zep(arXiv:2501.13956)**，**不是 LongMemEval**。
4. **工业界最缺的两层是 L0（感知缓冲）与 L4（程序记忆）**。综述结论：LLM 时代的 Agent 系统普遍缺失这两层，而 1980 年代的 SOAR/ACT-R/CLARION 反而齐全。**程序记忆缺少一等公民的表示格式与版本管理**，是当前最大的架构空白。

**关于处理机制**

5. **写入时机是最被低估的设计决策**：热路径写保一致性但推高延迟、**且破坏前缀缓存**；后台写保缓存但引入陈旧性。推荐默认 **"热路径只 append 原始记录，派生记忆全部后台生成"**。
6. **冲突消解正在从"写入期"迁移到"检索期"**：Mem0 v3 已改为**单趟 ADD-only**（删掉了 UPDATE/DELETE/NOOP）；Zep 采用**双时序作废而非删除**。这个分歧直接决定"记忆会不会丢"。
7. **三家记忆框架的 benchmark 不可横比**。Zep 的 LOCOMO 分数走过 84% → 被 Mem0 反驳 58.44% → 自认更正 75.14% → 2026 报 94.7%，**是方法论变迁而非单调改进**。

**关于缓存编排（用户最关心的部分）**

8. **KV-cache / prompt-cache 命中率是生产级 Agent 最重要的单一指标**（Manus 官方原话）。原因：Agent 的输入输出比约 **100:1**，prefill 是 compute-bound（决定 TTFT 与成本）；缓存命中价差达 **10×**（Claude Sonnet 缓存 0.30 vs 未缓存 3 USD/MTok）。
9. **只有一条主线**：**稳定内容在前 → 易变内容在后 → 只追加不改写 → 断点对齐稳定性边界**。多级断点、预热、工具 append-only、检索结果后置，全是这条主线的推论。
10. **缓存是架构约束，不是优化技巧**。Claude Code "整个 harness 围绕 prompt caching 构建"，对缓存命中率**打告警、按 SEV 处理**；Aider 一旦开 `--cache-prompts` 就**强制把 repo map 的自动重算降级**；Cursor **实测 1 小时 TTL 后主动放弃**（写入 2× 的成本超过减少 miss 的收益）。

---

## 1. 为什么必须分层：三个物理约束

任何记忆架构都是这三个约束下的工程妥协，脱离约束谈分层就是空谈。

| 约束 | 本质 | 对记忆架构的强制要求 |
|---|---|---|
| **注意力预算有限**（context rot） | Transformer 的 n² 成对关系；随 token 增长，准确召回能力**单调下降**。Anthropic 官方称之为 "attention budget"，且**所有模型都存在**，只是退化斜率不同 | 上下文必须当作**有限资源**管理；不能"全都塞进去"；需要按需加载与卸载 |
| **成本与延迟结构不对称** | Prefill 是 **compute-bound**（决定 TTFT 与成本）；Decode 是 **memory-bound**（决定 TPOT）。见 §5.1 | 记忆的价值必须按"**它消耗多少 prefill**"计价；长驻上下文 = 每轮重复付费 |
| **持久性需求分层** | 有的事只在一步内有意义，有的要跨会话，有的要跨年，有的一生不变 | 不同生命周期的信息**必须放不同介质**，否则要么丢、要么贵 |

🔧 **推论**：分层的本质是 **"按变化频率与生命周期对信息做隔离"**，从而同时满足"注意力预算只花在高信号 token 上"和"稳定内容可被缓存复用"。二者其实是同一件事的两面 —— **这也是为什么记忆分层和缓存编排必须一起设计**。

---

## 2. 记忆分层：有几种分法，各分几层

### 2.1 三种正交的切分视角

| 视角 | 切分依据 | 层数 | 适用场景 | 代表 |
|---|---|---|---|---|
| **A. 认知功能** | 这层记忆**做什么用** | 4~5（+2） | 能力评估、gap 分析、学术对标 | CoALA、Zenodo 综述、ZenBrain |
| **B. 存储介质与生命周期** | 这层**存在哪、活多久** | 5~6 | 容量规划、成本核算、系统设计 | 各家"推理栈"分层、基础设施分层 |
| **C. 组织作用域** | 这层**属于谁** | 3~5 | 多租户、权限、隐私合规 | Claude Code 四级、LangGraph namespace、AgentCore namespace |

⚠️ **混用会导致误判**：按认知口径，Claude Code、Cursor 这类系统"只有 3 层"；但只要换成介质口径，同一个系统立刻变成 6 层。**报层数时必须声明口径。**

### 2.2 主流分层方案对照表

| 层职责 | 3 层说法 | 4 层（CoALA） | 6 层（推理栈） | 6 层（L0–L5） | 7 层（ZenBrain） | 基础设施视角 |
|---|---|---|---|---|---|---|
| 原始输入缓冲 | — | — | — | **L0 感知** | — | — |
| 活跃任务状态 | 工作记忆 | working | KV Cache + 系统提示 + 工作记忆 | **L1 工作** | working | KV 前缀缓存 / 上下文窗口 |
| 会话内上下文 | 短期记忆 | working（含） | 会话记忆 | L1 工作（含） | short-term | 会话状态存储（checkpointer） |
| 具体经历（时间键） | 长期记忆（混） | **episodic** | 长期记忆（混） | **L2 情景** | episodic | 事件表 + 向量库 |
| 抽象事实（概念键） | 长期记忆（混） | **semantic** | 长期记忆（混）+ 外部知识库 | **L3 语义** | semantic | 向量库 / 图数据库 |
| 技能与流程 | —（散落 prompt） | **procedural** | 系统提示（隐式） | **L4 程序** | procedural | 文件系统（rules/*.md）、代码 |
| 身份 / 长期画像 | 长期记忆（混） | — | 长期记忆 | — | **core** | 文档存储 / 画像库 |
| 跨域迁移 | — | — | — | — | **cross-context** | 多租户命名空间 + 访问控制 |
| 外部知识 | — | semantic（含） | 外部知识库 | L3（含） | semantic（含） | 对象存储 / RAG 索引 |
| 记忆的元管理 | — | — | — | **L5 元记忆** | （置信+decay 算法承担） | TTL / LRU / 版本管理 |
| 权重内化 | — | — | — | — | — | **参数化记忆**（微调） |

📄 **学术界的统一五层**（Dmitrenko 2026, Zenodo 20618996，覆盖 19 个系统：5 学术 + 8 工业 + 6 PKM）：
`L0 sensory / L1 working / L2 episodic / L3 semantic / L4 procedural`
其核心结论：**L0 与 L4 在 LLM 时代的系统中普遍缺席**。该综述另给出七维评估（D1–D7，满分 17）：**Zep 11、Mem0 10、LangGraph/AutoGen/Cognee 9、CrewAI 8、Semantic Kernel 7、OpenAI Vector Stores 1**，无一家同时做到"架构强 + 结构风险低"。

📄 **ZenBrain 的 7 层**（Zenodo 19413933）在五层之上加了 **core**（永不衰减的身份事实）与 **cross-context**（跨域迁移）。它声称 LoCoMo F1 +21.6%（时间类 +176%）、睡眠巩固稳定性 +37% 且存储 −47.4%。⚠️ 但**其自身消融显示 working / short-term / cross-context 三层在 LoCoMo 上无影响** —— 引用时须谨慎。

### 2.3 推荐的工程六层（L0–L5）与每层职责

🔧 本报告综合各流派给出可直接落地的分层。**这一节是工程建议，不是任何官方文档的口径。**

| 层 | 名称 | 处理的业务问题 | 存什么 | 写入时机 | 读取时机 | 失效策略 | 典型载体 |
|---|---|---|---|---|---|---|---|
| **L0** | 感知/摄入缓冲 | "原始真相是什么"——审计、回溯、重放、记忆重建 | 用户消息、工具返回、文件变更、网页抓取的**原始副本** | 每次外部输入到达时立即落盘 | 几乎不直接读，供 L1 组装与回溯 | TTL 分钟~小时；不压缩，超时即丢或降采样归档 | 日志/对象存储/JSONL |
| **L1** | 工作记忆 | "当前这一步要做什么"——Agent 的 scratchpad | 计划、Todo、工具中间结果、思维链、当前任务状态 | 每步推理产出时**追加** | **每一轮 LLM 调用前全量注入**（必须常驻） | 任务结束即丢弃；接近上限时按"**可重算性**"裁剪；KV 前缀缓存随之自然失效 | prompt tokens / 内存 |
| **L2** | 会话记忆 | "这次对话聊过什么"——断点续跑、上下文压缩后重建 | 逐轮对话与工具调用消息流；压缩摘要 | 每轮后**追加**（thread-scoped） | 会话恢复、`--resume`、压缩后重建 | 会话结束 + 保留期到期即归档；摘要替代原文时保留摘要 | checkpointer / SQLite / Postgres / JSONL |
| **L3** | 情景记忆 | "上次类似的事怎么做的、成没成"——案例复用、失败复盘 | "发生了什么 + 结果 + 成败"的结构化事件轨迹 | 任务/会话**结束时**写入（后台） | 任务开始时按**时间窗 + 语义双路**召回；时间类查询 | Ebbinghaus 衰减 + 重要性加权；低价值条目降级为统计摘要；审计条目硬保留 | 事件表 + 向量库 |
| **L4** | 语义记忆 | "关于用户/世界我知道什么"——个性化、事实一致 | 事实、用户画像、偏好、领域概念、知识图谱 | 从 L3 **巩固（consolidation）** 或 LLM/人工抽取；画像走独立通道 | 任务规划前 RAG/图谱检索；回答事实问题前 | 事实冲突时 TTL + 置信度覆盖（新证据优先）；双时间轴保留历史版本 | 向量库 / 图数据库 / KV |
| **L5** | 程序记忆 | "这类任务该怎么做"——SOP、规范、技能 | 指令、规则、工具用法、验证流程、可复用代码/脚本 | **人工编写**或从成功轨迹沉淀；**必须走 code review** | 会话启动按作用域加载；大规则库按**路径/上下文条件懒加载** | Git 版本化，变更即失效旧版本；作用域优先级固定为 **组织 > 项目 > 用户 > 本地** | 文件系统（rules/*.md）、代码、技能包 |
| **L6** | 元记忆（横切） | "我到底记住了什么、准不准、有没有用"——调参、审计、遗忘决策 | 每层的条目清单、置信度、上次访问时间、命中率 | 每次读写记忆时旁路记录 | 遗忘决策、冲突检测、开放权重审计 | 跟随被管理条目 | 元数据表 / 索引 |
| **P** | 参数化记忆（横切） | "稳态技能能否内化" | 领域技能、风格、格式 | **离线**训练/蒸馏 | 推理时隐式生效 | 重新训练 | 模型权重 |

**两条工程红线**（🔧）：

- **前缀稳定性优先于任何缓存技巧**。所有厂商的缓存都是前缀匹配，把易变的（时间戳、随机 ID、当前任务状态）放尾部是**收益最高的单一优化**。
- **必须显式写出"记忆卸载点"**。不要指望模型自己记住一切；明确哪些信息必须落到 L2/L3/L5 的持久载体上，否则上下文一压缩必然丢失。

### 2.4 分层选型的三条经验

1. **三层的"长期记忆"折叠了 L3+L4+core**，选型时必须展开，否则会误判系统能力（一个只会存"用户偏好"的系统和一个有情景案例库的系统，都不叫"有长期记忆"）。
2. **容量规划看介质口径，能力评估看认知口径**，不要用同一张表回答两个问题。
3. **L0 别省**。没有 L0，你无法回答"这条被删除的记忆原本是什么"，也无法在记忆 pipeline 升级后重建历史。生产事故复盘时这是唯一的真相来源。

---

## 3. 每层"怎么处理"：写入、检索、遗忘的机制

### 3.1 写入路径：四种时机（最被低估的设计维度）

| 写入时机 | 一致性 | 延迟 | 对前缀缓存的影响 | 代表实现 |
|---|---|---|---|---|
| **热路径同步** | 最高（当轮即可读） | 推高每轮延迟 | ⚠️ **注入记忆 = 改进 system 前缀 = 破缓存** | LangMem hot path、Claude memory tool |
| **热路径异步**（fire-and-forget） | 中（下一轮可读） | 低 | 中 | Mem0 Platform、AgentCore |
| **后台批处理** | 低（秒~分钟级延迟） | 无 | **最优（前缀稳定）** | Letta "dreaming"、LangMem background、MemoryScope |
| **离线训练期** | 最低（需重训） | 无 | 无（不动 prompt） | 微调、蒸馏 |

🏢 **平台侧的硬约束**：AWS AgentCore Memory 的异步抽取官方描述为 **"seconds to minutes" 且无 SLA**；Azure AI Foundry 的 `update_delay` 单位是**秒**（默认 300）；Vertex Memory Bank 可 `wait_for_completion`；Mem0 需轮询 `event_id`。
🔧 **因此：若产品需要"同轮对话即可读到记忆"，架构上就必须放弃异步抽取，转而走热路径同步写 —— 而这必然破缓存。这是一个必须显式权衡的取舍，没有免费解。**

### 3.2 抽取与冲突消解：从"写入期决策"到"检索期排序"

这是 2024→2026 最显著的一次范式迁移。

| 方案 | 冲突处理 | 会不会丢历史 | 写入成本 |
|---|---|---|---|
| **Mem0（论文版）** | 对每条候选执行 **ADD / UPDATE / DELETE / NOOP** 四选一 | ⚠️ **DELETE 是硬删** | 中（每条候选一次 LLM 决策） |
| **Mem0 v3（现行）** | **单趟 ADD-only**；冲突处理**从写入期迁移到检索期排序** | 不丢，但**只增不减**（需靠检索排序压制过时事实） | 低 |
| **Zep / Graphiti** | **双时序作废而非删除**——旧边置 `invalid_at` 而非删除 | **不丢**（保留四条时间戳） | **最高**（抽取 + 消解 + 矛盾判定 + Cypher 写入） |
| **A-MEM** | 新记忆**反向改写**邻域记忆（演化） | 不丢（演化结果替换原 note，但 token 成本低） | 高（写入触发 N 次邻居 LLM 改写） |
| **MemoryScope / ReMe** | 后台定时整理 | 视策略 | 低 |
| **Claude Code auto memory** | 自动保存，用户不可逐条审阅 | — | 低 |

⚠️ **口径风险**：Mem0 的 benchmark 平台口径 LOCOMO 91.6 / 文档 92.5；Zep 的 LOCOMO 从 84%（2025-01）→ 被 Mem0 反驳 58.44% → 自认更正 75.14% → 2026 报 94.7%。**这是评测方法论的变迁，不是模型能力的单调改进**，该争议至今未调和。Cognee 的 BEAM 0.79/0.67 官方自陈"没做受控消融"。**A-MEM 是唯一有同行评审数字（NeurIPS 2025 + 消融）的**。

### 3.3 检索策略：五种范式

| 策略 | 机制 | 成本 | 代表 | 关键数字 |
|---|---|---|---|---|
| **全量注入**（长上下文） | 整段历史塞进窗口 | 极高 | 早期长上下文方案 | 📄 LongMemEval：离线通读 GPT-4o 0.9184 vs ChatGPT 0.5773，说明**长上下文本身不是解法** |
| **稠密向量 top-k** | 余弦相似度 | 低 | RAG、A-MEM | A-MEM 取 top-k(10) 候选，再由 LLM 决定真正链接（**嵌入只做过滤**） |
| **混合 + 重排** | BM25 + 向量 + reranker | 中 | Zep/Graphiti | `sim_min_score=0.6`、`limit=10`、`mmr_lambda=0.5`。⚠️ 默认 `graphiti.search()` **不含 BFS、不做 LLM 重排**；所谓 "cross-encoder" 实为 **LLM 二分类器** |
| **图遍历 / PPR** | 知识图谱 + Personalized PageRank | 高 | HippoRAG、Zep | 📄 HippoRAG：`τ=0.8`、PPR 阻尼 `0.5`、node specificity `s_i=|P_i|^(−1)`（IDF 的局部替代）；平均 R@2 **57.4** vs ColBERTv2 53.9；多跳 QA 最高 **+20%**；成本低 **10–30×** |
| **SQL / 精确查询** | 结构化过滤 | 最低 | ChatDB 类 | 适合时间窗、枚举、聚合类问题 |

**多因子打分**（📄 Generative Agents, arXiv:2304.03442）：
`score = α_rec·recency + α_imp·importance + α_rel·relevance`
⚠️ **论文与代码不一致**，引用必须注明：论文 §4.1 称 "all αs are set to 1"、衰减因子 0.995；但官方代码实际用 `gw=[0.5, 3, 2]`（顺序 recency/relevance/importance）、主流程 `recency_decay=0.99`。importance 由 LLM 打 1–10 分（"cleaning up the room"→2，"asking your crush out"→8，idle 强制 1），三项 min-max 归一化后取 **top-30**。

**遗忘曲线**（📄 MemoryBank, arXiv:2305.10250）：`R = e^(−t/S)`，S 首次提及初始化为 **1**，被回忆时 **S+=1 且 t 归零** —— 从而降低高频访问记忆的遗忘概率。⚠️ 论文自陈只模拟了 3 条 Ebbinghaus 原则，模型 "exploratory and highly simplified"。

### 3.4 六个原子算子（📄 Du 2025, arXiv:2505.00675）

形式化：`M_{t+Δt} = Consolidate(M_t, E_{[t,t+Δt]})`、`Forget(M_t, F)`

| 阶段 | 算子 |
|---|---|
| 形成 | **Consolidation**（巩固）、**Indexing**（编码/索引） |
| 演化 | **Updating**（更新）、**Forgetting**（遗忘） |
| 适配 | **Retrieval**（检索）、**Condensation**（压缩） |

该文批评 Zhang 2024（arXiv:2404.13501）只覆盖 writing/management/reading 这类高层操作、**漏掉 indexing**。
🔧 三篇综述互补用法：**Zhang 给操作生命周期、Wu 给存储坐标、Du 给原子算子**。

### 3.5 上下文耗尽时的三种官方对策（🏢 Anthropic）

| 技术 | 做什么 | 最适合 | 缓存代价 |
|---|---|---|---|
| **Compaction** | 把接近上限的对话摘要后重启新窗口 | 需要大量来回的任务 | ⚠️ 必然重写前缀 → 一次全量 miss（见 §5.6） |
| **Structured note-taking** | Agent 主动把笔记写到窗口之外（NOTES.md、Todo） | 有清晰里程碑的迭代开发 | ✅ 低（笔记读取发生在尾部） |
| **Sub-agent 架构** | 子代理用干净窗口深挖，只回传 **1,000–2,000 token** 的蒸馏摘要 | 可并行探索的研究/分析 | ✅ 低（子代理有独立前缀） |

🏢 Anthropic 原文还指出：Claude Code 的 compaction 会保留"架构决策、未解决的 bug、实现细节"，丢弃冗余工具输出，并**重新附加最近访问的 5 个文件**。

---

## 4. 分类体系：八个维度、多少种分类

🔧 综合 CoALA、三篇综述与工业实现，归纳出**八维记忆矩阵**。**"有多少种分类"的正确答案是：单一维度 3~4 种取值、八维正交组合 —— 不要指望一张表穷举。**

| 维度 | 取值 1 | 取值 2 | 取值 3 | 取值 4 |
|---|---|---|---|---|
| **① 时间跨度** | 轮内临时 | 会话内短期（窗口、KV cache） | 跨会话长期（天–月） | 准静态（权重、领域知识库） |
| **② 认知类型** | 工作记忆（prompt 装配层） | 情景记忆（对话、轨迹、案例） | 语义记忆（事实、画像、KG） | 程序记忆（权重、代码、工具、技能） |
| **③ 数据结构** | 非结构化文本流 | 向量索引 | 图结构（KG + PPR） | 结构化表 / SQL / KV |
| **④ 写入时机** | 热路径同步 | 热路径异步 | 后台批处理 | 离线训练期 |
| **⑤ 检索方式** | 全量注入 | 稠密向量 top-k | 混合 + 重排 | 图遍历 / PPR / 时间过滤 / SQL |
| **⑥ 作用域** | 单用户私有 | 单会话/单任务 | 单 Agent 跨会话 | 多 Agent 共享（⚠️ 隐私与投毒） |
| **⑦ 可控性** | 显式（可读可写可删可审计） | 半显式（可读，LLM 自主写） | 隐式参数化（只在权重里） | — |
| **⑧ 生命周期算子** | Consolidation / Indexing | Updating / Forgetting | Retrieval / Condensation | — |

**Wu 2025 的 3D-8Q 坐标**（arXiv:2504.15965）：`object × form × time` = personal/system × parametric/non-parametric × short/long，八象限依次映射到工作、情景、工作、语义、工作、程序、工作、语义+程序记忆（KV-Cache）。

🔧 **选型用法（三步定位法）**：
1. 先用 **②认知类型** 定"这层记忆要解决什么业务问题"；
2. 再用 **④写入时机 + ⑤检索方式** 定"工程形态"（这两维决定延迟与成本）；
3. 最后用 **⑥作用域 + ⑦可控性** 定"合规与权限"。
**先不要纠结数据结构** —— 那是第 4 步。

---

## 5. 上下文编排：如何让缓存命中尽量高

### 5.1 底层机制：为什么"改中间 = 后面全废"

| 阶段 | 计算形态 | 瓶颈 | 决定了什么 |
|---|---|---|---|
| **Prefill** | 一次前向处理全部 prompt token，大矩阵乘（GEMM），算术强度高 | **compute-bound**（FLOPS） | **TTFT**，长 prompt 的主要成本 |
| **Decode** | 每步仅 1 token，GEMV + 对全部历史 KV 的注意力 | **memory-bound**（显存带宽） | TPOT / 吞吐 |

🏢 **前缀命中 = 完全跳过该段 prefill**，TTFT 近似只与未命中 token 数成正比。厂商省下的是 compute-bound 算力，所以 cache read 能低到 0.1× 甚至 0.02×。
⚠️ **但缓存不能加速 decode** —— 这解释了为什么缓存"降价但不改变输出速度上限"。

**"改中间内容导致后面全部失效"的三层原因**（📄 + 🏢）：

1. **因果注意力 + 前缀哈希链**。vLLM 的块标识是链式 `hash(父块哈希, 块内 token, extra)`，前缀一变其后每块哈希全变。
2. **KV 本身是被前文"污染"过的计算结果**。CacheBlend 指出 KV cache 是"每个 token 的注意力与其前文关系的张量拼接"，位置 *i* 的变化会改变所有位置 >*i* 的 K/V。
3. **RoPE 使绝对位置参与 KV**。StreamingLLM §3.2 证实 key 在旋转前缓存、每步按新位置重旋转。**故即使内容完全相同，插入/删除造成的位置位移也会令后续 KV 失效。**

🏢 厂商原文佐证：
- Anthropic：*"Because the hash is cumulative… changing any block at or before the breakpoint produces a different hash"*，层级 `tools→system→messages`，"Changes at each level invalidate that level and all subsequent levels"。
- OpenAI：*"Cache reuse requires the entire rendered prefix to match."*
- Bedrock：*"Because the sections are chained, changing content in an earlier section invalidates the cache for later sections."*

### 5.2 各厂商缓存机制与计费对比

| 厂商 | 触发方式 | 写入 | 命中（读） | TTL | 最小可缓存长度 | 关键约束 |
|---|---|---|---|---|---|---|
| **Anthropic** | **显式** `cache_control`，或顶层自动缓存 | **1.25×**（5m）/ **2×**（1h） | **0.1×**（例外：Fable 5.1/Mythos 5.1 = 0.025×、Opus 5.5 = 0.05×） | 5 分钟（**命中免费续期**，滑动窗口）/ 1 小时 | **512 / 1024 / 2048 / 4096 按模型分档** ⚠️ "1024 Sonnet、2048 Haiku"的旧分法已过期 | 最多 **4** 断点；顺序 `tools→system→messages`；**回看 20 个 block**；TTL 自请求开始计（流式也占） |
| **OpenAI** | **默认开启**；GPT-5.6+ 支持 `implicit`/`explicit` + `prompt_cache_breakpoint` | GPT-5.6+ 1.25×；**旧模型无写入费** | GPT-5.6+ **0.1×**；旧模型分档 **0.25× / 0.5× / 0.1×** | GPT-5.6+ **30m**；旧模型 `in_memory` **5–10 分钟**、`24h` 档约 30 分钟 | GPT-5.6+ 为 **1024** 可见输入 token。"**128**"**是统计口径**（`cached_tokens` 向下取整到 128 的倍数）**而非最小长度** | 缓存**存于单机**，>15 req/min 可能溢出路由；命中 token **仍计入 TPM 限流**；每请求最多 4 次 write |
| **Google Gemini** | 隐式**默认开启**；显式走 cache 对象 | 隐式无额外费 | 隐式自动折扣 | 显式 TTL 可配，默认 1 小时 | **2048**（2.5 Flash/Pro）/ **4096**（3.x 系） | ⚠️ **显式缓存是二维计费**：cached-input 单价 **+** 按 **token-hour** 的存储价（2.5 Pro **$4.50 / 1M tokens / 小时**）。"无存储费"说法不成立 |
| **DeepSeek** | **默认开启**，**硬盘缓存**，无需改代码 | **无写入费** | 按 cache prefix unit 完整匹配 | 不用时自动清除，通常**几小时到几天** | 无固定值，以 **cache prefix unit** 为单位 | ⚠️ **"命中 0.1×"是 2024 年历史口径**；当前实际 ≈ miss 的 **2.0%（Flash）~3.3%（Pro）**。文档中**不存在**"64 token 最小单位"。字段 `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens` |
| **AWS Bedrock** | 隐式（尽力而为）+ 显式 `cachePoint` | 视模型；**可能高于**标准输入价 | 按模型费率 | 多数 **5 分钟**（命中重置）；新版 Claude/GPT-5.6 支持 1 小时 | **512 / 1024 / 4096** 分档 | 最多 4 检查点；⚠️ **长 TTL 检查点必须排在短 TTL 之前**；仅按需端点、不支持 Batch |
| **阿里云百炼 / Qwen** | **显式**（最多 4 个标记）+ **隐式**（无法关闭） | 显式 **125%**；隐式 100% | 显式 **10%**；隐式 **20%**（例外 GLM 25%、ds-flash 10%） | 显式 **5 分钟**（命中重置）；无 1 小时档 | 两者均 **1024** | **显式与隐式互斥**；20 块回看；⚠️ "工具定义不支持独立缓存"；官方算例：1 万输入 token、5 千命中 ⇒ 输入成本为无缓存的 **60%** |
| **智谱 GLM** | — | — | **50%** | — | 建议前缀 ≥ 500 token | ⚠️ 明确**不适用 Coding Plan 套餐** |
| **Kimi k3** | 显式 | 5m ¥20 / 1h ¥40 | **1/10** | 5m / 1h | — | 1 小时档命中 **2 次即回本**；k2.7/k2.6 不支持写入 |
| **MiniMax** | — | — | **20%（1/5）** | — | **512** | 顺序固定 **工具 → system → user** |

🏢 **横向总结**：Anthropic / Bedrock / 百炼的**显式缓存语义高度趋同**（1.25× 写 + 0.1× 读、5 分钟 TTL、可选 1 小时、约 20 块回看、最多 4 断点），差别在**最小长度分档**与读折扣；OpenAI 自 GPT-5.6 起补齐同样语义；Gemini 与 DeepSeek 走"默认隐式 + 极低价"路线，**DeepSeek 当前折扣已达 ~2–3%，显著低于业界 10%**。

### 5.3 为什么缓存命中率是第一指标

🏢 **Manus 官方（2025-07）**：*"If I had to choose just one metric, I'd argue that the KV-cache hit rate is the single most important metric for a production-stage AI agent."*

依据：

- Agent 每轮 = 动作 + 观察追加进上下文，**上下文持续增长而输出极短**。Manus 的平均**输入输出比约 100:1** —— prefill/decode 比例相比 chatbot 极度倾斜。
- 价差：Claude Sonnet 缓存输入 **0.30 USD/MTok** vs 未缓存 **3 USD/MTok**，**10×**。
- 🏢 OpenAI 官方示例命中率：**单轮判官约 70%、多轮 Agent >90%**；官方成本账：1 写 1 读 = **1.35×** vs 不缓存 2×；1 写 9 读 = **2.15×** vs 10×。

🏢 **Claude Code 的做法（值得逐条抄）**：*"we build our entire harness around prompt caching… we run alerts on our prompt cache hit rate and declare SEVs if they're too low."* Claude Code 把缓存 miss 定义为"**重算了 >5% 且 ≥2,000 tokens**"。

### 5.4 黄金布局：四段式前缀 + 断点部署

**🏢 Claude Code 的官方 system prompt 布局（FIG A 原文）**：

| # | 段 | 作用域 | 变化频率 | 断点建议 |
|---|---|---|---|---|
| 1 | **静态 system prompt + 工具定义** | **全局**（跨所有会话共享） | 几乎不变 | ✅ 必设断点（命中率最高的一段） |
| 2 | **CLAUDE.md / 项目指令 / rules** | **项目级** | 项目内不变 | ✅ 必设 |
| 3 | **会话上下文**（环境信息、MCP stub、输出风格、技能索引） | **会话级** | 会话内不变 | ✅ 建议设 |
| 4 | **对话消息**（含检索结果、工具输出） | 逐轮增长 | 每轮变 | ❌ 只在"需要反复追问同一长文档"时对文档本体设断点，**问题放断点之后** |

🏢 原文：*"The best way to do this is static content first, dynamic content last."* 这样能"最大化有多少会话共享缓存命中"。

🔧 **跨产品验证**：这份四段式在 Cursor、Aider、Cline、Gemini CLI、Codex CLI、Manus 上**全部收敛到同一形态**（见 §6），说明它不是 Claude Code 的特例，而是**前缀缓存这一物理约束的唯一解**。

### 5.5 破缓存反模式清单（12 条，全部有机制依据）

| # | 反模式 | 为什么会破 | 修法 |
|---|---|---|---|
| 1 | **在 system prompt 开头放秒级时间戳** | 前缀哈希每轮都变，而回看只找"历史上**写入过**"的条目 → **每次付写入费却永远读不到** | 时间放进 messages（🏢 Claude Code 用 `<system-reminder>` 挂在下一轮 user message / tool result 里） |
| 2 | **工具顺序非确定性** | 工具定义在缓存前缀最前端，顺序一变全废 | 固定排序；Claude Code 踩过这个坑 |
| 3 | **改工具参数**（如 Agent 工具可调用的 agent 列表） | 等同于改前缀 | 参数化能力改为运行时行为，而非动态定义 |
| 4 | **中途增删工具**（"只给模型它现在需要的工具"） | 工具在序列化后位于 system 前后，任何增删**令整段对话缓存失效** | 🏢 **Plan Mode 不换工具集**，改用 `EnterPlanMode`/`ExitPlanMode` **工具本身做状态转换**；MCP 用 `defer_loading: true` 的轻量 stub + ToolSearch，**不删只 defer** |
| 5 | **中途切模型** | **prompt cache 是按模型隔离的** | 需要换模型时用 **subagent**：让 Opus 准备一份 "hand-off" 消息给 Haiku。🏢 官方算例：100k token 对话里切 Haiku 比让 Opus 直接答**更贵** |
| 6 | **重写历史 turn / 重排消息** | 前缀匹配失败 | **append-only**；🏢 OpenAI：*"Append new messages rather than rewriting earlier turns."* |
| 7 | **JSON key 顺序抖动** | 很多语言/库不保证序列化 key 稳定顺序，**静默破缓存** | 用确定性序列化（固定 key 顺序 / canonical JSON） |
| 8 | **检索结果 / 工具输出放前缀里** | 它们每轮都变 | 放**最后一个断点之后**；🏢 断点之后按未缓存输入计费**且不产生 cache write 费用**，后置同时省写入费 |
| 9 | **断点放在"自动断点默认位置"** | 自动缓存常落在"最后一个可缓存块"，在"静态前缀 + 易变后缀"结构下**恰好落错位置** | 手动把断点放在"**最后一块跨请求相同的内容**"上 |
| 10 | **一轮新增超过 20 个 content block** | Anthropic 回看窗口只有 **20 块**（断点自身算第 1 个），旧写入会滑出窗口 | **预先**在更早位置再放一个断点（上限 4 个） |
| 11 | **压缩后不预热** | 压缩必然重写前缀 → 首个请求大面积 miss | 见 §5.6 三种缓解 |
| 12 | **忽略路由局部性** | 🏢 OpenAI 缓存**存在单机上**，超约 **15 req/min** 可能触发溢出路由 | 用 `prompt_cache_key` 把共享前缀的请求路由到同一机器；高流量按确定性映射拆多个 key；自建时用 session ID 做一致性路由 |

### 5.6 压缩与缓存的冲突：三种解法

**冲突本质**：压缩必须重写前缀，而前缀一变缓存全废。**最简单的压缩实现（另起一次 API 调用 + 独立 system prompt "summarize this" + 不挂工具）是最贵的** —— 它的前缀从第一个 token 就与主对话分歧，**全部按未缓存费率计费**，而对话越长（越需要压缩）这一单越贵。

**解法 1：Cache-safe forking（🏢 Claude Code 官方做法，推荐）**
- 压缩调用使用与父对话**完全相同**的 system prompt、user context、system context、工具定义；
- 前置父对话的历史消息，**把压缩指令作为新的 user message 追加在最后**；
- 从 API 视角看，该请求与父对话最后一次请求几乎一致 → **复用父的前缀缓存**，只有压缩指令本身是新 token；
- ⚠️ 代价：必须预留 **compaction buffer**，为压缩指令与摘要输出留出窗口空间。

**解法 2：追加式摘要**
- 不动原历史，把摘要**作为新消息追加**；
- 🏢 Anthropic 部分模型支持对话中途追加 `role: system` 而不失效；🏢 OpenAI 在 GPT-6 上可追加 `configuration_update` 改 effort 而保留前缀。

**解法 3：对齐 + 预热**
- 把压缩点对齐到断点边界、压缩阈值取断点结构的整数倍；
- 压缩后**立即预热**：🏢 Anthropic 支持 `max_tokens: 0`（只读入并写缓存，无输出计费）；🏢 OpenAI 支持 `prompt_cache_options.prewarm: true`（按 cache-write 费率计费）。

🏢 **OpenAI 的官方立场**：不必因噎废食 —— *"输入 token 更少时即使命中率下降也可能更省钱"*。🔧 **因此决策指标应该是"总输入成本"，而不是命中率本身。**

### 5.7 缓存作为架构约束的三个经典设计

| 设计 | 直觉做法（错） | 实际做法（对） | 收益 |
|---|---|---|---|
| **Plan Mode** | 进入计划模式时把工具集换成只读工具 | **保留全部工具**，用 `EnterPlanMode`/`ExitPlanMode` 作为工具本身 + 系统消息说明当前模式 | 零缓存破坏；**额外红利**：模型可自主进入 plan mode |
| **MCP 工具爆炸** | 每轮按需加载工具定义（RAG 式动态 action space） | `defer_loading: true` 发**轻量 stub**（只有名字），模型需要时用 tool search 发现完整 schema | 前缀稳定；🏢 Manus 实测：**除非绝对必要，不要在迭代中途动态增删工具** —— 既破缓存，又因"旧动作引用已不存在的工具"导致 schema 违规或幻觉动作 |
| **Aider 的 repo map** | repo map 内容变化时自动重算 | 🏢 源码里一句 `if cache_prompts and map_refresh == "auto": map_refresh = "files"` —— **一旦开启缓存，就禁止 repo map 自动重算** | 把缓存当作**架构约束**而非优化项 |

（🏢 Manus 的补充手段：用**上下文感知的状态机 + 解码期 logits mask** 控制工具可用性 —— 即"**mask, don't remove**"。工具名统一前缀 `browser_` / `shell_` 以便按前缀做 mask。）

### 5.8 路由与多级缓存：命中率不只是 prompt 结构问题

**请求路由层**
- 🏢 OpenAI：缓存**存在单机上**（不跨组织/区域共享）；超约 **15 req/min** 可能触发溢出路由；<GPT-5.6 需用稳定 `prompt_cache_key` 把共享前缀的请求路由到同一机器。
- 🔧 自建（vLLM/SGLang）时必须用 **session ID** 做分布式 worker 的一致性路由，否则前缀缓存形同虚设。

**推理框架层**

| 实现 | 索引结构 | 粒度 | 淘汰策略 | 关键数字 |
|---|---|---|---|---|
| **vLLM APC** | 全局哈希表 | 链式哈希，**块大小 16** | 先淘汰引用计数 0 者，其中 LRU | 🏢 `enable_prefix_caching` **默认 True**；默认 sha256（⚠️ 文档警告非加密哈希在多租户下**可能泄露私有信息**） |
| **SGLang RadixAttention** | **radix tree** | KV 分页布局，每页 1 token；树存 CPU | **LRU 且优先淘汰叶子** | 📄 吞吐最高 **6.4×**、延迟降 **3.7×**；cache-aware scheduling 达最优命中率 **96%**；🏢 生产（Chatbot Arena 一个月）命中率 **52.4%–74.1%**，首 token 延迟平均降 **1.7×** |
| **TensorRT-LLM** | paged KV + radix 搜索树 | 块键 = **SHA-256 摘要**，仅凭 digest 判定 | **优先级 LRU**（优先级 0–100，默认 **35**），**只有叶子块可淘汰** | 🏢 `enable_block_reuse` 默认 True；KV 量化 FP8 后吞吐 **+56%**（Llama-3.3-70B TP4 H100）；⚠️ 多模态**不兼容**块复用 |
| **DeepSeek 硬盘缓存** | 磁盘 | **cache prefix unit**（请求边界 / 公共前缀检测 / 固定 token 间隔三种生成时机） | 不用时自动清除 | 🏢 无写入费；"best-effort，不保证 100% 命中" |

**跨节点/跨层缓存（自建大规模场景）**
- **LMCache**：四层存储（显存 → DRAM → 本地盘 → 远程），索引块 **256 token**，LRU。
- 📄 **Mooncake**（FAST'25）：有效请求容量 **+59%~498%**，全局缓存命中率为本地的 **2.36×**，prefill 时间 **−48%**；128k 输入 prefill **−92%** / TTFT **−86%**；建议最低带宽 **100 Gbps**。

**KV 压缩研究（长上下文必需时）**

| 方法 | 机制 | 数字 |
|---|---|---|
| **H2O** | 少数 token 贡献大部分注意力（Heavy Hitters），动态保持"近期 token + H2O token" | **20%** heavy hitters 下吞吐最高 **29×** |
| **StreamingLLM** | **attention sink**：只保留最初 **4 个 token** 的 KV + 滑动窗口 | Llama-2-13B PPL：纯滑窗 **5158.07** vs 4+1020 = **5.40**；稳定处理 >400 万 token |
| **SnapKV** | 从 prompt 末尾 128 token 观察窗提前观测注意力模式，投票 + 池化 + 每头 topk | 16K 输入生成速度 **3.6×**、显存效率 **8.2×**；单卡 A100-80GB 支持 **380K** |
| **CacheBlend** | 直击"复用的 chunk 不是前缀"：**选择性重算少量 token 的 KV** | 更新比例 **<15%** 即达同等质量；TTFT 降 **2.2–3.3×** |

### 5.9 成本模型与回本点

🏢 **OpenAI 官方 break-even 公式**：最小可缓存长度 `M`、原前缀 `L<M`、读倍率 `r`、写倍率 `w`、请求数 `N`：

```
L_break-even = M · (r + (w − r) / N)
```

**例**：`M=1024, r=0.1, w=1.25` ⇒ `102.4 + 1177.6/N`；**10 次请求下，前缀只要 ≥221 token 就值得主动"撑"到 1024**。

🏢 **Anthropic 口径一致**：缓存写只多花 25%、命中省 90%，**只要发生至少一次命中，总成本即低于不用缓存**（5 分钟档 1 次读取回本，1 小时档 2 次回本）。

🔧 **推论：短前缀（< 最小可缓存长度）在所有厂商都拿不到收益。此时主动把有用内容撑到阈值反而最优** —— 这与直觉相反，但公式如此。

### 5.10 度量与 SLO

| 指标 | 定义 | 目标 |
|---|---|---|
| **Cache hit rate** | `cached_tokens / input_tokens`（OpenAI 字段 `usage.input_tokens_details.cached_tokens`；Gemini `usage.total_cached_tokens`；DeepSeek `prompt_cache_hit_tokens`） | 🏢 多轮 Agent >90%（OpenAI 官方示例）；低于此值即排查 |
| **Cache miss 事件** | 🏢 Claude Code 定义：**重算了 >5% 且 ≥2,000 tokens** | **当 uptime 一样告警，按 SEV 处理** |
| **总输入成本** | 含 cache write 溢价 | 这是**最终决策指标**（而非命中率） |
| **TTFT** | 首 token 时间 | 与前缀命中长度近似成正比 |
| **记忆检索精度** | LongMemEval / LoCoMo / DMR | ⚠️ 三家口径不可横比，只做**纵向**对比 |

### 5.11 落地 Checklist（按收益排序）

1. ☑️ **把秒级时间戳、随机 ID、当前任务状态从 system prompt 移出**，改挂 messages（收益最高、成本最低）。
2. ☑️ **固定工具定义与顺序**；用 `defer_loading` stub + tool search 替代动态增删；用工具做状态转换（Plan Mode 模式）。
3. ☑️ **全链路 append-only**；确保序列化确定性（canonical JSON，固定 key 顺序）。
4. ☑️ **按"全局 → 项目 → 会话 → 消息"四段式重排 prompt**，让前缀单调增长。
5. ☑️ **在"最后一块跨请求相同的内容"上手动设断点**，最多 4 个；一轮新增可能 ≥20 块时**预先**加断点。
6. ☑️ **检索结果、工具输出、用户提问放最后一个断点之后**。
7. ☑️ **压缩改为 cache-safe forking**（复用父前缀，只在末尾追加压缩指令）+ 预留 compaction buffer。
8. ☑️ **不中途切模型**；必须切时走 subagent hand-off。
9. ☑️ **自建推理时开启前缀缓存 + session 一致性路由**；高流量用 `prompt_cache_key` 拆分。
10. ☑️ **接监控**：命中率、miss 事件、总输入成本、TTFT 四项进看板；命中率破线告警。
11. ☑️ **把记忆写入移到后台**（热路径只 append 原始记录），从架构上消除"注入记忆 → 破前缀"的问题。
12. ☑️ **短前缀主动撑到最小可缓存长度**（用 §5.9 公式判断）。

---

## 6. 十个成熟产品 / 开源项目实例

### 6.1 Claude Code（Anthropic）—— 把缓存写进架构的教科书

| 维度 | 实现 |
|---|---|
| **记忆分层** | ① Managed policy（企业级，Windows `C:\Program Files\ClaudeCode\CLAUDE.md` / Linux `/etc/claude-code/CLAUDE.md`）→ ② User `~/.claude/CLAUDE.md` → ③ Project `./CLAUDE.md` 或 `./.claude/CLAUDE.md` → ④ Local `./CLAUDE.local.md`。**拼接不覆盖**，顺序"文件系统根 → 工作目录" |
| **加载策略** | 🏢 **子目录 CLAUDE.md 按需加载**（Claude 读到该目录文件时才加载）；`@import` **递归上限 4 跳**，启动时展开、**不省 token**；>4 MiB 跳过，建议 <200 行 |
| **程序记忆** | **Skills**（技能包）+ `CLAUDE.md` + **auto memory**（自动保存，取代旧的 `#` 快捷记忆） |
| **上下文管理** | auto-compact **无百分比阈值**（按模型上限 / 200K / 原生 1M 约 967K）；官方行为："**clears older tool outputs first**, then summarizes"。压缩后重注入 CLAUDE.md / auto memory / plan，**重读最多 5 个最近修改文件**（>5000 tokens 退化为路径引用）；skill 正文每 skill ≤5000、合计 ≤25000，**skill 描述清单不重注入** |
| **API 侧记忆** | memory tool `{"type":"memory_20250818","name":"memory"}`，**完全客户端执行**，6 个命令；API 自动注入 *"ALWAYS VIEW YOUR MEMORY DIRECTORY BEFORE DOING ANYTHING ELSE"* |
| **缓存策略** | 四段式布局（§5.4）；**显式 `cache_control` 断点 ≤4**；`tools→system→messages` 链式失效；**20 block 回看**；`<system-reminder>` 挂 messages 更新动态信息；Plan Mode 保留全工具；`defer_loading` stub；**cache-safe forking** 压缩；**对命中率打告警、按 SEV 处理** |
| **可借鉴** | ⭐ **"用工具做状态转换"** 而非改 prompt/工具集；⭐ **cache-safe forking**；⭐ **压缩后"结构化笔记 + 5 个最近文件"** 是重注入的最小可行集合 |

⚠️ **注意**：`clear_tool_uses_20250919`（context editing）官方**明说会失效缓存**：*"Invalidates cached prompt prefixes when content is cleared"*，建议用 `clear_at_least` 判断是否值得。官方现已把 **server-side compaction（`compact_20260112`）列为首选**，SDK 的 `compaction_control` 已废弃。

---

### 6.2 Cursor —— 缓存友好 + 代码索引的双轨设计

| 维度 | 实现 |
|---|---|
| **程序记忆** | `.cursor/rules`（MDC 格式）**四档激活**：alwaysApply / globs 匹配 / description 触发 / 手动；规则被**显式放在 "the start of the model context"**；优先级 Team → Project → User |
| **长期记忆** | **Memories**（自动记忆）+ Rules；`@` 引用（@file / @folder / @git / @docs / @web）作为**即时上下文** |
| **代码索引** | 客户端 **Merkle tree** 哈希 + **每 10 分钟**比对增量上传 → 服务端 chunk/embedding 存 **turbopuffer**（附混淆路径 + 行号 + chunk hash 缓存）→ 推理时最近邻**回传路径**，**在客户端本地读文件再上传**（代码不出本地的安全模式） |
| **缓存策略** | 🏢 员工在官方论坛确认 *"we build the prompt in a cache-friendly way"*；用 Anthropic 默认 **~5 分钟滑动窗口**，**跨会话也能命中**（system prompt 很少变）；"biggest wins come from keeping the early part of the conversation stable"；**切模型 / 改早期消息 / 开关 tools 或 rules 都会 re-seed 缓存** |
| **⚠️ 最有价值的负面结论** | **实测 1 小时 TTL 后主动放弃** —— 写入 2× 的额外成本超过减少 miss 带来的收益 |
| **可借鉴** | ⭐ **"索引回传路径、本地读文件"** 是隐私与 token 成本的双赢；⭐ **缓存收益要实测，不要照搬 TTL 档位** |

---

### 6.3 Manus —— 把 KV-cache 命中率当第一指标

| 维度 | 实现 |
|---|---|
| **记忆分层** | 虚拟机文件系统 = **外部记忆的终极形态**；todo.md = 工作记忆的持久化 + 复述载体；上下文窗口 = 工作记忆 |
| **核心论断** | 🏢 *"KV-cache hit rate is the single most important metric for a production-stage AI agent."* 输入输出比 **≈100:1** |
| **前缀稳定三招** | ① 保持 prompt 前缀稳定（**明确点名"system prompt 开头放秒级时间戳"是典型错误**）；② **append-only** + **确定性 JSON 序列化**（key 顺序抖动会静默破缓存）；③ 需要时**显式标 cache breakpoint**，至少覆盖 system prompt 末尾 |
| **工具管理** | **Mask, don't remove** —— 用上下文感知状态机 + **解码期 logits mask** 控制工具可用性，而非增删工具定义；工具名统一前缀（`browser_` / `shell_`） |
| **对抗 lost-in-the-middle** | **todo.md 复述**：把当前目标/进度**写到上下文末尾**（而非开头），利用近因效应 |
| **错误处理** | **保留错误轨迹**，不清理失败的尝试（让模型学会避开） |
| **压缩** | 文件系统作为**可恢复压缩**的载体：需要时重新读取文件即可重建上下文 |
| **few-shot 陷阱** | 用**结构化扰动**打破 few-shot 模式惯性（避免"注意力劫持"） |
| **可借鉴** | ⭐ **文件系统 = 可恢复压缩**；⭐ **recitation 放末尾**对抗 lost-in-the-middle；⭐ **mask 而非 remove** |

---

### 6.4 Aider —— 把缓存当架构约束的开源标杆

| 维度 | 实现 |
|---|---|
| **程序记忆 / 检索** | **repo map**：tree-sitter 提取符号 → 依赖图 → **PageRank（个人化排序）** → 按 token 预算裁剪（`--map-tokens`） |
| **prompt 结构** | `system + examples + readonly_files + repo + done + chat_files + cur + reminder`（⚠️ 官方文档与代码不一致：**代码里 read-only 在 repo map 之前**） |
| **缓存策略** | `--cache-prompts` 开启，**最多 3 个断点**；断点打在**末尾可连带覆盖前三段**；`--cache-keepalive-pings` 默认 0（可按间隔 ping 保活） |
| **⭐ 最精妙的设计** | 源码：`if cache_prompts and map_refresh == "auto": map_refresh = "files"` —— **一旦开启缓存，就强制把 repo map 从"自动重算"降级为"仅文件变更时重算"** |
| **可借鉴** | ⭐ **把缓存当约束而非优化**：宁可牺牲 repo map 新鲜度，也不破前缀 |

---

### 6.5 Letta / MemGPT —— 学术分层最完整的开源实现（⚠️ 已两次重构）

| 维度 | 实现 |
|---|---|
| **MemGPT 原始设计**（📄 arXiv:2310.08560） | **main context（=RAM，即 prompt tokens）** vs **external context（=disk：archival + recall）**。main context 三段：**system instructions**（只读：控制流 + 层级说明 + function schema）、**working context**（定长读写块，**只能经函数调用写**）、**FIFO 队列**（队首为"被逐出消息的递归摘要"） |
| **分页机制** | 超 **warning（约 70% 窗口）** 插入"内存压力"消息让模型主动 flush；超 **flush（约 100%）** 逐出**约 50% 窗口**消息并生成新递归摘要，被逐出者永久留在 recall |
| **benchmark** | 📄 DMR：GPT-3.5 Turbo 38.7%→**66.9%**；GPT-4 32.1%→**92.5%**；GPT-4 Turbo 35.3%→**93.4%** |
| **⚠️ 2026 现状** | `docs.letta.com/v1-sdk/*` 各页 frontmatter 明确 `status: legacy`。当前权威是 **Context Constitution**（窗口内 System Prompt / Tools / Messages / Skills 四原语 + external context 两层）。**recall storage 已不再是独立层**（由 compaction + 工具检索取代） |
| **⚠️ 记忆形态已变** | 已从 memory blocks 变为 **MemFS —— git-backed 记忆文件系统**：`system/` 下文件每轮进 prompt，文件树常驻当导航，后台 agent 用 **git worktree** 并发合并；sleep-time 更名为 **"dreaming"** |
| **控制流** | `request_heartbeat` 的替代物是 **ToolRule 系统**：`ContinueToolRule`（= `request_heartbeat=True` 的结构化等价物）、`TerminalToolRule`（挂在 `send_message` / `memory_finish_edits`） |
| **可借鉴** | ⭐ **"窗口内四原语 + 外部上下文"** 是干净的心智模型；⭐ **git-backed 记忆**解决了多 agent 并发写记忆的合并问题；⭐ **分页阈值（70%/100%/50%）** 是量化 compaction 的早期范本 |
| **局限** | ⚠️ 覆盖写 block 会丢历史，且并发写是 **last-write-wins** |

---

### 6.6 Mem0 —— 生产级记忆抽取流水线（⚠️ v3 架构已变）

| 维度 | 实现 |
|---|---|
| **原始设计**（论文版） | 两阶段流水线：**extraction**（LLM 从对话抽取候选记忆）+ **update**（对每条候选执行 **ADD / UPDATE / DELETE / NOOP** 决策，与已有记忆比对） |
| **⚠️ v3 现行架构** | **单趟 ADD-only**（不再有 UPDATE/DELETE/NOOP）；**冲突处理从写入期迁移到检索期排序**；**图记忆已从 OSS 彻底移除**（删约 4000 行驱动），降为 Platform 专属 |
| **作用域** | 多级过滤 `user_id` / `agent_id` / `run_id`；⚠️ 实体 ID **必须在 `filters` 内** |
| **存储** | 向量存储 + 可选图存储（Neo4j / Mem0^g 的实体-关系抽取） |
| **API** | 主端点 `/v3/memories/*`；webhook 事件名小写（`memory_add` 等） |
| **benchmark** | ⚠️ 平台口径 LOCOMO **91.6** / 文档口径 **92.5**（**厂商自测，与 Zep 口径不可横比**） |
| **可借鉴** | ⭐ **"写入期做冲突消解"→"检索期做排序"** 的迁移降低了写入延迟，对缓存友好 —— 但代价是**只增不减**，长期需要更重的检索侧排序 |
| **局限** | 只增不减 → 过时事实需要靠排序压制；图能力仅 Platform |

---

### 6.7 Zep / Graphiti —— 时序知识图谱记忆

| 维度 | 实现 |
|---|---|
| **数据结构** | **时序知识图谱**：episode / entity / community 三层子图 |
| **双时序模型** | 四条时间戳：`valid_at` / `invalid_at`（事实有效期）+ ingestion time（摄入时间）。**旧边置 `invalid_at` 而非删除** → **不丢历史** |
| **检索** | 混合检索：cosine similarity + BM25 + 图遍历（BFS），RRF/MMR rerank。源码级参数：`sim_min_score=0.6`、`bfs_max_depth=3`、`mmr_lambda=0.5`、`limit=10` |
| **⚠️ 三个常见误解** | ① 广为流传的 `search_type` **不是 API 参数**（托管 API 用 `scope` + `reranker`）；② **默认 `graphiti.search()` 不含 BFS、不做 LLM 重排**；③ 所谓 "cross-encoder" 实为 **LLM 二分类器**；④ **社区层是周期性全量重建，并非增量** |
| **API** | `graph.search(scope, reranker, return_raw_results)`（⚠️ **不是** `return_raw_facts`）；SDK v3 但 REST 仍 `/api/v2/` |
| **可用性** | ⚠️ **CE（社区版）已废弃**，仅 Graphiti OSS 或 BYOC；RBAC/审计仅 Enterprise、SSO 为 OIDC |
| **benchmark** | ⚠️ LOCOMO 从 84%（2025-01）→ 被 Mem0 反驳 58.44% → 自认更正 75.14% → 2026 报 94.7%。**方法论变迁，非单调改进** |
| **可借鉴** | ⭐ **双时序 = 不删记忆也能保证事实一致性**；⭐ 图 + BM25 + 向量的混合检索是企业级事实查询的正确形态 |
| **局限** | 写入成本最高（抽取 + 消解 + 矛盾判定 + Cypher） |

---

### 6.8 LangGraph / LangMem + AWS AgentCore —— 企业级记忆的"平台化"两条路

**A. LangGraph / LangMem（可完全自托管）**

| 维度 | 实现 |
|---|---|
| **短期记忆** | **thread-scoped checkpointer**：`InMemorySaver` / `SqliteSaver` / `PostgresSaver` / `MongoDBSaver` / `CosmosDBSaver`；`checkpoint_ns`（根图 `""`、子图 `"node_name:uuid"`，嵌套用 `|` 连接）；`get_state` / `get_state_history` / `update_state` / **replay**（时间旅行）；`durability="exit|async|sync"` |
| **长期记忆** | **`BaseStore`**：命名空间**元组前缀匹配**（如 `("user_id","memories")`）；`aput/aget/adelete/asearch/alist_namespaces`；语义检索索引 `index={"embed","dims","fields"}`，可按次 `put(index=[...])` 或 `index=False` |
| **三种记忆落地** | 语义 = Profile / Collection；情景 = Episode few-shot；程序 = prompt 规则 |
| **写入架构** | 🏢 官方给出 **hot path vs background** 取舍表。热路径工具：`create_manage_memory_tool` / `create_search_memory_tool`；后台：`create_memory_store_manager` / `ReflectionExecutor.submit(after_seconds=)`（**带去抖取消**）；程序记忆：`create_prompt_optimizer(kind=gradient|prompt_memory|metaprompt)` |
| **压缩** | `trim_messages` / `RemoveMessage` / `SummarizationNode` / `SummarizationMiddleware` |
| **⭐ 缓存** | 🏢 官方明确：**Anthropic 缓存完整前缀，裁剪/摘要/删除都会使其失效**；提供 `AnthropicPromptCachingMiddleware(type="ephemeral", ttl="5m"|"1h")` |
| **可借鉴** | ⭐ 唯一**可完全自托管且不牺牲语义检索**的组合（LangGraph + Postgres/MongoDB/Redis） |

**B. AWS Bedrock AgentCore Memory（GA，托管）**

| 维度 | 实现 |
|---|---|
| **短期记忆** | `CreateEvent`（`memoryId` / `actorId` / `sessionId` / `eventTimestamp` / `payload`）；`PayloadType` union：`conversational` / `blob` / `json`（**仅前两者参与长期提取**） |
| **长期记忆** | 5 种策略 union key：`semanticMemoryStrategy` / `summaryMemoryStrategy` / `userPreferenceMemoryStrategy` / `customMemoryStrategy` / `episodicMemoryStrategy` |
| **作用域** | `namespaceTemplates`（`namespaces` 已 deprecated），占位符含 `{memoryStrategyId}`；≤5 个自定义 `namespaceKeys`。⚠️ **变量缺失会导致静默不提取** |
| **检索** | `RetrieveMemoryRecords`（`topK` 默认 10、`maxResults` 默认 20），⚠️ **仅语义检索** |
| **⚠️ 硬约束** | 异步抽取官方描述 **"seconds to minutes" 且无 SLA**；$0.25/1k 事件、$0.75/1k 记录/月 |
| **可借鉴** | ⭐ "事件（短期）+ 策略（长期）"的 API 分层清晰；⭐ namespace 模板是多租户隔离的标准做法 |

**C. 其他平台要点**（表格速览）

| 平台 | 短期单位 | 长期类型 | 写入 | 作用域 | 自托管 |
|---|---|---|---|---|---|
| **Vertex Memory Bank**（GA） | session | 4 托管 topic + 自定义 | `GenerateMemories`（可 `wait_for_completion`） | `scope` 任意 KV，**精确匹配且 immutable** | ❌ |
| **Azure AI Foundry**（Preview） | — | store 级开关 `chat_summary_enabled` / `user_profile_enabled` / `procedural_memory_enabled` | `update_delay`（**秒**，默认 300） | `scope` | ❌ |
| **Redis Agent Memory Server** | Redis | 向量化 | ⚠️ 仓库已一分为二，**开源版移入 `V0/` 且已停维护** | — | 部分 |
| **MongoDB** | `MongoDBSaver` | `MongoDBStore` + `$vectorSearch` | — | — | ✅ |

---

### 6.9 OpenAI（ChatGPT 记忆 + Responses API + Codex CLI）

| 维度 | 实现 |
|---|---|
| **ChatGPT 记忆（两条独立管线）** | ① **saved memories**（经 `bio` 工具写入，逐条可删）；② **reference chat history**（画像层，用户**不可查看/编辑**，官方称无存储上限） |
| **⚠️ 注入方式官方从未说明** | 第三方逆向（Embrace The Red, 2025-05）证明是 **system 提示静态拼接** 6 个分段：`Model Set Context` / `ASSISTANT RESPONSE PREFERENCES` / `NOTABLE PAST CONVERSATION TOPICS HIGHLIGHTS` / `HELPFUL USER INSIGHTS` / `RECENT CONVERSATION CONTENT`(~40 条) / `USER INTERACTION METADATA`(17 项)，**且不是向量检索 RAG**。🔧 若属实，这是一个**为缓存友好而牺牲检索精度**的典型取舍 |
| **Projects** | 有 project instructions + Sources，可切换"仅项目记忆" |
| **Responses API** | `store=true` + `previous_response_id`；`truncation` 仅 `auto`/`disabled`（⚠️ **默认 disabled，超窗直接 400**），`auto` 丢弃的是**对话中间**的 items；⚠️ `previous_response_id` **不携带顶层 instructions**，且链上历史 token **仍全额计费**；默认存 30 天 |
| **Agents SDK** | `Session` 抽象（SQLiteSession / SQLAlchemySession）；⚠️ **Session 不能与 `conversation_id`/`previous_response_id` 混用**；`session_input_callback`、`SessionSettings(limit=N)`；`RunContextWrapper` **不发 LLM**（仅本地上下文）；handoff 默认传**完整历史** |
| **Codex CLI** | `AGENTS.md` **逐级拼接、越近优先级越高**；`project_doc_max_bytes` 默认 **32 KiB**；`model_auto_compact_token_limit` + **`PreCompact`/`PostCompact` hooks**；`/compact`；`codex resume`；`/status` 显示剩余上下文容量 |
| **缓存** | 自动前缀缓存；GPT-5.6+ 1.25× 写 / 0.1× 读 / 30m TTL；旧模型无写入费、读 0.1–0.5× 分档 |
| **可借鉴** | ⭐ **`PreCompact`/`PostCompact` hooks** 把压缩变成可编程事件；⭐ `project_doc_max_bytes` 用**硬预算**约束项目指令长度 |

---

### 6.10 补充三例：Cline / Gemini CLI / Devin(Cognition)

**Cline —— 记忆银行的 Markdown 流派**

| 维度 | 实现 |
|---|---|
| **记忆分层** | **Memory Bank**：一组自定义 markdown 文件的体系 —— `projectbrief.md` / `productContext.md` / `activeContext.md` / `systemPatterns.md` / `techContext.md` / `progress.md`。**程序记忆与情景记忆都用纯文件表达** |
| **规则** | `.clinerules` |
| **上下文管理** | checkpoint + 上下文窗口管理 + auto-condense |
| **⭐ 缓存策略** | **优先删"同一文件的旧版本读取"而非激进截断历史**（后者会 break cache）；**Auto Compact 复用既有缓存** |
| **工具治理** | MCP 文档从占 **30% system prompt** 改为 `load_mcp_documentation` **按需取** |
| **可借鉴** | ⭐ **"同一文件的重复读取"是最该优先清理的上下文垃圾** —— 既省 token 又不破缓存 |

**Gemini CLI —— 源码级发现：GEMINI.md 挂在最末尾**

| 维度 | 实现 |
|---|---|
| **⚠️ 反直觉发现** | 🏢 源码级：**GEMINI.md 实际挂在 system instruction 的最末尾**（不是最前），`tools` 是独立 API 字段 |
| **配置** | `contextFileName` 可配置；`/memory` 命令 |
| **缓存** | 隐式缓存默认开（最小 **2048 / 4096** token）；显式缓存按 token-hour 计费 |
| **⭐ 设计原则** | **"一个事实只存一份"** + 禁止写入会话态/改动摘要；`[Active Topic]` **追加到系统指令末尾**（而非开头） |
| **可借鉴** | ⭐ 与 Cline/Manus 一致：**易变信息追加到尾部** |

**Devin / Cognition —— 多 Agent 上下文共享的反面教材**

| 维度 | 实现 |
|---|---|
| **记忆分层** | Knowledge / Wiki / Playbooks（程序记忆）/ DeepWiki 索引；会话与 workspace 持久化 |
| **🏢 核心论断**（《Don't Build Multi-Agents》） | **写操作必须单线程**，子代理只读；否则并行子代理各自做**隐式决策**导致冲突 |
| **⚠️ 唯一被验证的例外** | **生成-验证环故意不共享上下文**（避免自我确认偏误）—— Devin Review 每 PR 抓 2 个 bug，**58% 为严重级别** |
| **可借鉴** | ⭐ **"子代理只读 + 主代理单线程写"** 是并发记忆的一致性问题的最小解；⭐ 验证者**必须**有独立上下文 |

---

## 7. 参考架构与落地路线

### 7.1 一张图：分层 × 缓存断点

```
┌─────────────────────────────────────────────────────────────┐
│ 请求组装顺序（自顶向下 = prompt 从前到后）                        │
├─────────────────────────────────────────────────────────────┤
│ ① 静态 system prompt + 工具定义      [全局共享]  ◆ 断点 1      │  L5 程序记忆（基座）
│ ② 项目指令 / rules / AGENTS.md        [项目共享]  ◆ 断点 2      │  L5 程序记忆（项目）
│ ③ 长期记忆快照（画像 / 事实）          [用户级]    ◆ 断点 3      │  L4 语义记忆
│ ④ 会话上下文（环境、MCP stub、技能索引）[会话级]   ◆ 断点 4      │  L5 + L1
│────────────────────── 缓存边界 ──────────────────────────────│
│ ⑤ 对话历史 + 工具输出 + 检索结果      [逐轮追加，不缓存/后置]     │  L2 + L3 + L1
│ ⑥ <system-reminder> / [Active Topic] / todo recitation         │  L1 工作记忆（末尾复述）
│ ⑦ 当前用户提问                                                 │
└─────────────────────────────────────────────────────────────┘
        ↑ 越靠上越稳定、越共享 → 缓存收益越高
        ↑ 所有"会变的信息"都走 ⑥ 而不是改 ①~④
```

**配套的后台管道**（不占前缀、不破缓存）：

```
对话结束 → [后台] L3 情景抽取（事件 + 结果 + 成败）
                ↓ consolidation
              L4 语义抽取（事实 / 画像）→ 冲突消解（作废 or ADD-only）
                ↓ 下次会话开始
              以"快照"形式注入 ③（整个会话内不变 → 可缓存）
```

🏢 参考实现印证：AWS AgentCore 的"事件 + 策略"、LangMem 的 `ReflectionExecutor.submit(after_seconds=)`、Letta 的 "dreaming"、Claude Code 的 auto memory —— **四家独立收敛到"后台抽取 + 会话开始时注入快照"这一形态**。

### 7.2 分阶段落地路线

| 阶段 | 目标 | 必做项 | 验收指标 |
|---|---|---|---|
| **阶段 1（1–2 周）** | 不破缓存 | 清时间戳出 system；固定工具顺序；append-only；确定性序列化；接命中率监控 | 命中率 >80%；miss 事件为 0 |
| **阶段 2（2–4 周）** | 结构正确 | 四段式重排 + 4 断点部署；检索结果后置；压缩改 cache-safe forking | 多轮会话命中率 >90%；总输入成本下降 >60% |
| **阶段 3（1–2 月）** | 记忆成形 | L2 checkpointer 落库；L3/L4 后台抽取管道；L5 规则文件化 + 按需加载 | 跨会话任务可续跑；记忆召回精度（用 LongMemEval 子集纵向对比） |
| **阶段 4** | 治理与规模化** | L0 原始日志；L6 元记忆与遗忘策略；路由亲和 + 多级缓存（自建时） | 记忆可审计可回滚；容量与成本可预测 |

### 7.3 最小可用伪代码（缓存友好版）

```python
# ── 会话启动：一次性组装稳定前缀，此后本会话内绝不修改 ──────────────
def build_cached_prefix(cfg):
    return [
        seg(STATIC_SYSTEM_PROMPT, cache=True),      # 断点 1：全局共享
        seg(load_project_rules(cfg.repo), cache=True),   # 断点 2：项目级
        seg(load_long_term_snapshot(cfg.user_id), cache=True),  # 断点 3：会话开始时快照
        seg(session_context(cfg), cache=True),      # 断点 4：环境 / MCP stub / 技能索引
    ]
# ⚠️ 严禁在此之后修改上述任何一段 —— 包括"更新时间"

# ── 每轮：只允许 append ─────────────────────────────────
def step(messages, tools):
    msgs = build_cached_prefix(cfg) + messages      # messages 单调增长
    if reminders:                                    # 动态信息走 messages，不改 system
        msgs.append(seg(f"<system-reminder>{reminders}</system-reminder>"))
    if todo_state:                                   # recitation 放末尾，对抗 lost-in-the-middle
        msgs.append(seg(f"<todo>{todo_state}</todo>"))
    msgs.append(current_user_turn)                   # 提问放最后
    return llm(messages=msgs, tools=tools)           # tools 恒定，用 logits mask / defer_loading 控制

# ── 压缩：cache-safe forking ─────────────────────────────
def compact(parent_prefix, parent_messages, reserve=0.15 * WINDOW):
    assert used_tokens(parent_messages) > WINDOW - reserve   # 预留 compaction buffer
    return llm(
        system   = parent_prefix.system,       # ← 与父完全一致
        tools    = parent_prefix.tools,        # ← 与父完全一致
        messages = parent_messages + [user("Summarize the conversation above.")],
    )                                          # 只有最后一句是新 token → 命中父前缀

# ── 记忆写入：后台，不占前缀 ─────────────────────────────
@background(after_seconds=30)   # 去抖
def consolidate(session_id):
    events = fetch_events(session_id)             # L3 情景
    facts  = extract_facts(events)                # L4 语义候选
    for f in facts:
        existing = search(f.user_id, f.key)
        if not existing:            store.add(f)                  # ADD-only
        elif contradicts(existing): invalidate(existing, at=now)  # 作废不删（双时序）
    # 下次会话开始时，把结果作为"快照"注入前缀 ③
```

---

## 8. 事实核验说明（引用前必读）

⚠️ **本报告纠正了流传甚广但已过期或有误的说法**：

| 常见说法 | 实际情况 |
|---|---|
| CoALA 的记忆分类源自 Tulving | ❌ CoALA **全文未引用 Tulving**；其路径是 **Soar + Baddeley & Hitch(1974) + Atkinson & Shiffrin(1968) + Lindes & Laird(2016) + Nuxoll & Laird(2007)** |
| "DMR 基准太简单"出自 LongMemEval | ❌ 出处是 **Zep, arXiv:2501.13956**；LongMemEval 全文未提 DMR/MemGPT |
| Anthropic 最小可缓存 1024(Sonnet)/2048(Haiku) | ❌ 已改为**按模型分档 512 / 1024 / 2048 / 4096** |
| OpenAI prompt cache 最小 128 token | ❌ 1024 才是最小长度；**128 是 `cached_tokens` 的统计取整口径** |
| DeepSeek 缓存命中 0.1× | ⚠️ 这是 **2024-08 历史公告**；当前实际 ≈ miss 的 **2.0%–3.3%** |
| DeepSeek 缓存粒度 64 token | ❌ 当前文档中**不存在**"64 token 最小单位"，以 **cache prefix unit** 为单位 |
| Gemini 显式缓存无存储费 | ❌ **显式缓存按 token-hour 收存储费**（2.5 Pro $4.50/1M/hr） |
| Graphiti README 有 benchmark 表 | ❌ **从未有过**（已核 main + 4 个 tag） |
| Graphiti 用 `search_type` 参数 | ❌ **不是 API 参数**；托管 API 用 `scope` + `reranker` |
| MemoryScope 有 LongMemEval 89.4% | ❌ **该仓库无任何论文与 benchmark**；数字属于后继项目 **ReMe** |
| Generative Agents 论文权重 α 全为 1 | ⚠️ 论文如此写，但**官方代码是 `gw=[0.5,3,2]`**、衰减 0.99（论文 0.995）—— 引用须注明 |
| Zep LOCOMO 94.7% 是能力提升 | ⚠️ 分数走过 84%→58.44%→75.14%→94.7%，**是评测方法论变迁** |
| Claude Code `TodoWrite` / `#` 快捷记忆 | ⚠️ `TodoWrite` 现已默认关闭，由 `TaskCreate/TaskGet/TaskList/TaskUpdate` 取代；`#` 快捷记忆在现行官方文档中不存在 |
| Letta 的 memory blocks / recall storage | ⚠️ **均为 legacy**；现为 Context Constitution + **MemFS（git-backed）** |
| Manus "缓存成本差 10 倍" 是通用值 | ⚠️ 是 **Claude Sonnet 的实测值**；其他模型不同（DeepSeek 当前约 30–50×） |

**未核验事项**（下游使用需谨慎）：Vertex AI 显式缓存单价、Anthropic-on-Bedrock 的 1.25×/0.1× 乘数、百炼 token-hour 存储费、Codex 自动压缩触发阈值（社区称约 94.7%，无官方确认）、TensorRT-LLM early reuse。

---

## 9. 参考来源（按主题）

**官方工程博客 / 一手设计文档**
- [Lessons from building Claude Code: Prompt caching is everything](https://claude.dev/blog/lessons-from-building-claude-code-prompt-caching-is-everything/) — 四段式布局、cache-safe forking、Plan Mode、defer_loading
- [The new rules of context engineering for Claude 5 generation models](https://claude.dev/blog/the-new-rules-of-context-engineering-for-claude-5-generation-models/) — 删除 80% system prompt 的六条新旧规则对照
- [Anthropic: Effective context engineering for AI agents](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) — compaction / structured note-taking / sub-agent 三件套、context rot
- [Manus: Context Engineering for AI Agents](https://manus.im/blog/Context-Engineering-for-AI-Agents-Lessons-from-Building-Manus) — KV-cache 命中率、mask don't remove、文件系统即上下文
- [Claude Code Memory 文档](https://code.claude.com/docs/en/memory) ｜ [Prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) ｜ [Context editing](https://platform.claude.com/docs/en/build-with-claude/context-editing) ｜ [Memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- [OpenAI Prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching) ｜ [Better prompt caching for GPT-6](https://openai.com/index/better-prompt-caching-for-gpt-6/)
- [Gemini API Caching](https://ai.google.dev/gemini-api/docs/caching) ｜ [DeepSeek Context Caching](https://api-docs.deepseek.com/guides/kv_cache) ｜ [阿里云百炼上下文缓存](https://help.aliyun.com/zh/model-studio/context-cache) ｜ [AWS Bedrock Prompt Caching](https://docs.aws.amazon.com/bedrock/latest/userguide/prompt-caching.html)
- [vLLM Prefix Caching 设计文档](https://docs.vllm.ai/en/latest/design/prefix_caching/) ｜ [SGLang RadixAttention 博客](https://www.lmsys.org/blog/2024-01-17-sglang/) ｜ [TensorRT-LLM KV Cache](https://nvidia.github.io/TensorRT-LLM/features/kvcache.html)
- [LangGraph Memory 概念](https://langchain-ai.github.io/langgraph/concepts/memory/) ｜ [AWS AgentCore Memory](https://docs.aws.amazon.com/bedrock-agentcore/latest/devguide/memory.html) ｜ [Vertex AI Memory Bank](https://cloud.google.com/vertex-ai/generative-ai/docs/agent-engine/memory-bank/overview) ｜ [Azure AI Foundry Memory](https://learn.microsoft.com/en-us/azure/ai-foundry/agents/concepts/what-is-memory)
- [Letta Context Constitution](https://docs.letta.com/) ｜ [Zep / Graphiti 文档](https://help.getzep.com/) ｜ [Mem0 文档](https://docs.mem0.ai/)
- [Cursor Rules](https://docs.cursor.com/context/rules) ｜ [Aider repo map](https://aider.chat/docs/repomap.html) ｜ [Cline Memory Bank](https://docs.cline.bot/) ｜ [Gemini CLI](https://github.com/google-gemini/gemini-cli) ｜ [Codex CLI AGENTS.md](https://github.com/openai/codex)

**论文**
- CoALA: *Cognitive Architectures for Language Agents* — [arXiv:2309.02427](https://arxiv.org/abs/2309.02427)
- MemGPT — [arXiv:2310.08560](https://arxiv.org/abs/2310.08560) ｜ Generative Agents — [arXiv:2304.03442](https://arxiv.org/abs/2304.03442) ｜ MemoryBank — [arXiv:2305.10250](https://arxiv.org/abs/2305.10250)
- HippoRAG — [arXiv:2405.14831](https://arxiv.org/abs/2405.14831) ｜ HippoRAG 2 — [arXiv:2502.14802](https://arxiv.org/abs/2502.14802) ｜ A-MEM — [arXiv:2502.12110](https://arxiv.org/abs/2502.12110)
- 综述：Zhang 2024 — [2404.13501](https://arxiv.org/abs/2404.13501) ｜ Wu 2025 (3D-8Q) — [2504.15965](https://arxiv.org/abs/2504.15965) ｜ Du 2025 (六算子) — [2505.00675](https://arxiv.org/abs/2505.00675) ｜ Dmitrenko 2026 — [Zenodo 20618996](https://zenodo.org/records/20618996)
- 评测：LongMemEval — [2410.10813](https://arxiv.org/abs/2410.10813) ｜ LoCoMo — [2402.17753](https://arxiv.org/abs/2402.17753) ｜ Zep — [2501.13956](https://arxiv.org/abs/2501.13956)
- KV cache：H2O — [2306.14048](https://arxiv.org/abs/2306.14048) ｜ StreamingLLM — [2309.17453](https://arxiv.org/abs/2309.17453) ｜ SnapKV — [2404.14469](https://arxiv.org/abs/2404.14469) ｜ CacheBlend — [2405.16444](https://arxiv.org/abs/2405.16444) ｜ SGLang — [2312.07104](https://arxiv.org/abs/2312.07104) ｜ Mooncake (FAST'25)

---

*本报告由 7 路并行深度调研 + 一手文档核验综合而成，底稿见 `research/` 目录。*
