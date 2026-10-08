# ClipRiva v1.0 实施设计与验收计划

- **交付姿态：** Local Link Preview / Alpha；不是 Beta
- **本轮工程验收状态：** **通过（自动化与同机原生闭环）；真实双 Mac 与 Beta 证据仍未完成**
- **Beta 总门槛：** **No-Go**，直至同一候选版本完成双真实 Mac 证据
- **范围：** 在用户显式开启后，于同一局域网两台已明确配对的 Mac 之间传递一次受策略允许的 UTF-8 文本
- **不授权：** push、tag、签名、公证、发布或 Beta 文案

本计划将 v0.9 评审反馈落实为可实现、可测试的 v1.0 边界。它不增加账号、云中继、远程模型、自动同步、离线队列、图片/富文本/文件传输或跨平台支持。自动化、浏览器 fixture、单机 loopback 与同机双进程结果都不能作为双真实 Mac 的 Beta 证据。

## 实施设计

### 1. 显式网络生命周期

1. 新安装与升级默认关闭 Local Link；打开应用、设置、History、Quick Paste 或 Labs 都不能启动网络。
2. 只有用户显式开启当前版本的 Local Link，原生服务才读取/创建本机 Keychain 身份、绑定私有 TCP listener，并启动 Bonjour/mDNS publish/browse。任一启动步骤失败时，设置必须回滚为关闭，并且不得留下 listener、mDNS 注册或活动会话。
3. 关闭、身份重置和进程退出必须停止 listener、注销/停止 mDNS、断开会话、清除内存 endpoint、配对暂态和未决定正文；关闭不删除本地 History。撤销必须使旧 peer key 不能再认证，并清除该 peer 的未决定正文和活动会话。
4. mDNS 仅发布随机 `ll-<UUID>` 实例/主机标识和 allow-list 的版本/配对可见性元数据；名称、IP、端口和 endpoint 仅在原生进程内存中短暂存在，不能进入 SQLite、Transfers、诊断、日志或 WebView。mDNS 发现不是信任依据。
5. 剪贴板正文的唯一允许外部目的地是用户明确配对的同一局域网 Mac。mDNS 本地多播不携带正文；没有账户目录、Internet 目的地、云中继、自动同步、离线队列或明文/旧协议 fallback。

### 2. 信任、传输与正文边界

| 边界 | v1.0 设计 |
| --- | --- |
| 身份 | 长期 Noise 静态身份只保存在 macOS Keychain，使用 `AccessibleWhenUnlockedThisDeviceOnly` 且不参与同步；私钥不进入 SQLite、日志、诊断或 IPC。缺失身份而仍有可信 peer 时 fail closed，要求重配/显式 reset。 |
| 配对 | 通过 Noise XX 建立认证会话，比较 transcript-bound SAS/指纹并由双方明确确认后才持久化 peer 静态公钥绑定。取消、超时、失败不会创建可信记录。 |
| 传输 | 每次发送从当前、已通过 pause/排除应用/敏感内容策略复核的单个非空 UTF-8 文本生成新的 session、transfer ID 与 nonce；上限 256 KiB。Noise `Noise_XX_25519_ChaChaPoly_BLAKE2s` 加密 TCP 帧；无 plaintext codec 或降级路径。 |
| 接收正文 | 已认证、通过 replay/rate/capacity 检查的正文只存在 native process memory，最长 60 秒，由独立 deadline worker 清理；不得写入 SQLite/WAL、文件、blob、日志、诊断或 WebView。 |
| WebView | 只调用具名 Tauri command 并接收内容无关 DTO。它没有 raw socket、endpoint、key、raw packet、信任锚或未决定正文能力；secure reveal 仅提交 opaque transfer ID，IPC 返回 `void`。 |
| 记录 | Transfers 最多保留 20 条、最长 24 小时的内容无关摘要；replay tombstone 最长 24 小时。两者均禁止正文、preview、ciphertext、digest、endpoint、session key、私钥和旧 payload 重试。 |

### 3. 接收方的唯一副作用

- **Copy：** 只写入 macOS system clipboard，并抑制 ClipRiva 自捕获；不创建 History。
- **Save：** 只写入带 Local Link 来源标记的本地 History；不修改 system clipboard。
- **Reject / Escape：** 清除未决定正文；不写 clipboard 或 History。
- **Cancel / timeout / disable / revoke / disconnect / exit：** 清除未决定正文，并至多留下有限、内容无关终态摘要。重复包、并发动作和重放只能产生一个最终副作用；重试必须从当前选中 Item 新建一次发送。

锁屏、用户切换以及已显示 native secure view 的强制关闭，必须由真实 macOS 集成验收验证；本设计不把它们描述为已由当前代码完成。同样，系统通知权限不是本轮已声明的能力，若后续增加必须单独更新隐私边界和测试。

## 测试用例与证据矩阵

### A｜自动化（代码、协议和静态边界）

| ID | 用例 | 期望结果 |
| --- | --- | --- |
| A-01 | 新安装/升级、打开各 UI 面，未显式开启 | `enabled=false`；无 listener、mDNS、会话或外部正文传输。 |
| A-02 | 显式开启；listener 或 mDNS 任一启动失败 | 仅成功时同时运行 native listener 与 mDNS；失败回滚为关闭且清理已创建资源。 |
| A-03 | 检查 mDNS advertisement、endpoint cache 和持久化 schema | 实例/主机随机，TXT 仅 `v`/`pair`；endpoint 只在内存，禁止出现在 DB、DTO、诊断和日志。 |
| A-04 | Noise 握手、错误帧、未知字段、版本不兼容和抓取的 plaintext sentinel | 仅认证加密帧可进入协议；无 plaintext/legacy fallback，异常 fail closed。 |
| A-05 | 配对的双方确认、错误 SAS、取消与 60 秒超时 | 只有双方确认可创建 trust；其余路径无可信记录且清理暂态。 |
| A-06 | peer pin、重放、重复 Receipt、撤销与重新认证 | 首个终态获胜；重复/旧 identity 不产生第二次动作或 post-revoke 会话。 |
| A-07 | 发送前的 pause、排除 App、敏感内容、类型、UTF-8 与 256 KiB 复核 | 不合格 Item 在正文进入 transport 前被拒绝；UI/快捷键/重试不能绕过。 |
| A-08 | pending body、DTO、IPC、DOM、SQLite/WAL、文件、日志和诊断 sentinel 扫描 | 未决定正文仅 native memory，最长 60 秒；WebView 不能接收 raw socket/endpoint/key/body。 |
| A-09 | Copy、Save、Reject 的并发/重复调用 | 仅一个赢家：Copy 仅 clipboard 且无 History，Save 仅 History 且不改 clipboard，Reject 两者皆无。 |
| A-10 | cancel、timeout、disable、revoke、disconnect、exit 与 identity reset | 清除正文、会话和 endpoint；仅保留允许的终态摘要，History 不被 disable/reset 删除。 |
| A-11 | Keychain 创建、读取、缺失、重置和失败映射 | `AccessibleWhenUnlockedThisDeviceOnly`、不同步；失败保持 off，私钥不越过 native boundary。 |
| A-12 | 20 条/24 小时 Transfers、24 小时 replay tombstone、清除摘要 | 只保留内容无关字段；清除不影响 History、trust 或活动请求。 |

### I｜集成（两独立进程，但仍非双真机）

| ID | 用例 | 期望结果 |
| --- | --- | --- |
| I-01 | 两个独立应用进程、独立数据库与测试身份的 mDNS/TCP 生命周期 | 只有两端均显式开启后才可发现/建立加密会话；关闭一端后另一端不保留可用 endpoint。 |
| I-02 | 两进程配对与 Trust pin 的成功、SAS 不同、取消、超时、重启 | 成功路径双方写入一致绑定；所有负例零错误 trust。 |
| I-03 | 两进程 200 条边界 UTF-8 文本、Receipt 丢失/重复/乱序、Cancel | 内容精确、无错 peer/重复副作用，最终状态收敛且不使用离线队列。 |
| I-04 | 两进程接收 Copy/Save/Reject 和 revoke 竞争 | 每个 transfer 精确一个副作用；revocation 后旧连接/正文不可再使用。 |
| I-05 | 存储、IPC、日志与网络帧扫描 | 不发现正文落盘、WebView body、endpoint/key 或 plaintext payload。 |

I 层可证明实现集成，但它不是两台不同 Mac、不同网络栈或真实系统权限的证据，不能计入下述 P 层样本。

### P｜真实双 Mac 物理验收（Beta 必需）

| ID | 用例 | Beta 通过条件 |
| --- | --- | --- |
| P-01 | 两台真实、独立 Mac 在同一 LAN 上执行配对 | 同一候选 SHA 完成 **50 次**双方明确确认配对，零错误 trust、零未清理暂态。 |
| P-02 | 双机正文传输 | 同一候选 SHA 完成 **200 次**文本传输，含 Unicode、边界大小和失败注入；零损坏、错 peer、重复或虚假“已送达”。 |
| P-03 | Copy/Save/Reject | 各动作及并发/重复动作在真实系统 clipboard/History 上符合唯一副作用。 |
| P-04 | 隐私与加密 | 抓包确认正文只在 Noise 加密帧内；两机 DB/WAL/file/log/diagnostic 扫描为零正文、endpoint、key 和 preview 泄露。 |
| P-05 | 生命周期与网络变化 | disable、revoke、cancel、60 秒 timeout、退出、peer 重启、Wi-Fi 切换、AP isolation、sleep/wake、防火墙，以及 lock/user switch 均清理或安全失败。 |
| P-06 | 原生系统行为 | 真实 Keychain、Local Network、防火墙和 AppKit secure reveal 行为符合预期；若随后引入通知，单独验证其权限和无正文 fallback。 |
| P-07 | 可用性与性能 | 键盘、VoiceOver、Reduce Motion、大字体、44 px 目标、用户可理解的失败文案，以及 p50/p95 指标均记录并评审。 |

任一 plaintext fallback、错误 trust、正文持久化/IPC 泄露、重复 Copy/Save、撤销后会话、失败却标为送达，或 P-01/P-02 样本不足均为绝对 **No-Go**。

## 验收记录

| 结论 | 当前状态 | 允许填写“通过”的条件 |
| --- | --- | --- |
| A 层自动化 | **通过（2026-07-29）** | `pnpm release:verify`、Rust `fmt` / `test` / `clippy`、隐私边界静态测试均通过；前端 115 项、Rust 112 项通过，另有 1 个既有 release benchmark 被标记 ignored。 |
| I 层集成 | **未完成** | 已有同机双 service 的原生 TCP、Noise-SAS、双边确认、Copy receipt 闭环测试；尚未完成两独立应用进程矩阵，且该结论不替代 P 层。 |
| Preview/Alpha 工程验收 | **仅工程级通过** | A 层与同机原生闭环通过、无本轮 P0/P1 自动化缺陷；不得把此结论描述为真实双 Mac 验收或 Beta。 |
| Beta 总门槛 | **No-Go** | 同一 SHA 的 P-01~P-07 完成，尤其 50 次配对与 200 次传输，绝对 No-Go 为 0，并有维护者批准。 |

本文件不记录个人主机路径、真实设备名、IP、端口、mDNS 实例、配对代码、指纹、密钥或正文。网络、权限、存储或协议发生变化时，必须与 `PRIVACY.md`、`docs/privacy/data-flow.md`、`docs/open-source/module-boundary.md` 和相应测试同一任务更新。
