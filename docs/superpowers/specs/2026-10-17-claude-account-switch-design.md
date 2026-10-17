# Claude Code Pro 多账号切换

Date: 2026-10-17  
Status: approved  
Approach: 克隆现有 Grok 账号管理（方案 1）

## Goal

在 Agent Dock 里保存、切换多个 Claude Code Pro（claude.ai OAuth）账号。交互、数据目录形态、切号后关终端，都对齐 Grok 账号管理。两个号共用同一套 `~/.claude/projects` 会话历史。

## Background

Grok 已有完整闭环：`src-tauri/src/grok_accounts.rs` 快照 `~/.grok/auth.json`，对话框 `GrokAccountDialog.vue`，菜单「系统 → Grok 账号」，切号后杀掉 `toolId === 'grokbuild'` 的 PTY。

Claude Code Pro 登录落在两处，缺一就会继续按旧号计费：

| 文件 | 字段 | 作用 |
| --- | --- | --- |
| `{claude_home}/.credentials.json` | `claudeAiOauth` | accessToken / refreshToken / expiresAt / subscriptionType |
| `~/.claude.json` | `oauthAccount` | emailAddress、accountUuid、organizationUuid、displayName |

本机（Windows）已验证：credentials 在 `%USERPROFILE%\.claude\.credentials.json`，`oauthAccount` 在 `%USERPROFILE%\.claude.json`。`claude auth status --json` 能读出 email、orgId、subscriptionType。

标题栏已有的 **CC Switch** 管第三方 API 供应商，不在本功能范围内。

## Non-goals

- 状态栏 / 对话框显示 5 小时或 7 天额度
- 按账号隔离会话、复制 `projects/`、设置 `CLAUDE_CONFIG_DIR`
- 两个 Claude 终端同时挂不同号
- API Key / 第三方网关账号（继续用 CC Switch）
- 抽通用「多工具账号」模块
- 调用 ccs / claude-swap / aicli-switch 等外部 CLI

## Success

1. 当前已登录的 Pro 号可「保存当前」，列表出现该号（显示名 + 邮箱）。
2. 「登录新号」保存旧号后弹出 Claude 登录；登完第二个 Pro 号自动入库，可来回切。
3. 切换写回该号 token 和 `oauthAccount`，关掉所有已开 Claude 终端，toast 提示已关终端。
4. 左边会话列表仍是同一套 Claude 历史。
5. 删除只拿掉 Dock 快照，不登出当前号、不吊销 refresh token。
6. 浏览器预览不能改本机账号，提示去桌面端。

## UX

「系统」菜单在「Grok 账号」下增加「Claude 账号」。两个独立对话框，不合并。

对话框克隆 `GrokAccountDialog.vue`（宽度 664px、当前登录、保存当前、登录新号、已保存列表、删除确认）。差异：

- `aria-label` / 标题：Claude 账号
- 提示：快照存在本机。切换后会关掉已打开的 Claude 终端，请再新建会话。
- 空列表：还没有保存的账号。登录后点「保存当前」，再登录新号即可来回切。
- 登录中：会弹出窗口和浏览器，请用新号登录，完成后再回到这里。
- 删除确认：删除后只是拿掉本机快照，不会退出当前 Claude。确认删除 {name}？

按钮忙碌态文案与 Grok 相同：保存中… / 登录中… / 切换中… / 删除中…。

切号或登录新号成功后：

- 关掉所有 `toolId === 'claude'` 的 live PTY（`ptyKill` + `termRef.dispose` + `markPtyExit`）
- 有关掉的终端：`已切换 Claude 账号，并关掉旧终端`
- 否则：`已切换 Claude 账号`
- 当前会话筛选是 Claude 时 `refreshSessions()`

第一版不加 `claudeAuthRev` 之类的 store 计数：没有用量轮询要刷新。

## Data model

前端 / IPC 与 Grok 同形，字段名 camelCase：

```ts
interface ClaudeAccount {
  id: string
  name: string
  email: string
  accountUuid: string
  subscriptionType: string
  updatedAt: string
  active: boolean
}

interface ClaudeAccountList {
  loggedIn: boolean
  currentEmail: string
  currentName: string
  accounts: ClaudeAccount[]
}
```

`subscriptionType` 来自 live/snapshot 的 `claudeAiOauth.subscriptionType`（例如 `pro`），没有则空字符串。列表主行仍是显示名，邮箱在第二行；不单独做套餐徽章。

身份匹配（保存时判断是否已有快照）：`accountUuid` 非空且相等，否则 `email` 非空且相等。

显示名：用户输入（trim，最长 40）→ `oauthAccount.displayName` 或 `fullName` → email → `Claude 账号`。

## On-disk layout

应用数据目录（`app.path().app_data_dir()`）下：

```
claude-accounts/
  index.json
  {id}/
    credentials.json    # 固定为 { "claudeAiOauth": { ... } }，不含 mcpOAuth
    oauth-account.json  # oauthAccount 对象
```

`index.json`：

```json
{
  "accounts": [
    {
      "id": "uuid",
      "name": "工作号",
      "email": "a@example.com",
      "accountUuid": "…",
      "subscriptionType": "pro",
      "updatedAt": "RFC3339"
    }
  ]
}
```

index 不含 token。`id` 非法（空、含 `/` `\` `.`）拒绝读写。

写文件用与 Grok 相同的 tmp + rename。Windows/Linux 的 credentials 文件权限：Unix 上 `0o600`。

Live 路径：

| 角色 | 路径 |
| --- | --- |
| Claude home | `CLAUDE_HOME`，否则 `~/.claude`（复用 `tools::claude::claude_home()`） |
| OAuth token | `{claude_home}/.credentials.json` |
| 账号缓存 | `{home}/.claude.json`（在用户主目录，不在 `.claude/` 里） |
| CLI 设置 | `{claude_home}/settings.json` |

不要读 `{claude_home}/.claude.json` 当账号缓存：本机该文件没有 `oauthAccount`。

## Commands

Tauri 命令，镜像 Grok：

| Command | 行为 |
| --- | --- |
| `list_claude_accounts` | `persist_live` 后返回列表 |
| `save_claude_account(name)` | 把当前 live 存成快照 |
| `switch_claude_account(accountId)` | 先 persist 当前，再写回目标快照 |
| `delete_claude_account(accountId)` | 删 index 项和 `{id}/` 目录 |
| `login_claude_account` | 异步 `spawn_blocking`，保存当前 → 清本地 live → `claude auth login --claudeai` → 保存新号 |

前端 `src/lib/api.ts`：`listClaudeAccounts` / `saveClaudeAccount` / `switchClaudeAccount` / `deleteClaudeAccount` / `loginClaudeAccount`。非 Tauri 返回空列表。

## Live credential helpers

内部函数必须能在测试里用临时目录调用（不要把核心逻辑绑死在 `AppHandle` 上）。建议：

```rust
struct LivePaths {
    credentials: PathBuf,   // .credentials.json
    claude_json: PathBuf,   // ~/.claude.json
    settings: PathBuf,      // settings.json
}

struct Identity {
    email: String,
    account_uuid: String,
    display: String,
    subscription_type: String,
}
```

读身份：优先 `oauthAccount` 的 `emailAddress` + `accountUuid` + `displayName`/`fullName`；`subscriptionType` 从 `claudeAiOauth` 补。没有 uuid 且没有 email 视为未登录。

写 credentials：读出现有 JSON 对象（没有或损坏则新建对象），只替换 `claudeAiOauth`，保留其它键（例如 `mcpOAuth`）。原子写回。

写 `oauthAccount`：读出 `~/.claude.json`（没有则报错「没有找到 ~/.claude.json」，不要整文件重建以免丢掉 projects/mcpServers）。只替换或插入 `oauthAccount` 键。清登录时 `remove("oauthAccount")`。

macOS：token 可能在钥匙串，服务名 `Claude Code-credentials`。

- 读：文件里已有 `claudeAiOauth` 则用文件；否则读钥匙串，解析为同一 JSON。
- 写：写文件，并 `security add-generic-password` / 先 delete 再 add，服务名 `Claude Code-credentials`。
- 清：删文件中的 `claudeAiOauth`，并删该钥匙串项。

Windows / Linux 只走文件。钥匙串逻辑用 `#[cfg(target_os = "macos")]`，测试仍用文件夹具。

## Switch algorithm

1. 若当前能读到 Identity，先 `save_current`（空名字，走 normalize）。这样 live 里刚刷新的 token 会写回旧快照。
2. 确认 index 有该 `accountId`，且 `{id}/credentials.json`、`{id}/oauth-account.json` 都在；否则：`这个账号的登录文件丢了，请重新登录并保存。`
3. 把 snapshot 的 `claudeAiOauth` 写进 live credentials。
4. 把 snapshot 的 `oauthAccount` 写进 `~/.claude.json`。
5. 从 `{claude_home}/settings.json` 的 `env` 对象删除键：`ANTHROPIC_API_KEY`、`ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_BASE_URL`。文件不存在则跳过。不要改其它设置。
6. 返回 `list_accounts`。

**禁止**在切换时调用 `claude auth logout`。官方 logout 会吊销当前凭证，已保存的 refresh token 会失效。

## Login-new algorithm

1. 若当前已登录，`save_current`（空名字）。
2. **只清本地 live**：去掉 credentials 里的 `claudeAiOauth`；去掉 `~/.claude.json` 的 `oauthAccount`；macOS 同步清钥匙串。不要调用 `claude auth logout`。
3. `tools::resolve_binary(ToolId::Claude, settings)` 得到 `claude`。
4. 按设置注入 `proxy_env(true, default_proxy_url)`（与 Grok 登录相同）。
5. 可见控制台运行 `claude auth login --claudeai`（Windows `CREATE_NEW_CONSOLE`，与 Grok `apply_window(visible=true)` 相同）。
6. 登录进程退出后读 live Identity；没有邮箱/uuid 则：`登录没有完成。请在弹出的窗口或浏览器里登完新号后再试。`
7. `save_current` 后返回列表。

`claude auth login --claudeai` 走订阅号，不要加 `--console`。

## persist_live

`list_accounts` 开头调用。当前 Identity 能在 index 里匹配到时：更新 email、subscriptionType、updatedAt，并把 live credentials / oauthAccount 拷回该 id 目录。未匹配则不自动建档（用户要点「保存当前」）。

## Error copy

| 情况 | 文案 |
| --- | --- |
| 保存时未登录 | 当前没有登录的 Claude 账号，请先登录。 |
| 保存时没有 credentials 文件 | 没有找到 Claude 登录文件，请先 claude auth login。 |
| 切换未知 id | 没有这个 Claude 账号快照。 |
| 快照文件缺失 | 这个账号的登录文件丢了，请重新登录并保存。 |
| 登录未完成 | 登录没有完成。请在弹出的窗口或浏览器里登完新号后再试。 |
| 找不到 ~/.claude.json | 没有找到 ~/.claude.json，请先成功登录过一次 Claude Code。 |
| 非法 id | 账号编号不合法。 |
| 找不到 claude 二进制 | 沿用 `resolve_binary` 现有错误 |

不要把 token 写进日志、toast 或错误字符串。

## Frontend files

- Create `src/components/ClaudeAccountDialog.vue`（从 Grok 对话框复制后改 API 与文案）
- Modify `src/components/TitleBar.vue`：emit `claude-accounts`；菜单项「Claude 账号」紧挨「Grok 账号」下面
- Modify `src/App.vue`：`claudeAccountOpen`；`onClaudeSwitched` 杀 Claude PTY
- Modify `src/lib/types.ts`、`src/lib/api.ts`
- Modify `README.md` 控制台能力表，在 Grok 多账号下增加：`**Claude 多账号** | 切换账号会关掉旧 Claude 终端，避免混用身份`

## Backend files

- Create `src-tauri/src/claude_accounts.rs`
- Modify `src-tauri/src/lib.rs`：`mod claude_accounts`、五个 command、invoke_handler 注册

不改 `tools/claude.rs` 的 session 扫描。不改 pty 打开参数（新终端读 live 文件即可）。

## Tests

`claude_accounts.rs` 的 `#[cfg(test)]`，用 `tempfile` 目录，不启 Tauri、不跑真实 login。

必须覆盖：

1. 从 `oauthAccount` + `claudeAiOauth` 读出 email、accountUuid、displayName、subscriptionType。
2. 写 `claudeAiOauth` 时保留 credentials 里其它键（`mcpOAuth`）。
3. 写 `oauthAccount` 时保留 `~/.claude.json` 其它键（`projects`、`mcpServers`）。
4. 清登录时去掉 `oauthAccount` 和 `claudeAiOauth`，其它键仍在。
5. 切到 OAuth 快照时删除 settings.json `env` 里的三个 ANTHROPIC_* 键，其它 env 保留。
6. 按 accountUuid 匹配已有快照并更新，不新建重复项。
7. 删除快照后 index 与目录都消失，live 文件不动。
8. 非法 id 被拒绝。

运行：`cargo test claude_accounts`（Windows 上若 os error 5，按现有惯例直接跑 `target/debug/deps/agent_dock_lib-*.exe claude_accounts`）。

前端不强制单测对话框；api 非 Tauri 空列表行为与 Grok 相同即可。

## Risks

- **logout 吊销 token**：切号和登录新号都禁止 `claude auth logout` / `/logout`。只改本地文件。
- **残留 oauthAccount**：登录新号前必须删掉该键，否则 Claude 会报登录成功仍用旧号（anthropics/claude-code#23906）。
- **CC Switch 留下的 API Key**：切到 Pro 快照时清 settings env 中的三个键，否则 Key 优先于 OAuth。
- **macOS 钥匙串**：只改文件在 Mac 上可能无效；macOS 构建必须同步 `Claude Code-credentials`。
- **并发 Claude 进程**：切号前关 Dock 内终端；用户自己在外部开的 `claude` 不会被杀，可能把旧 token 写回。文案已要求关掉后新建会话。

## Implementation order

1. `claude_accounts.rs` + 单测（路径注入、读写、切换、删除）
2. `lib.rs` 注册命令
3. types + api
4. `ClaudeAccountDialog.vue` + TitleBar + App.vue 关终端
5. README 一行
6. 桌面端手测：保存当前号 → 登录新号 → 切回 → 删除快照 → 确认会话列表仍在
)