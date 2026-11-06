# postTool 实现进度与验证记录

2026-11-05，实施中。当前已具备主要调用链路，**尚未完成需求清单的全部功能与性能验收**。完整范围见 [核心复刻清单](superpowers/specs/2026-11-05-posttool-core-design.md)。

## 已接入的功能

| 模块 | 当前状态 |
| --- | --- |
| 项目入口 | “使用量”右侧 `postTool`；独立前端分包，首次进入才加载 |
| HTTP | 自定义方法、重复 Query/Header、JSON/XML/文本/表单/多文件/二进制上传；真实取消和超时 |
| 网络配置 | 直连/系统/自定义代理、TLS 校验开关、PEM CA 和客户端身份、重定向、Cookie Jar 开关 |
| 响应 | 状态、大小、总耗时、首字节耗时、Headers、受限文本/JSON 预览、完整响应保存；流式文本增量读取 |
| 集合 | 集合/嵌套文件夹/请求保存，父级鉴权、集合变量、前后置脚本编辑，分页搜索与删除 |
| 环境 | 环境/全局/集合/运行数据/局部变量，嵌套展开与循环检测，敏感值隐藏 |
| 持久化 | SQLite 分页，文档 AES-GCM 加密，Windows 主密钥通过 DPAPI 保护；批量导入单事务回滚 |
| 鉴权 | No Auth、继承、Basic、Bearer、API Key、OAuth2 Code + PKCE / Client Credentials / Password / 手动刷新 Token |
| 脚本 | QuickJS WASM 独立 Worker；核心 pm 变量/请求/响应/断言/派生请求/流程 API，独立堆和执行预算 |
| Runner | 本机串行执行、CSV/JSON 数据、迭代、间隔、暂停/停止、失败停止、跳转请求；保留最近 200 条结果摘要 |
| 迁移 | 常用 cURL、Postman v2/v2.1、环境/globals、基础 OpenAPI/Swagger JSON/YAML；不支持项会报告 |
| WebSocket | ws/wss、握手 Header、文本/Base64 二进制、Ping、消息筛选/复制、暂停画面、手动断开 |
| Socket.IO | Engine.IO 4 WebSocket 传输、namespace、auth JSON、事件消息、服务器 Ping 响应 |
| MQTT | MQTT 3.1.1、TLS、账号密码、Client ID、Clean Session、发布/订阅/取消订阅、QoS/Retain |
| gRPC | 本地 proto 与依赖目录、Reflection v1alpha、服务/方法选择、Metadata、四种调用、半关闭与 trailers |

所有网络请求由 Rust 发出，浏览器预览不会降级为行为不同的 fetch。网络、磁盘与脚本功能需在桌面应用运行。

## 已落地的资源约束

- HTTP 同时最多 8 个请求；连接配置缓存最多 4 组；完成响应最多 24 份。
- 响应流式落盘，单份 128 MiB、总量 512 MiB；超过上限终止并反馈，释放时回收配额。
- 每次正文 IPC 读取最多 64 KiB；页面文本预览最多 256 KiB。
- 长连接同时最多 4 个，完成连接缓存最多 8 个；单消息 1 MiB；后端消息环形缓冲最多 5,000 条或 8 MiB。
- 长连接前端最多保留 500 条或 512 KiB 文本；页面隐藏/暂停后停止定时读取，不往隐藏 DOM 推送消息。
- 脚本只使用 release 同步版 QuickJS；按需启动 Worker，64 MiB 堆、512 KiB 栈、3 秒执行预算、30 秒总时限，结束/异常/取消都销毁 Worker。
- 脚本派生请求最多 32 次，走同一 Rust 引擎和并发限制；变量展开最多 16 层、4,096 次、2 MiB。
- 请求标签最多 12 个。隐藏后销毁编辑区 DOM；无 HTTP 任务时 30 秒后释放响应和 HTTP 客户端缓存。
- Cookie Jar 独立于 HTTP 客户端缓存，切页释放客户端不会丢失登录 Cookie。最多 512 项、每项 4 KiB；到达数量上限时重置旧项，请求 Cookie Header 最多 32 KiB。
- 历史最多 1,000 条、保留 30 天；普通历史不保留正文与鉴权敏感值，重新调用前需要补齐。
- 响应缓存使用实例目录与 OS 文件锁，崩溃后清理无主目录，避免误删仍活跃实例的文件。
- proto 描述符最多缓存 4 组；目录输入与描述符有大小上限。YAML 在展开前拒绝锚点/别名，防止别名扩张。

## 当前验证记录

前端逻辑测试命令：`rtk npm run test:posttool`。

Rust 测试命令：`rtk proxy cargo test --manifest-path src-tauri/Cargo.toml --lib posttool:: --no-fail-fast`。本环境下载时使用临时 Cargo 镜像配置，未修改全局配置。

已验证项目：

- 变量优先级、循环和过大展开、URL 编码、重复 Query/Header、鉴权头冲突。
- cURL 惰性解析、正文模式、URL fragment、表单编码、代理设置、JSON 文件正文及导入导出。
- Postman 集合嵌套/脚本/多文件映射、OpenAPI 本地 `$ref`、CSV 引号和换行。
- QuickJS 宿主隔离、变量更新、断言、死循环中断、内存分配上限。
- 真实 TCP 取消、HTTP 错误正文与重复 Header、100 MiB 响应流式落盘。
- 加密数据无法直接检出明文、重复 ID 导入整体回滚、无主缓存清理及活跃实例保护。
- 本机 WebSocket echo、Socket.IO 握手、gRPC JSON/Protobuf 映射与四类真实 HTTP/2 调用。
- 本机 MQTT CONNECT/SUBSCRIBE/PUBLISH/接收消息；发送命令不会中断正在推进的握手。
- OAuth 回调的 state 校验、拒绝非 loopback 地址和授权结束回收。
- 1,000 次普通请求结束后，响应文件、任务、缓存配额回到零，连接配置缓存不超过 4 组。

本轮最终结果：18 项 Rust 测试、12 项前端逻辑测试通过；既有布局与类型测试通过；`npm run build` 通过。构建只输出一个约 503 KiB 的 QuickJS release WASM，直到执行脚本才加载运行时。尚未验证打包后 WebView 中的完整脚本/协议流程。

浏览器交互测试：启动 Vite 1426 端口和独立 `posttool` Playwright 会话后运行 `rtk proxy python scripts/run-posttool-ui.py`。测试使用内存 Tauri 适配器，验证界面流程；协议真实性由上面的 Rust 测试验证。

交互测试通过 HTTP 发送/保存、前后置脚本/断言/Console、Runner 三次执行及 200 次显示/隐藏。本轮采样中，强制 GC 后 JS 堆增加 129,816 字节（约 0.12 MiB），事件监听器增量为 -76，DOM 节点增量为 -598。此为隔离 Chrome 组件测试结果，不是整棵 Tauri/WebView 进程树的资源基准，也不能证明所有场景无泄漏。

## 本轮界面调整

- 请求方法改为带明确箭头的下拉框，直接提供 GET、POST、PUT、PATCH、DELETE、HEAD、OPTIONS 与自定义方法；空白自定义方法禁止发送。
- 选择 POST/PUT/PATCH 自动打开 Body；没有请求体时默认 JSON，已有表单等请求体保持原样。自定义方法自动转为大写。
- 统一侧栏、集合列表、请求标签、环境工具栏、URL 输入与响应状态区域；提供深色/浅色配色及轻量 SVG 图标，无新增组件库或常驻监听器。
- 请求与响应区支持拖动分隔条及方向键调整；默认格式化完整 JSON 响应，流式接收中保持原始预览以避免反复解析。
- 浏览器回归逐个验证全部标准方法与 PROPFIND，覆盖表单保留、空白方法拦截、分隔条键盘操作，以及 1440×960、1280×800、1100×640 的深浅两种主题布局，未出现页面溢出。
- 显示/隐藏状态各采样约 3 秒空闲开销，均无新增 IPC 调用；Chrome 主线程 TaskDuration 占采样时长约 0.017% / 0.025%。这是短时组件采样，不能代表桌面应用总 CPU 占用。
- 本轮前端类型检查、12 项逻辑测试、生产构建和浏览器交互回归通过；未修改 Rust 请求引擎。

## 后续必须补齐的清单

1. 发布包整棵进程树 CPU/Private Bytes 基线、1 GiB 上传、长连接两小时、10,000 次 Runner 和更全面的取消/故障压力验证。
2. Digest、OAuth1、JWT 签名、AWS SigV4、NTLM、Hawk、EdgeGrid、ASAP 等鉴权助手与各自协议样本；OAuth 的真实提供商兼容验证。
3. 独立 Cookie 管理器、PFX/P12、更多请求级网络选项，以及长连接代理/自定义证书的完整 UI。
4. SSE 结构化事件、GraphQL Schema/补全/Subscription、Socket.IO ack/二进制/版本矩阵、MQTT Last Will/版本矩阵、gRPC deadline 与更多证书选项。
5. HTML/图片/Hex 预览、响应示例与差异对比、请求标签排序/批量关闭、更多快捷键与布局调整。
6. 全量会话自动恢复、完整本机备份/迁移、集合结构/排序无损导出、更多代码生成与更完整的 OpenAPI 映射。当前 UI 导出以当前请求为主。
7. 逐项脚本兼容矩阵、JSON Schema/常用辅助库、完整运行报告持久化。当前 pm/expect 为明确子集，不能认定任意 Postman 脚本无改动运行。

当前 UI 及主链路可以继续迭代和试用，但不能标记“Postman 核心功能完整复刻完成”。
