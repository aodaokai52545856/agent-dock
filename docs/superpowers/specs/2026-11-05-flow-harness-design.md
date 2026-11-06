# 编排演进：从消息接力到文件协议状态机

日期：2026-11-05。状态：设计已确认；第 1 期已实现，并于 2026-11-06 在桌面端和真实 Codex 上验证通过，尚未提交；第 2 期起未开始。

依据：《Harness Engineering 实践篇》的总览、Agents、Workflow、Scripts（前半）、Memory、Workshops 章节。

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
- **会话**：`session: fresh | reuse`。新任务默认用干净上下文，开发和审查不得共用会话。

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
3. 配置了 `judge` 时，判定节点可输出 `LESSON: NEEDED 理由`。作者写了 N/A 而判定节点认为需要，引擎就让作者多跑一轮，只补写经验，不改代码，不计入重试次数。

三档共用同一条下游：

```
作者写草稿（task 级） → 流程通过 → 人工审批（可逐条勾选） → harvest 节点落盘（project 级） → 下次开发读 index.md
```

- 只有通过人工审批才晋升到项目级，失败尝试不污染记忆。
- harvest 是确定性程序：逐条原样写入，不合并不改写，缺段落只标记不补写。
- 条目用五段式：症状、根因、修复、防复发措施、复发检测。
- 读取分两步：先扫索引，命中 scope 再读全文。
- 归档时检查“防复发措施”写的路径是否存在；不存在只告警并留痕，不阻断。

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
- `.agentDock/tasks/` 被 gitignore 后，需要确认 Cursor SDK 和 PTY 工具仍能按路径读取。第 2 期开工前验证。
- Dock 不覆盖用户的 Codex 模型配置；模型不可用时要把 Codex 的原始错误显示出来。
