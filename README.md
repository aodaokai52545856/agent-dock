<p align="center">
  <img src="src-tauri/icons/128x128.png" width="88" height="88" alt="Agent Dock">
</p>

<h1 align="center">Agent Dock</h1>

<p align="center">
  <strong>把本机 AI CLI 收进同一个桌面窗口。</strong><br>
  控制台续 OpenCode / Grok / Kimi 会话；编排把 Cursor 开发与 Codex 审查串成过闸流水线。
</p>

<p align="center">
  <img alt="version" src="https://img.shields.io/badge/version-0.1.0-111111?style=flat-square">
  <img alt="platform" src="https://img.shields.io/badge/Windows%20%7C%20macOS%20%7C%20Linux-0d0d0d?style=flat-square">
  <img alt="stack" src="https://img.shields.io/badge/Tauri%202%20%2B%20Vue%203%20%2B%20Rust-141414?style=flat-square">
  <img alt="license" src="https://img.shields.io/badge/license-Apache%202.0-3d9a6a?style=flat-square">
</p>

<p align="center">
  <a href="#启动">启动</a> ·
  <a href="#两种模式">模式</a> ·
  <a href="#打包">打包</a> ·
  <a href="#编排模式">编排</a> ·
  <a href="#开发">开发</a>
</p>

<p align="center">
  <img width="1879" alt="Agent Dock 控制台截图" src="https://github.com/user-attachments/assets/21ed1a89-c54f-4a85-8b8a-5a6a4c4f4d43">
</p>

---

## 它解决什么

本地已经有 `opencode`、`grok`、`kimi`、Cursor、Codex，但它们散落在各自终端和窗口里：会话找不到、代理环境不一致、审查和开发抢同一条 thread。

Agent Dock 是一个 **frameless 深色桌面启动器**：

| 你想做的事 | Dock 怎么接 |
| --- | --- |
| 挂上几个项目文件夹 | 左侧工作区，按目录扫 CLI session |
| 续上一次对话 | 内嵌 xterm，Windows 走 ConPTY，macOS / Linux 走系统 PTY |
| 只给某个项目走代理 | 写入该终端的 `HTTP_PROXY` / `HTTPS_PROXY`，不动系统环境变量 |
| 写完一块再让 Codex 审 | 编排模式：`review/start` **detached**，未通过不能进下一片 |
| Cursor 自动写、Codex 自动审 | 设置里填 Cursor API Key，走 `@cursor/sdk`（与 IDE 额度分开） |

**不代登录。** 三个 CLI 以及 Codex / Cursor 都请在本机自行安装并登录。

---

## 两种模式

标题栏切换，默认 **控制台**。切到编排 **不会关掉** 已经打开的终端。

### 控制台

左侧挂项目，按 OpenCode / Grok / Kimi 扫描磁盘上的 session，点开即在内嵌终端续聊。

同时带上这些控制台能力：

- **文档栏**：Grok / Kimi 会话写出的 plan / spec / doc，可预览 Markdown，也可引用对话片段
- **Grok 多账号**：切换账号会关掉旧的 Grok 终端，避免混用身份
- **用量与花费**：状态栏 / 面板读取 Grok 额度与 token 花费
- **外观**：浅色 / 深色 / 跟随系统，强调色、字体、对比度、侧栏半透明、窗口透明度（下限 0%）

### 编排（施工中）

同一套项目，主画布没有终端。把「开发 → 审查 → 过闸」收成工作台：

1. 左侧选项目、勾选本机 Codex 线程当审查上下文（**不会** `thread/resume`，不往桌面 live turn 里塞字）
2. 中间写本轮任务，提交审查或启动自动开发
3. 右侧看 App Server、审查目标（未提交 diff / commit / 相对分支 / 自定义）、git 摘要
4. 审查结果做成时间线；读到 `VERDICT: PASS/FAIL` 或「通过 / 未通过」才自动开闸，否则人工标记

同一工作区同一时刻只应有一个写者。控制台里该项目若还开着终端，画布会提示不要两边一起改同一棵树。

---

## 环境要求

| 平台 | 运行时 | WebView |
| --- | --- | --- |
| Windows | Rust + Node.js，终端默认 PowerShell | WebView2 |
| macOS 10.15+ | Rust + Node.js，默认 `$SHELL` / zsh | 系统 WKWebView |
| Linux | Rust + Node.js，默认 `$SHELL` / bash | WebKitGTK 4.1 |

CLI 按需自备：`opencode`、`grok`、`kimi`；编排还需要已登录的 **Codex**（桌面端与 CLI 共用 `CODEX_HOME`）。自动开发另需 **Cursor API Key**。

从 Finder / 开始菜单 / 应用菜单打开时，Dock 会补上 Homebrew、nvm、`~/.local/bin`、`~/.kimi-code/bin` 等常见 PATH，避免找不到这些命令。

---

## 启动

```bash
git clone https://github.com/aodaokai52545856/agent-dock.git
cd agent-dock
npm install
npm run icon:gen
npm run tauri:dev
```

开发热更新固定走 `http://localhost:1421`。浏览器里打开只能预览壳子（mock 数据），完整能力在桌面端。

---

## 打包

**必须在对应系统上打包**，不能从 Windows 打出 `.dmg` / `.AppImage`。打好的安装包把界面打进二进制，不再监听开发端口。产物落到 `release/`。

```bash
npm run pack          # 当前系统
npm run pack:win      # AgentDock.exe + AgentDock-Setup.exe
npm run pack:mac      # Agent Dock.app + AgentDock.dmg
npm run pack:linux    # AgentDock.AppImage + AgentDock.deb
```

`npm run pack:exe` 仍可用，等同 `pack:win`。

GitHub 上打 tag `v*` 或手动跑 [`release`](.github/workflows/release.yml) workflow，会分别在 Windows / macOS / Ubuntu runner 上打包并上传 artifact。

---

## 代理

创建或编辑项目时可勾选「使用代理」，默认 `http://127.0.0.1:7890`。

- 只注入该项目打开的控制台终端
- 不改系统环境变量
- 切换到编排模式也不会关掉已开的 PTY

---

## 编排模式

本机 Codex 需已登录。Dock 自己拉 `codex app-server`：

- `thread/list` 按项目目录列出桌面 / CLI session 当审查上下文。Windows 上桌面线程目前多标成 `vscode`，cwd 对得上就能勾选
- `review/start` 使用 `delivery: "detached"`，避免和 Codex 桌面抢同一条 thread 的 writer
- **半自动**：你在 Cursor IDE 里写完，回到 Dock 点审查
- **自动开发**：设置里填写 Cursor API Key 后走 `@cursor/sdk`

```bash
npm run test:bridge
npm run spike:app-server -- "/path/to/project"
```

---

## 开发

```text
agent-dock/
├── src/                 Vue 3 界面（控制台 + 编排）
│   ├── components/
│   └── lib/             store / pipeline / appearance / 会话文档
├── src-tauri/           Rust：PTY、CLI 适配、Codex 桥、Cursor 开发
├── scripts/             图标、打包、Codex spike、Cursor agent
└── .github/workflows/   跨平台 release
```

常用检查：

```bash
npm run test:fit
npm run test:layout
npm run test:bridge
npm run test:usage
npm run test:appearance
npm run test:cite
npm run test:live
npm run test:pack
```

技术栈：**Tauri 2** · **Vue 3** · **Vite** · **Rust**（`portable-pty`）· **xterm.js**。

---

## 许可证

[Apache License 2.0](LICENSE)
