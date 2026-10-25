# 记忆分层模型横向对比 & 中国大模型厂商记忆/上下文缓存能力

> 调研时间：2026-09；方法：web_search + web_fetch 一手官方文档与论文。
> 全文用 **【官方】** 标注官方文档/论文原文明确写出的数字，用 **【推断】** 标注笔者基于原文的计算或工程判断。

---

## 摘要

记忆分层在业界并未收敛，真正稳定的分歧只有两处：**(1) 情景/语义是否必须拆成两层；(2) 感知缓冲（L0）与程序记忆是否值得作为一等公民。** 综述（Zenodo 20618996）统计 19 个系统后结论是：L1 + 某种长期存储普遍存在，L0 与程序记忆在 LLM 时代系统里"大面积缺席"。而中国厂商已把"推理层记忆"做成了可计费商品：百炼显式缓存命中 10%、隐式 20%；智谱 50%；Kimi k3 为 1/10；MiniMax 为 1/5；DeepSeek 落到 1/30~1/50。

---

# A. 记忆分层模型的横向对比

## A.1 三层说法：短期 / 长期 / 工作

最常见的工程口语分层，本质是把"时间尺度"当唯一维度：

| 层 | 存什么 | 解决什么业务问题 |
|---|---|---|
| 工作记忆 Working | 当前这一步推理的活跃状态：scratchpad、工具调用中间结果、当前计划 | 让单次任务闭环，不让模型"想到一半忘了" |
| 短期记忆 Short-term | 当前会话的对话历史、最近工具结果、当前文件内容，物理载体=上下文窗口 | 多轮对话连贯性 |
| 长期记忆 Long-term | 跨会话持久化的偏好、事实、经验 | 跨会话"记得住人" |

**问题**：三层把"情景"和"语义"混在长期记忆里。综述指出，情景查询需要时间键索引（"X 那天发生了什么"），语义查询需要概念键索引（"我对 Y 知道什么"），**用同一个存储原语无法同时满足**，规模上来后检索成本会惩罚这种混淆。

## A.2 四层说法：CoALA（working / episodic / semantic / procedural）

CoALA（Sumers et al., *Cognitive Architectures for Language Agents*, arXiv:2309.02427）是把认知架构词汇引入 LLM Agent 的连接性论文，其记忆组件 = 短期工作记忆 + 长期记忆三分。

| 层 | 存什么 | 工程落地形态（【推断】） |
|---|---|---|
| Working | 当前决策周期的活跃信息 | 上下文窗口 + scratchpad |
| Episodic | 带时空上下文的经历（做过什么、结果如何） | 带时间戳的事件表 + 时间检索 |
| Semantic | 抽离情境的事实与概念 | 向量库 / 知识图谱 |
| Procedural | 技能与流程（怎么做事） | system prompt、工具描述、workflow 模板 |

**注意**：此层集没有"感知缓冲"，且 procedural 在实践中往往散落在代码、配置、prompt 字符串里，没有独立版本管理与审计——这正是综述批评的"架构性不完整"。

## A.3 五层 / 六层说法

**六层（推理栈视角）**：KV Cache → 系统提示 → 工作记忆 → 会话记忆 → 长期记忆 → 外部知识库。这是**按物理载体自下而上**切的，不是按认知功能切的：

| 层 | 物理载体 | 生命周期 |
|---|---|---|
| KV Cache | GPU 显存 / CPU DRAM / SSD（前缀缓存） | 秒~小时 |
| 系统提示 | 静态 prompt 前缀 | 版本级 |
| 工作记忆 | 上下文窗口内 | 单次任务 |
| 会话记忆 | 线程状态（thread state / checkpointer） | 单个会话 |
| 长期记忆 | DB / 向量库 | 跨会话、长期 |
| 外部知识库 | 对象存储 / 文件系统 / RAG 索引 | 与组织同寿 |

**L0–L5 元记忆版本**：L0 感知 / L1 工作 / L2 情景 / L3 语义 / L4 程序 / L5 元记忆。第六层指**对记忆本身的记忆**：知道"我记住了什么、置信度多少、什么时候该忘"。

## A.4 神经科学启发：ZenBrain 的 7 层架构

ZenBrain（Bering, Zenodo 19413933 v4；github.com/zensation-ai/zenbrain）整合 12 个神经科学模型，**7 层为：working、short-term、episodic、semantic、procedural、core、cross-context。**

- **working**：活跃任务焦点，容量 ~7 项（Miller 1956），任务结束后逐出到 short-term。
- **short-term**：会话上下文，会话边界时向 episodic / semantic 巩固。
- **episodic**：带时间戳的具体经历，支持时间查询。
- **semantic**：知识图谱 + Hebbian 加权边，由 episodic 巩固而来。
- **procedural**：成功工具调用模式、工作流模板、行为策略，靠重复成功执行强化。
- **core**：持久身份事实（用户偏好、人格特质、关键生平），**永不衰减、常驻上下文**（对应 MemGPT 的 pinned memory）。
- **cross-context**：跨隔离域（个人/工作/学习）的实体消解与选择性知识迁移，隐私感知、可配访问控制。

编排器 5 个操作：`store / recall / consolidate / decay / review`。

**数据【官方】**：多层路由 vs 扁平存储，LoCoMo F1 +21.6%（p=0.005），时间类查询 +176%；MemoryArena 跨会话依赖 +19.5%（依赖链 +53.5%）；三阶段睡眠巩固（SWS/REM/SHY）稳定性 +37%（Cohen's d=9.17）同时存储量 -47.4%；Hebbian 图谱 P@5 0.955（vs 均匀权重 0.200）；贝叶斯置信传播 AUC 0.533→0.797。
**消融【官方】**：去掉 episodic 掉 -11.8%、semantic -10.6%、procedural -0.6%，而 working / short-term / cross-context 在 LoCoMo 上**无可测影响**——作者承认这与 LoCoMo 只考长期对话记忆有关。

## A.5 学术综述：Zenodo 20618996 的统一五层 + 七维评估

Dmitrenko, *Memory Architectures for AI Agents in 2026: A Survey, a Synthesis, and an Evaluation Framework*（Zenodo 10.5281/zenodo.20618996，2026-06-10，CC-BY-4.0）。覆盖 19 个系统、3 个社区（5 个学术架构、8 个工业框架、6 个 PKM 方法论）。

**统一五层架构（C1 贡献）【官方】**：

| 层 | 名称 | 认知构念来源 | 工程对应物（原文 Table 1） |
|---|---|---|---|
| L0 | sensory | 感觉寄存器（Atkinson-Shiffrin 1968） | 预分类的摄入缓冲 |
| L1 | working | 短时存储 / 工作记忆（Baddeley-Hitch 1974） | 上下文窗口、scratchpad、当前任务状态 |
| L2 | episodic | 情景记忆（Tulving 1972） | 时间键事件存储，按新近度检索 |
| L3 | semantic | 语义记忆（Tulving 1972） | 图或向量的长期知识存储 |
| L4 | procedural | 程序记忆（Squire 1992） | **带版本管理的**提示词、工作流、决策规则 |

**收敛性发现【官方】**：L1 + 至少一层长期存储普遍存在；L2/L3 显式拆分只在 Mem0(graph)、Zep、Cognee 三家实现，其余五家合并；**L0 与 L4 在整个工业集合中缺席**（LLM 时代学术系统同样缺席），而 1980–90 年代的 SOAR/ACT-R/CLARION 反而五层齐全。作者判断这是"框架图省事发什么就发什么"，而非架构完整性推导的结果。

**七维评估框架（C4）【官方】**：D1 记忆类型覆盖（0–5）、D2 存储格式可移植性（0–2）、D3 图支持（0–2）、D4 时间感知（是否双时间轴 valid time vs transaction time，Zep 为 level 2 参考实现）、D5 LLM 无关性（0–2）、D6 MCP 兼容性（0–2）、D7 结构性风险（0–2），总分 0–17。
八框架实测：Zep/Graphiti 11、Mem0 10、LangGraph 9、AutoGen 9、Cognee 9、CrewAI 8、Semantic Kernel 7、OpenAI Vector Stores 1；**没有任何已部署工业框架同时做到"架构强 + 结构风险低"**。D7 的经验基础是 2018–2026 年 102 个开源数据基础设施项目中的 35 起 license/可持续性事件：**单厂商风投支持类约 44% vs 基金会治理类约 2.5%，约 18 倍差**。作者声明这是"风险透镜"而非预测器。

**其他可引用数字【官方】**：A-MEM 多跳推理准确率约翻倍 + 一个数量级 token 减少；Mem0 中位检索延迟 148 ms、p95 总延迟 1.44 s、LOCOMO 上约 90% token 削减（相对全上下文保留）。

## A.6 数据库 / 基础设施视角的分层

| 层 | 技术载体 | 擅长 | 典型失效模式 |
|---|---|---|---|
| 参数化记忆 | 微调权重 / 记忆编辑 | 内化风格与技能，零检索开销 | 更新贵、不可审计、易灾难性遗忘 |
| KV / 前缀缓存 | vLLM APC、SGLang RadixAttention、LMCache、Mooncake | 前缀完全相同的重复 prefill | 前缀一处变动即全失效；不省 decode |
| 会话状态 | Redis / Postgres / LangGraph checkpointer（thread-scoped） | 线程内状态恢复、断点续跑 | 与会话级存储混淆是生产环境最常见的记忆 bug 形态 |
| 向量库 | 语义检索（embedding + ANN） | 模糊语义召回 | 时间查询、多跳关系差 |
| 图数据库 | 关系/多跳推理（Zep/Graphiti 双时间轴） | 事实随时间变化、依赖链 | 运维复杂、写入放大 |
| 文档 / KV 存储 | 结构化事实、用户画像 | 精确读写、事务 | 不做检索 |
| 对象存储 / 文件系统 | Agent "外部记忆"（CLAUDE.md、.claude/rules/） | 人类可读、可版本控制、可迁移 | 无自动检索，需路径/导入机制 |

## A.7 分层方案对照表（行 = 层职责，列 = 流派）

| 层职责 | 3 层 | 4 层（CoALA） | 6 层（推理栈） | 6 层（L0–L5） | 7 层（ZenBrain） | 基础设施视角 |
|---|---|---|---|---|---|---|
| 原始输入缓冲 | — | — | — | L0 感知 | —（由 working 前置处理） | — |
| 活跃任务状态 | 工作记忆 | working | KV Cache + 系统提示 + 工作记忆 | L1 工作 | working | KV 前缀缓存 / 上下文窗口 |
| 会话内上下文 | 短期记忆 | working（含） | 会话记忆 | L1 工作（含） | short-term | 会话状态存储（checkpointer） |
| 具体经历（时间键） | 长期记忆（混） | episodic | 长期记忆（混） | L2 情景 | episodic | 事件表 + 向量库 |
| 抽象事实（概念键） | 长期记忆（混） | semantic | 长期记忆（混）+ 外部知识库 | L3 语义 | semantic | 向量库 / 图数据库 |
| 技能与流程 | —（散落在 prompt） | procedural | 系统提示（隐式） | L4 程序 | procedural | 文件系统（rules/*.md）、代码 |
| 身份 / 长期画像 | 长期记忆（混） | — | 长期记忆 | — | **core** | 文档存储 / 画像库 |
| 跨域迁移 | — | — | — | — | **cross-context** | 多租户命名空间 + 访问控制 |
| 外部知识 | — | semantic（含） | 外部知识库 | L3（含） | semantic（含） | 对象存储 / RAG 索引 |
| 记忆的元管理 | — | — | — | **L5 元记忆** | （Bayesian 置信 + decay 算法承担） | TTL / LRU / 版本管理 |
| 权重内化 | — | — | — | — | — | **参数化记忆**（微调） |

**映射要点【推断】**：
1. 3 层的"长期记忆"**折叠了 L2+L3+core**，选型时必须展开，否则会误判系统能力。
2. 7 层的 core 与 cross-context 在 5 层架构里**没有对位**，它们解决"人格稳定性"与"多域隔离下的知识迁移"，属业务诉求驱动而非认知构念驱动。
3. "6 层推理栈"与"L0–L5 元记忆"数量相同但**切法完全不同**：前者按存储介质与生命周期切，后者按认知功能切。容量规划用前者，能力评估用后者。
4. 综述与 ZenBrain 都指向同一空缺：**程序记忆缺少一等公民的表示格式与版本管理**。综述明确写"我们对 L4 的规格化程度远不如 A-MEM 对 L3 提供的经验证据"。

## A.8 推荐的工程分层（L0–L5）与生命周期策略

综合上述流派，给出可直接落地的 6 层。**（本节为【推断】的工程建议，非官方文档）**

| 层 | 名称 | 写入时机 | 读取时机 | 失效策略 |
|---|---|---|---|---|
| **L0** | 感知/摄入缓冲 | 每次外部输入（用户消息、工具返回、文件变更、网页抓取）到达时立即落原始副本 | 几乎不直接读；供 L1 组装与后续回溯 | TTL 分钟~小时级；原样保留不压缩，超时即丢或降采样归档 |
| **L1** | 工作记忆 | 每步推理产出（计划、工具中间结果、scratchpad）时追加 | 每一轮 LLM 调用前全量注入（必须常驻上下文） | 任务结束即丢弃；上下文接近上限时按"可重算性"裁剪；KV 前缀缓存随之前缀变动自然失效 |
| **L2** | 会话记忆 | 每轮对话与工具调用后追加（thread-scoped checkpointer） | 会话恢复、断点续跑、上下文压缩后重建时 | 会话结束 + 保留期到期即归档或删；压缩摘要替代原文时保留摘要 |
| **L3** | 情景记忆 | 任务/会话结束时写入"发生了什么 + 结果 + 成败"结构化事件 | 时间类查询、相似任务复用、失败复盘；具体任务开始时按时间窗 + 语义双路召回 | 按 Ebbinghaus 曲线衰减 + 重要性加权；低价值条目降级为统计摘要；保留硬性审计条目 |
| **L4** | 语义记忆 | 从 L3 巩固（consolidation）或人工/LLM 抽取事实时写入；用户画像走独立写入通道 | 每次任务规划前做 RAG/图谱检索；回答事实类问题前 | 事实冲突时用 TTL + 置信度覆盖（新证据优先）；双时间轴保留历史版本；低置信度条目不进入检索候选 |
| **L5** | 程序记忆 | 人工编写（指令型记忆）或从成功轨迹中沉淀（学习型记忆）；必须走 code review | 会话启动时按作用域加载；大型规则库按**路径/上下文条件**懒加载 | 版本化管理（Git），变更即失效旧版本；作用域优先级固定为 组织 > 项目 > 用户 > 本地 |

**横切扩展层（按需）**：
- **L6 元记忆**：记录"每层有哪些条目、置信度、上次访问时间、命中率"，用于自动调参与遗忘决策。开放权重评审时，这一层是唯一能回答"它到底记住了什么"的地方。
- **参数化记忆**：只有稳态、低频变更的领域技能才值得进微调；高频变化的事实永远留在 L4。

**工程红线（【推断】）**：
1. **前缀稳定性优先于缓存策略**：所有厂商的缓存都是前缀匹配，把易变的（时间戳、随机 ID、当前任务状态）放尾部是收益最高的单一优化。
2. **写出"记忆卸载点"**：不要依赖模型自动记住一切；明确哪些信息必须落到 L2/L3/L5 的持久载体上，否则上下文压缩后必然丢失。

---

# B. 中国大模型厂商与开源生态的记忆 / 上下文缓存能力

## B.1 阿里云百炼 / 通义千问

官方文档：https://www.alibabacloud.com/help/zh/model-studio/context-cache

**显式缓存（Context Cache）【官方】**

| 项目 | 值 |
|---|---|
| 开启方式 | messages 中加 `"cache_control": {"type": "ephemeral"}` |
| 创建缓存计费 | 标准输入单价 **125%** |
| 命中计费 | 标准输入单价 **10%**（qwen3.8-max/0902、qwen3.8-flash、qwen3.8-2.4t-a95b 例外，见控制台） |
| 最小 token | **1024** |
| 有效期 | **5 分钟**，命中后重置 |
| 回溯窗口 | 每个标记向前最多 **20 个 content 块** |
| 标记数量 | 单请求最多 **4 个**（超出则最后 4 个生效） |
| 返回字段 | `usage.prompt_tokens_details.cache_creation_input_tokens` / `cached_tokens`；Anthropic 兼容为 `cache_creation_input_tokens` / `cache_read_input_tokens` |
| 隔离 | 账号级 + 模型级隔离，不跨账号/跨模型共享 |

**隐式缓存【官方】**：自动开启且**无法关闭**；命中按标准输入单价 **20%**（GLM 部署版本 25%，deepseek-v4.1-flash 10%）；最小 token 1024（智谱部署的 GLM 为 512）；**无固定 TTL**，系统定期清理。显式/隐式互斥。

**长期记忆（记忆库）【官方】**：`https://dashscope.aliyuncs.com/api/v2/apps/memory/`，分**事实记忆 / 用户画像 / 技能记忆**三类，另有事实记忆多模态版本。**2026-08-20 10:00 起商业化计费**：

| 计费项 | Pro | Lite |
|---|---|---|
| 事实记忆写入 | ¥0.03/次 | ¥0.018/次 |
| 事实记忆多模态写入 | ¥0.1/次 | — |
| 画像记忆写入 | ¥0.03/次 | ¥0.025/次 |
| 技能记忆写入 | ¥0.3/次 | — |
| 记忆检索 | ¥0.001/次（含 Rerank） | ¥0.00002/次（无 Rerank） |
| 存储 | ¥0.002/万条/小时 ≈ ¥1.44/万条/月 | — |

免费的额度：写入 1,500 次、检索 5,000 次（3 个月有效）、存储 10,000 条（长期）。**注意【官方】**：检索结果注入 Prompt 会额外消耗大模型 token，费用另计。

## B.2 智谱 GLM

**Coding Plan 记忆机制文档【官方】**：https://docs.bigmodel.cn/cn/coding-plan/learning-resources/memory-mechanism

- 记忆分为 **自动记忆、项目记忆、会话记忆**；架构为 `Short-term → Session context` + `Long-term → semantic / episodic / procedural`。
- 主要记忆类型 5 种：**Session Memory / Project Memory / Semantic Memory / Episodic Memory / Procedural Memory**。
- **分层管理 5 个作用域**：组织级 > 项目级 > 用户级 > 本地级 > 子代理（角色）级；用户级规则先于项目级加载，但项目级优先级更高。
- **关键原则**：区分**指令型记忆**（人写的规范，如 CLAUDE.md）与**学习型记忆**（Agent 从纠正/偏好/失败经验中积累的），"混为一体会导致系统行为逐渐偏离预期"。
- 工程约束：主记忆文件建议 **≤200 行**；规则拆到 `rules/*.md` 按主题与**路径**限定加载；支持 `@path/to/import` 递归导入与 symlink 复用（规则包）。subagent 系统提示会自动包含 `MEMORY.md` 前 200 行。
- 故障排查明确指出：**上下文压缩后，只有写入记忆文件的内容才持续存在**——这条与 A.8 的"记忆卸载点"完全一致。

**上下文缓存【官方】**：https://docs.bigmodel.cn/cn/guide/capabilities/cache
隐式自动缓存；建议触发前缀 **≥500 token**；命中价**通常为标准价格的 50%**；返回字段 `usage.prompt_tokens_details.cached_tokens`。文档明确标注"仅适用于标准 API 计费，**不包括资源包和 GLM Coding Plan 套餐**"——即 Claude Code/ZCode 类订阅制用量不享受缓存折扣计价。

## B.3 DeepSeek

**机制【官方】**：https://api-docs.deepseek.com/zh-cn/guides/kv_cache + /news/news0802/

- **默认全量开启**，无需改代码、无需换接口。
- **落盘规则三条**：① 每次请求的"用户输入结束位置"与"模型输出结束位置"各产生一个**缓存前缀单元**；② 检测到多次请求存在公共前缀时，把公共前缀作为独立单元落盘；③ 长输入/长输出中按**固定 token 间隔**截取前缀单元落盘。
- **命中条件是"完整匹配缓存前缀单元"**（官方说明：受 Sliding Window Attention 影响，每条缓存前缀是独立完整单元，不再是无级前缀匹配）。
- **粒度**：以 **64 token 为一个存储单元，不足 64 token 的内容不会被缓存**。
- **TTL**：无手动配置项；"缓存不再使用后会自动被清空，时间一般为**几个小时到几天**"；缓存构建耗时为秒级。
- **返回字段**：`usage.prompt_cache_hit_tokens` 与 `usage.prompt_cache_miss_tokens`。
- **隔离**：每用户缓存独立、逻辑互不可见；缓存占用存储不额外收费。

**价格【官方】**：2024-08-02 公告为**命中 0.1 元/百万 tokens**（未命中 1 元/百万，即 **0.1x**），并称"最高可节省 90% 费用，即使不做优化，用户整体节省也超 50%"；128K 输入且大部分重复时，实测首 token 延迟从 **13 秒降到 500 毫秒**。
**2026 年现行价格【官方】**（https://api-docs.deepseek.com/zh-cn/quick_start/pricing）：deepseek-flash 命中 0.02 元（空闲）/0.04 元（高峰），未命中 1 元/2 元；deepseek-v4-pro 命中 0.15/0.30 元，未命中 4.5/9.0 元。
**【推断】** 按现行价目表计算，命中/未命中比约为 **1/50（Flash）与 1/30（Pro）**，已远优于早期公告的 0.1x；"命中 0.1x"应作为历史结论引用，不宜当作当前价格承诺。

## B.4 字节豆包 / 火山引擎、月之暗面 Kimi、MiniMax

### 火山引擎豆包（火山方舟 Context API）
官方文档：https://www.volcengine.com/docs/82379/1396491 （该页现标注为"历史文档/已下线"，官方推荐迁移到 **Responses API**）

- **两种显式缓存**：`Context Create API`（创建 `context_id`，mode 为 `session` 或 `common_prefix`）与 `Responses API`（`"caching": {"type": "enabled"}`，加 `"prefix": true` 即前缀缓存）。
- **TTL**：Context API 支持配置 TTL，**最大 168 小时**；过期机制为"当前时刻 − 最近使用时刻"，**使用后重新激活并重置生命周期**。Responses API 配置的是**过期时刻**（UTC 秒级时间戳），最大 `当前时间 + 604800` 秒（**7 天**），且**不随使用重置**。
- **差异**：Context API 前缀缓存的初始信息"一旦写入不可更改"；Responses API 可删除任意一轮缓存信息、可调用往期缓存 ID；前缀缓存**支持并发**，且**不支持 Partial（Prefill Response）模式**，仅支持在线推理。
- **计费四项【官方】**：输入（元/千 token，不含缓存前缀）、输出、**缓存输入**（"通常低于输入 token 费用"，具体倍数未在文档给出，需查后付费价目表）、**存储**（元/千 token/小时，按自然小时内所有前缀存储的最大 token 数计）。返回字段 `usage.prompt_tokens_details.cached_tokens`。
- **【推断】** 火山是本次调研中**唯一把"存储费"单列为计费项的主流厂商**（Kimi 也拆了 Cache Write，但那是写入费而非按小时存储费）。长 TTL 场景下需要按"命中次数 × 缓存输入单价"对比"存储时长 × 存储单价"做盈亏平衡。

### 月之暗面 Kimi
官方文档：https://platform.kimi.com/docs/guide/context-caching

**kimi-k3 定价【官方】**（每 1M tokens）：Input（未命中）¥20；Cache Write（5m）¥20；Cache Write（1h）¥40；**Cached Input（命中）¥2** —— 官方原文"缓存命中价格仅为未命中价格的 **1/10**"。

- **TTL 两档**：`5m`（默认）与 `1h`，通过 `prompt_cache_options: {"mode": "implicit", "ttl": "..."}` 设置。Anthropic Messages API 用顶层 `cache_control`（消息体内的同名标记被忽略）。
- **TTL 首次写入即锁定**，不可改写；命中**免费按原 TTL 续期**；`5m` 与 `1h` 两个缓存池**不互通**。
- **无手动清除**，无活动超过 TTL 后自动过期。缓存按**组织（org）隔离**，组织内共享。
- **盈亏平衡【官方】**：前缀 1M tokens 时，`1h` 比 `5m` 多付 ¥20 写入费，但每次命中省 ¥18，**命中 2 次即净省 ¥16**。
- **支持范围**：`kimi-k3` 支持 Cache Write；`kimi-k2.7` / `kimi-k2.7-highspeed` / `kimi-k2.6` **不支持**。
- **字段**：Chat Completions 用 `prompt_tokens_details.cached_tokens` + `cache_write_tokens`；Messages API 用 `cache_read_input_tokens` / `cache_creation_input_tokens`，并有 `cache_creation.ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens` 细分两档。

### MiniMax
官方文档：https://platform.minimax.io/docs/api-reference/text-prompt-caching

- **两套并存**：Prompt Caching（被动/隐式，自动识别，不改调用方式）与 Explicit Prompt Caching（Anthropic API 的 `cache_control`）。
- **触发门槛：输入 ≥512 token**。
- **前缀匹配顺序固定为「工具列表 → system prompts → user messages」**，任一模块内容变化都会影响命中。
- **定价【官方】**（MiniMax-M3，输入 ≤512k）：input $0.60/1M、output $2.40/1M、**cache hit $0.12/1M** → **命中为标准的 20%（1/5）**。官方示例：5 万输入 token、4.5 万命中时，总成本从 $0.0324 降到 $0.0108，**省约 66.7%**。注意：>512k 输入时**长上下文价格同样适用于命中 token**。
- **过期**：被动缓存"过期时间根据系统负载自动调整"；显式缓存 5 分钟过期、持续使用自动续期，且**首次写入额外收费**。

## B.5 开源侧：vLLM / SGLang / LMCache / Mooncake

### vLLM Automatic Prefix Caching（APC）
源码：`vllm/config/cache.py`，文档：https://docs.vllm.ai/en/latest/features/automatic_prefix_caching/

- **默认开启【官方源码】**：`enable_prefix_caching: bool = True`；`DEFAULT_BLOCK_SIZE = 16`。文档 API 仍写"Set `enable_prefix_caching=True` in vLLM engine to enable APC"（历史写法）。
- 相关参数：`prefix_match_unit`（前缀缓存键计算的最细 token 边界，可细于物理 block size）、`prefix_caching_hash_algo`（默认 sha256）、`kv_offloading_size` + `kv_offloading_backend`（native / lmcache）。
- **限制【官方】**：APC **只减少 prefill 时间，不减少 decode 时间**；答案很长或新查询不与任何已有查询共享前缀时**无收益**。

### SGLang RadixAttention
博客：https://lmsys.org/blog/2024-01-17-sglang/ ；论文：arXiv:2312.07104

- **机制【官方】**：用 **radix tree（压缩前缀树）** 保留 prompt 与生成结果的 KV cache，支持高效前缀搜索/插入/驱逐；**LRU 驱逐**（递归驱逐叶节点）+ **cache-aware 调度**提升命中率；KV tensor 以 paged layout 存于 GPU，page = 1 token；CPU 侧维护树结构，维护开销小。
- **命中率/吞吐数据【官方】**：论文摘要称最高 **6.4x 吞吐**（相对当时 SOTA 推理系统）；博客称最高 **5x 吞吐**（基线 vLLM 0.2.5 / Guidance 0.1.8 / TGI 1.3.0，Llama-7B on A10G、Mixtral-8x7B on 8×A10G）。博客明确指出"消融研究显示**即使在无缓存命中时也没有可察觉开销**，因此我们在运行时**始终开启** RadixAttention"。
- **【推断】** 6.4x 来自论文全量任务集，5x 来自博客的图 1/图 2 任务子集；对外引用建议标注"论文 6.4x / 博客 5x"。且该数据是 2023 年末基线，对今日 vLLM/SGLang 的对比已不具代表性。

### LMCache
文档：https://docs.lmcache.ai/ （架构页）

- **四层存储【官方】**：GPU 显存（活跃工作集）→ CPU DRAM（热缓存，固定内存，高效 GPU-CPU 传输）→ 本地存储（本地磁盘 / NVMe GDS，大容量层）→ 远程存储（Redis、Mooncake、InfiniStore 等，持久层）。
- **缓存索引默认块粒度 256 token**；哈希方案可配；**默认 LRU 逐出**。
- 两种模式：**存储模式**（KV 卸载，跨查询/跨会话持久化，可在进程重启后存续）与**传输模式**（P/D 分离，P2P + NIXL 低延迟高带宽传输）。
- 控制器 API：lookup / clear / compress(CacheGen) / move / pin / health，支持集中管理单实例与分布式部署。

### Mooncake（Moonshot AI + 清华，FAST'25 最佳论文级工作）
论文：https://www.usenix.org/system/files/fast25-qin.pdf

- **架构【官方】**：KVCache-centric 分离式架构。分离 prefill 集群与 decoding 集群，并**把 GPU 集群闲置的 CPU / DRAM / SSD / NIC 资源pooling 成 disaggregated KVCache（Mooncake Store）**，通过 RDMA（最高 8×400 Gbps）互联。中心是全局调度器 **Conductor**，做 KVCache-aware 调度（swapping + replication：最热块多副本防取回拥塞，最冷块换出）。
- **存储细节【官方】**：KVCache 以 **paged block** 存储，block size **16–512 token**（由模型规模与最优网络传输尺寸决定）；每个 block 的 hash key = 自身哈希 + 前缀哈希，用于去重；**LRU 逐出**（正在被请求访问的块不逐出）；对象 API `put / get / change_replica`。
- **收益数字【官方】**：
  - 真实 trace 下，**有效请求容量提升 59%~498%**（同时满足 SLO）；
  - 生产环境 Kimi 在 **A800 / H800 集群上分别多处理 115% / 107%** 的请求；
  - 全局缓存 vs 本地缓存的**缓存命中率最高提升 2.36x**（论文摘要口径）/ "最大提升 136%"（§5.3.2 口径），**prefill 计算时间最多减少 48%**；
  - 自研 transfer engine 比 TCP **快 2.4x（4×200 Gbps）/ 4.6x（8×400 Gbps）**；
  - 128k 输入下，前缀缓存**减少 prefill 时间 92%**、**TTFT 降 86%**（含调度/传输/加载开销后）；
  - 容量分析：本地 3M token 缓存在多数工作负载下达不到**理论最大命中率的 50%**；**50M token 容量接近理论上限**，需 pooling 至少 20 个节点的 DRAM；
  - 建议**最低网络带宽 100 Gbps**，低于该值 TTFT 急剧恶化；
  - 现网规模：**数千节点、每天处理超 1000 亿 token**。

## B.6 "官方明说" vs "推断"速查

| 结论 | 性质 |
|---|---|
| 百炼显式创建 125% / 命中 10% / 最小 1024 token / TTL 5 分钟 | 【官方】 |
| 百炼隐式命中 20%，GLM 部署 25%，deepseek-v4.1-flash 10% | 【官方】 |
| 百炼记忆库 2026-08-20 商业化，Pro/Lite 单价如上 | 【官方】 |
| 智谱缓存命中"通常为标准价的 50%" | 【官方】（"通常"为其原文措辞） |
| 智谱缓存折扣**不适用** Coding Plan 套餐 | 【官方】 |
| DeepSeek 64 token 存储单元、无手动 TTL、几小时~几天自动清空 | 【官方】 |
| DeepSeek "命中 0.1 元/M、0.1x" | 【官方】但为 **2024-08 历史公告** |
| DeepSeek 现行折扣约 1/50（Flash）、1/30（Pro） | **【推断】**（据现行价目表计算） |
| 火山 Context API TTL 最大 168h、Responses 最大 7 天且不随使用重置 | 【官方】 |
| 火山"缓存输入通常低于输入 token 费用"（无具体倍数） | 【官方】但无数字 |
| Kimi k3 命中 1/10；1h 档命中 2 次即回本 | 【官方】 |
| MiniMax 门槛 512 token、命中 20%、前缀顺序"工具→system→user" | 【官方】 |
| vLLM `enable_prefix_caching` 默认 True、block size 16 | 【官方源码】 |
| SGLang 论文 6.4x / 博客 5x 吞吐，无命中时无可感开销 | 【官方】，但基线为 2023 年末 |
| Mooncake 全部收益数字 | 【官方论文】 |
| A.8 的 L0–L5 分层与生命周期策略 | **【推断】**（工程建议） |
| 分层对照表的映射关系 | **【推断】**（基于各流派原文的对应） |

---

## C. 结论

1. **分层数量之争是伪命题，切分维度才是真问题。** "6 层推理栈"按存储介质与生命周期切，适合容量与成本规划；"L0–L4 认知五层"按认知功能切，适合能力评估与选型。混用会导致"我们已经有 6 层了"这类误判——按认知口径，多数系统只有 3 层（L1 + L2/L3 合并）。
2. **最被低估的两层是 L0 感知缓冲与程序记忆。** 综述在 19 个系统上的统计、ZenBrain 的消融、智谱"Coding Agent 指令型 vs 学习型记忆"的提醒，三条独立证据同向：把 system prompt / rules / workflow 当成**有版本、有作用域、可审计的程序记忆层**管理，是当前投产比最高的一次架构升级。
3. **缓存定价已明显分化，且折扣不等于总成本。** 命中折扣从 MiniMax 的 1/5、百炼隐式的 1/5、智谱的 1/2，到 Kimi k3 与百炼显式的 1/10，再到 DeepSeek 的 1/30~1/50。但火山的按小时存储费、Kimi 的 Cache Write 前置费、百炼显式 125% 的创建费，都会让"命中次数少"的场景反而变贵——选型应先算**预期命中次数**，再比单价。
4. **所有厂商共享同一条物理约束：前缀匹配。** 把稳定内容（system prompt、工具定义、代码库、参考文档）放前面、易变内容放后面，是唯一不需要写代码就能拿到的收益。百炼文档甚至给出"工具定义序列化顺序必须一致""并行 tool 消息要合并以减少 content 块数"这类细节——失效成本是整段缓存归零。
5. **开源栈的分层已比厂商 API 清晰。** LMCache 的四层存储与 Mooncake 的 disaggregated KVCache 池，把"KV 前缀缓存"从 server 优化开关变成了**可独立规划容量的基础设施层**。Mooncake 那条"本地 3M token 达不到理论命中率一半、50M 才接近上限"的容量分析，应作为自建推理集群容量规划的第一性参考。

---

## 参考链接

**A. 分层模型**
- Dmitrenko, *Memory Architectures for AI Agents in 2026: A Survey, a Synthesis, and an Evaluation Framework*, Zenodo, 2026-06-10 — https://zenodo.org/records/20618996
- Bering, *ZenBrain: A Neuroscience-Inspired 7-Layer Memory Architecture for Autonomous AI Systems* (v4), Zenodo, 2026-04-04 — https://zenodo.org/records/19413933 ；代码 https://github.com/zensation-ai/zenbrain
- Sumers et al., *Cognitive Architectures for Language Agents* (CoALA) — https://arxiv.org/abs/2309.02427

**B. 厂商与开源**
- 阿里云百炼 上下文缓存 — https://www.alibabacloud.com/help/zh/model-studio/context-cache
- 阿里云百炼 长期记忆 API 概览 / 计费 — https://help.aliyun.com/zh/model-studio/memory/api-overview/ 、https://help.aliyun.com/zh/model-studio/memory/billing
- 智谱 上下文缓存 — https://docs.bigmodel.cn/cn/guide/capabilities/cache
- 智谱 Coding Plan 记忆机制 — https://docs.bigmodel.cn/cn/coding-plan/learning-resources/memory-mechanism
- DeepSeek 上下文硬盘缓存 — https://api-docs.deepseek.com/zh-cn/guides/kv_cache
- DeepSeek 硬盘缓存公告（2024-08-02） — https://api-docs.deepseek.com/zh-cn/news/news0802/
- DeepSeek 模型 & 价格 — https://api-docs.deepseek.com/zh-cn/quick_start/pricing
- 火山方舟 上下文缓存（Context API） — https://www.volcengine.com/docs/82379/1396491
- Kimi 上下文缓存 — https://platform.kimi.com/docs/guide/context-caching
- MiniMax Prompt Caching — https://platform.minimax.io/docs/api-reference/text-prompt-caching
- vLLM Automatic Prefix Caching — https://docs.vllm.ai/en/latest/features/automatic_prefix_caching/ ；源码 https://github.com/vllm-project/vllm/blob/main/vllm/config/cache.py
- SGLang RadixAttention 博客 — https://lmsys.org/blog/2024-01-17-sglang/ ；论文 https://arxiv.org/abs/2312.07104
- LMCache 架构文档 — https://docs.lmcache.ai/
- Mooncake, FAST'25 — https://www.usenix.org/system/files/fast25-qin.pdf
