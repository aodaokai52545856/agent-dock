# 使用量页（CC Switch 库 + Claude 副号）

Date: 2026-10-27  
Status: approved  
Approach: 方案 B — 只读 `~/.cc-switch/cc-switch.db`，另扫 Dock 多 Claude 账号目录补副号 token

## Goal

顶栏增加「使用量」页签。页面按 CC Switch 用量统计整页抄：总成本、总请求、真实消耗 Tokens、缓存命中率、时间范围、按应用筛选、趋势/热力图、请求日志 / 供应商 / 模型。

核心缺口：CC Switch 只扫默认 `~/.claude`，看不到 Dock 里用独立 `CLAUDE_CONFIG_DIR` 的副号。这一页必须把每个副号的本机 jsonl token 算进去，和 CC Switch 已导入的其他 agent 合成总用量。

不调 Anthropic / Claude 官方额度接口，不展示 5h/7d 额度或重置时间。

## Background

CC Switch 仓库：https://github.com/farion1231/cc-switch  
用量页：侧栏全局页，数据进 `~/.cc-switch/cc-switch.db` 的 `proxy_request_logs`（会话日志 `data_source` 如 `grok_session`，路由日志默认 `proxy`）。文档：[4.4 Usage Statistics](https://github.com/farion1231/cc-switch/blob/main/docs/user-manual/en/4-proxy/4.4-usage.md)。

Agent Dock 现状：

- 顶栏页签只有「控制台」「编排」（`AppMode = 'console' | 'bridge'`）。
- Grok 已有状态栏额度 + `grok_spend.rs`（按 CC Switch `session_usage_grokbuild` 扫 `updates.jsonl`，并只读补读 `proxy_request_logs` 里 `app_type = 'grokbuild' AND data_source = 'grok_session'`）。
- Claude 多账号是目录身份：系统号 `~/.claude`；副号 `{app_data}/claude-accounts/profiles/{id}`，PTY 设 `CLAUDE_CONFIG_DIR`。会话在各自 `projects/`。`claude_accounts::session_homes()` 已列出全部 home。续接会把原 jsonl 整份拷到目标号（`claude_session_copy.rs`），旧 `usage` 会跟着走。
- 没有 Claude token 看板；官方 `/api/oauth/usage` 明确不做。

用户选择：整页抄 CC Switch；第一期全 agent；副号按当前项目的 `config_dir` 取；不展示额度；不因统计去调 Claude API。

## Non-goals

- 调用 `GET /api/oauth/usage` 或任何 Anthropic 计费/额度接口
- 5 小时 / 7 天额度条、重置时间、套餐剩余
- 改 `cc-switch.db` schema 或改 `model_pricing`
- 自动写入 CC Switch 库（仅用户点「同步到 CC Switch」时 INSERT OR IGNORE 副号行）
- 触发 CC Switch 进程去扫会话日志
- 把 Dock 嵌进 CC Switch，或把 CC Switch 嵌进 Dock
- 使用量页上打开/恢复会话
- DeepSeek 没有会话 jsonl 时编造 token
- 第一期改价（改价仍去 CC Switch）

## Success

1. 顶栏「控制台 / 编排 / 使用量」三页签可切换；切到使用量时 PTY 继续跑。
2. 使用量页结构对齐 CC Switch：页头同步、时间范围、应用芯片、四张汇总卡、趋势/热力图、请求日志/供应商/模型。
3. 「全部」= CC Switch 库里已导入的请求 + 每个 Claude 副号 jsonl 的新用量。
4. 系统 Claude 只出现在库这一路（`app_type = 'claude'`），不因 Dock 再扫 `~/.claude` 而双计。
5. 每个副号有独立芯片「Claude · {环境名}」，点开只看该 `config_dir` 的 token。
6. 不出现对 `api.anthropic.com` 的用量/额度请求。
7. 没有 `cc-switch.db` 时副号仍出数；其他 agent 空态说明先让 CC Switch 扫过。
8. 续接时写入拷贝边界；副号只计边界之后的 assistant `usage`。
9. 解析、归属、双计、无库、坏行有 Rust 单测。

## Architecture

两路只读，在 Rust 合成 `UsageRecord`，前端只渲染。

```
┌─ 使用量页 ─────────────────────────────────────────┐
│  TitleBar  控制台 | 编排 | 使用量                    │
│  UsagePane  筛选 → 汇总卡 → 图 → 日志/供应商/模型     │
└────────────┬───────────────────────────────────────┘
             │ invoke
┌────────────▼───────────────────────────────────────┐
│  usage.rs（内存缓存 + 筛选聚合）                      │
│    路 1  只读 ~/.cc-switch/cc-switch.db               │
│           proxy_request_logs / model_pricing         │
│    路 2  每个非系统 Claude Profile.config_dir         │
│           projects/**/*.jsonl  assistant.usage       │
└────────────────────────────────────────────────────┘
```

| 部分 | 位置 | 职责 |
|------|------|------|
| 页签 | `TitleBar.vue`、`AppMode`、`parseAppMode` | 第三模式 `usage` |
| 工作区 | `App.vue` + `UsagePane.vue` | 全宽全局页，无侧栏/终端/文档栏 |
| 命令 | `src-tauri/src/usage.rs`（可分子模块） | 读库、扫副号、聚合、分页 |
| 续接边界 | `claude_session_copy.rs` | jsonl 写入 `agent-dock-copy-boundary` |
| 账号列表 | 现有 `claude_accounts` | 副号 `id/name/config_dir`；系统号不扫盘 |
| 前端 API | `src/lib/api.ts` | `usageDashboard` / `usageLogs` / `usageSync` |
| 测试 | `usage.rs` 内 `#[cfg(test)]`；拷贝边界测放 `claude_session_copy` | 见 Testing |

`grok_spend` 保持独立（状态栏 Grok 花费弹层）。使用量页不调用 `grok_spend`；Grok 数字以库为准。`grok_spend` 现有的 db 补集逻辑不动。

视觉用 Dock `--ad-*` token，不搬 CC Switch 配色。日期范围预设与 `grokSpend` 对齐，便于复用日历。

## Data flow

### 路 1 — CC Switch 库

路径：`{home}/.cc-switch/cc-switch.db`。  
打开：默认 `SQLITE_OPEN_READ_ONLY`，`busy_timeout` 2s（同 `grok_spend`）。用户点「同步到 CC Switch」时用读写打开，`busy_timeout` 5s，只 INSERT OR IGNORE 副号行。

主表 `proxy_request_logs` 用到的列：

`request_id, created_at, app_type, provider_id, model, input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, input_token_semantics, total_cost_usd, session_id, status_code, data_source, latency_ms, first_token_ms`

缺列则整页失败，文案见 Error handling。不按 `data_source` 丢掉路由日志：库里有的都算，和 CC Switch 用量页一致。读库时跳过 `data_source = 'claude_extra'` 或 `request_id` 以 `claude_extra:` 开头的行，避免把已写回的副号和路 2 jsonl 双计。

`model_pricing` 只读，给路 2 套价。路 1 成本用行上的 `total_cost_usd`。

系统 Claude：`app_type = 'claude'` 的库行。Dock **不**扫 `~/.claude/projects`。

### 路 2 — Claude 副号 jsonl

账号来源：`claude_accounts` 的 index（`accounts` + `retired`）。`system == true` 或 `id == "system"` 的跳过。`retired` 仍扫：目录还在，历史副号用量不能丢。

每个 profile 扫 `{config_dir}/projects/**/*.jsonl`（含 subagent 的 `agent-*.jsonl`）。跳过文件名以 `.` 开头的临时文件。同一 `request_id` 在多个文件出现时只留一条（subagent 与主会话若重复 uuid，不双计）。

只采 `type == "assistant"` 且带 `message.usage` 的行：

| jsonl | UsageRecord |
|-------|-------------|
| `message.usage.input_tokens` | 新增输入（fresh） |
| `output_tokens` | 输出 |
| `cache_read_input_tokens` | 缓存命中 |
| `cache_creation_input_tokens` | 缓存写入 |
| `message.model` | 模型 |
| `timestamp` | `created_at`（unix 秒；解析失败则跳过该行） |
| `sessionId` 或文件名 | `session_id` |
| `uuid` / `message.id` | 去重键一部分 |

`input_token_semantics = 0`（fresh 与缓存分列，和 Claude jsonl 一致）。

合成字段：

- `app_filter`：`claude:{profile.id}`
- `app_label`：`Claude · {profile.name}`（名为空则邮箱，再空则 `环境` + 短 id）
- `provider_name`：与 `app_label` 相同（会话日志分不出供应商）
- `data_source`：`claude_extra`
- `status_code`：200
- `request_id`：`claude_extra:{profile.id}:{session_id}:{message.uuid 或 uuid 字段}`；两者都空则用该行内容的稳定哈希，前缀仍是 `claude_extra:`
- `total_cost_usd`：按 `model_pricing` 的百万 token 单价算；没有该模型则为 0，并标 `unpriced`

拷贝边界：扫描一个文件时，遇到 `type == "agent-dock-copy-boundary"` 之前的 assistant `usage` 全部不计；之后的计入。文件没有这条记录时，整份计入（旧续接会和主号历史重叠，见 Limitations）。

`rewrite_history` 在写完原记录、写 `custom-title` **之前**插入一条：

```json
{"type":"agent-dock-copy-boundary","sessionId":"<new>","sourceSessionId":"<old>","copiedAt":"<RFC3339>"}
```

只加这一条，不改原 `usage` 行。

### 合成与筛选

`UsageRecord.app_filter`：路 1 用 `app_type` 原值（`claude`、`grokbuild`、`opencode`、`kimi`、`pi`、`dsh`、`codex`…）；路 2 用 `claude:{id}`。

总用量 = 当前时间范围内路 1 全部 + 路 2 全部。  
芯片「Claude」= 路 1 且 `app_type = 'claude'`。  
芯片「Claude · 名」= 该 profile 的路 2。  
「全部」= 两路。

Token 卡「真实消耗 Tokens」= 新增输入 + 输出 + 缓存写入 + 缓存命中。  
缓存命中率 = 缓存命中 ÷ (新增输入 + 缓存写入 + 缓存命中)；分母 0 则 0。

## Commands

前端 camelCase，Rust snake_case。

```ts
usageSync(): Promise<UsageSyncResult>
usageDashboard(query: UsageQuery): Promise<UsageDashboard>
usageLogs(query: UsageQuery & { page: number; pageSize: number }): Promise<UsageLogPage>
usageExportCcswitch(): Promise<UsageExportResult>
```

`UsageQuery`：`start`/`end` unix 秒；`appFilter` 为 `'all'` 或某个 `app_filter`；可选 `providerName`、`model`。

`usage_sync`：失效缓存，重读库 + 重扫副号 jsonl，返回 `{ imported, filesScanned, extraAccounts, dbOk, message }`。  
`usage_dashboard`：汇总卡、按小时或按日的趋势点（范围 ≤ 36h 按小时，否则按日，同 `grok_spend`）、日热力图、供应商表、模型表、芯片列表（库里出现过的 `app_type` + 每个有目录的副号，即使当前范围为 0）。  
`usage_logs`：同一筛选分页；`pageSize` 默认 20。  
`usage_export_ccswitch`：把路 2 副号行 INSERT OR IGNORE 进 `proxy_request_logs`。`app_type='claude'`（CC Switch 芯片只认已知应用，多个副号合并进 Claude），`provider_id` 用账号标签以便供应商筛选分开，`data_source='claude_extra'`，`request_id` 保持 `claude_extra:{id}:{session}:{uuid}`。不改 schema、不定价、不写 `session_log_sync`。返回 `{ written, skipped, extraAccounts, dbOk, message }`。

命令在 `spawn_blocking` 里跑，避免卡住 UI。jsonl 按文件 mtime + len 增量缓存，同 `grok_spend`。

不提供改价、触发 CC Switch 扫描的命令。

## UX

**页签：** 顶栏 `role="tablist"` 增加「使用量」。`setAppMode('usage')` 写入现有 `MODE_STORAGE_KEY`（含按项目那份）。`parseAppMode` 认 `'usage'`，其余仍回 `'console'`。`jumpToLive` 仍切回控制台。

**工作区：** `v-show="store.appMode === 'usage'"` 全宽 `UsagePane`。控制台/编排 `v-show` 隐藏，PTY 不杀。`TextConvertFab` 仍只在控制台。底栏保留；使用量模式下隐藏「刷新会话」和「文档」。Grok 额度条规则不变（本就只在控制台）。桌面玻璃窗下 `.usage-ws` / `.pane` 用实色 `--ad-ink`，避免 IDE 字透出来。

**页内（自上而下，对齐 CC Switch UsageDashboard）：**

1. 页头「使用量」；右「立即同步」（相对时间，如「刚刚」「3 分钟前」）+「同步到 CC Switch」+ 设置。
2. 筛选：时间（当天 / 1d / 7d / 14d / 30d / 全部 / 自定义，默认当天；前五项与 `SPEND_PRESETS` 同一套 id）→ 应用芯片 → 供应商/模型。
3. 四张卡：总成本、总请求、真实消耗 Tokens、缓存命中率。点选一张，图跟该指标。默认 Tokens。
4. 趋势图 + 热力图。
5. 页签：请求日志 / 供应商 / 模型。未定价模型成本列显示「未定价」。

设置抽屉：扫描间隔 5 / 10 / 30 / 60 秒（默认 60，记 localStorage）；「同步副号到 CC Switch」；只读定价表（来自 `model_pricing`）。没有「记录请求用量」——那是 CC Switch 路由设置。

进入使用量页：先 `usage_sync` 再 `usage_dashboard`。页开着按间隔再 sync+拉数。离开页停轮询。

## Error handling

| 情况 | 表现 |
|------|------|
| 没有 db 文件 | `dbOk: false`。副号照常。其他 agent 空。提示：请先打开 CC Switch，让它扫描会话日志 |
| 库 busy 超时 | 提示：用量库正被 CC Switch 占用，请稍后重试。保留上一帧数据（若有） |
| 缺表或缺列 | 提示：这份 CC Switch 数据库对不上，请升级 CC Switch |
| 单行 jsonl / 单行 SQL 损坏 | 跳过该行，整页继续 |
| 时间范围内 0 条 | 这个时间段还没有用量记录 |
| 某应用没有会话日志（如 DeepSeek） | 该芯片下就是 0 |
| 浏览器预览 | 与其他桌面命令相同：提示去桌面窗口 |

中文文案写死在命令返回的 `message` / 前端空态，不走 i18n 文件。

## Testing

Rust（夹具目录，不碰用户真库）：

1. 副号 jsonl：四列 token 加总正确；坏行跳过。
2. 系统 home 不在路 2 扫描列表。
3. 库行 `app_type=claude` 与副号记录同时存在时，筛选 `claude` 不含副号，筛选 `claude:{id}` 不含库行，「全部」两条都在且 request_id 不撞。
4. 带 `agent-dock-copy-boundary` 的文件只计边界后的 usage。
5. 无 db 文件时 dashboard 仍返回副号芯片和副号汇总。
6. 最小 `proxy_request_logs` sqlite：按 `app_type`、时间范围过滤。
7. `rewrite_history` 写出边界记录。

前端：`parseAppMode('usage') === 'usage'`；未知值仍是 `console`。不强制 Vue 组件快照。

## Limitations

- 其他 agent 的新鲜度 = CC Switch 上次写入 db 的时间。Dock「立即同步」只重读库 + 扫副号。
- 旧续接（无边界记录）副号会重复计入拷贝过去的历史 token。新续接有边界。
- CC Switch 升级改列名会导致「数据库对不上」。不猜测迁移。
- 成本是估算（库内已算值 + 只读价表），以供应商账单为准。

## Files

新建：`src-tauri/src/usage.rs`（或 `usage/`）、`src/components/UsagePane.vue`、`src/lib/usage.ts`（范围/格式，能复用 `grokSpend` 的就复用）。

改：`lib.rs` 注册命令；`TitleBar.vue`、`App.vue`、`StatusBar.vue`、`store.ts`、`types.ts`、`api.ts`；`claude_session_copy.rs` 写边界。

## Implementation notes

用户批准本文后再写 `docs/superpowers/plans/` 实施计划。第一期可按切片交：① 路 2 副号解析 + 边界；② 路 1 读库 + 合成命令；③ 页签和 UsagePane。
