# Agent 记忆分类体系与 KV / Prompt Cache 机制调研报告

> 覆盖：A. Agent 记忆的学术分类体系；B. KV Cache / Prompt Cache 的底层机制、前缀缓存实现、淘汰压缩研究与云厂商计费。
> **标注**：📄 论文结论（附 arXiv/DOI）；🏢 厂商官方文档；🔧 本报告工程推断或跨来源归纳。
> **时效性**：调研时点官方文档的模型代际已是 Claude Opus 5.5 / GPT‑5.6 / Gemini 3.8 Flash / DeepSeek‑V4.1‑Flash / Qwen3.8‑max。任务书中若干"经典数字"（Anthropic 最小 1024/2048、OpenAI 最小 128、DeepSeek 0.1×）已被官方更新，下文给出当前官方值并标注差异。

---

## A. Agent 记忆的学术分类体系

### A.1 认知科学来源

| 概念 | 原始出处 | Agent 中的对应物 |
|---|---|---|
| 多存储模型（感觉/短时/长时） | Atkinson & Shiffrin (1968)（CoALA 引用） | 多模态输入 → 上下文窗口 → 外部持久存储 |
| 工作记忆 | Baddeley & Hitch (1974), *Psych. of Learning and Motivation* 8:47–89 | **上下文窗口 / prompt**，被显式当作工作记忆 |
| 情景 vs 语义 | Tulving (1972), *Organization of Memory*, pp. 381–403（[PsycNet](https://psycnet.apa.org/record/1973-08477-007)）；Tulving (1985) *Elements of Episodic Memory*, ISBN 9780198521259 | 情景 = 对话/轨迹/事件序列；语义 = 事实、KG、用户画像、RAG 库 |
| 程序记忆（非陈述性） | Squire (1992), DOI [10.1162/jocn.1992.4.3.232](https://pubmed.ncbi.nlm.nih.gov/23964880/)；Squire & Zola (1996), DOI [10.1073/pnas.93.24.13515](https://www.pnas.org/doi/10.1073/pnas.93.24.13515) | 权重（隐式）+ 代码、工具、提示模板、技能库（显式） |

📄 **溯源更正**：把三分法带进语言智能体的**不是 Tulving**——CoALA 全文**未出现 "Tulving"**。其映射路径是 **Soar**：CoALA §2.3 称工作记忆（Baddeley and Hitch, 1974）存放"近期感知输入、目标与中间推理结果"，长时记忆分程序（存放产生式系统本身）、语义（世界事实，Lindes and Laird 2016）、情景（过去行为序列，Nuxoll and Laird 2007），整体框架源自 Atkinson & Shiffrin（[CoALA §2.3](https://arxiv.org/html/2309.02427v3)）。论文若需 Tulving 出处须独立引用。🔧 该路径顺畅的原因是二者近乎同构：权重≈程序/语义、上下文≈工作记忆、外部存储≈情景；真正错配在于人脑情景记忆是**重建式**的，Agent 的是**逐字记录式**（[arXiv:2505.00675 §5](https://arxiv.org/html/2505.00675v3) 称"认知差距"）。

### A.2 CoALA 的四分类

[CoALA（arXiv:2309.02427）](https://arxiv.org/abs/2309.02427) §4.1："short-term working memory and several long-term memories: episodic, semantic, and procedural."

| 模块 | 存什么（📄 原文要点） | 读/写动作 | 人类对应 |
|---|---|---|---|
| **Working memory** | 当前决策周期中"active and readily available information as symbolic variables"：感知输入、active knowledge（推理所得或自长时记忆检索）、上轮延续的目标。关键论断：它是**"a data structure that persists across LLM calls"** | reasoning ＝**读+写 WM**；每次调用从 WM 抽子集合成 LLM 输入，输出解析回 WM 变量 | 工作记忆 |
| **Episodic** | "experience from earlier decision cycles"：训练输入输出对、事件流、游戏轨迹 | 规划时检索进 WM；learning 时把经验 WM→情景 | 情景记忆 |
| **Semantic** | "knowledge about the world and itself"。经典 RAG ＝对固定、**只读**语义记忆的读取 | 检索；亦可写入 LLM 推理所得新知识 | 语义记忆 |
| **Procedural** | **隐式**（LLM 权重）+ **显式**（代码：实现动作的过程 + 实现决策的过程） | 前向计算即执行；写＝改权重或改代码。📄 强调它"必须由设计者初始化"，写它"风险显著更高…容易引入 bug 或让智能体颠覆设计者意图" | 程序记忆 |

📄 CoALA 使读写语义精确化：**retrieval ＝自长时记忆读入 WM；reasoning ＝用 LLM 更新 WM；learning ＝写入长时记忆**。检索按类型分化（§4.3）：Voyager→程序记忆、Generative Agents→情景记忆、DocPrompting→语义记忆。学习动作（§4.6）四类：更新情景、更新语义、更新 LLM 参数、更新代码。

### A.3 三篇综述的分类维度

| 综述 | 维度 | 取值 |
|---|---|---|
| [Zhang 2024, arXiv:2404.13501](https://arxiv.org/abs/2404.13501) | 来源 / 形式 / 操作 | 来源：inside-trial、cross-trial、external knowledge；形式：textual、parametric；操作：**writing / management / reading** |
| [Wu 2025, arXiv:2504.15965](https://arxiv.org/abs/2504.15965) | **3D-8Q**：object × form × time | personal/system × parametric/non-parametric × short/long；八象限依次对应工作、情景、工作、语义、工作、程序、工作（KV-Cache）、语义+程序记忆（[Table 1](https://arxiv.org/html/2504.15965v2)） |
| [Du 2025, arXiv:2505.00675](https://arxiv.org/abs/2505.00675) | 表征 + 时间尺度 + 功能类型 + 原子操作 | 表征：parametric / contextual-unstructured / contextual-structured；**六算子＝Consolidation、Indexing（Encoding）；Updating、Forgetting（Evolving）；Retrieval、Condensation（Adapting）** |

📄 第三篇批评 Zhang et al. 只覆盖 writing/management/reading 这类高层操作、**漏掉 indexing**，故给出形式化算子（`M_{t+Δt} = Consolidate(M_t, E_{[t,t+Δt]})`、`Forget(M_t, F)`，[§2.4](https://arxiv.org/html/2505.00675v3)）。🔧 三者互补：Zhang 给**操作生命周期**、Wu 给**存储坐标**、Du 给**原子算子**。

### A.4 代表性架构的机制细节

| 架构 | 机制（📄） | 关键数字 |
|---|---|---|
| **MemGPT**（[2310.08560](https://arxiv.org/abs/2310.08560)） | OS 类比：**main context（＝RAM，即 prompt tokens）** vs **external context（＝disk：archival + recall）**。main context 三段拼成：**system instructions**（只读：控制流＋层级说明＋function schema）、**working context**（定长读写块，**只能经函数调用写**）、**FIFO 队列**（队首为"被逐出消息的递归摘要"）。分页：超 **warning（约 70% 窗口）**插入"内存压力"消息让模型主动 flush；超 **flush（约 100%）**逐出**约 50% 窗口**消息并生成新递归摘要，被逐出者永久留在 recall。编辑与检索"entirely self-directed"；`request_heartbeat=true` 归还控制权实现函数链式调用 | DMR（基于 Multi-Session Chat：5 个 session×约 12 条消息 + 第 6 个 session 单条 QA；ROUGE-L＋GPT-4 判官）：GPT-3.5 Turbo 38.7%→**66.9%**；GPT-4 32.1%→**92.5%**；GPT-4 Turbo 35.3%→**93.4%**。嵌套 KV：140 个 UUID 对（约 8k token）、30 种排序，GPT-4 到 3 层嵌套即 0%，MemGPT+GPT-4 "unaffected" |
| **Generative Agents**（[2304.03442](https://arxiv.org/abs/2304.03442)） | memory stream（自然语言记录流）；检索 `score = α_rec·recency + α_imp·importance + α_rel·relevance`；**reflection**：近期事件重要性累计超阈值即对更高层洞察递归生成 | 📄 论文 §4.1 称 **"all αs are set to 1"**、衰减因子 **0.995**；但[官方代码](https://raw.githubusercontent.com/joonspk-research/generative_agents/main/reverie/backend_server/persona/cognitive_modules/retrieve.py)实际用 `gw=[0.5,3,2]`（顺序 recency/relevance/importance），`scratch.py` 主流程 `recency_decay=0.99`——**论文与代码不一致，引用须注明**。importance 由 LLM 打 1–10 分（"cleaning up the room"→2，"asking your crush out"→8，idle 强制 1）；relevance＝余弦相似度；三项 min-max 归一化；每次取 **top-30**。反思触发：**重要性累计 >150**，实际**每天 2–3 次**；生成 3 个 focal point、每个 5 条洞察；洞察以 thought 节点存，**expiration＝+30 天**，**depth＝1+max(被引用节点 depth)**（树深递归定义、无固定上限） |
| **MemoryBank**（[2305.10250](https://arxiv.org/abs/2305.10250)） | 分层事件摘要（日→全局）+ 动态人格画像；DPR 式双塔 + **FAISS** 检索。**Ebbinghaus**：`R = e^(−t/S)`，S 首次提及初始化为 **1**，被回忆时 **S+=1 且 t 归零**，从而降低遗忘概率 | 📄 论文自陈只模拟 3 条 Ebbinghaus 原则（遗忘速率、时间衰减、间隔效应），模型"exploratory and highly simplified"。评测：10 天、15 个虚拟用户、**194 道探针题**（97 英+97 中）；英文 ChatGPT 检索 0.763 / 正确性 0.716 / 连贯性 0.912 |
| **HippoRAG**（[2405.14831](https://arxiv.org/abs/2405.14831), NeurIPS'24） | 海马体索引理论：LLM＝新皮层、开放 KG＝人工海马索引、检索编码器＝合成 PHR。离线：1-shot OpenIE **两步**（先抽实体，再把实体喂进抽三元组的 prompt）建无 schema KG，对 `cos>τ` 的实体加**同义边**；建 \|N\|×\|P\| 的"名词短语→段落"计数矩阵。在线：查询 NER → 查询节点 → KG 上跑 **Personalized PageRank**，段落得分 `P^T π′`；**node specificity** `s_i=\|P_i\|^(−1)` 在 PPR 前乘到查询节点概率上（IDF 的局部替代） | 📄 τ=**0.8**、PPR 阻尼=**0.5**（在 100 条 MuSiQue 样本上调参）。平均 R@2/R@5：HippoRAG(ColBERTv2) **57.4/72.9** vs ColBERTv2 53.9/65.6、BM25 46.5/58.4；多跳 QA 最高 **+20%**；单步检索媲美/超过 IRCoT 而成本低 **10–30×**、快 **6–13×**；离线索引约慢 10×、每万段落贵 $15。消融：去 node specificity→54.7/70.9，只用查询节点不做 PPR→50.7/56.2。后继 [HippoRAG 2 (2502.14802)](https://arxiv.org/abs/2502.14802) 报告关联记忆 **+7%** |
| **A-MEM**（[2502.12110](https://arxiv.org/abs/2502.12110), NeurIPS'25） | Zettelkasten 原子 note：`m_i={c_i,t_i,K_i,G_i,X_i,e_i,L_i}`（内容、时间戳、LLM 生成的关键词/标签/上下文描述、嵌入、链接集）。**链接生成**：余弦取 **top-k（k=10）** 作候选，再由 **LLM 决定真正链接**（嵌入只做过滤）。**记忆演化**：`m*_j ← LLM(m_n ‖ M_near\m_j ‖ m_j ‖ P_s3)`，演化结果**替换**原 note——新记忆会反向改写邻域记忆的上下文/关键词/标签 | LoCoMo 上（GPT-4o-mini，F1/BLEU-1）：多跳 **45.85/36.67** vs LoCoMo 18.41/14.77、MemGPT 25.52/19.44；时间推理 12.14/12.00；token 消耗 **2,520** vs LoCoMo 16,910 / MemGPT 16,977。注意**对抗问题上 MemGPT 更强**（69.23 vs 50.03） |
| **评测基准** | **LongMemEval**（[2410.10813](https://arxiv.org/abs/2410.10813), ICLR'25）：**5 项能力**（信息抽取、多会话推理、时间推理、知识更新、弃答），实际标注 **7 种题型**（single-session-user 14%、-assistant 11%、-preference 6%、multi-session 27%、knowledge-update 16%、temporal 27% ＋ 由他型改写的 30 道"假前提"弃答）；把记忆拆成 **indexing→retrieval→reading**。**LoCoMo**（[2402.17753](https://arxiv.org/abs/2402.17753)）：问答分单跳/多跳/时间/开放域/对抗。**DMR**：MemGPT 提出的会话级检索基准 | LongMemEval：**500** 题；_S≈**115k** token（约 40 session），_M＝500 session≈**1.5M** token；长上下文 LLM 掉 **30%–60%**（离线通读 GPT-4o 0.9184 vs ChatGPT 0.5773）；事实 key 扩展 **+9.4% recall@k / +5.4% 准确率**，时间感知 query 扩展 **+6.8–11.3%**。LoCoMo：**50** 段对话，平均 **304.9 轮 / 19.3 session / 9,209 token**，最多 35 session，**7,512** 条 QA；gpt-4-turbo 最佳 F1 **32.4** vs 人类 **87.9**；摘要任务弃用 BLEU/ROUGE 改用 **FactScore**。**DMR 批评出处更正**：LongMemEval 全文**未提及** DMR/MemGPT；权威出处是 **Zep（[2501.13956](https://arxiv.org/abs/2501.13956)）**——"每段仅 60 条消息，轻易塞进现代 LLM 上下文"、"只考单轮事实检索"、"全上下文方法几乎刷满，凸显该基准不足以评估记忆系统" |

### A.5 记忆分类的多维矩阵

🔧 综合 A.1–A.4 归纳：

| 维度 | 取值 1 | 取值 2 | 取值 3 | 取值 4 |
|---|---|---|---|---|
| **① 时间跨度** | 轮内临时 | 会话内短期（窗口、KV cache） | 跨会话长期（天–月，MemoryBank/Mem0） | 准静态（权重、领域知识库） |
| **② 认知类型** | 工作记忆（prompt 装配层） | 情景记忆（对话、轨迹、案例） | 语义记忆（事实、画像、KG） | 程序记忆（权重、代码、工具、技能） |
| **③ 数据结构** | 非结构化文本流（小镇 memory stream） | 向量索引（RAG、A-MEM 链接） | 图结构（HippoRAG KG+PPR、Zep/Graphiti） | 结构化表/SQL/KV（ChatDB 类） |
| **④ 写入时机** | 热路径同步写（每轮 append） | 热路径异步写（fire-and-forget） | 后台批处理（会话后反思/摘要/建图） | 离线训练期写入（微调/蒸馏进权重） |
| **⑤ 检索方式** | 全量注入（长上下文） | 稠密向量 top-k | 混合+重排（BM25+向量+reranker） | 图遍历/PPR、时间过滤、SQL 精确查询 |
| **⑥ 作用域** | 单用户私有 | 单会话/单任务 | 单 Agent 跨会话（用户画像） | 多 Agent 共享（注意隐私与投毒） |
| **⑦ 可控性** | 显式（可读可写可删、可审计） | 半显式（可读，但由 LLM 自主写） | 隐式参数化（只在权重里） | — |
| **⑧ 生命周期算子** | Consolidation / Indexing | Updating / Forgetting | Retrieval / Condensation | —（取自 [2505.00675](https://arxiv.org/abs/2505.00675)） |

---

## B. KV Cache / Prompt Cache 的底层机制与工程影响

### B.1 prefill / decode 的计算特性

📄 自回归生成 `P(w_i|w_<i)` 逐 token 采样；为免每步重算历史而缓存每层每头 K/V，其显存随序列长度与 batch **线性增长**（[H2O §1](https://arxiv.org/abs/2306.14048)）。🔧 两阶段特性：

| 阶段 | 计算形态 | 瓶颈 | 结果 |
|---|---|---|---|
| **Prefill** | 一次前向处理全部 prompt token，大矩阵乘（GEMM），算术强度高 | **compute-bound**（FLOPS） | 决定 **TTFT**，长 prompt 的主要成本 |
| **Decode** | 每步仅 1 token，GEMV ＋ 对全部历史 KV 的注意力 | **memory-bound**（显存带宽） | 决定 TPOT/吞吐，算力大量闲置 |

🔧 故**前缀命中＝完全跳过该段 prefill**，TTFT 近似只与未命中 token 数成正比；厂商省下的是 compute-bound 算力，故 cache read 常低至 0.1× 乃至 0.02×（B.5）。反之缓存**不能**加速 decode——这解释了"降价但不改变输出速度上限"。

### B.2 前缀缓存的四种实现

| 实现 | 索引结构 | 粒度与哈希 | 淘汰策略 | 关键事实 |
|---|---|---|---|---|
| **vLLM APC** | **全局哈希表**（不为块维护树） | 🏢 块标识＝链式哈希 `hash(父块哈希, 块内 token, extra hashes)`，extra 含 LoRA ID、多模态哈希、多租户 `cache_salt`；**只缓存整块**；**`DEFAULT_BLOCK_SIZE = 16`**，另有 `prefix_match_unit` 表示命中可落到的最细 token 边界 | 空闲块耗尽时先淘汰引用计数 0 者，其中选 **LRU**；访问时间相同则淘汰"位于最长前缀末端"的块 | 🏢 官方称该策略在 full attention 下**等价于 RadixAttention**；`enable_prefix_caching` **默认 True**；哈希默认 **sha256**（另有 `sha256_cbor`/`xxhash`/`xxhash_cbor`），文档警告非加密哈希"在多租户下可能泄露私有信息"；释放块**逆序**入队尾（"请求最后一块哈希最多、最不可能被复用，应最先淘汰"）（[设计文档](https://docs.vllm.ai/en/latest/design/prefix_caching/)） |
| **SGLang RadixAttention** | **radix tree**：token 序列→KV 张量；KV 为非连续分页布局，**每页＝1 token**；树存 CPU | 前端永远发完整 prompt，运行时自动前缀匹配、复用、插入 | **LRU 且优先淘汰叶子**——"淘汰叶子后可继续复用共同祖先，直到祖先也成为叶子"；节点引用计数为 0 才可淘汰 | 🏢📄 论文：吞吐最高 **6.4×**、延迟最低 **3.7×**，命中率 **50%–99%**；cache-aware scheduling 平均达最优命中率 **96%**（Theorem 3.1：按 radix tree DFS 顺序访问即达最优）；100 条 ShareGPT 请求总耗时 74.3 s 中结构管理仅 0.2 s（**<0.3%**）。博客口径 **5×**；生产（Chatbot Arena 一个月）命中率 LLaVA-Next-34B **52.4%**、Vicuna-33B **74.1%**，首 token 延迟平均降 **1.7×**（[博客](https://www.lmsys.org/blog/2024-01-17-sglang/)、[2312.07104](https://arxiv.org/abs/2312.07104)） |
| **TensorRT-LLM** | paged KV ＋ **radix 搜索树**；按（窗口、head 数）分池；块大小须为 **>1 的 2 的幂** | 块键须为 **256 位摘要的加密哈希（SHA-256）**——"前缀匹配仅凭 digest 相等判定，不再逐 token 比对" | **优先级 LRU**：块优先级 0–100（默认 **35**），同优先级取 LRU，**只有叶子块可淘汰** | 🏢 `enable_block_reuse` **默认 True**（需 `--use_paged_context_fmha enable`）；`tokens_per_block` 新 LLM API 默认 **32**、旧 `trtllm-build` 为 **128**；`free_gpu_memory_fraction` 默认 0.9；`max_num_tokens` 自 v0.11 默认 8192；KV cache 可量化 FP8/NVFP4（Llama-3.3-70B TP4 H100：**3389.5→5299.6 tok/s，+56%**）；多模态**不兼容**块复用；KV cache events 可让外部路由（如 Dynamo）导向持有该前缀的引擎（[官方文档](https://nvidia.github.io/TensorRT-LLM/features/kvcache.html)） |
| **DeepSeek Context Caching on Disk** | 🏢 **硬盘缓存**，默认开启、无需改代码、**无写入费** | 🏢 以 **cache prefix unit** 为单位，**必须完整匹配**。三类生成时机：① **请求边界**（用户输入结束处、模型输出结束处各产生一个单元）；② **公共前缀检测**（发现共享前缀则持久化为独立单元）；③ **固定 token 间隔**（长输入/输出切出多个单元）。**"64 token 最小单位"在当前文档中不存在** | 不用时自动清除，通常**几小时到几天**；构建需数秒；"best-effort，不保证 100% 命中" | 🏢 `prompt_cache_hit_tokens`/`prompt_cache_miss_tokens`。示例：`A+B` 与 `A+C` 不命中，但系统识别公共前缀 `A` 并持久化，第三轮 `A+D` 命中 `A`；输出仍受 temperature 影响（[官方文档](https://api-docs.deepseek.com/guides/kv_cache)） |

### B.3 淘汰与压缩研究

| 方法 | 机制（📄） | 关键数字 |
|---|---|---|
| **H2O**（[2306.14048](https://arxiv.org/abs/2306.14048)） | 少数 token 贡献大部分注意力分数（Heavy Hitters），且**移除它们性能显著下降**；据此**动态保持"近期 token＋H2 token"的平衡**，并把 KV 淘汰形式化为**动态子模问题**并给出理论保证 | **20%** heavy hitters 配置下，吞吐相比 DeepSpeed Zero-Inference / HF Accelerate / FlexGen 最高 **29× / 29× / 3×**；同 batch 延迟最高降 **1.9×** |
| **StreamingLLM**（[2309.17453](https://arxiv.org/abs/2309.17453), ICLR'24） | **attention sink**：Softmax 须归一化，多余注意力被倾倒到全局可见的初始 token；**仅保留最初 4 个 token 的 KV ＋ 滑动窗口**即可恢复 window attention 性能。实现关键：**Keys 在施加 RoPE 之前即被缓存**，解码每步再按（缓存内）位置旋转 | Llama-2-13B 困惑度：纯滑窗 0+1024＝**5158.07** vs 4+1020＝**5.40**；1–2 个 sink 不够（7B：1+4095=11.88、2+4094=10.51、4+4092=9.59）；稳定处理 **>400 万 token**；相比滑窗重算基线最高 **22.2×** |
| **SnapKV**（[2404.14469](https://arxiv.org/abs/2404.14469)） | 每个注意力头持续关注 prompt 中特定特征，该模式可从 **prompt 末尾的 observation window**（探测用 128 token）提前观测；据此"投票＋池化"：窗口注意力权重在 prefix key 上求和，`pool1d` 聚簇后每头 `topk`，再把选中 prefix KV 与**整段**窗口 KV 拼接；免微调 | 16K 输入下生成速度 **3.6×**、显存效率 **8.2×**；16 个长序列数据集精度相当；单张 A100-80GB 支持 **380K** 上下文，Needle-in-a-Haystack 几乎无损 |
| **CacheBlend**（[2405.16444](https://arxiv.org/abs/2405.16444), EuroSys'25） | 直击"**复用的 chunk 不是前缀**"：前缀缓存精确是因为 KV 与前文无关，但 RAG 中"除第一个 chunk，其余 chunk 的 KV 都无法复用"；直接复用会**忽略该 chunk 与前文之间的 cross-attention** 从而答错。CacheBlend **选择性重算少量 token 的 KV**（按 **KV deviation** 选 token，逐层用缓存的未选 token KV 扩展 K/V，使注意力仍覆盖全序列） | **更新比例 <15%** 即达同等生成质量；TTFT 降 **2.2–3.3×**、吞吐升 **2.8–5×**（对比全量重算，质量无损）；相比"全量 KV 复用"，QA 绝对 F1 高 **+0.1–0.2**、摘要 Rouge-L 高 **+0.03–0.25** |

#### 为什么"改中间内容导致后面全部失效"

📄 三层原因：① **因果注意力＋前缀哈希链**——vLLM 块标识为链式 `hash(prefix tokens + block tokens)`，前缀一变其后每块哈希全变；② **KV 本身是被前文"污染"过的计算结果**——CacheBlend 指出 KV cache 是"每个输入 token 的注意力与其前文关系的张量拼接"，位置 *i* 的变化会改变所有位置 >*i* 的 K/V；③ **RoPE 使绝对位置参与 KV**——RoPE 以旋转矩阵编码绝对位置（[2104.09864](https://arxiv.org/abs/2104.09864)），StreamingLLM §3.2 证实 key 在旋转前缓存、每步按新位置重旋转。**故即使内容完全相同，插入/删除造成的位置位移也会令后续 KV 失效**。

🏢 厂商原文佐证：OpenAI——"Cache reuse requires the entire rendered prefix to match. If content or a relevant setting changes before a breakpoint, the prefix after that change cannot match the existing cache entry."；Anthropic——"Because the hash is cumulative… changing any block at or before the breakpoint produces a different hash"，层级 `tools→system→messages`，"Changes at each level invalidate that level and all subsequent levels"（改工具定义→全失效；改 `system`→system+messages 失效；改 `tool_choice` 或增删图片→仅 messages 失效）；Bedrock——"Because the sections are chained, changing content in an earlier section invalidates the cache for later sections"。

### B.4 缓存友好上下文编排原则

🔧 以下 12 条为工程推断，各条附机制依据：

1. **Append-only**：新增一律追加到末尾，绝不重写既有 turn。🏢 OpenAI："Append new messages rather than rewriting earlier turns… summarization, compaction, or context truncation can change the prefix and reset cache reuse."
2. **稳定前缀、单调增长**：跨请求不变的内容放最前（工具定义→系统指令→项目指令→长期记忆→会话历史），使可复用前缀随轮次单调变长——这正是命中率随会话增长上升的原因。
3. **易变内容后置**：时间戳、用户 ID、当前提问、检索结果放**最后一个断点之后**。🏢 断点之后按未缓存输入计费且**不产生 cache write 费用**，故后置同时省写入费。
4. **时间戳/动态值不进 system**：🏢 Anthropic 的典型陷阱——system 含时间戳会使前缀哈希每次都变，而"lookback 只找历史上**写入过**的条目"，结果是"每次付写入费却永远读不到"。
5. **断点放在"最后一块跨请求相同的内容"上**，而非自动断点默认的"最后一个可缓存块"（🏢 Anthropic 自动缓存在"静态前缀＋易变后缀"结构下恰好落错位置）。
6. **断点覆盖"每轮增长量"**：🏢 Anthropic **回看窗口 20 个 block**（断点自身算第 1 个），一轮新增 ≥20 块会让旧写入滑出窗口，须**预先**在较早位置再放断点；上限 **4 个**。🏢 Bedrock 与百炼同为"回看约 20 个 content block"。
7. **多级缓存与断点一一对应**：`tools`→`system`→**项目/仓库指令**→**长期记忆（画像/事实）**→**会话历史**→**检索结果/工具输出**。🏢 Anthropic 的 4 断点＋分层失效模型、Bedrock 的链式语义正为此设计。
8. **检索结果/工具输出尽量放最后一个断点之后**；若必须缓存（如反复追问同一长文档），把**文档本体**放前缀设断点、**问题**放断点后（🏢 DeepSeek"长文本问答"示例即此形态）。
9. **工具定义与顺序保持稳定**：🏢 Anthropic 提示 JSON key 顺序被某些语言随机化会破坏缓存，改 `tool_choice` 亦失效；🏢 OpenAI 建议用 `allowed_tools` 替代"删工具定义"，用 `defer_loading`＋append 动态加载。
10. **压缩与缓存的冲突**：压缩必然重写前缀 → 首个压缩后请求大面积 miss。🏢 OpenAI 立场是不必因噎废食："**输入 token 更少时即使命中率下降也可能更省钱**"。🔧 三种缓解：① **追加式摘要**——不动原历史，把摘要作为新消息追加（🏢 Anthropic 部分模型支持对话中途追加 `role:system` 而不失效；🏢 OpenAI 在 GPT‑6 上可追加 `configuration_update` 改 effort 而保留前缀）；② **压缩点对齐断点边界**并对新前缀**预热**；③ 压缩阈值取断点结构的整数倍。
11. **主动预热**：🏢 Anthropic 支持 `max_tokens:0`（只读入并写缓存，无输出计费）；🏢 OpenAI 支持 `prompt_cache_options.prewarm:true`（按 cache-write 费率计费）。
12. **路由与缓存局部性**：🏢 OpenAI 明确缓存**存在单机上**，超约 **15 req/min** 可能触发溢出路由；<GPT‑5.6 须用稳定的 `prompt_cache_key` 把共享前缀请求路由到同一机器，并把高流量按确定性映射拆到多个 key。🔧 结论：**命中率不只是 prompt 结构问题，也是流量路由问题**。

**回本点公式**（🏢 OpenAI 官方成本模型）：最小可缓存长度 `M`、原前缀 `L<M`、读倍率 `r`、写倍率 `w`、请求数 `N` ⇒ `L_break-even = M·(r + (w−r)/N)`；`L > L_break-even` 时把前缀"撑"到 M 反而更便宜。例：`M=1024, r=0.1, w=1.25` ⇒ 约 `102.4 + 1177.6/N`，10 次请求下 ≥221 token 就值得撑到 1024。🏢 Anthropic 口径一致：缓存写只多花 25%、命中省 90%，**只要发生至少一次命中总成本即低于不用缓存**（5 分钟档 1 次读取回本，1 小时档 2 次）。

### B.5 云厂商 Prompt / Context Cache 计费对比

| 厂商 | 触发方式 | 缓存写入 | 命中（读） | TTL | 最小可缓存长度 | 其他要点 |
|---|---|---|---|---|---|---|
| **Anthropic**（[定价表](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)） | 显式 `cache_control`，或顶层 `cache_control` 自动缓存（断点自动前移） | **1.25×** 基础输入（5 分钟）；**2×**（1 小时） | **0.1×** | 5 分钟默认（命中免费续期）/ 1 小时 | **512**（Fable 5.1、Mythos 5.1、Opus 5.5/5、Fable 5、Mythos 5）；**1024**（Opus 4.8、Sonnet 5/4.6/4.5、Opus 4/4.1、Sonnet 4）；**2048**（Mythos Preview、Opus 4.7、Haiku 3.5）；**4096**（Opus 4.6/4.5、Haiku 4.5）。**"1024 Sonnet/Opus、2048 Haiku"的分法已过期** | 例外：Fable 5.1/Mythos 5.1 读为 **0.025×**、Opus 5.5 为 **0.05×**；最多 **4** 断点；生命周期**自请求开始计**（流式时间也占 TTL）；层级 `tools→system→messages`；回看 **20 块**；缓存条目须等首个响应开始才可用（并发需串行化首个请求）；`max_tokens:0` 预热；乘数与 Batch 折扣、数据驻留（美国区 1.1×）叠加 |
| **OpenAI**（[官方文档](https://developers.openai.com/api/docs/guides/prompt-caching)） | **默认开启**；GPT‑5.6+ 支持 `implicit`/`explicit` 与 `prompt_cache_breakpoint` | GPT‑5.6+：**1.25×**；**旧模型无写入费** | GPT‑5.6+：**0.1×**；旧模型分档——**0.25×**（gpt‑4.1 系、o3、o4‑mini）、**0.5×**（gpt‑4o 系、o1、o3‑mini）、**0.1×**（GPT‑5 系、gpt‑5.5） | GPT‑5.6+：**30m**（`ttl` 唯一取值）；旧模型 `in_memory` 约 **5–10 分钟**不活跃（最长 1 小时）、`24h` 约 30 分钟（最长 24 小时）；无 ZDR 默认 `24h` | GPT‑5.6+ 为 **1024** 可见输入 token；旧模型随请求设置（tools/图像/输出 schema/effort/verbosity）变化。**"128"是统计口径而非最小长度**：`cached_tokens` "向下取整到 128 的倍数" | 官方成本账：1 写 1 读＝**1.35×** vs 不缓存 2×；1 写 9 读＝**2.15×** vs 10×。每请求最多 **4** 次 cache write；GPT‑5.5 隐式断点按 **2048** token 间隔；查询自最长前缀向短遍历；缓存存于单机、>15 req/min 可能溢出、不跨组织/区域共享；官方示例命中率：单轮判官约 **70%**、多轮 Agent **>90%**；命中 token **仍计入 TPM 限流** |
| **Google Gemini**（[隐式](https://ai.google.dev/gemini-api/docs/caching)、[显式](https://ai.google.dev/gemini-api/docs/generate-content/caching)、[定价](https://ai.google.dev/gemini-api/docs/pricing)） | **隐式默认开启**（Gemini 2.5 及以后）；显式走 generateContent API 的 cache 对象 | 隐式：无额外费（按普通输入计） | 隐式：命中部分自动折扣 | 隐式：系统管理；显式：TTL 可配，**默认 1 小时** | **4096**（Gemini 3.8/3.7/3.6/3.5 Flash、3.1 Pro Preview）；**2048**（2.5 Flash / 2.5 Pro） | **显式缓存是二维计费：cached-input 单价 ＋ 按 token-hour 的存储价**（付费档，USD）：3.8/3.7/3.6 Flash cached **$0.075/1M**（2026‑12‑31 前）＋存储 **$0.50/1M tokens/小时**（此后 $0.15＋$1.00），输入 $0.75；3.5 Flash $0.15＋$1.00/hr；**2.5 Pro $0.125/1M（≤200k）或 $0.25（>200k）＋$4.50/1M tokens/小时**；2.5 Flash‑Lite $0.01＋$1.00/hr。**"2.5 Pro $1.00/hr、Flash $0.10/hr"及"Gemini API 显式缓存无存储费"均与当前官方页不符**；Vertex AI 另有独立价目表（数值**未核验**）。命中量看 `usage.total_cached_tokens`/`usage_metadata` |
| **DeepSeek**（[缓存文档](https://api-docs.deepseek.com/guides/kv_cache)、[定价](https://api-docs.deepseek.com/quick_start/pricing)） | **默认开启**，硬盘缓存，无需改代码 | **无额外写入费** | 远低于 miss 价，按"缓存前缀单元"完整匹配 | 不用时自动清除，通常**几小时到几天** | 文档不给固定最小长度，以 **cache prefix unit** 为单位 | 官方价（USD/1M，低谷/高峰）：`deepseek-flash` 命中 **$0.003/$0.006** vs miss **$0.15/$0.30**（≈ miss 的 **2.0%**）；`deepseek-v4-pro` 命中 **$0.022/$0.044** vs miss **$0.66/$1.32**（≈ **3.3%**）。**早期 V2/V3 时代"命中＝0.1×"的口径已不适用**（0.2 vs 2 CNY 那一对亦已从当前页移除） |
| **AWS Bedrock**（[官方文档](https://docs.aws.amazon.com/bedrock/latest/userguide/prompt-caching.html)） | 隐式（自动、尽力而为）＋显式（`cachePoint`/`cache_control` 检查点） | 视模型而定，"**可能高于**标准输入价"；GPT‑5.5 及更早的 OpenAI 模型在 Bedrock 上**无写入费** | 按模型 cache-read 费率；**书面写出 1.25×/0.1× 的只有 Bedrock 上的 GPT‑5.6**（"write 1.25×, reads 90% discount"）。**Anthropic 模型在 Bedrock 上的 1.25×/0.1× 未在 AWS 文档中找到（未核验）** | 多数模型 **5 分钟**（命中重置）；新版 Claude 及 GPT‑5.6 支持 1 小时；**GPT‑5.6 为 30 分钟** | **512**（Opus 5.5/5、Fable 5/5.1、Mythos 5/5.1）；**1024**（Opus 4.8、Sonnet 5/4.6/4.5、Claude 3.7/3.5 Sonnet、GPT‑5.6 系列）；**4096**（Opus 4.7/4.6/4.5、Haiku 4.5）。最小长度按 `tools`+`system`+`messages` **累计**计算，检查点之间无最小间隔 | 最多 **4** 检查点；顺序 `tools→system→messages`，改早期段失效后期段；**长 TTL 检查点须排在短 TTL 之前**；简化模式回看约 **20** 个 content block；仅按需端点、不支持 Batch；跨区域推理"可能增加缓存写入"；`total input tokens = inputTokens + cacheReadInputTokens + cacheWriteInputTokens` |
| **阿里云百炼 / Qwen**（[上下文缓存](https://help.aliyun.com/zh/model-studio/context-cache)） | **显式**：消息 content 数组内 `cache_control:{"type":"ephemeral"}`，最多 **4** 标记；**隐式**：自动、**无法关闭** | 显式：创建缓存 token 按输入单价 **125%**；隐式：**100%**（无额外费） | 显式：命中 token 按输入单价 **10%**；隐式：**20%**（例外：`deepseek-v4.1-flash` 10%、GLM 5.3/5.2 为 25%、qwen3.8-max 等按控制台价） | 显式 **5 分钟**（命中后重置），**未文档化 1 小时档**；隐式不承诺，系统定期清理 | 两者均 **1024** token（隐式达 1024 仅代表"具备命中条件"） | **显式与隐式互斥**；20 块回看；缓存创建发生在模型响应**之后**；"工具定义不支持独立缓存"；官方算例：1 万输入 token、5 千命中 ⇒ 输入成本为无缓存的 **60%**；**未有按 token-hour 的存储费文档（未核验）**；PTU 部署按折扣系数折算 |

🏢 **横向总结**：Anthropic / Bedrock / 百炼的**显式缓存语义高度趋同**（1.25× 写＋0.1× 读、5 分钟 TTL、可选 1 小时、约 20 块回看、最多 4 断点），差别在**最小长度分档**与**读折扣**；OpenAI 自 GPT‑5.6 起补齐"1.25× 写＋0.1× 读"并给出 30 分钟 TTL，旧模型呈 0.1/0.25/0.5× 三档历史遗留；Gemini 与 DeepSeek 走"默认隐式＋极低命中价"路线，**DeepSeek 当前实际折扣已达 ~2–3%**，显著低于业界 10%；Gemini 是唯一**显式收取 token-hour 存储费**的主流厂商。

---

## C. 结论与工程建议

1. **分类体系已收敛**：CoALA 的 working/episodic/semantic/procedural 是事实标准骨架，2025 年综述补上**表征维度**与**算子维度**；选型先在这两维定位，再谈数据结构。
2. **溯源准确性两处更正**：CoALA 的人类记忆映射来自 **Soar + Baddeley & Hitch + Atkinson & Shiffrin**，**不是 Tulving**；DMR "太简单"的批评来自 **Zep（arXiv:2501.13956）**，**不是 LongMemEval**。这是二手资料最常见的错误归属。
3. **写入时机最被低估**：热路径写保一致性但推高延迟、且**破坏前缀缓存**；后台写保缓存但引入陈旧性。🔧 建议默认**"热路径只 append 原始记录，派生记忆全部后台生成"**——同时满足记忆质量与缓存友好。
4. **缓存编排只有一条主线**："**稳定内容在前、易变内容在后、只追加不改写、断点对齐稳定性边界**"；多级断点、prewarm、工具 append-only、后置检索结果皆其推论。**压缩应被视为"开一个新前缀"**：压缩后必然一次全量 miss（可摊销），关键是降低频率、压缩后立即预热、并以**总输入成本**而非命中率决策。**短前缀（<1024 token）在所有厂商都拿不到收益**，此时主动把有用内容撑到阈值反而最优。

---

### 主要来源清单

**A**：CoALA <https://arxiv.org/abs/2309.02427>（全文 <https://arxiv.org/html/2309.02427v3>）｜<https://arxiv.org/abs/2404.13501>｜<https://arxiv.org/abs/2504.15965>｜<https://arxiv.org/abs/2505.00675>｜MemGPT <https://arxiv.org/abs/2310.08560>｜Generative Agents <https://arxiv.org/abs/2304.03442>（检索实现见仓库 retrieve.py）｜MemoryBank <https://arxiv.org/abs/2305.10250>｜HippoRAG <https://arxiv.org/abs/2405.14831>（2 代 <https://arxiv.org/abs/2502.14802>）｜A-MEM <https://arxiv.org/abs/2502.12110>｜LongMemEval <https://arxiv.org/abs/2410.10813>｜LoCoMo <https://arxiv.org/abs/2402.17753>｜Zep/DMR 批评 <https://arxiv.org/abs/2501.13956>｜Tulving 1972 <https://psycnet.apa.org/record/1973-08477-007>｜Squire 1992 <https://pubmed.ncbi.nlm.nih.gov/23964880/>

**B**：vLLM APC <https://docs.vllm.ai/en/latest/design/prefix_caching/>｜SGLang <https://www.lmsys.org/blog/2024-01-17-sglang/> / <https://arxiv.org/abs/2312.07104>｜TensorRT-LLM <https://nvidia.github.io/TensorRT-LLM/features/kvcache.html>｜DeepSeek <https://api-docs.deepseek.com/guides/kv_cache> / <https://api-docs.deepseek.com/quick_start/pricing>｜H2O <https://arxiv.org/abs/2306.14048>｜StreamingLLM <https://arxiv.org/abs/2309.17453>｜SnapKV <https://arxiv.org/abs/2404.14469>｜CacheBlend <https://arxiv.org/abs/2405.16444>｜RoPE <https://arxiv.org/abs/2104.09864>｜Anthropic <https://platform.claude.com/docs/en/build-with-claude/prompt-caching>｜OpenAI <https://developers.openai.com/api/docs/guides/prompt-caching>｜Gemini <https://ai.google.dev/gemini-api/docs/caching>｜Bedrock <https://docs.aws.amazon.com/bedrock/latest/userguide/prompt-caching.html>｜百炼 <https://help.aliyun.com/zh/model-studio/context-cache>

> **仍未核验**：Vertex AI 显式缓存的 token-hour 单价（页面 JS 渲染）；Anthropic 模型在 Bedrock 上的 1.25×/0.1× 乘数（AWS 仅对 GPT‑5.6 on Bedrock 明示）；百炼是否存在 token-hour 存储费；TensorRT-LLM "early reuse" 仅见于 NVIDIA 开发者博客；DeepSeek 固定间隔的具体 token 数未公开。**所有价格为调研时点快照，落地前请复核官方定价页。**
