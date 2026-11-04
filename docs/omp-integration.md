# OMP（oh-my-pi）

OMP 是独立于 Pi 的工具。支持原生终端和封装界面两种启动方式，兼容已有设置时默认使用封装界面。本机首个验收基线为 OMP 18.15.0；建议使用稳定版。

新建 OMP 时在“OMP 启动方式”中选择：

- 原生模式：直接运行 OMP 自带的 TUI，使用 OMP 自己的快捷键、命令和多智能体界面。
- 封装模式：通过 RPC 使用 Dock 的聊天面板、子代理查看和自动编排。

“设置 → 终端与工具 → OMP 默认启动方式”控制新会话的默认值。重新打开历史会话时可选择方式，并记住上次选择。切换已打开会话的方式需要先关闭该会话；两种模式共用 OMP 的配置、凭证和历史。原生会话升级后仍按原生方式恢复，不会自动重发任务。

## 安装与更新

在“版本与更新”中检测 OMP 的实际路径、本地版本和最新稳定版。安装入口会下载官方独立二进制到 Dock 托管目录，不要求额外安装 Bun。

已有安装按来源更新：Bun 使用 `bun install -g @oh-my-pi/pi-coding-agent@latest`，Homebrew 使用 `brew upgrade can1357/tap/omp`，官方独立程序使用自身 `omp update`。显式配置的无效路径会报错，不会静默换装另一份程序。更新后重新执行原路径的 `omp --version`，版本未匹配时不会报告成功。

后台更新任务不依赖弹窗。下载有字节进度，结果和日志可回看，失败可重试。官方二进制先核对 SHA-256，替换后无法启动会恢复旧文件。

活动 OMP 会话或登录终端存在时，更新进入“待应用”。确认应用后，Dock 停止相关实例、执行更新并重新打开已保存的会话，不自动重发中断的请求。仅存在于内存、尚未写入磁盘的内容不能恢复。卸载保留配置和历史；外部独立二进制不由 Dock 删除。

## Profiles、登录与代理

新建 OMP 会话时可选择已存在的 profile；设置中可配置默认 profile 和程序路径。未配置时依次沿用 `OMP_PROFILE`、`PI_PROFILE`、`default`。

OMP 自己管理配置与凭证。原生面板的“登录与配置”支持 OAuth、退出登录、打开配置文件位置和终端登录。API Key、自定义模型及 MCP 等设置继续在 OMP 配置中维护。项目代理同时传给模型请求、登录与发现流程；版本中心代理独立控制更新下载。

尚未登录或没有默认模型时，Dock 会打开首次配置面板。先登录，再从上方选择可用模型，即可开始对话。首次配置使用的临时模型仅用于启动 RPC 登录入口，不写入默认设置，也不发送模型请求。启动失败会显示 OMP 的具体错误信息。

不同 profile 的历史与用量分开。Dock 的 OMP 会话 ID 是包含 profile 的不透明引用，不是原始 CLI UUID；恢复时自动解析准确的原生文件。手动运行 CLI 时请使用 OMP 原始会话 ID 或文件路径。

## 日常操作

- 原生模式按顺序处理终端输出与尺寸变化，初始网格与后端保持一致，避免未解析完的画面在错误宽度下换行。官方独立程序直接启动；包管理器的 shell shim 继续使用对应 shell。
- 原生模式顶部的“重绘终端”会重新同步画面，保留输入草稿和正在执行的任务，不向 OMP 发送聊天内容。
- Windows 原生模式在大量输出停稳后自动同步终端视口，修复长回答底部内容被截住的情况；查看历史或选择文本时会延后到返回底部再处理，重绘产生的输出不会反复触发同步。
- 原生面板支持流式对话、图片、思考折叠、工具卡片、审批和提问响应。
- 运行状态展示思考、工具执行、重试及后台任务阶段和时长。30 秒没有新进展时显示等待提示并定期同步原生状态；同步不会重发任务，也不会清除尚未完成的思考内容。
- 运行中发送消息可选择 steer 或 follow-up；队列支持移除与提升。
- 子代理面板可查看内容、发送指令和停止；会话树可恢复分支或派生新会话。
- 回合和文档引用写入输入草稿；“发送到当前聊天”会提交消息。
- 右下角“文”按钮可拖动并记住位置；窗口缩小时会限制在可见区域内，转换面板也会随位置调整。
- 用量读取 OMP 原生持久化记录，含辅助模型调用，并对 fork 继承的记录去重。没有费用数据时显示未定价。
- 编排选择“OMP 原生会话”，绑定已打开的实例。推进既等待对应请求的 `prompt_result`，也等待后台工作全部结束的 `session_settled`；需要回答时显示等待输入。

## 开发验证

```powershell
rtk npm run build
rtk npm run test:fit
rtk npm run test:live
rtk npm run test:flow
rtk npm run test:cite
rtk npm run test:versions
rtk npm run test:usage
rtk cargo test --manifest-path src-tauri/Cargo.toml
rtk cargo test --manifest-path src-tauri/vendor/omp-rpc/Cargo.toml
rtk proxy node scripts/omp-rpc-smoke.mjs <omp-executable>
```

`omp-rpc-smoke.mjs` 创建临时 profile、测试工作区和本地模拟模型，验证真实 RPC、读取工具、ask 往返、回合结束、原生用量落盘及恢复，不使用真实凭证或付费模型。临时目录会输出以便检查。

`scripts/omp-ui-smoke.js` 与 `scripts/omp-version-ui-smoke.js` 可通过 `playwright-cli run-code` 在 Vite 1423 端口运行，所有 IPC 和模型数据均为合成夹具。`src-tauri/examples/omp_rpc_smoke.rs` 单独验证 Windows 无控制台启动时的协议协商与状态查询。
