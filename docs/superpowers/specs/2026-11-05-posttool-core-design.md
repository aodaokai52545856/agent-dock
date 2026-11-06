# postTool 核心功能复刻清单与资源验收要求

日期：2026-11-05。状态：实施中；具体实现、验证与未完成项见 [进度记录](../../posttool-progress.md)。下列复刻范围与性能预算仍为验收目标，未全部实现或实测。

## 1. 目标与边界

在当前 Agent Dock 中新增 `postTool` 顶层页签，位于“使用量”右侧：

`控制台 | 编排 | 使用量 | postTool`

目标是完整覆盖本清单中的本地 API 调用、调试、测试和管理能力。功能与操作习惯参考 Postman，界面使用项目自身的视觉体系。所有集合、环境、历史、脚本和运行结果保存在本机，正常使用无需 Postman 账号或配套服务器。调用用户指定的远程 API、OAuth 授权端点属于正常请求能力。

“完整”以本清单及后续兼容矩阵为验收依据，不表示兼容 Postman 所有历史版本、插件、脚本依赖和平台服务。P0/P1/P2 表示实施顺序；三阶段内列出的功能均属于本次核心范围。只完成 P0 时只能称为基础可用版。

明确不纳入核心范围：云同步、云存储、团队协作、账号体系、权限与审批、公共 API 网络、在线发布文档、云 Mock、云监控、云调度、AI Agent/AI 生成、MCP 专用客户端。独立命令行、可视化 Flows、抓包代理、浏览器 Cookie 同步、本地 Mock 服务和专门的负载压测平台作为后续扩展，不阻塞 API 客户端核心验收。本机 Runner 的正常批量测试仍在范围内。

功能参考：[请求创建](https://learning.postman.com/docs/use/send-requests/create-requests/create-requests)、[集合与环境等基本元素](https://learning.postman.com/docs/getting-started/basics/postman-elements)。

## 2. 当前项目接入位置

根据当前代码检查：

| 位置 | 已有结构 | postTool 接入要求 |
| --- | --- | --- |
| `src/components/TitleBar.vue` | 三个顶层模式按钮 | 在“使用量”后增加 `postTool`，支持键盘与选中状态 |
| `src/lib/types.ts` | `AppMode`、`parseAppMode` | 增加 `postTool` 模式；旧设置继续可读 |
| `src/lib/store.ts` | `setAppMode` 保存当前模式 | 延续模式记忆，仅保存轻量状态 |
| `src/App.vue` | 控制台、编排、使用量工作区 | 增加按需加载的独立工作区，保留原有 PTY 运行行为 |
| `src/lib/api.ts` | Tauri invoke 封装 | 只增加薄接口，业务状态单独放置 |
| `src-tauri/src/lib.rs` | Rust 模块与命令注册 | 注册请求、取消、分页读取、存储、Runner 等命令 |
| Rust 依赖 | 已有 `ureq`、`rusqlite`、`aes-gcm` 等 | 可复用存储与加密基础；现有网络依赖不等于完整满足多协议及可取消流式请求 |

当前使用量工作区使用 `v-show`。postTool 必须单独管理懒加载、可见性和资源释放，不能以“页面隐藏”代替“后台工作停止”。

建议新增边界：`PostToolPane.vue` 及其子组件、`src/lib/posttool/`、`src-tauri/src/posttool/`。请求正文和响应正文不进入全局应用 store 或 localStorage。

## 3. 需要复刻的功能

以下均为待实现项目。

### F01 工作区与请求标签（P0）

- [ ] 左侧集合 / 历史 / 环境导航，中间请求编辑区，下方响应区；面板可调整大小。
- [ ] 多请求标签，新建、复制、关闭、关闭其他、重命名、拖动排序。
- [ ] 未保存标识、保存到集合、草稿恢复、最近打开请求恢复。
- [ ] 发送、取消、重新发送、复制请求、快捷键；运行状态与请求标签对应。
- [ ] 切换顶层页签时保留编辑状态；不重复发送请求、不丢失已启动任务。
- [ ] 冷启动只恢复标签元数据；点开具体标签才加载正文和编辑器。

### F02 HTTP 请求编辑与发送（P0）

- [ ] GET、POST、PUT、PATCH、DELETE、HEAD、OPTIONS 和自定义方法；HTTP/HTTPS，HTTP/1.1 与 HTTP/2。
- [ ] URL、路径变量、Query 参数双向编辑；重复键、空值、启停、排序、批量粘贴与编码控制。
- [ ] Headers 支持重复项、描述、启停、批量编辑；区分用户头与自动生成头，发送前可检查有效请求。
- [ ] Body：none、raw text、JSON、XML、HTML、JavaScript、form-urlencoded、multipart/form-data、binary 文件。
- [ ] multipart 同名字段、多文件、字段级 Content-Type；大文件直接流式读取，不整文件进前端内存。
- [ ] JSON/XML 格式化、语法错误定位、搜索替换；格式化不会擅自改变实际发送正文。
- [ ] SOAP 通过 HTTP + XML + SOAPAction 调用；不要求专门的 WSDL 建模平台。
- [ ] 请求快照记录本次实际生效的参数、环境和配置，避免发送途中编辑影响已发请求。
- [ ] 区分 DNS、代理、连接、TLS、超时、主动取消等错误；HTTP 4xx/5xx 保留完整响应。
- [ ] 取消覆盖连接、TLS、上传、下载和流式读取阶段；不能只停止界面 loading。

### F03 响应查看与调试（P0）

- [ ] 状态码、总耗时、大小、Headers、Cookies、Body；保留重复响应头及原始顺序信息。
- [ ] Raw / Pretty / Preview / Hex；支持 JSON、XML、文本、图片和二进制下载。
- [ ] 搜索、复制、保存到文件、JSON 路径取值、响应值提取为变量。
- [ ] 大 JSON 延迟展开、长列表虚拟渲染；超阈值默认提供局部预览和完整保存入口。
- [ ] 编码识别与手动选择；处理 gzip 等内容解压，并分别限制压缩传输量和解压后数据量。
- [ ] 时间信息尽可能细分排队、DNS、连接、TLS、首字节和下载；无法准确获得的字段显示“不可用”。
- [ ] 本地 Console 显示请求、跳转、脚本输出、断言和错误；敏感值默认脱敏，日志可关闭与清空。
- [ ] HTML/SVG 等预览使用受限沙箱，不执行响应脚本，不自动加载外部资源，不访问 Tauri 能力。
- [ ] 保存响应示例到请求；支持本次与上次/示例响应的按需文本或 JSON 对比，限制对比规模。

响应与 SSE 参考：[API response structure](https://learning.postman.com/docs/use/send-requests/response-data/responses)。

### F04 集合、文件夹、历史与本地存储（P0）

- [ ] 集合、嵌套文件夹、请求的新建、复制、移动、排序、删除；名称、描述、搜索与筛选。
- [ ] 集合/文件夹级公共鉴权与脚本；集合级变量；请求可以覆盖或继承。
- [ ] 历史按时间/集合/方法/状态搜索，可重新打开、保存成请求或清空。
- [ ] 历史保存发送时的快照；草稿恢复不自动重放请求。
- [ ] SQLite 分页与事务；正文单独存文件或分块存储，不随列表查询一并读出。
- [ ] 本地导出、备份、恢复、数据版本迁移；写入失败可见，导入失败不破坏原集合。
- [ ] 集合正文按编辑需要加载；列表不遍历全部历史，不将几千条正文同时做 Vue 深层响应式。

### F05 变量与环境（P0/P1）

- [ ] 全局、集合、环境、运行数据、请求/运行局部变量，支持 `{{name}}` 引用。
- [ ] 同名变量优先级：局部 > 运行数据 > 环境 > 集合 > 全局；发送前可查看变量来源。
- [ ] 环境新建、复制、重命名、切换、启停变量、导入导出；一个请求使用确定的环境快照。
- [ ] 支持 URL、参数、Headers、Body、鉴权中的变量；未定义变量给出定位提示。
- [ ] 常用动态变量：时间戳、UUID、随机数等；替换深度、展开大小有上限，循环引用报错。
- [ ] 普通值与本机敏感值分开；密码/Token 默认隐藏，导出默认不带敏感值。
- [ ] 脚本可读取/更新变量；Runner 中各次迭代的局部数据隔离，共享写入规则明确。

变量作用域参考：[Variables and environments](https://learning.postman.com/docs/sending-requests/variables/variables/)。

### F06 鉴权与网络配置（P0/P1/P2）

- [ ] P0：No Auth、Basic、Bearer、API Key（Header/Query）、继承父级鉴权；生成头与手填头的冲突可检查。
- [ ] P1：OAuth 2.0，含 Authorization Code + PKCE、Client Credentials、Token 刷新；历史协议的 Implicit/Password 流以目标端支持为前提。
- [ ] OAuth 本机回调接收器按需启动，绑定 loopback，校验 state，超时/取消/完成后关闭；支持粘贴已有 Token。
- [ ] P1/P2：Digest、OAuth 1.0、JWT Bearer 签名、AWS Signature V4、NTLM、Hawk、Akamai EdgeGrid、ASAP；分别列协议测试，不把“能手填 Header”当作已实现鉴权助手。
- [ ] 系统代理、自定义 HTTP/HTTPS/SOCKS5 代理、代理鉴权、直连与绕过规则；配置的生效范围可见。
- [ ] 自定义 CA、客户端证书及私钥、PFX/P12、证书口令；TLS 校验开关支持单请求覆盖。
- [ ] 连接/读取/总超时、重定向开关与次数、HTTP 版本、自动解压、Cookie 开关。
- [ ] Cookie Jar 管理域、路径、过期、Secure 等属性；查看、编辑、导入/导出、清空；按本地工作区隔离。
- [ ] 跨域重定向不随意转发 Authorization、Cookie 等敏感头；连接池按代理/TLS/证书配置隔离且有总上限。

鉴权清单参考：[Authorization types](https://learning.postman.com/docs/use/send-requests/authorization/authorization-types/)。

### F07 前置脚本、后置脚本与断言（P1）

- [ ] 请求、文件夹、集合级 pre-request / post-response JavaScript；嵌套层级的执行顺序以兼容测试固定。
- [ ] 核心 `pm` API：request、response、variables、environment、globals、collectionVariables、iterationData、info、test、expect、sendRequest、execution。
- [ ] 状态码、Header、响应字段、JSON Schema、响应时间等断言；通过/失败/异常可定位到脚本和请求。
- [ ] 响应提取 Token/ID 并供后续请求使用；支持异步回调/Promise 和执行结束判定。
- [ ] `pm.execution.setNextRequest`、skipRequest、跳过/结束迭代等流程能力；限制循环步数和递归深度。
- [ ] 常用编码、哈希、签名、日期等辅助能力；明确支持的库与版本，缺少的依赖导入时报告。
- [ ] 单独编写 Postman 脚本兼容矩阵，区分完整支持、部分支持、不支持；不静默吞掉不支持的 API。
- [ ] 超大响应进入脚本前先检查字节上限；无法完整执行 `pm.response.json()` 等操作时显式报错，不对截断数据继续给出“通过”。
- [ ] 脚本在独立、可中断且有堆上限的 JS 运行时中执行；不能在页面主线程直接 eval，也不能暴露 Node、Tauri 或任意文件系统入口。
- [ ] 脚本网络调用走同一请求调度器，继承超时、取消、日志脱敏和并发限制，不能无限派生任务。

脚本参考：[Pre-request and post-response scripts](https://learning.postman.com/v11/docs/tests-and-scripts/write-scripts/intro-to-scripts)、[请求执行顺序控制](https://learning.postman.com/docs/tests-and-scripts/running-collections/building-workflows/)。

### F08 Collection Runner 批量运行（P1）

- [ ] 选择集合/文件夹/请求，调整执行顺序，设置迭代次数、间隔、超时及错误后继续/停止。
- [ ] 本地 CSV/JSON 数据驱动；预览、编码/列名检查、逐条读取，避免整个数据文件复制到渲染层。
- [ ] 默认串行执行，支持有上限的独立任务并发；存在共享变量与 Cookie 的请求链保持顺序，避免结果漂移。
- [ ] 暂停后不再派发新请求，当前请求按既定策略结束；停止会取消在途请求、脚本和重试队列。
- [ ] 实时结果、断言统计、失败定位、耗时分布、变量变化；报告支持本地 JSON/HTML 导出。
- [ ] 默认只保留摘要和失败诊断；完整响应持久化可选，正文落盘，运行结果按页加载。
- [ ] 数千/数万次执行不创建同等数量的驻留组件，不在内存累计全部正文。
- [ ] 重试显式启用且次数有限，非幂等请求不默认自动重试；结果中能区分每次尝试。

Runner 参考：[Collection Runner](https://learning.postman.com/docs/tests-and-scripts/running-collections/intro-to-collection-runs)、[本地数据文件](https://learning.postman.com/docs/tests-and-scripts/running-collections/test-data/working-with-data-files)。

### F09 导入导出与迁移（P0/P1）

- [ ] cURL 导入与导出，覆盖常见 Headers、鉴权、表单、文件、原始正文；区分 POSIX/PowerShell/cmd 引号规则。
- [ ] cURL 只解析为请求，不作为 shell 命令执行；无法表达的选项逐项提示。
- [ ] Postman Collection v2/v2.1、环境和 globals JSON 导入导出；保留目录、脚本、鉴权、描述、示例及变量。
- [ ] Swagger 2.0 / OpenAPI 3.0、3.1 JSON/YAML 导入，生成请求、示例与鉴权配置；`$ref` 解析有深度和大小限制。
- [ ] 导入结果预览、重复处理策略、错误定位、未知字段/不支持特性报告；不显示虚假的“全部兼容”。
- [ ] 上传文件路径、证书私钥和敏感值不混入普通集合导出；迁移后提示重新选择缺失本地文件。
- [ ] Postman 的其他新格式/多协议格式按实际样本建立映射，不能强塞进无法表示它们的 v2.1 格式；postTool 自有备份必须可无损恢复这些请求。
- [ ] 生成 cURL、JavaScript fetch、Python requests、Java HTTP Client 等常用代码片段，按需加载生成器。

迁移参考：[导入数据](https://learning.postman.com/latest-v-12/docs/getting-started/importing-and-exporting/importing-data)、[导出数据](https://learning.postman.com/v11/docs/getting-started/importing-and-exporting/exporting-data)。

### F10 SSE / HTTP 流式响应（P0）

- [ ] SSE 的 data、event、id、retry 和多行事件解析；支持 GET、POST、请求 Body、自定义鉴权头。
- [ ] 增量显示事件与普通 chunked/NDJSON 数据，适用于 AI API；收到字节不等于收到完整事件。
- [ ] UTF-8 跨块解码与 JSON 帧边界正确，不能每个数据块都重新解析全部累积正文。
- [ ] 消息筛选、搜索、复制、暂停画面、清空、保存；暂停画面和断开连接是两个明确动作。
- [ ] 重连策略显式可控，支持 Last-Event-ID；不对非幂等 POST 自动无限重连。
- [ ] 持续流有环形缓冲、字节上限、消息上限和溢出标记；需要保留完整流时有配额地落盘。

### F11 GraphQL（P1/P2）

- [ ] Query / Mutation 编辑、Variables、operationName、Headers、鉴权、响应与 errors 定位。
- [ ] Schema introspection、从本地导入 Schema、按需文档和补全；introspection 被禁止时仍可发送手写请求。
- [ ] GraphQL over HTTP 请求兼容集合与 Runner；Subscription 的传输协议单独列兼容范围。
- [ ] Subscription 支持明确列出的 `graphql-transport-ws` 和 SSE 模式；旧版 `graphql-ws` 以协议样本单独验收。

### F12 WebSocket / Socket.IO（P2）

- [ ] ws/wss 连接、Headers、Query、鉴权、子协议、证书、代理、手动断开与有上限重连。
- [ ] 文本/JSON/二进制消息收发、时间与方向、消息搜索、常用消息保存、导出。
- [ ] Ping/Pong、连接状态和错误可见；断开后释放 socket、心跳、订阅和消息缓冲。
- [ ] Socket.IO 的 namespace、event、ack 与版本兼容范围单独验收，不能以普通 WebSocket 连接代替。
- [ ] 所有持续消息视图复用 SSE 的限额、背压和虚拟列表策略。

### F13 gRPC / MQTT（P2）

- [ ] gRPC：导入 proto 与依赖目录、Server Reflection、服务/方法浏览、JSON 消息编辑、Metadata、TLS/mTLS。
- [ ] Unary、Server Streaming、Client Streaming、Bidirectional Streaming，含 deadline、取消、trailers 和 gRPC 状态码。
- [ ] 大消息大小受限；描述符缓存有上限；不会每次输入都重新编译整个 proto 树。
- [ ] MQTT：连接配置、用户名/密码、TLS、发布/订阅、主题通配符、QoS 0/1/2、retain、会话与 Last Will、手动断开。
- [ ] MQTT 协议版本及代理支持列入矩阵；消息缓存、重连次数、订阅数量均有上限。

多协议参考：[Other API protocols](https://learning.postman.com/docs/use/send-requests/protocols/protocols/)。

## 4. 资源占用判断

目前只能识别风险，不能给出“已低占用”或“无泄漏”的实测结论。使用 Tauri/Rust 本身不保证资源低；是否高占用取决于正文复制、编辑器实例、长连接、脚本与数据保留策略。

| 场景 | 主要风险 | 必须采用的控制 |
| --- | --- | --- |
| 未打开 postTool | 提前加载编辑器、协议栈、历史数据 | 前端代码分包，首次进入再初始化，无常驻轮询 |
| 大响应/文件 | Rust bytes → JSON/base64 → JS string → Pretty 树多份副本 | 后端流式落盘，前端只拿元数据和受限预览；避免正文整体经过 JSON IPC |
| JSON 格式化/搜索 | 主线程阻塞、重复构建整棵树 | 大正文限额、Worker/后端处理、延迟展开、任务可取消 |
| 多标签/大集合 | 每标签一个编辑器、深层代理全部对象 | 单活跃编辑器或小型有界池；正文按需加载，列表分页/虚拟化 |
| SSE/WS/gRPC/MQTT | 无限消息数组、逐字节渲染、无背压 | 有界队列、环形缓冲、批量刷新、慢消费者策略与明确溢出提示 |
| Runner | 全部响应和日志常驻、无限并发 | 串行默认、调度总限额、摘要持久化、正文可选落盘 |
| 脚本 | 死循环、内存膨胀、异步任务遗留 | CPU/堆/总时限、任务树取消、定时器登记和释放 |
| 频繁重连/取消 | socket、句柄、线程、事件监听积累 | 状态机与幂等销毁，覆盖所有结束路径，检查资源计数 |
| 历史与临时文件 | RAM 和磁盘持续增长 | 条数/年龄/字节三重配额；启动时清理遗留临时文件 |

完整保留 Runner 响应本身就是明显的资源风险；Postman 官方也说明大型运行可能耗尽内存，并提供关闭响应保留和日志的选项。[参考](https://learning.postman.com/docs/tests-and-scripts/running-collections/intro-to-collection-runs)

## 5. 实现约束与默认限额

下列数值是初始设计值，允许经过基准测试调整；所有修改必须留下测试依据。设置界面只暴露用户需要的超时、并发、保留策略等选项。

1. 网络、文件、存储、协议解析由 Rust 侧处理，UI 不承担跨域请求代理；采用能真实取消在途 I/O 的实现。不得为每个请求创建一个永久线程或独立进程。
2. 活跃普通 HTTP 请求默认总上限 8，Runner 默认串行且共用此上限；持续连接默认总上限 4。排队数量也有限额。按 TLS/代理隔离的连接池使用 LRU/TTL 淘汰。
3. 正文内联预览默认最多 2 MiB，语法高亮默认最多 1 MiB；全部标签的正文预览缓存默认最多 32 MiB。超限后仍可流式下载或分页读取。
4. 大响应、上传、导出走流式 I/O。有界二进制分块通道或后端文件切片读取，不以整段 base64 模拟流式传输。IPC 的队列、未确认块数也必须受限。
5. 网络接收缓冲、单请求预览缓冲、前端队列分别有上限。不能因为响应未知长度就持续扩容；压缩炸弹、超大事件和单行数据同样受限。
6. 持续消息每连接最多保留 5,000 条或 8 MiB，任一先到即淘汰最早消息。界面活动时最多每 50 ms 合批刷新；隐藏时停止 DOM 更新，只保留有界摘要。
7. 脚本默认一次执行一个运行时，堆上限 64 MiB、CPU 执行预算 3 秒、总时限 30 秒；网络等待与 CPU 时间分别计量。可配置但设产品硬上限，超限必须真正中断。
8. 脚本运行时应支持中断与堆约束。若选型需要隔离子进程，则按需启动、数量受限、空闲退出；这部分开销包含在整棵进程树预算中。
9. 历史默认最多 1,000 条、保留 30 天；自动响应缓存与临时文件总配额默认 512 MiB，下载中预留配额，磁盘不足时停止并报错。用户显式保存的集合/示例/导出文件不按临时缓存误删。
10. 响应临时文件的清理遵循引用计数/租约：关闭最后一个引用才可删除。退出异常后，下次启动回收过期孤儿文件。
11. 离开 postTool 不自动中断用户已发出的请求、Runner 或长连接；界面展示后台任务摘要，提供“一键停止全部”。无任务时停止刷新/心跳/扫描，30 秒内释放编辑器和大缓存，仅保留轻量草稿。
12. 每个任务有唯一 ID、取消句柄和所有者；完成、失败、超时、取消、关闭标签、关闭窗口、异常恢复均走统一幂等回收流程。关闭带任务的标签需要明确停止或继续后台，不能留无主任务。
13. 注册的事件监听、Observer、定时器、Worker、Object URL、文件句柄和 OAuth 回调端口都有对应释放点。Vue 组件销毁必须取消订阅，但不能误杀明确继续后台的任务。
14. 敏感值不进入普通历史、Console、报告和默认导出；复用已有加密基础前检查密钥保护方式，Windows 可复用 DPAPI 路线，其他平台单独验证密钥保护与文件权限。

## 6. 性能验收预算

以下均为目标门槛，尚无实测数据。最终先记录硬件/系统/WebView 版本、Release 包、测试负载和基线，再逐项给出结果；不得用开发模式代替发布包数据。

统计 Agent Dock 主进程、其 WebView 子进程以及新增脚本进程的总 Private Bytes/私有内存和 CPU。CPU 统一按“一个逻辑核满载 = 100%”计算；资源比较使用相同控制台/终端/使用量负载的前后差值，排除现有任务变化造成的误判。

| 场景 | 初始目标 |
| --- | --- |
| 从未进入 postTool | 增量私有内存 ≤ 5 MiB；无新增常驻任务/进程/定时扫描；启动耗时增量中位数 ≤ 50 ms |
| 空白页首次打开并稳定 | 增量私有内存 ≤ 60 MiB；无任务的 60 秒平均增量 CPU ≤ 单逻辑核 1% |
| 切出页面且无活跃任务 | 30 秒后增量私有内存目标 ≤ 25 MiB；60 秒平均增量 CPU ≤ 单逻辑核 0.5% |
| 典型工作区 | 1,000 个请求元数据、10 个标签、当前响应 ≤ 1 MiB，增量私有内存目标 ≤ 120 MiB |
| 集合/历史操作 | 10,000 条条目下只渲染可见行；本地切换/搜索 p95 ≤ 150 ms，不含请求网络时间 |
| 大文件传输 | 单请求 100 MiB 下载与 1 GiB 上传不整份进内存；相对小文件场景峰值额外私有内存目标 ≤ 64 MiB |
| UI 交互 | 活跃可视页面发送/取消立即反馈；滚动、输入、切换 p95 主线程单任务 < 50 ms，不能被网络/格式化阻塞 |
| 请求取消 | 本地可控测试端点中，取消后 1 秒内请求/脚本进入终态；最终 socket/任务计数不随重复取消增长 |

高吞吐下载、复杂脚本和主动批量运行会消耗 CPU，不能要求工作时占用始终接近零。此时重点验收并发有上限、内存有界、UI 可响应，以及停止后资源能回落。纯内存分配器高水位不等于泄漏，必须结合堆对象/资源计数和稳定趋势判断。

## 7. 泄漏与稳定性验证

| 测试 | 操作与通过条件 |
| --- | --- |
| 页面生命周期 | 连续进入/离开 200 次，重复开关 20 个请求标签；空闲回收后监听器、Worker、编辑器实例回到预期数量 |
| 普通请求 | 本地服务器执行 5,000 次请求；每 500 次取样，响应缓存受限、正文不随次数增长 |
| 大响应 | 1/10/100 MiB JSON、深层 JSON、长单行、二进制、无 Content-Length、压缩后小但解压后大等场景；到达限额能降级或中止 |
| 上传/下载取消 | 在连接、TLS、上传、响应头、下载各阶段取消/超时/关闭标签，共 500 轮；不遗留任务、文件、锁和句柄 |
| 长连接 | 4 个连接运行 2 小时，包含空闲与 100 条/秒负载；缓冲容量稳定，主窗口可操作；服务端断开/重连不累积连接 |
| Runner | 10,000 次执行，分别开启/关闭完整响应保留；结果按页读取、内存不随总次数线性增长 |
| 脚本故障 | 死循环、巨量分配、循环 sendRequest、定时器、异常、超时 Promise；均在预算内终止，后续请求仍可执行 |
| 导入与存储故障 | 大集合、损坏 JSON/YAML、循环引用、磁盘满、写入中断、应用强退；集合不半写，重启不自动重放请求 |
| 系统资源 | 记录进程树、线程数、句柄/FD、socket、JS heap、Rust 任务数、缓存与磁盘字节；结束后恢复到基线加已说明的有界缓存 |

内存趋势评估先预热、触发正常缓存淘汰，再观察同样负载的稳定窗口。最后五个同规格批次的空闲后私有内存极差目标 ≤ 20 MiB，JS 活对象与活动资源计数无持续上升；超出时结合快照定位，不以“重启后恢复”判定通过，也不以一次内存回落宣称无泄漏。2 小时测试是发布门槛之一，不能证明所有情况下绝无泄漏。

## 8. 功能与兼容验收

- 请求测试使用本机可控 HTTP/TLS/代理/协议服务，检查服务端实际收到的方法、编码、重复参数、Header、Body 和文件哈希。
- 覆盖多字节字符、URL 编码、重定向、Cookie 路径/域、证书错误、鉴权刷新、网络中断和代理鉴权。
- Collection/Environment 导入 → 编辑 → 导出 → 再导入，核对目录、变量、鉴权、脚本、示例和未知字段报告。
- 选择有代表性的 Postman 集合作为兼容样本：登录取 Token → 环境变量 → 后续请求 → 断言 → 数据迭代 → 失败报告，保存版本与预期结果。
- 多协议、鉴权算法、脚本 API、辅助库、导入格式分别维护矩阵，逐项给出版本、支持程度、限制和测试证据。
- 单元测试验证解析/优先级/调度/取消等规则；集成测试验证真实协议和存储；发布包 UI 验证标签、编辑、任务反馈与生命周期。性能测试单独给出数据，不能以构建通过替代。
- 原有控制台 PTY、编排、使用量刷新、模式记忆和窗口操作在新增页签后通过回归；postTool 请求不能阻塞它们。

## 9. 建议实施顺序

| 阶段 | 交付范围 | 完成定义 |
| --- | --- | --- |
| P0 基础与资源架构 | 页签接入、HTTP、请求/响应、集合/历史、基础环境与鉴权、cURL/集合迁移、SSE、存储和取消 | 日常调试可用；先通过空闲、大响应、连续取消及基本释放验证 |
| P1 自动化与迁移 | 完整变量层级、OAuth2、脚本/断言、Runner、OpenAPI、代码片段、GraphQL 基础 | 导入代表性集合能完成请求链和数据驱动测试；Runner/脚本资源预算通过 |
| P2 协议与兼容补齐 | WebSocket/Socket.IO、gRPC、MQTT、GraphQL Subscription、其余鉴权与脚本兼容项 | 核心清单和兼容矩阵逐项关闭，完成进程树资源采样、长连接和稳定性验证 |

每个阶段均需要附上“已完成 / 部分支持 / 未完成”表。只有核心清单全部实现并通过功能、资源和兼容验收后，才标记本次核心复刻完成。

首个实现验证应先证明四件事：未进入 postTool 几乎不增加开销、100 MiB 响应不整份进入 WebView、取消能停止真实 I/O、重复开关页面不积累资源。确认资源架构成立后，再持续补齐功能，避免最后返工。
