<p align="center">
  <img src="src-tauri/icons/128x128.png" width="112" height="112" alt="Agent Dock">
</p>

<h1 align="center">Agent Dock</h1>

<p align="center">
  <em>本机 AI 特工的统一驾驶舱</em>
</p>

<p align="center">
  无边框深色桌面端 · 一窗接管 OpenCode / Grok / Kimi / Claude / Pi / DeepSeek<br>
  编排把 <b>Cursor 开发</b> 与 <b>Codex 审查</b> 串成过闸流水线
</p>

<p align="center">
  <a href="https://github.com/aodaokai52545856/agent-dock/releases/tag/v0.1.6"><img src="https://img.shields.io/badge/Download-v0.1.6-0A0A0A?style=for-the-badge&logo=github&logoColor=white" alt="Download v0.1.6"></a>
  &nbsp;
  <a href="#快速开始"><img src="https://img.shields.io/badge/Quick_Start-文档-1a1a1a?style=for-the-badge" alt="Quick Start"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.1.6-111111?style=flat-square" alt="version">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-0d0d0d?style=flat-square" alt="platform">
  <img src="https://img.shields.io/badge/stack-Tauri%202%20·%20Vue%203%20·%20Rust-141414?style=flat-square" alt="stack">
  <img src="https://img.shields.io/badge/license-Apache%202.0-3d9a6a?style=flat-square" alt="license">
</p>

<br>

<p align="center">
  <img width="92%" alt="Agent Dock · 控制台" src="https://github.com/user-attachments/assets/1380fac6-b7d7-41b8-97e1-3379e781547b" />
</p>
<p align="center">
  <sub>控制台 · 项目工作区 · 内嵌终端 · 用量与文档栏</sub>
</p>

<p align="center">
  <img width="92%" alt="Agent Dock · 编排" src="https://github.com/user-attachments/assets/f89b0c59-6351-43d1-b84c-9265ff1c0da3" />
</p>
<p align="center">
  <sub>编排 · 流程图 · 开发-审查闸 · 通道桥接</sub>
</p>

<br>

## 为什么需要 Dock

本机已经有 `opencode`、`grok`、`kimi`、`claude`、Pi、DeepSeek、Cursor、Codex，但它们散落在各自终端和窗口里：会话难找、代理不一致、开发和审查抢同一条 thread。

**Agent Dock** 把它们收进一个 frameless 深色窗口。客户只装安装包即可运行，不必装 Rust。

<table>
  <tr>
    <td width="33%" valign="top">
      <h3>一窗接管</h3>
      <p>挂上项目目录，按 CLI 扫描磁盘 session。点开即在内嵌 xterm 续聊（Windows ConPTY · macOS / Linux PTY）。</p>
    </td>
    <td width="33%" valign="top">
      <h3>按项目代理</h3>
      <p>只写入该终端的 <code>HTTP_PROXY</code> / <code>HTTPS_PROXY</code>。不动系统环境变量，项目之间互不干扰。</p>
    </td>
    <td width="33%" valign="top">
      <h3>开发 · 审查过闸</h3>
      <p>Cursor SDK 写完一块，Codex <code>review/start</code> 以 <b>detached</b> 审查。未通过不能进下一片。</p>
    </td>
  </tr>
  <tr>
    <td width="33%" valign="top">
      <h3>用量与密钥</h3>
      <p>Grok 额度 / token 花费、DeepSeek 余额一目了然。密钥在 Dock 内托管，明文不进仓库。</p>
    </td>
    <td width="33%" valign="top">
      <h3>意见桥接</h3>
      <p>流程图节点可绑 Grok 窗口。边可选自动或手动桥接，把审查意见交给下一角色。</p>
    </td>
    <td width="33%" valign="top">
      <h3>外观可控</h3>
      <p>浅色 / 深色 / 跟随系统。强调色、字体、对比度、侧栏半透明、窗口透明度与毛玻璃。</p>
    </td>
  </tr>
</table>

---

## 下载

> **v0.1.6** · 跨平台安装包 · 无需本机 Rust

| 文件 | 平台 | 说明 | 大小约 |
| :---: | :---: | --- | ---: |
| [**AgentDock-0.1.6-Setup.exe**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.6/AgentDock-0.1.6-Setup.exe) | Windows | NSIS 安装包（推荐） | ~4.4 MB |
| [**AgentDock-0.1.6.exe**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.6/AgentDock-0.1.6.exe) | Windows | 绿色便携版 | ~17 MB |
| [**AgentDock-0.1.6.dmg**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.6/AgentDock-0.1.6.dmg) | macOS | 磁盘映像 | ~7.5 MB |
| [**AgentDock-0.1.6.app.tar.gz**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.6/AgentDock-0.1.6.app.tar.gz) | macOS | `.app` 压缩包 | ~6.8 MB |
| [**AgentDock-0.1.6.AppImage**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.6/AgentDock-0.1.6.AppImage) | Linux | 免安装可执行 | ~82 MB |
| [**AgentDock-0.1.6.deb**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.6/AgentDock-0.1.6.deb) | Linux | Debian / Ubuntu | ~7.7 MB |

<p align="center">
  <a href="https://github.com/aodaokai52545856/agent-dock/releases/tag/v0.1.6">→ 打开 Releases · v0.1.6</a>
</p>

**运行前提**

| | |
| --- | --- |
| **Windows** | Win10 / 11 + [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)（多数机器已自带） |
| **macOS / Linux** | 系统 WebView / WebKitGTK 等运行时 |
| **CLI** | `opencode` / `grok` / `kimi` / `claude` 等自行安装并登录；Dock **不代登录** |
| **编排** | 自动开发需 Cursor API Key；Codex 审查需本机已登录 Codex |

<details>
<summary><b>v0.1.6 更新要点</b></summary>

<br>

- 侧栏可多选删除会话，并记住每个项目的筛选
- Grok 用量在额度重置前加快刷新
- Claude 窗口可 <kbd>Ctrl</kbd>+<kbd>V</kbd> 粘贴文字和图片
- README 升级为产品页呈现

</details>

> **macOS 提示：** CI 产物暂未 Apple 公证。若提示「已损坏」，终端执行 `xattr -cr ~/Downloads/AgentDock-0.1.6.dmg`，或对 app **右键 → 打开**。

---

## 两种模式

标题栏一键切换。默认 **控制台**；切到编排 **不会关掉** 已打开的终端。

### 控制台

左侧挂项目，按 OpenCode / Grok / Kimi / Claude Code / Pi / DeepSeek 扫描 session，点开即在内嵌终端续聊（DeepSeek 走 Web 嵌入）。

| 能力 | 说明 |
| --- | --- |
| **文档栏** | Grok / Kimi 写出的 plan / spec / doc，可预览 Markdown，可引用对话片段 |
| **Grok 多账号** | 切换账号会关掉旧 Grok 终端，避免混用身份 |
| **用量与花费** | 状态栏读取 Grok 额度与 token；DeepSeek 显示剩余人民币 |
| **CLI 版本** | 探测、升级、卸载本机工具；可安装并启动 CC Switch |
| **DeepSeek 密钥** | 在 Dock 管理 `.credentials.yaml`，明文不进仓库 |
| **外观** | 浅色 / 深色 / 跟随系统；强调色、字体、对比度、侧栏半透明、窗口透明度与毛玻璃 |

### 编排

同一套项目，主画布没有终端。左上项目、左下已保存流程；右栏在 **编排** 画图，在 **运行** 预览并人工干预。

- 默认一对一：**开发者 → 审查者**（通过则结束，未通过回到开发者）
- 角色库预置开发者 / 审查者 / 项目经理，也可自建
- 节点绑通道：Codex 应用程序、Grok 窗口、Cursor SDK、人工
- 边可选自动或手动桥接
- 内置模板：**开发-审查闸** · **Codex → Grok**
- 同一工作区同一时刻只应有一个写者

---

## 环境要求

| 平台 | 开发 | 客户运行 | WebView |
| --- | --- | --- | --- |
| **Windows** | Rust + Node.js · 终端默认 PowerShell | 安装包即可 | WebView2 |
| **macOS 10.15+** | Rust + Node.js · `$SHELL` / zsh | 安装包即可 | 系统 WKWebView |
| **Linux** | Rust + Node.js · `$SHELL` / bash | 安装包即可 | WebKitGTK 4.1 |

从 Finder / 开始菜单 / 应用菜单打开时，Dock 会补上 Homebrew、nvm、`~/.local/bin`、`~/.kimi-code/bin` 等常见 PATH。

---

<a id="快速开始"></a>

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

**必须在对应系统上打包**，不能从 Windows 打出 `.dmg` / `.AppImage`。产物把界面打进二进制，不再监听开发端口，落到 `release/`。

版本号读 `package.json` 的 `version`。当前 `0.1.6` 会打出 `AgentDock-0.1.6-Setup.exe` 等文件名。

```bash
npm run pack          # 当前系统
npm run pack:win      # .exe + -Setup.exe
npm run pack:mac      # .app + .dmg
npm run pack:linux    # .AppImage + .deb
```

GitHub 打 tag `v*`，或手动跑 [`release`](.github/workflows/release.yml) workflow，会在 Windows / macOS / Ubuntu runner 上打包并上传 artifact。

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
npm run test:fit && npm run test:layout && npm run test:bridge
npm run test:flow && npm run test:usage && npm run test:appearance
npm run test:cite && npm run test:ccswitch && npm run test:dsh
npm run test:live && npm run test:pack
```

**技术栈** · Tauri 2 · Vue 3 · Vite · Rust（`portable-pty`）· xterm.js

---

## 许可证

[Apache License 2.0](LICENSE)
