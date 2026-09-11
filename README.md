# Agent Dock

Windows / macOS / Linux 桌面启动器，同一窗口两种顶层模式（标题栏切换，默认控制台）：

- **控制台**：左侧挂项目文件夹，中间按 OpenCode / Grok / Kimi 扫描 CLI session，在内嵌终端里续对话。
- **编排**：同一套项目，主画布没有终端。把 Cursor（开发）和 Codex（审查）串成过闸流水线：写完一块再 `review/start` detached，未通过不能进入下一片。不会去遥控 Codex 桌面窗口里的 live turn。

代理只写进控制台打开的终端的 `HTTP_PROXY` / `HTTPS_PROXY`。切到编排不会关掉已开的 PTY。

Windows 使用 PowerShell + ConPTY；macOS 使用系统 Shell（默认 `$SHELL` / zsh）+ Unix PTY；Linux 使用系统 Shell（默认 `$SHELL` / bash）+ Unix PTY。

## 启动

```bash
cd agent-dock
npm install
npm run icon:gen
npm run tauri:dev
```

需要本机已安装 Rust。Windows 还需要 WebView2；macOS 使用系统 WKWebView；Linux 需要 WebKitGTK 4.1。三个 CLI 本身请自行安装并登录；本工具不代登录。

从 Finder / 应用菜单打开时，应用会自动补上 Homebrew / nvm / `~/.local/bin` / `~/.kimi-code/bin` 等常见 PATH，避免找不到 `kimi`、`grok`、`opencode`、`npm`。

## 打包

开发热更新固定走 `http://localhost:1421`。打好的安装包把界面打进二进制，不再监听端口。

**必须在对应系统上打包**，不能从 Windows 打出 `.dmg` / `.AppImage`。

```bash
npm run pack          # 当前系统
npm run pack:win      # Windows：AgentDock.exe + AgentDock-Setup.exe
npm run pack:mac      # macOS：Agent Dock.app + AgentDock.dmg
npm run pack:linux    # Linux：AgentDock.AppImage + AgentDock.deb
```

产物都落到 `release/`。`npm run pack:exe` 仍可用，等同 `pack:win`。

GitHub 上打 tag `v*` 或手动跑 `release` workflow，会分别在 Windows / macOS / Ubuntu runner 上打包并上传 artifact。

## 代理

创建或编辑项目时可勾选「使用代理」，默认 `http://127.0.0.1:7890`。只影响该项目打开的终端，不改系统环境变量。

## 编排模式

本机需要已登录的 Codex（桌面端与 CLI 共用 `CODEX_HOME` 线程库）。编排自己拉 `codex app-server`：

- `thread/list` 按项目目录列出桌面/CLI session，当作审查上下文，**不** `thread/resume`。Windows 上桌面端线程目前多标成 `vscode`，cwd 对得上就能勾选
- `review/start` 使用 `delivery: "detached"`，避免和桌面抢同一条 thread 的 writer
- 半自动：你在 Cursor IDE 里写代码，Dock 里提交审查；官方结论是文本，读到 `VERDICT: PASS/FAIL` 或「通过/未通过」才自动开闸，否则请人工标记
- 自动开发：设置里填写 Cursor API Key 后走 `@cursor/sdk`（与 IDE 额度分开）。同一工作区同一时刻只应有一个写者

```bash
cd agent-dock
npm run test:bridge
npm run spike:app-server -- "D:\\path\\to\\project"
```
<img width="1879" height="1168" alt="image" src="https://github.com/user-attachments/assets/21ed1a89-c54f-4a85-8b8a-5a6a4c4f4d43" />

