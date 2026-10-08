# ClipRiva v0.9 实施与验收计划

- **交付姿态：** Local Link Preview / Alpha 工程候选
- **当前单机工程验收：** **通过（2026-07-28）**；仅覆盖下述冻结检查点，不是完整 v0.9/Beta
- **当前 Beta 总门槛：** **No-Go**；签名双真机物理证据尚未完成
- **范围：** 真实双 Mac 的单条 UTF-8 文本交接闭环
- **不授权：** push、tag、签名、公证、发布或 Beta 文案

## 长期目标结果

完整 v0.9 目标把 v0.8 的安全候选边界推进为真实 listener、Bonjour、双方配对、认证加密、
Offer/Body/Receipt/Cancel、原生安全接收视图和内容无关 Transfers。它不扩展图片、文件、富文本、
自动同步、账户、云中继、离线队列或跨平台。

实现完成不等于 Beta。只有同一签名候选 Git SHA 通过自动化、同机双进程和完整双真机矩阵，且
绝对 No-Go 为 0，才可在后续独立发布变更中评估 Beta 文案。

### 本轮冻结工程检查点

本轮不激活 production listener/Bonjour，不创建真实 trust/session，也不投递正文。检查点范围是：

- 严格 pairing/Receipt/Ack/Status wire schema 与 first-terminal-wins 纯状态机。
- 独立于 WebView 轮询的 60 秒 pending deadline worker，以及已有 terminal/disable/revoke/exit
  清理路径。
- `AccessibleWhenUnlockedThisDeviceOnly` Keychain 项、缺失身份 fail-closed 和显式 identity reset。
- 公开 Transfer DTO 无正文/preview、opaque-ID/`void` native reveal seam、terminal summary clear。
- Devices/Pairing/Send/Incoming/Transfers 的内容无关 Preview UI、键盘与可访问性回归。

不在本轮通过项：真实双端 pairing、生产 Receipt 查询循环、listener/Bonjour/权限声明、两个 OS
进程 TCP harness、已显示 native view 的 lock/user-switch/timeout/revoke 强制关闭，以及任一双真机
P 层证据。这些继续 fail-closed / Deferred。

## 两个独立验收结论

本计划必须分别记录以下结论，禁止用其中一个覆盖另一个：

| 结论 | 通过条件 | 当前状态 |
| --- | --- | --- |
| 单机工程验收 | 本轮冻结检查点的适用自动化、静态边界、前端/Rust 完整回归与构建通过；Deferred 项未被误报；无本轮适用 P0/P1 缺陷 | **通过（2026-07-28）** |
| Beta 总门槛 | 单机工程验收通过，并且 P-01~P-20 在同一签名 SHA 的两台真实 Mac 上通过、绝对 No-Go 为 0、release owner 签字 | **No-Go** |

因此，本次任务允许出现“v0.9 Preview/Alpha 冻结检查点单机工程验收通过；完整 v0.9/Beta 总门槛
仍 No-Go”的结论。
同机真实 TCP、两个独立进程、loopback 或 pre-gate harness 的实际配对/传输数应回填到 QA 和
Beta evidence，但必须标记 **非双真机证据**，不得计入 P 层 50/200 样本。

### 2026-07-28 单机执行记录

为适配 8 GB 开发机，本轮始终串行执行，Vitest 固定一个 worker、Node heap 上限 768 MB、Cargo
固定一个 build job；前端、Rust、Vite 和 bundle 从未并发运行。每个较重步骤前后检查系统内存，
观察到可用比例约 31%~43%，没有遗留 Cargo、Vitest 或 Vite 进程。

| 检查 | 结果 | 本轮能证明的范围 |
| --- | --- | --- |
| 定向前端 | 5 文件、61/61 通过；视觉修复后 Local Link 16/16 通过 | UI、浏览器 repository、DTO/IPC 静态边界与文档契约 |
| 前端完整回归 | 15 文件、109/109 通过 | Core + Local Link React 回归 |
| Frontend lint / typecheck / build | 全部通过；Vite production build 通过 | 静态质量、类型与 Web 前端产物 |
| 定向 Rust | Local Link 33/33 通过 | identity、协议、deadline、receipt、storage/DTO 边界 |
| Rust 完整回归 | 110 通过、0 失败、1 个既有 release benchmark 按设计 ignored | Core + Local Link Rust 回归 |
| Rust fmt / Clippy | 全部通过，Clippy `-D warnings` | 格式与静态质量 |
| localhost TCP pre-gate | 单测试进程、两套独立合成 identity、200/200 payload 与 Receipt 故障收敛通过 | 仅 A 层真实 socket/framing 预门槛；不是两个 OS 进程或双 Mac |
| 浏览器 fixture 视觉验收 | 7 张当次截图；Incoming、Reject、Transfers、Devices、Pairing、Send、Not delivered 路径通过；console 0 warning/error | 合成 UI 布局与交互；不证明 native view、VoiceOver、系统权限或网络 |
| 边界与工作树检查 | `git diff --check` 通过；依赖 manifest/lock 无变化 | 无新增依赖、无 license/SBOM 触发项 |

视觉验收曾发现失败终态仍把 “Secure connection / Waiting for recipient” 标为已完成；本轮已修复
为失败发生在 “Secure connection” 且后续步骤保持未执行，并新增回归断言，避免虚构中间阶段。

故意未执行：`pnpm install --frozen-lockfile`（依赖与 lockfile 未变且已安装环境可完成全部回归）、
`pnpm release:verify`（会重复已经串行执行的检查，且本任务不授权 release）、
`pnpm release:bundle:unsigned`（8 GB 机器上的高占用 Tauri bundle，且不能形成签名/双 Mac 证据）
以及 ignored 的 release-mode 10k Quick Paste benchmark（不属于本轮 Local Link 检查点）。

仍为生产 No-Go 的关键项包括：Copy/Save 外部副作用与 Transfer 最终数据库状态尚非事务原子；
已经打开的 AppKit reveal 还没有 lock/user-switch/timeout/revoke/reset 强制关闭、单实例 registry 和
可验证的原生字符串清零；生产 listener/Bonjour/pairing/delivery 与 I/P 层全部未实现或未执行。

## 规范索引与权威边界

| 文档 | 唯一职责 |
| --- | --- |
| [`product/local-link-v09.md`](product/local-link-v09.md) | 定位、范围、七步路径、US-09-01~07、非目标与成功标准 |
| [`design/local-link-v09.md`](design/local-link-v09.md) | 页面职责、视觉、文案、键盘、VoiceOver、通知与原生安全视图 |
| [`design/local-link-v09-state-matrix.md`](design/local-link-v09-state-matrix.md) | Canonical states、合法迁移、Receipt、失败词典与双方投影 |
| [`security/local-link-boundary.md`](security/local-link-boundary.md) | 身份、网络、正文、存储、DOM/IPC、诊断和绝对 No-Go 边界 |
| [`qa/local-link-v09-matrix.md`](qa/local-link-v09-matrix.md) | 自动化、同机双进程和双真机的用例、样本与结论规则 |
| [`release/local-link-beta-evidence.md`](release/local-link-beta-evidence.md) | 同一候选 SHA 的证据记录和维护者 Go / No-Go 模板 |

协议 wire format 和威胁模型仍由 `docs/local-link/PROTOCOL.md` 与
`docs/local-link/THREAT_MODEL.md` 负责；代码激活网络时必须同步更新它们，不能用本计划替代。

## v0.8 基线

截至 2026-07-28，v0.8 已具备：

- 默认关闭和升级不继承网络同意的迁移边界。
- macOS Keychain identity 候选、Noise XX/framing 候选和 Bonjour metadata builder。
- 内容无关 Local Link schema、20 条/24 小时摘要、replay/rate/capacity 状态机。
- Devices、Pairing、Send、Incoming 和 Transfers Preview UI/fixture。
- Copy/Save/Reject 互斥、撤销/disable 清理和 Core 隔离的单元测试基础。

仍未具备：

- 生产 listener、Bonjour browse/publish、两端配对和 remote session lifecycle。
- 真实 Offer/Body/Receipt/Cancel 的端到端适配。
- 原生 AppKit 按需正文视图、系统通知权限和 lock/user-switch lifecycle。
- Receipt 丢失/乱序的幂等终态查询。
- 两独立进程 harness、签名 bundle 和双真机 50/200 证据。

因此 v0.8 历史文档的 M2/M4 No-Go 结论继续有效，不能当作 v0.9 已交付证据。

### v0.9 当前工程检查点实现

本轮在继续 fail-closed 的生产适配器外完成：

- `Receipt/ReceiptAck/StatusQuery/StatusResponse`、pairing proposal/confirm/commit 的严格 finite
  schema，以及 first-terminal-wins、丢失/重复/乱序收敛纯状态机。
- 同一进程内两套独立 identity 通过真实 localhost TCP I/O 传 200 条合成 UTF-8 payload 的
  pre-gate harness；它属于 A 层协议预检，不是两个 OS 进程、更不是双真机证据。
- 自主 pending deadline worker、Keychain `AccessibleWhenUnlockedThisDeviceOnly`、缺失 identity
  fail-closed、显式 reset-to-`needsRePairing`、terminal-summary clear。
- 公开 Transfer DTO 删除 `preview`；WebView reveal 只传 opaque transfer ID 并接收 `void`；
  AppKit `NSAlert` seam 使用短生命周期 zeroizing owner。
- 紧凑 Devices/Pairing/Send/Incoming/Transfers Preview UI、44 px 目标、焦点/快捷键、内容无关
  诊断 ID 与对应自动化。

仍未完成：生产 listener/Bonjour/远端 pairing/delivery、两个 OS 进程 harness、生产 Receipt 重试
循环、通知/Local Network 权限、已打开 native view 的 lock/user-switch/timeout/revoke 强制关闭、
真实 Keychain 签名/重装行为、VoiceOver/人工视觉和 P-01~P-20。因此完整目标仍 No-Go。

## 完整目标实施顺序

以下 Phase 是完整 v0.9 路线，不表示本轮全部完成。本轮只验收上文冻结检查点中适用的子项；
网络、系统 lifecycle 和 I/P 层必须保持 Deferred/No-Go。

### Phase 0｜规格冻结

目标：产品、设计、原生、安全和 QA 对同一范围、状态与 No-Go 达成一致。

必须完成：

- 评审本目录七份 v0.9 文档；状态与 Receipt 只保留一个权威矩阵。
- 冻结 Save 遇到本地隐私策略时的结果、通知拒绝 fallback、诊断导出范围和 Receipt 查询期限。
- 将 v0.9 实际 wire/state 变化同步到 protocol、threat model 和错误白名单。
- 标记产品文案为 Preview/Alpha，任何 Beta 文案测试在物理 gate 前应失败。

退出条件：G0 Spec 评审通过；不再新增内容类型、平台、账号或同步范围。

### Phase 1｜EPIC-01 Transport Foundation

目标：在显式同意后激活真实原生网络生命周期，同时保持 Core 与 WebView 隔离。

工作项：

- listener、Bonjour browse/publish、瞬时 endpoint cache、enable/disable/exit lifecycle。
- 准确最小的 Local Network/Bonjour bundle 声明和权限允许/拒绝状态。
- Noise authenticated session、bounded framing、peer pin、version/TTL/replay/rate/capacity。
- Offer/Body/Receipt/Cancel 和内容无关终态查询；没有 plaintext/legacy fallback。
- 两独立进程、独立数据库/测试 identity 的故障注入 harness。

自动验收：A-01~A-10、A-18、A-20、A-29、A-30 和 I-01、I-03、I-06~I-10。

退出条件：两进程可稳定建立认证 session、传输合成正文、取消并在 Receipt 故障下收敛；本阶段
仍不是双 Mac Go。

### Phase 2｜EPIC-02 Trust & Device Lifecycle

目标：真实双方确认、设备状态、身份变化、撤销和恢复形成单一可信生命周期。

工作项：

- Keychain accessibility/reset/re-pairing 边界和显式错误状态。
- 用户启动的 60 秒 pairing window、transcript-bound SAS、双方确认和临时状态清理。
- Devices 紧凑面板、在线/离线/需重配/阻止、空态和版本不兼容。
- 撤销关闭 session、清除 pending、旧 peer identity 新认证失败。
- 系统权限撤销、sleep/wake、lock/user-switch 事件接入原生状态机。

自动验收：A-05、A-06、A-19、A-23、A-26 和 I-02、I-05、I-08。

退出条件：同机两进程 50 次配对预门槛通过；所有负例创建零错误 trust；真实 Mac 行仍为 No-Go。

### Phase 3｜EPIC-03 Send & Policy

目标：从一个当前选中 Item 创建一次新的、可取消的安全发送。

工作项：

- 在 native transport 前复核 pause、excluded App、sensitive content、类型和 256 KiB 上限。
- Send 的四个用户阶段、在线可信设备选择、倒计时和任一非终态 Cancel。
- 失败文案同时说明原因、是否送达和下一步；工程 raw error 不进入 UI。
- 重试回到当前 Item 并创建新 session/transfer/nonce；Transfers 无旧正文 Retry。

自动验收：A-08~A-10、A-18、A-20、A-24、A-30 和 I-03、I-05~I-07。

退出条件：同机双进程 200 条边界文本预门槛通过，零损坏/错 peer/重复，Receipt 故障最终收敛。

### Phase 4｜EPIC-04 Secure Incoming

目标：通知不泄露正文，接收方在原生安全视图按需了解内容并作唯一决定。

工作项：

- 系统通知授权的上下文请求和拒绝后的菜单栏/应用内无正文 fallback。
- Incoming 元数据卡和 opaque transfer ID action。
- AppKit 原生安全视图；正文不进入 React、DOM、Tauri IPC/event 或 fixture。
- Copy/Save/Reject 的精确互斥副作用、self-capture suppression 与双方 Receipt。
- Lock/User Switch/Timeout/Cancel/Revoke/Disconnect/Disable/Exit 的同步视图关闭和 buffer 清理。

自动验收：A-11~A-17、A-20、A-24、A-26 和 I-04~I-09。

退出条件：并发动作 100 组只有一个赢家；DOM/IPC/storage sentinel 为 0；原生视图生命周期测试
通过。单机结果仍不能填写物理 P-09。

### Phase 5｜EPIC-05 Transfers & Diagnostics

目标：用户可确认“发生了什么”，但不会得到第二份内容历史或可关联诊断。

工作项：

- 统一六种 Receipt 和有限失败原因；All/Received/Sent/Failed 过滤。
- 最多 20 条、最长 24 小时的写入时和启动时清理。
- 随机、内容无关诊断编号；不可恢复正文、endpoint、peer identity 或旧 payload。
- 若包含本地诊断包：显式导出、固定字段、导出前说明、无自动上传。

自动验收：A-20~A-22、A-25、A-30 和 I-06、I-09。

退出条件：retention、schema、export allow-list 与 sentinel 测试通过；清除不影响 History/trust。

### Phase 6｜EPIC-06 Design QA & Beta Evidence

目标：在冻结 SHA 上完成视觉、键盘、a11y、打包、隐私、性能与真实双 Mac 证据。

工作项：

- Devices/Pairing/Send/Incoming/Transfers 全状态视觉和键盘/焦点检查。
- VoiceOver、Reduce Motion、高对比度、系统大字体和 44 px 点击区。
- Clean checkout、license/SBOM/vulnerability、最终 Info.plist/entitlements/capabilities。
- 两台真实 Mac 上的 P-01~P-20；50 配对、200 传输、动作、抓包、盘/日志、网络故障和性能。
- 填写 Beta evidence；绝对 No-Go 为 0 后提交维护者决定。

退出条件：G0~G7 全部 Go。只完成 A/I 层时，本 Phase 结论必须仍为 No-Go。

## Epic 与优先级

| Epic | 优先级 | 主要产物 | 关键退出证据 |
| --- | --- | --- | --- |
| EPIC-01 Transport Foundation | P0 | listener/discovery/session/Receipt/Cancel、two-process harness | I-01/I-03/I-06，无 plaintext fallback |
| EPIC-02 Trust & Device Lifecycle | P0 | Keychain、SAS、双方确认、Devices、revoke | I-02 50 次预门槛，零错误 trust |
| EPIC-03 Send & Policy | P0 | send eligibility、阶段、cancel、error copy | I-03 200/200 预门槛 |
| EPIC-04 Secure Incoming | P0 | notification、native reveal、Copy/Save/Reject、atomic winner | DOM/IPC marker 0，竞态单赢家 |
| EPIC-05 Transfers & Diagnostics | P1 | Receipt 摘要、20/24、诊断编号/可选导出 | schema/retention/export allow-list |
| EPIC-06 Design QA & Beta Evidence | P1 | screenshots、a11y、signed bundle、双真机 evidence | P-01~P-20、绝对 No-Go 0 |

EPIC-06 不能在前五项完全稳定前制造“Beta 已完成”证据；fixture 截图只能用于布局 review。

## 自动化测试策略

新增或更新测试必须至少覆盖：

- Rust：resource lifecycle、identity、discovery、protocol/framing、peer pin、pairing、offer pressure、
  state machine、atomic claim、Receipt query、retention、diagnostic schema 和 storage sentinel。
- Native macOS seam：Local Network/Notification 状态、lock/user switch、AppKit secure view lifecycle、
  clipboard/history writer 和 Keychain error mapping。可注入逻辑不替代真实系统测试。
- React：唯一状态投影、用户化文案、入口、modal coordinator、键盘/focus/a11y、DOM sentinel、
  Transfers 无 Retry 和 Core 回归。
- Integration：两进程真实 TCP、独立 identity/data、50 配对、200 payload、fault injection、crash/
  restart 和边界扫描。
- Boundary docs：当前检查点必须继续断言生产 listener 不启动；只有后续真实实现激活网络时，
  才把测试替换为 default-off/explicit consent/disable 生命周期断言。

完整命令与 A/I/P 用例见 [`qa/local-link-v09-matrix.md`](qa/local-link-v09-matrix.md)。

## 物理验收边界

当前工作区只有单机上下文，不能自动完成：

- 两台不同真实 Mac 的 Bonjour/Local Network 权限和 50 次双方 SAS 配对。
- 200 条真实 LAN 内容精确性和 Copy/Save/Reject 各 50 次系统副作用。
- Apple 签名身份下的 Keychain、Local Network、Notification、安装/升级/重装/Gatekeeper。
- AP isolation、防火墙、Wi-Fi 切换、sleep/wake、lock/user switch 和远端 peer restart。
- 物理接口抓包、两机 SQLite/WAL/file/log/diagnostic 扫描和真实 p50/p95。
- VoiceOver/Reduce Motion/系统大字体观察及至少 5 人无指导可用性。

这些行必须在 [`release/local-link-beta-evidence.md`](release/local-link-beta-evidence.md) 保持
No-Go，直到真实环境完成；同机 harness、截图或 unsigned bundle 不能替代。

## 变更跟随项

网络激活、通知、诊断导出、新存储字段、Keychain lifecycle 或 OS permission 变化时，同一任务
必须更新：

- `PRIVACY.md`
- `docs/privacy/data-flow.md`
- `docs/open-source/module-boundary.md`
- `SECURITY.md`
- `docs/local-link/PROTOCOL.md`
- `docs/local-link/THREAT_MODEL.md`
- `src/test/privacyBoundary.test.ts`
- README、CHANGELOG、test inventory、beta matrix 和 release checklist 中与 fail-closed 现状有关的
  事实描述

若 `package.json`、`pnpm-lock.yaml`、`src-tauri/Cargo.toml` 或 `src-tauri/Cargo.lock` 变化，按根
`AGENTS.md` 重跑 Node/Rust license/SBOM；直接依赖变化时更新 dependency inventory。新增或复制
图标、截图、插图、字体或其他素材前先确认再分发权并更新 third-party provenance。

## 验收命令

```bash
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm lint:rust
pnpm test:rust
pnpm release:verify
pnpm release:bundle:unsigned
git diff --check
```

`release:bundle:unsigned` 只生成本地预检 artifact，不满足签名、权限或双 Mac gate。任何故意跳过的
命令必须在 handoff 和 Beta evidence 中说明。

## Rollback 与 fail-closed

- 保留一个独立于 Core 的 default-off transport switch。
- Disable 立即停止 listener/browse/publish、关闭 session/native secure view、清除 pending body；
  不改变 History/Capture/Quick Paste。
- Protocol/version/identity mismatch 不降级，要求更新或重新配对。
- Keychain identity 缺失/损坏不静默替换既有信任。
- 安全、隐私或收敛回归使下一候选保持 transport off 和 Preview/Alpha。
- 失败 Transfer 不保留 payload；重新发送必须回到当前 Item。

## Definition of Done

“本轮冻结检查点开发完成”要求：

1. 代码与七份 v0.9 规格中的本轮适用边界一致，目标-only/Deferred 项已明确标注；相关现有边界
   文档按实际 fail-closed 实现同步。
2. 本轮适用的自动化、静态边界、完整前端/Rust 回归和构建通过；A/I/P 未实现项保留
   Deferred/No-Go，不以 fixture、UnixStream、同机或 unsigned bundle 替代。
3. Privacy、Security、License 影响在 handoff 中明确；触发的 audit 已更新。
4. 工作树只包含任务内文件；使用合成 fixture；无 secret、用户数据、个人路径或生产 endpoint。
5. Rollback 可验证，Core 在 Link off/失败/攻击路径保持可用。

“完整 v0.9/Beta 验收完成”还额外要求：

6. A-01~A-30、I-01~I-10 和 P-01~P-20 在各自规定环境中全部通过；P 层同一签名 SHA 的
   50/200/动作/性能样本达标。
7. 抓包和本机数据扫描无绝对 No-Go；双方 Receipt 一致且所有故障注入收敛。
8. 签名、notarization、clean install/upgrade、SBOM、权限和 a11y 证据完整。
9. [`release/local-link-beta-evidence.md`](release/local-link-beta-evidence.md) 由各 owner 签字，最终
   release owner 明确 Go。

当前工程实现与验收结果必须在任务末回填；无论本轮适用项是否通过，完整 v0.9/Beta 的物理项
尚未完成，所以总门槛保持 **No-Go**。

## 实施 handoff 模板

每个 Epic 完成时报告：

- 改动范围和关联 US/A/I/P ID。
- 实际执行的命令、结果、Git SHA 与故意跳过项。
- Privacy、Security、License、migration、permission 和 rollback 影响。
- 新增/更新的截图仅作为设计证据还是来自真实 native/双机路径。
- 剩余物理 Mac 证据和当前 Go / No-Go。

本计划允许本地、聚焦的 Git commit 记录工程候选；不授权 push、tag、发布、签名或将 Preview 改为
Beta。
