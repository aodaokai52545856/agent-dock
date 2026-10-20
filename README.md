<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" height="96" alt="Agent Dock">
</p>

<h1 align="center">Agent Dock</h1>

<p align="center">
  <strong>本机 AI 特工的统一驾驶舱</strong>
</p>

<p align="center">
  无边框深色桌面端 · 一窗接管 OpenCode / Grok / Kimi / Claude / Pi / DeepSeek<br>
  编排把 <b>Cursor 开发</b> 与 <b>Codex 审查</b> 串成过闸流水线
</p>

<p align="center">
  <a href="https://github.com/aodaokai52545856/agent-dock/releases/tag/v0.1.11"><img src="https://img.shields.io/badge/⬇%20Download-v0.1.11-0A0A0A?style=for-the-badge&logo=github&logoColor=white" alt="Download v0.1.11"></a>
  &nbsp;
  <a href="#快速开始"><img src="https://img.shields.io/badge/Quick_Start-启动-1a1a1a?style=for-the-badge" alt="Quick Start"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.1.11-111111?style=flat-square" alt="version">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-0d0d0d?style=flat-square" alt="platform">
  <img src="https://img.shields.io/badge/stack-Tauri%202%20·%20Vue%203%20·%20Rust-141414?style=flat-square" alt="stack">
  <img src="https://img.shields.io/badge/license-Apache%202.0-3d9a6a?style=flat-square" alt="license">
</p>

<p align="center">
  <a href="#下载">下载</a>
  ·
  <a href="#v0111-新能力">新能力</a>
  ·
  <a href="#能力">能力</a>
  ·
  <a href="#两种模式">模式</a>
  ·
  <a href="#快速开始">启动</a>
  ·
  <a href="#打包">打包</a>
  ·
  <a href="#开发">开发</a>
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
  <sub>编排 · 流程图 · 开发–审查闸 · 通道桥接</sub>
</p>

<br>

## 为什么需要 Dock

本机已经有 `opencode`、`grok`、`kimi`、`claude`、Pi、DeepSeek、Cursor、Codex，但它们散落在各自终端和窗口里：会话难找、代理不一致、开发和审查抢同一条 thread。

**Agent Dock** 把它们收进一个 frameless 深色窗口。客户只装安装包即可运行，不必装 Rust。

---

<a id="v0111-新能力"></a>

## v0.1.11 新能力

<table>
  <tr>
    <td width="50%" valign="top">
      <h3>字符集下拉</h3>
      <p>全部转换一次做完简繁和洗符号。下拉可选台湾繁体、香港繁体、标准繁体。</p>
    </td>
    <td width="50%" valign="top">
      <h3>发送到当前聊天</h3>
      <p>复制改成纸飞机，把结果写入当前终端并发送。结果框仍可全选复制。</p>
    </td>
  </tr>
</table>

---

<a id="v0110-新能力"></a>

## v0.1.10 新能力

<table>
  <tr>
    <td width="50%" valign="top">
      <h3>台湾繁体字形</h3>
      <p>简繁转换按 OpenCC s2tw 字符级：里面→裡面、为→為。不用台湾词组，软件仍是軟件。</p>
    </td>
    <td width="50%" valign="top">
      <h3>港式异体也会洗</h3>
      <p>已经是繁体的港式字形（裏爲啓）会再收成台湾字形（裡為啟）。</p>
    </td>
  </tr>
</table>

---

<a id="v019-新能力"></a>

## v0.1.9 新能力

<table>
  <tr>
    <td width="50%" valign="top">
      <h3>简繁与符号清洗</h3>
      <p>右下角浮动按钮：简体转繁体字形，中文/全角标点改成英文半角。对不上的符号和 emoji 直接丢掉。</p>
    </td>
    <td width="50%" valign="top">
      <h3>结果可改再复制</h3>
      <p>转繁体、洗符号可分开点，也可一次全做。结果在面板里改完再复制。</p>
    </td>
  </tr>
</table>

---

<a id="v018-新能力"></a>

## v0.1.8 新能力

<table>
  <tr>
    <td width="50%" valign="top">
      <h3>Claude 多账号</h3>
      <p>系统菜单可保存、登录新号并切换 Claude Code Pro。切号只换身份，会话历史共用。</p>
    </td>
    <td width="50%" valign="top">
      <h3>切号关终端</h3>
      <p>切换后关掉已打开的 Claude 终端，避免新旧号混在同一窗口。</p>
    </td>
  </tr>
</table>

---

<a id="能力"></a>

## 能力

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

<a id="下载"></a>

## 下载

> **v0.1.11** · 跨平台安装包 · 无需本机 Rust

| 文件 | 平台 | 说明 | 大小约 |
| :---: | :---: | --- | ---: |
| [**AgentDock-0.1.11-Setup.exe**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.11/AgentDock-0.1.11-Setup.exe) | Windows | NSIS 安装包（推荐） | ~5.7 MB |
| [**AgentDock-0.1.11.exe**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.11/AgentDock-0.1.11.exe) | Windows | 绿色便携版 | ~18 MB |
| [**AgentDock-0.1.11.dmg**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.11/AgentDock-0.1.11.dmg) | macOS | 磁盘映像 | ~8.8 MB |
| [**AgentDock-0.1.11.app.tar.gz**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.11/AgentDock-0.1.11.app.tar.gz) | macOS | `.app` 压缩包 | ~8.1 MB |
| [**AgentDock-0.1.11.AppImage**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.11/AgentDock-0.1.11.AppImage) | Linux | 免安装可执行 | ~84 MB |
| [**AgentDock-0.1.11.deb**](https://github.com/aodaokai52545856/agent-dock/releases/download/v0.1.11/AgentDock-0.1.11.deb) | Linux | Debian / Ubuntu | ~9.0 MB |

<p align="center">
  <a href="https://github.com/aodaokai52545856/agent-dock/releases/tag/v0.1.11"><b>→ 打开 Releases · v0.1.11</b></a>
</p>

**运行前提**

| | |
| --- | --- |
| **Windows** | Win10 / 11 + [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)（多数机器已自带） |
| **macOS / Linux** | 系统 WebView / WebKitGTK 等运行时 |
| **CLI** | `opencode` / `grok` / `kimi` / `claude` 等自行安装并登录；Dock **不代登录** |
| **编排** | 自动开发需 Cursor API Key；Codex 审查需本机已登录 Codex |

> **macOS 提示：** CI 产物暂未 Apple 公证。若提示「已损坏」，终端执行 `xattr -cr ~/Downloads/AgentDock-0.1.11.dmg`，或对 app **右键 → 打开**。

---

## 两种模式

标题栏一键切换。默认 **控制台**；切到编排 **不会关掉** 已打开的终端。

### 控制台

左侧挂项目，按 OpenCode / Grok / Kimi / Claude Code / Pi / DeepSeek 扫描 session，点开即在内嵌终端续聊（DeepSeek 走 Web 嵌入）。

| 能力 | 说明 |
| --- | --- |
| **文档栏** | Grok / Kimi 写出的 plan / spec / doc，可预览 Markdown（含 Mermaid），可引用对话片段 |
| **城市时钟** | 顶部栏按城市看当地时间；搜索、大洲 / 国家分类；自选城市本机持久化 |
| **Grok 多账号** | 切换账号会关掉旧 Grok 终端，避免混用身份 |
| **Claude 多账号** | 切换账号会关掉旧 Claude 终端，避免混用身份 |
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

<a id="打包"></a>

## 打包

**必须在对应系统上打包**，不能从 Windows 打出 `.dmg` / `.AppImage`。产物把界面打进二进制，不再监听开发端口，落到 `release/`。

版本号读 `package.json` 的 `version`。当前 `0.1.11` 会打出 `AgentDock-0.1.11-Setup.exe` 等文件名。

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

<a id="开发"></a>

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
