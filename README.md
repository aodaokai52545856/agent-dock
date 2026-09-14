<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" height="96" alt="Agent Dock">
</p>

<h1 align="center">Agent Dock</h1>

<p align="center">
  <strong>本机 AI 特工的统一驾驶舱。</strong><br>
  一个窗口接管 OpenCode / Grok / Kimi / Claude / Pi / DeepSeek 会话，<br>
  再用编排把 Cursor 开发与 Codex 审查串成过闸流水线。
</p>

<p align="center">
  <a href="https://github.com/aodaokai52545856/agent-dock/releases/tag/v0.1.2"><img alt="download" src="https://img.shields.io/badge/%E2%AC%87%20%E6%B5%8B%E8%AF%95%E5%8C%85%20v0.1.2-111111?style=for-the-badge"></a>
</p>

<p align="center">
  <img alt="version" src="https://img.shields.io/badge/version-0.1.2-111111?style=flat-square">
  <img alt="platform" src="https://img.shields.io/badge/Windows%20%7C%20macOS%20%7C%20Linux-0d0d0d?style=flat-square">
  <img alt="stack" src="https://img.shields.io/badge/Tauri%202%20%2B%20Vue%203%20%2B%20Rust-141414?style=flat-square">
  <img alt="license" src="https://img.shields.io/badge/license-Apache%202.0-3d9a6a?style=flat-square">
</p>

<p align="center">
  <a href="#%E9%A6%96%E7%89%88%E6%B5%8B%E8%AF%95%E4%B8%8B%E8%BD%BD">下载</a> ·
  <a href="#%E4%B8%A4%E7%A7%8D%E6%A8%A1%E5%BC%8F">模式</a> ·
  <a href="#%E5%90%AF%E5%8A%A8">启动</a> ·
  <a href="#%E6%89%93%E5%8C%85">打包</a> ·
  <a href="#%E7%BC%96%E6%8E%92%E6%A8%A1%E5%BC%8F">编排</a> ·
  <a href="#%E5%BC%80%E5%8F%91">开发</a>
</p>

<p align="center">
    <img width="1412" height="986" alt="image" src="https://github.com/user-attachments/assets/1380fac6-b7d7-41b8-97e1-3379e781547b" />
    <img width="1412" height="986" alt="image" src="https://github.com/user-attachments/assets/f89b0c59-6351-43d1-b84c-9265ff1c0da3" />
</p>

---

## 测试下载

> **v0.1.2** · 跨平台测试包 · 不需要本机装 Rust

| 文件 | 平台 | 说明 | 大小约 |
| --- | --- | --- | --- |
| [**AgentDock-0.1.2-Setup.exe**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.2/AgentDock-0.1.2-Setup.exe) | Windows | NSIS 安装包（推荐） | ~3.5 MB |
| [**AgentDock-0.1.2.exe**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.2/AgentDock-0.1.2.exe) | Windows | 绿色便携版 | ~14 MB |
| [**AgentDock-0.1.2.dmg**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.2/AgentDock-0.1.2.dmg) | macOS | 磁盘映像 | ~6.3 MB |
| [**AgentDock-0.1.2.app.tar.gz**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.2/AgentDock-0.1.2.app.tar.gz) | macOS | `.app` 压缩包 | ~5.6 MB |
| [**AgentDock-0.1.2.AppImage**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.2/AgentDock-0.1.2.AppImage) | Linux | 免安装可执行 | ~79 MB |
| [**AgentDock-0.1.2.deb**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.2/AgentDock-0.1.2.deb) | Linux | Debian / Ubuntu | ~5.4 MB |

发布页：[Releases · v0.1.2](https://github.com/aodaokai52545856/agent-dock/releases/tag/v0.1.2)

**运行前提**

- **Windows：** Win10/11 + [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)（多数机器已自带）
- **macOS / Linux：** 按系统装好 WebView / WebKitGTK 等运行时即可
- CLI（`opencode` / `grok` / `kimi` / `claude` 等）请自行安装并登录；Dock **不代登录**
- 编排自动开发需要 Cursor API Key；Codex 审查需要本机已登录 Codex

---

## 它解决什么

本地已经有 `opencode`、`grok`、`kimi`、`claude`、Pi、DeepSeek、Cursor、Codex，但它们散落在各自终端和窗口里：会话找不到、代理环境不一致、审查和开发抢同一条 thread。

Agent Dock 是一个 **frameless 深色桌面启动器**：

| 你想做的事 | Dock 怎么接 |
| --- | --- |
| 挂上几个项目文件夹 | 左侧工作区，按目录扫 CLI session |
| 续上一次对话 | 内嵌 xterm，Windows 走 ConPTY，macOS / Linux 走系统 PTY |
| 只给某个项目走代理 | 写入该终端的 `HTTP_PROXY` / `HTTPS_PROXY`，不动系统环境变量 |
| 写完一块再让 Codex 审 | 编排：`review/start` **detached**，未通过不能进下一片 |
| Cursor 自动写、Codex 自动审 | 设置里填 Cursor API Key，走 `@cursor/sdk`（与 IDE 额度分开） |
| 把审查意见交给 Grok | 流程图节点绑 Grok 窗口，边可选自动或手动桥接 |

**客户只需安装 exe / dmg / AppImage，不需要 Rust。** 源码开发才需要本机工具链。

---

## 两种模式

标题栏切换，默认 **控制台**。切到编排 **不会关掉** 已经打开的终端。

### 控制台

左侧挂项目，按 OpenCode / Grok / Kimi / Claude Code / Pi / DeepSeek 扫描磁盘上的 session，点开即在内嵌终端续聊（DeepSeek 走 Web 嵌入）。

同时带上这些控制台能力：

- **文档栏**：Grok / Kimi 会话写出的 plan / spec / doc，可预览 Markdown，也可引用对话片段
- **Grok 多账号**：切换账号会关掉旧的 Grok 终端，避免混用身份
- **用量与花费**：状态栏 / 面板读取 Grok 额度与 token 花费；DeepSeek 显示剩余人民币
- **CLI 版本**：探测、升级、卸载本机工具；可安装并启动 CC Switch
- **DeepSeek 密钥**：在 Dock 里管理 `.credentials.yaml`，不把明文写进仓库
- **外观**：浅色 / 深色 / 跟随系统，强调色、字体、对比度、侧栏半透明、窗口透明度与毛玻璃

### 编排

同一套项目，主画布没有终端。左边上面是项目，下面是已保存的流程。右边在 **编排** 里画流程图，在 **运行** 里预览并人工干预。

默认一对一：**开发者 → 审查者**，通过则结束，未通过回到开发者。角色库预置开发者 / 审查者 / 项目经理，也可以自己加角色。节点还要绑通道（Codex 应用程序、Grok 窗口、Cursor SDK、人工）。边可选自动或手动桥接。

内置模板：

- **开发-审查闸**：Cursor SDK 开发 → Codex 审查，未通过带意见返工
- **Codex → Grok**：审查意见桥接到已打开的 Grok 窗口

同一工作区同一时刻只应有一个写者。

---

## 环境要求

| 平台 | 开发 | 客户运行 | WebView |
| --- | --- | --- | --- |
| Windows | Rust + Node.js，终端默认 PowerShell | 安装包即可 | WebView2 |
| macOS 10.15+ | Rust + Node.js，默认 `$SHELL` / zsh | 安装包即可 | 系统 WKWebView |
| Linux | Rust + Node.js，默认 `$SHELL` / bash | 安装包即可 | WebKitGTK 4.1 |

从 Finder / 开始菜单 / 应用菜单打开时，Dock 会补上 Homebrew、nvm、`~/.local/bin`、`~/.kimi-code/bin` 等常见 PATH。

---

## 启动

```bash
git clone https://github.com/aodaokai52545856/agent-dock.git
cd agent-dock
npm install
npm run icon:gen
npm run tauri:dev
```

开发热更新固定走 `http://localhost:1421`。浏览器预览只有壳子（mock 数据），完整能力在桌面端。

---

## 打包

**必须在对应系统上打包**，不能从 Windows 打出 `.dmg` / `.AppImage`。打好的安装包把界面打进二进制，不再监听开发端口。产物落到 `release/`。

版本号读 `package.json` 的 `version`，产物文件名带这个版本，例如当前 `0.1.2` 会打出 `AgentDock-0.1.2-Setup.exe`。

```bash
npm run pack          # 当前系统
npm run pack:win      # AgentDock-<version>.exe + AgentDock-<version>-Setup.exe
npm run pack:mac      # Agent Dock.app + AgentDock-<version>.dmg
npm run pack:linux    # AgentDock-<version>.AppImage + AgentDock-<version>.deb
```

GitHub 打 tag `v*` 或手动跑 [`release`](.github/workflows/release.yml) workflow，会在 Windows / macOS / Ubuntu runner 上打包并上传 artifact。

---

## 代理

创建或编辑项目时可勾选「使用代理」，默认 `http://127.0.0.1:7890`。只注入该项目打开的控制台终端，不改系统环境变量。

---

## 编排模式

本机 Codex 需已登录。Dock 自己拉 `codex app-server`：

- `thread/list` 按项目目录列 session 当审查上下文，**不** `thread/resume`
- `review/start` 使用 `delivery: "detached"`，避免抢桌面 writer
- Grok 窗口可走 `ptyWrite` 桥接；手动边默认不回车，自动边回车并轮询新回合
- 内置「开发-审查闸」「Codex → Grok」模板

```bash
npm run test:bridge
npm run test:flow
npm run spike:app-server -- "/path/to/project"
```

---

## 开发

```text
agent-dock/
├── src/                 Vue 3 界面（控制台 + 编排）
├── src-tauri/           Rust：PTY、CLI、Codex 桥、Cursor、DeepSeek Web
├── scripts/             图标、打包、spike
└── .github/workflows/   跨平台 release
```

```bash
npm run test:fit
npm run test:layout
npm run test:bridge
npm run test:flow
npm run test:usage
npm run test:appearance
npm run test:cite
npm run test:ccswitch
npm run test:dsh
npm run test:live
npm run test:pack
```

技术栈：**Tauri 2** · **Vue 3** · **Vite** · **Rust**（`portable-pty`）· **xterm.js**。

---

## 许可证

[Apache License 2.0](LICENSE)
