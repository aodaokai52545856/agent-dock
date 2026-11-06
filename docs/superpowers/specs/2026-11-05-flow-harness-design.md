# 编排演进：从消息接力到文件协议状态机

日期：2026-11-05。状态：设计已确认；第 1、2、3 期已实现，并于 2026-11-06 在桌面端验证通过（第 1、2 期用真实 Codex，第 3 期用真实的 Claude Code、Grok Build、OMP）；第 4 期起未开始。

依据：《Harness Engineering 实践篇》的总览、Agents、Workflow、Scripts（前半）、Memory、Workshops 章节。

主力通道是 Claude Code、Grok Build、OMP，功能和验证优先保证这三个。Codex 只需保持可用，不为它做专门设计。

## 1. 目标

解决三个问题：

1. 编排灵活度不够：不能分支，流程中段不能设闸，结论不分问题归属。
2. 节点没有自己的文档、输入和输出，下游只能看到上一节点的回复文字。
3. 经验产物无法进入下一次开发的上下文。

设计原则：**引擎只认契约、产物、结论，不认具体角色。** BA、SA、Dev 这些角色都是模板内容，不是引擎概念。

## 2. 已确认的决策

| 决策 | 结论 |
| --- | --- |
| 流程定义位置 | 项目仓库根目录的 `.agentDock/` |
| 任务产物位置 | 同样放在 `.agentDock/`（`tasks/` 子目录） |
| 预设模板 | 不是重点，不单独投入 |
| 经验写入规则 | 可配置三档：按尝试次数、每次都写、模型判定 |
| 并行 | 不做。节点严格串行，与方法论铁律⑥和“同一工作区只有一个写者”一致 |

`.agentDock` 的大小写按此拼写固定，因为 Linux 上路径区分大小写。

## 3. 目录结构

```
<项目根>/.agentDock/
├── .gitignore          # 默认只有一行 tasks/
├── flows/*.json        # 流程定义
├── roles/<角色>/        # role.md（身份/职责/禁止事项）、模板
├── memory/
│   ├── index.md        # 一条经验一行，按 scope 分类
│   └── entries/        # YYYY-MM-DD__scope__slug.md
└── tasks/<任务>/
    ├── run.json        # 运行记录，含每步的输入 hash
    ├── experience/     # 经验草稿，审批后才进 memory/
    └── *.md            # 各阶段产物
```

- `tasks/` 由目录内的 `.gitignore` 排除，不改项目根的 `.gitignore`。这样审查“未提交改动”时不会混入任务文档。想把产物纳入版本管理，删掉这一行即可。
- `flows/`、`roles/`、`memory/` 进 git，跟代码一起评审。
- `ptyId`、`sessionId`、`agentId`、`threadIds` 是本机绑定，留在 App 里，不进仓库。

## 4. 节点契约

```jsonc
{
  "id": "dev",
  "role": "developer",
  "inputs": [
    { "ref": "task:design.md", "required": true },
    { "ref": "task:code-review.md", "when": "reworked" },
    { "ref": "project:memory/index.md", "required": true, "read": "index-first" }
  ],
  "outputs": [
    { "path": "task:dev-log.md", "capture": "file", "template": "dev-log.md",
      "sections": ["## 验证证据", "## 结论"] }
  ],
  "exits": [
    { "on": "PASS", "to": "verify" },
    { "on": "BLOCK", "to": "@human" }
  ],
  "maxAttempts": 5,
  "session": "fresh"
}
```

- **输入**：默认只传路径和必读清单，不内联全文。不能读文件的通道才内联。每步记录实际输入的路径和 hash。
- **输出**：能写文件的通道由 agent 自己写（`capture: file`）；只读通道由 Dock 捕获回复后落盘（`capture: reply`）。
- **节点后校验**：文件存在、无模板残留、有结论段。校验由 Dock 在所有通道外层统一执行，不依赖单个工具的 Hook。
- **会话**：`session: fresh | reuse`。新任务默认用干净上下文，开发和审查不得共用会话。（未实现，随第 4 期）

第 2 期落地时确定的细节：

- **何时写仓库**：只有用户编辑、新建、改名、删除流程时才写 `flows/`。查看流程或运行没有契约的旧流程，都不碰仓库。
- **同步**：切换项目和启动流程时读取 `flows/*.json`，文件定义优先，本机绑定、草稿和分片号保留在 App 里。文件名决定流程 id，格式错误的文件跳过并提示。
- **模板**：`template` 指向 `roles/<节点角色>/<文件名>`。文件型产物由 Dock 先用模板生成，节点必须改动它；内容与运行前相同视为“没有更新产物”。占位符写法是 `{{...}}`。
- **输出范围**：节点只能写 `task:`。`project:` 由后续的 harvest 独占，保持记忆单作者。
- **结论**：带闸门的节点，文档 `## 结论` 优先于回复。只认独占一行的结论，带理由的句子读不出来时交给人工。
- **失败重跑**：从失败节点重跑沿用同一个任务目录。
- **未确认的发送**：PTY 通道没等到回合时，声明了产物的节点直接判失败，不放行。

通道能力，2026-11-06 实测：

| 通道 | 读 gitignore 内的路径 | 写 | 处理 |
| --- | --- | --- | --- |
| Codex 审查（read-only） | 可以 | 不能 | 传路径，捕获回复落盘 |
| Claude Code | 可以 | 可以 | 传路径，自己写文件 |
| Codex `workspace-write` | 可以 | 可以 | 同上（当前没有这种通道） |
| Grok Build | 可以 | 可以 | 传路径，自己写文件 |
| OMP | 可以 | 可以 | 传路径，自己写文件 |
| Kimi | 未测 | 未测 | 按 PTY 处理，靠节点后校验兜底 |
| 其他 PTY 工具 | 未测 | 未测 | 确认不了回合结束，启动时拒绝声明输出 |
| Cursor SDK | 未测（无 API Key） | 未测 | 按读不到处理：内联内容，捕获回复落盘 |

## 5. 路由

用出口取代现有的 `gate`/`backTo`：

```jsonc
"exits": [
  { "on": "PASS", "to": "te" },
  { "on": "REJECT", "owner": "dev", "to": "dev" },
  { "on": "REJECT", "owner": "upstream", "to": "@human" }
]
```

- 结论：PASS / FAIL / REJECT / BLOCK，可带 owner。
- 结论来源优先级：脚本退出码 > 文档的 `## 结论` > 回复末行标记。读不出就交给人工，保持 fail-closed。
- 新增 `script` 节点，以退出码判定。
- `human` 节点真正暂停。
- profile 在同一条流程里定义，用于跳过或插入节点。
- 重试按节点计数。
- 启动前预检：按 profile 走一遍路径，确认每个必读输入都有上游产出。

## 6. 经验闭环

```jsonc
"experience": {
  "policy": "attempts",     // attempts | always | model
  "minAttempts": 2,         // policy=attempts 时生效
  "authors": ["dev"],       // 写草稿的节点
  "judge": "cr"             // policy=model 时可选的独立判定节点
}
```

| 策略 | 什么时候要求写 | 注意 |
| --- | --- | --- |
| `attempts` | 该节点尝试次数达到 `minAttempts` | 默认档，对应方法论的“被打回才写” |
| `always` | 每次都写 | 容易产生流水账，靠人工审批过滤 |
| `model` | 由模型判断 | 见下 |

`model` 策略要避免自审失效：

1. 作者的 `## 可复用经验` 段必填，内容是经验条目，或 `N/A：理由`。跳过因此是可见的。
2. 判定标准写进提示词：问题源于可复用的机制、未来会再次触发、能写出可执行的防复发措施。需求或方案本身错误、一次性细节不写。
3. 配置了 `judge` 时，判定节点可输出 `LESSON: NEEDED 理由`。作者写了 N/A 而判定节点认为需要，引擎就在流程通过后让作者多跑一轮，只补写经验，不改代码，不计入重试次数和步数。

三档共用同一条下游：

```
作者写草稿（task 级） → 流程通过 → 人工审批（可逐条勾选） → harvest 落盘（project 级） → 下次开发读 index.md
```

- 只有通过人工审批才晋升到项目级，失败尝试不污染记忆。
- harvest 是确定性程序：逐条原样写入，不合并不改写，缺段落只标记不补写。
- 条目用五段式：症状、根因、修复、防复发措施、复发检测。
- 读取分两步：先扫索引，命中 scope 再读全文。
- 归档时检查“防复发措施”写的路径是否存在；不存在只告警并留痕，不阻断。

第 3 期落地时确定的细节：

- **harvest 不是图上的节点**：它是配置了 `experience` 的流程走完后的收尾阶段，不需要连线，也就不会因为漏接节点而丢草稿。等待审批时流程状态是“等待”，写入或全部跳过之后才算走完；这期间不能运行、切换、新建或删除流程，只能审批或停止。
- **草稿位置**：`task:experience/<节点>-<第几次>.md`。这是 Dock 为该步临时追加的产物，不写进流程文件，复用节点后校验。能写文件的通道写这个文件，只读通道捕获回复。
- **条目格式**：在 `## 可复用经验` 段里用 `### 标题` 开始一条，紧跟 `范围：xxx`，五段用 `#### 症状` 这样的小标题。校验只要求至少有一条（`model` 策略也可以是 `N/A：理由`），缺小节不算失败。
- **N/A**：只有 `model` 策略接受。另外两档已经明确要求写，写不出来就让这一步失败。
- **同一条经验**：范围和标题都相同时，只保留最后一次尝试的写法。
- **记忆注入**：作者节点每次都自动读 `memory/index.md`，非必读，第一次运行时不存在也没关系。其他节点要读就自己在 `inputs` 里声明。
- **补写的时机**：判定节点的请求记在运行上，返工轮提出的也算。等流程通过后才补写，这时修复已经定型。如果作者这时已写了条目就不再补写。补写失败只记录在该步，不推翻已经通过的结果。“不改代码”靠提示词约束，Dock 不校验。
- **重跑**：从失败节点重跑时累计尝试次数，`attempts` 策略不会因为重跑而清零，`when: reworked` 的输入也会生效。
- **索引行**：`- 标题 — .agentDock/memory/entries/<文件>.md`，放在 `## 范围` 标题下。
- **条目文件**：开头是元数据（范围、日期、任务、来源，以及 `missing_sections`、`missing_paths`），之后是原文。写入失败时审批保持打开，重试不会产生重复文件。
- **写权限**：`memory/` 可写不可删。删除经验要走 git。

## 7. 不照搬的部分

| 方法论内容 | 处理 | 原因 |
| --- | --- | --- |
| LLM 扮演的 PM 与心跳 | 不做 | Dock 引擎是确定性代码，Run 时间线就是心跳 |
| 三边契约校验 | 不做 | 提示词由契约渲染，只有一个来源 |
| Cursor Hook | 不做 | Dock 在外层统一校验，可覆盖所有通道 |
| 固定 7 个角色 | 不做 | 角色只是模板 |
| 向量记忆 | 暂不做 | 文件加索引足够 |

## 8. Codex 实测结果

2026-11-05，Codex CLI 0.153.2，隔离的临时仓库：

| 调用 | 结果 |
| --- | --- |
| `thread/start` 传 `sandbox: readOnly` 或 `workspaceWrite` | 拒绝，只接受 `read-only`、`workspace-write`、`danger-full-access` |
| `review/start` + `detached`，新建 thread | 拒绝：`paginated threads do not support detached review` |
| `review/start` + `detached`，已有回合的 thread | 同样拒绝 |
| `uncommittedChanges` + `inline` | 可用，但输出没有 VERDICT |
| `custom` + `inline` | 可用：自行发现工作区改动，遵守任务和合同，输出 VERDICT |
| `thread/fork` + `inline` | 可用，原 thread 不受影响，fork 会留在历史记录里 |
| `thread/fork` 带 `ephemeral: true` | 要求 `excludeTurns: true`，会丢失上下文，不采用 |

2026-11-06，用 Dock 的 Rust 代码接真实 Codex 验证：

| 场景 | 结果 |
| --- | --- |
| `uncommittedChanges`、`commit`、`baseBranch` 目标 | 都走 `custom`，范围正确，都输出 VERDICT |
| 已有 thread | fork 后审查，原 thread 回合数不变 |
| 干净改动 | `VERDICT: PASS`，不被误判为 FAIL |
| 不可用的模型 | 报错显示 Codex 的原始消息 |
| 桌面端“人工 → Codex 审查 → 结束” | PASS 走完；FAIL 退回人工节点；人工未通过后终止 |

这次还发现并修复了一个旧问题：`AppServer::spawn` 在设置 stdin 管道后调用 `apply_no_window`，后者把 stdin 置空，导致桌面端的所有 Codex 桥接调用都以“Codex App Server 没有 stdin”失败。

## 8.1 第 3 期实测结果

2026-11-06，Claude Code 2.1.319，两个真实窗口分别当作者和判定，隔离的临时仓库：

| 场景 | 结果 |
| --- | --- |
| 作者先说一句话再调工具（会话里出现 6 次） | Dock 等到回合结束，交接的是最后一条回复 |
| `capture: file` | 作者自己写文件，校验通过，输出 hash 写入 `run.json` |
| `always` 策略 | 两条草稿进审批，缺小节和不存在的路径都有提示 |
| 只勾选一条 | 只写入这一条，原文不变，`missing_paths` 留痕，索引不含正文 |
| 下一次运行 | 见下面三 Agent 的结果（这一轮同窗口复用，提示词里出现过口令，不能单独作为证据） |
| `attempts` 策略，审查打回 | 第 1 次不要求，第 2 次要求；返工读到的审查文件 hash 与上一步输出一致 |
| `model` 策略，作者 N/A，判定要求 | 流程通过后补写一轮，`run.json` 标为 lesson，不计重试和步数 |
| 审批等待中重载页面 | 审批还在，运行按钮禁用；全部跳过后记忆不变 |

同一轮用真实 Codex 回归第 2 期的 7 个场景，全部通过。

三 Agent 同跑一条流程：Claude 2.1.319 写设计，Grok 1.0.70 开发，OMP 18.16.0 审查。

| 场景 | 结果 |
| --- | --- |
| 接力与返工 | 设计 → 开发 → 审查打回 → 返工 → 通过；四处输入 hash 都等于上游输出 hash；只有第 2 次开发要求写经验 |
| 审批 | Grok 写的经验进入 `memory/` |
| 读回 | 测试直接往 `memory/` 写入一条条目，口令不出现在任何提示词里，三个 Agent 都读到并写进各自的产物 |
| `model` 策略 | OMP 判定要求补写，Grok 补写一轮，不计重试 |
| OMP 当作者 | 自己写草稿文件，进入审批 |

同一个窗口跨任务复用时，上一次的提示词还在上下文里。实测中 Grok 在 `memory/` 不存在的情况下写出了口令，来源就是之前的提示词。这说明第 4 期的 `session: fresh` 是必需的，尤其是审查节点。

这次发现并修复的旧问题：

- Claude 不在 `AUTO_PTY_TOOLS` 里：发送后立刻算完成，不等回合。声明了产物的 Claude 节点因此总是失败，第 2 期的通道表写错了。现在加入自动通道。这会改变现有流程的行为：Claude 节点要先完成一轮对话才能启动，并且真正等回复。
- `detectNewAssistantTurn` 取第一条新消息，工具调用前的开场白会被当成结果。现在取最后一条，并用 `stop_reason` 判断回合是否结束。
- PTY 等待固定 8 分钟。现在只算窗口的静默时间。
- `openWindowForNode` 没有传 `account`，编排页打开的 Claude 窗口要等下一次刷新才可能绑定会话。
- `setRun` 按当前选中的项目保存，切换项目后运行记录会写到别的项目下。
- Grok 同样把开场白当结果（真实转录里 185 条带 `tool_calls`，收尾只有 14 条），并且从文件开头读、只取前 80 条，长会话读不到新回合。现在用 `tool_calls` 判断回合，从文件尾部读，用行偏移做 id。
- OMP 有 8 分钟绝对超时，到点中止正在工作的 agent。

## 9. 分期

1. **修地基**（不改数据模型）
   - Codex：sandbox 改用新值并兼容旧值，不再回退到可写沙箱；没选 thread 时用 inline；选了 thread 时先试 detached，不支持就 fork + inline。
   - Codex：所有目标统一走 `custom`，带上范围、角色合同、任务、上游内容和 VERDICT 要求。
   - Codex：审查失败要报错，不能把 `Reviewer failed` 当作审查正文。
   - `human` 节点真正暂停。
   - 任意节点只要出口有 passFail 闸门，就解析结论并要求输出 VERDICT。
2. **契约与产物**：`.agentDock/` 读写、输入输出、节点后校验、每步追溯。Rust 端新增限定在该目录内的读写命令。旧流程自动转换，行为不变。
3. **经验闭环**：三档策略、harvest、记忆注入。用“Dev ↔ 审查 → 人工 → harvest”线性流程先跑通。
4. **灵活路由**：多出口、中段闸门、script 节点、profile、预检。移除 `repairPassFailEdges` 和未生效的 `edge.transform`。
5. **界面**：节点面板分“说明／输入／输出／出口”，画布显示产物。

## 10. 风险

- `script` 节点让仓库内容能触发命令。clone 来的项目可能带恶意节点，首次运行必须展示命令，并按 hash 确认。
- Cursor SDK 还没实测。配置 API Key 后验证读写，确认后可去掉 `channelReadsFiles` 里的内联兜底。
- Codex 的 `review/start` 是审查模式，最后的消息是审查总结，不保证照提示词的格式输出。2026-11-06 实测：判定节点的回复没有 `VERDICT`，作者节点丢掉了 `## 可复用经验` 段。第 8 节“输出 VERDICT”只说明它通常遵守，不是保证。后果：闸门会停下等人工，不会误放行；`LESSON: NEEDED` 可能丢失，等于没有判定；所以 Codex 审查节点不能当经验作者，启动时拒绝。要根治，可以把审查改成只读沙箱里的普通回合（`turn/start`），未验证。
- Kimi 的回合没有“还在工作”标记，且按位置编号、最多取 80 条。Claude 用 `stop_reason` 和记录 uuid，Grok 用 `tool_calls` 和行偏移，都已解决这两点。Grok 如果重写 `chat_history.jsonl`（比如压缩），行偏移会变，未验证。
- OMP 的等待改为只算静默时间，有进度或对话框等人时不会中止。
- Dock 不覆盖用户的 Codex 模型配置；模型不可用时要把 Codex 的原始错误显示出来。
