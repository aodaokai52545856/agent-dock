# 使用量页 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 顶栏增加「使用量」页：只读 CC Switch `proxy_request_logs` 作为各 agent 用量，再扫 Dock Claude 副号 `config_dir` 的 jsonl 补上 CC Switch 看不到的 token。

**Architecture:** Rust `usage.rs` 两路只读合成 `UsageRecord`。路 1 打开 `~/.cc-switch/cc-switch.db`（不写库）。路 2 扫非系统 Claude profile 的 `projects/**/*.jsonl`。前端 `UsagePane.vue` 是第三 `appMode`。续接 jsonl 写入 `agent-dock-copy-boundary`，副号只计边界之后的 assistant usage。不调 Anthropic。

**Tech Stack:** Tauri 2、Rust（rusqlite 只读）、Vue 3、现有 `--ad-*` token、`npm run test:usage` / `cargo test`。

**Spec:** `docs/superpowers/specs/2026-10-27-usage-dashboard-design.md`

## Global Constraints

- 不调用 Anthropic / Claude 额度 API，不展示 5h/7d 额度。
- 不写、不改 schema `cc-switch.db`；不触发 CC Switch 扫描进程。
- 系统 Claude 只走路 1（`app_type = 'claude'`），不扫 `~/.claude/projects`。
- 中文文案写死在命令/`message`/空态。
- Token 合计 = 新增输入 + 输出 + 缓存写入 + 缓存命中。缓存命中率 = 命中 ÷ (新增输入 + 写入 + 命中)，分母 0 则 0。
- 命令在 `spawn_blocking` 里跑。
- 浏览器预览走桌面命令的既有提示，不编造用量。

---

## File structure

| File | Responsibility |
|------|----------------|
| Modify: `src-tauri/src/claude_session_copy.rs` | `rewrite_history` 在 `custom-title` 前写入边界记录 |
| Create: `src-tauri/src/usage.rs` | 记录、扫副号、读库、聚合、IPC 类型 |
| Modify: `src-tauri/src/claude_accounts.rs` | `extra_usage_profiles()` 列出非系统账号（含 retired） |
| Modify: `src-tauri/src/lib.rs` | `mod usage`；注册 `usage_sync` / `usage_dashboard` / `usage_logs` |
| Modify: `src/lib/types.ts` | `AppMode` 加 `'usage'`；用量 DTO |
| Modify: `src/lib/store.ts` | `parseAppMode` 认 `'usage'` |
| Create: `src/lib/usage.ts` | 时间范围（复用 grokSpend 预设 + `all`）、格式、轮询间隔 |
| Create: `src/lib/usage.test.ts` | parseAppMode 通过 store 测；范围/命中率 |
| Create: `src/components/UsagePane.vue` | 用量页 |
| Modify: `TitleBar.vue` / `App.vue` / `StatusBar.vue` / `api.ts` / `package.json` | 页签、工作区、隐藏刷新、invoke |

---

### Task 1: 续接边界 + 副号 jsonl 解析

**Files:**
- Modify: `src-tauri/src/claude_session_copy.rs`
- Create: `src-tauri/src/usage.rs`（本任务只放解析，命令下一任务）
- Modify: `src-tauri/src/lib.rs`（`mod usage;` 即可）

**Interfaces:**
- Consumes: `claude_session_copy::rewrite_history`（模块内私有，测 `copy_session` 产物）
- Produces:
  - jsonl 边界：`{"type":"agent-dock-copy-boundary","sessionId":"<new>","sourceSessionId":"<old>","copiedAt":"<RFC3339>"}`，位于原记录之后、`custom-title` 之前
  - `pub struct ExtraClaudeProfile { pub id: String, pub name: String, pub email: String, pub config_dir: PathBuf }`
  - `pub struct UsageRecord`：`request_id, created_at, app_filter, app_label, provider_name, model, session_id, input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, cost_usd, unpriced, status_code, data_source`
  - `pub fn extra_app_label(name: &str, email: &str, id: &str) -> String`
  - `pub fn scan_extra_profiles(profiles: &[ExtraClaudeProfile]) -> Vec<UsageRecord>`
  - `request_id` = `claude_extra:{id}:{session}:{uuid}`；无 uuid 则对该行做稳定哈希
  - `app_filter` = `claude:{id}`；`data_source` = `claude_extra`；`status_code` = 200
  - 边界之前的 assistant usage 不计；无边界则整份计；坏行跳过；同 request_id 只留一条

- [ ] **Step 1: 写失败测试（边界）**

在 `claude_session_copy.rs` 的 `copies_only_history_...` 里，拷贝后断言存在边界记录且在 `custom-title` 之前。另加单元：直接 `rewrite_history` 一条带 usage 的 assistant。

- [ ] **Step 2: 跑测确认失败**

`cargo test --manifest-path src-tauri/Cargo.toml copies_only_history -- --nocapture`

Expected: 没有 `agent-dock-copy-boundary`。

- [ ] **Step 3: `rewrite_history` 写入边界**

签名改为带 `source_id`。写完原记录后、`custom-title` 前插入边界。`copiedAt` 用 `chrono::Utc::now().to_rfc3339()`。

- [ ] **Step 4: 跑测通过**

同上命令，PASS。

- [ ] **Step 5: 写失败测试（副号扫描）**

`usage.rs` `#[cfg(test)]`：

1. 两行 assistant usage（10/2/100/20 与 3/1/0/0）合计 input=13。坏 JSON 行跳过。
2. 边界前 usage 不计、边界后计入。
3. `system` id 的 profile 不由本函数过滤（过滤在调用方）；本函数扫传入的每个 config_dir。调用方测试放到 Task 2。这里测 `scan_extra_profiles` 只扫传入目录。
4. 主会话与 `agent-xxx.jsonl` 同一 uuid 只计一次。
5. 无 uuid 仍生成 `claude_extra:` 前缀 request_id。
6. 点文件名跳过。

- [ ] **Step 6: 实现 `scan_extra_profiles` 至测试绿**

`cargo test --manifest-path src-tauri/Cargo.toml usage:: -- --nocapture`

- [ ] **Step 7: Commit** `feat(usage): parse extra Claude jsonl and mark copy boundary`

---

### Task 2: 读 CC Switch 库 + 合成 dashboard/logs

**Files:**
- Modify: `src-tauri/src/usage.rs`
- Modify: `src-tauri/src/claude_accounts.rs`（`extra_usage_profiles`）
- Modify: `src-tauri/src/lib.rs`（命令）

**Interfaces:**
- Consumes: Task 1 的 `UsageRecord` / `scan_extra_profiles`
- Produces:
  - `pub enum DbStatus { Ok, Missing, Busy, Schema }`
  - `pub fn read_ccswitch_db(path: &Path) -> Result<(Vec<UsageRecord>, Vec<PricingRow>), DbStatus>`
  - 路 1 `app_filter` = `app_type`；`provider_name` = `provider_id`；`cost_usd` 解析 `total_cost_usd`
  - 缺表/缺列 → `Schema`（文案：这份 CC Switch 数据库对不上，请升级 CC Switch）
  - 无文件 → `Missing`（请先打开 CC Switch，让它扫描会话日志）
  - busy 超时 2s → `Busy`（用量库正被 CC Switch 占用，请稍后重试）
  - `apply_pricing(extra, &pricing)`：按 `model_id` 套百万 token 单价；没有则 `unpriced=true`、`cost_usd=0`
  - `pub struct UsageQuery { start: i64, end: i64, app_filter: String, provider_name: Option<String>, model: Option<String> }`（`app_filter=="all"` 不过滤应用）
  - `filter_records`：`created_at` 满足 `start <= ts < end`
  - `pub fn build_dashboard(records, chips, query) -> UsageDashboard`
  - `pub fn build_logs(records, query, page, page_size) -> UsageLogPage`
  - `UsageDashboard`：`ok, dbOk, message, totalCostUsd, totalRequests, totalTokens, cacheHitPercent, inputTokens, outputTokens, cacheReadTokens, cacheCreationTokens, points[], heatmap[], providers[], models[], apps[]`
  - `points`：范围 ≤ 36h 按小时，否则按日（同 grok_spend）
  - `apps[]`：`{ id, label }`，库里出现过的 app_type + 每个副号芯片（即使当前范围 0）
  - `usage_sync(app) -> UsageSyncResult { imported, filesScanned, extraAccounts, dbOk, message }`
  - `usage_dashboard(app, query)` / `usage_logs(app, query, page, page_size)`
  - `claude_accounts::extra_usage_profiles() -> Vec<ExtraClaudeProfile>`：accounts+retired，跳过 `system`

- [ ] **Step 1: sqlite 夹具测试（先失败）**

最小 `proxy_request_logs` 含 spec 所列列。插入 claude 一行、grokbuild 一行。断言筛选 `claude` 只有库的 claude；筛选 `claude:work` 只有副号；`all` 三条 request_id 不撞。无 db 文件 `DbStatus::Missing` 且副号汇总仍在。缺 `app_type` 列 → `Schema`。

- [ ] **Step 2: 实现读库、定价、聚合、命令，测绿**

`cargo test --manifest-path src-tauri/Cargo.toml usage::`

- [ ] **Step 3: Commit** `feat(usage): merge cc-switch db with extra Claude accounts`

---

### Task 3: 页签 + UsagePane

**Files:**
- Modify: `src/lib/types.ts`（`AppMode`、DTO）
- Modify: `src/lib/store.ts`（`parseAppMode`）
- Create: `src/lib/usage.ts`、`src/lib/usage.test.ts`
- Modify: `src/lib/types.test.ts` 或 `usage.test.ts`：`parseAppMode('usage')==='usage'`，未知仍 `console`
- Modify: `src/lib/api.ts`：`usageSync` / `usageDashboard` / `usageLogs`；非 Tauri 返回空 dashboard + 去桌面提示
- Create: `src/components/UsagePane.vue`
- Modify: `TitleBar.vue`、`App.vue`、`StatusBar.vue`、`package.json`（`test:usage` 加上 `usage.test.ts`）

**Interfaces:**
- Consumes: Task 2 的 camelCase DTO
- Produces: 顶栏第三页签；全宽 `UsagePane`；进入页 `usageSync`+`usageDashboard`；间隔 5/10/30/60s（默认 60，localStorage `ad-usage-refresh-ms`）；立即同步；时间预设当天/1d/7d/14d/30d/全部/自定义；应用芯片；四张卡默认 Tokens；趋势+热力图；日志/供应商/模型；设置抽屉只读定价+间隔
- StatusBar：`usage` 下隐藏刷新按钮（文档按钮已是 console-only）
- `jumpToLive` 仍切回 console
- 空态文案按 spec Error handling

- [ ] **Step 1: `parseAppMode` / 范围辅助测试先红再绿**

`node --experimental-strip-types src/lib/usage.test.ts`

- [ ] **Step 2: 接线 TitleBar / App / UsagePane / api**

页面结构按 spec UX。复用 `formatTokenCount` / `formatSpendCost` / `formatCacheHit` / `SpendChart`（把 dashboard.points 映射成 SpendChart 所需结构，或 UsagePane 内简单 SVG 柱/折线；优先复用 `buildSpendChart`）。

热力图：按日格子，用 `monthCells` 或 范围天数一行色块。

- [ ] **Step 3: `npm run test:usage` 与 `cargo test --manifest-path src-tauri/Cargo.toml usage::` 全绿**

- [ ] **Step 4: Commit** `feat(usage): add usage mode tab and dashboard pane`

---

## Spec coverage

| Spec | Task |
|------|------|
| 顶栏使用量页签、PTY 继续 | 3 |
| CC Switch 页结构 | 3 |
| 全部 = 库 + 副号 | 2 |
| 系统 Claude 不扫 ~/.claude | 2（只读 db） |
| 副号独立芯片 | 2+3 |
| 不调 Anthropic | 全程 |
| 无 db 副号仍出数 | 2 |
| 拷贝边界 | 1 |
| 解析/双计/坏行测试 | 1+2 |
| 定价只读、改价去 CC Switch | 3 设置抽屉 |
| 立即同步不触发 CC Switch 扫描 | 2 usage_sync 只读库+扫副号 |
