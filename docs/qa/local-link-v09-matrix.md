# Local Link v0.9 QA 与验收矩阵

- **状态：** Preview / Alpha 候选矩阵
- **当前总结果：** **No-Go**
- **原因：** 当前冻结检查点不激活生产 listener/Bonjour/pairing/delivery；完整 v0.9 与同一签名
  SHA 的双真机证据尚未完成
- **产品规格：** [`../product/local-link-v09.md`](../product/local-link-v09.md)
- **状态规格：** [`../design/local-link-v09-state-matrix.md`](../design/local-link-v09-state-matrix.md)
- **安全边界：** [`../security/local-link-boundary.md`](../security/local-link-boundary.md)

本矩阵把证据分为自动化、同机双进程和双真机三层。三层可以复用合成 fixture 和断言，但不得
互相替代。当前工作区只提供单机上下文；所有物理双 Mac 行必须保持“未执行 / No-Go”，不能用
browser fixture、mock、loopback、同机两个进程、截图或 unsigned bundle 填写通过。

## 证据等级

| 等级 | 允许环境 | 能证明 | 不能证明 |
| --- | --- | --- | --- |
| A · 自动化 | Vitest、Rust unit/property、静态检查、构建 | 状态、解析、边界、DOM/IPC schema、幂等、纯逻辑与回归 | 真实 listener、远端身份、系统权限、真实 Keychain/网络/剪贴板时序 |
| I · 双进程集成 | 同一 Mac 上两个独立进程、独立数据库和测试 identity、真实 TCP loopback | 进程隔离、wire protocol、故障注入、Receipt 收敛和内容精确性预门槛 | 两台 Mac、Bonjour 跨机、签名权限、真实 AP/防火墙/睡眠/用户切换 |
| P · 物理验收 | 两台不同真实 Mac、同一签名候选 SHA、真实网络 | Beta 所需的端到端行为、权限、性能、恢复与隐私证据 | 不替代源码/自动化审查 |

所有 fixture 必须使用清晰合成的正文、设备名、路径和 identifier。证据不得包含真实剪贴板、
Keychain 值、fingerprint、IP/port、个人路径、账号或未经脱敏的 packet capture。

## 本轮冻结检查点执行摘要（2026-07-28）

- **单机工程检查点：通过。** Frontend 15 文件 109/109；Rust 110 通过、1 个既有 release
  benchmark ignored；lint、typecheck、build、fmt、Clippy 与边界测试通过。
- **A 层 localhost 预门槛：通过。** 一个测试进程内两套独立合成 identity 通过真实 TCP socket
  完成 200/200 payload、SAS/pin 与 Receipt 丢失/重复收敛；不计入 I/P 样本。
- **当次 UI 视觉检查：部分通过。** 7 张合成 fixture 截图覆盖 Incoming、Reject、Transfers、
  Devices、Pairing、Send 和 Not delivered；console 0 warning/error。VoiceOver、系统大字体、
  Reduce Motion、高对比度与原生 AppKit 生命周期未执行。
- **完整 v0.9/Beta：No-Go。** Copy/Save 外部副作用与最终 DB 终态仍非原子；reveal 强制关闭
  lifecycle、生产网络、two-process harness、签名 bundle 和 P-01~P-20 均为 Deferred/未执行。

## A · 自动化矩阵

下表是完整 v0.9 目标矩阵，不等于本轮冻结检查点的 DoD。最终回填使用三种口径：

- **适用：** 当前 fail-closed 检查点可以完整证明，实际执行后才能写通过。
- **部分 / Deferred：** 只能证明纯逻辑、schema、fixture 或 seam；行中的生产/系统 lifecycle
  仍未实现，不得整行写通过。
- **目标 / Deferred：** 依赖 listener、真实双方配对、远端状态查询、系统权限或已打开 native
  view lifecycle，本轮不执行。

特别是 A-02、A-06、A-13、A-20、A-29 不能用纯逻辑或 fixture 代替；A-28 当前应继续验证
production fail-closed，而不是删除“不启动 listener”的断言。

| ID | 关联故事 | 场景 | 必须断言 | 当前状态 |
| --- | --- | --- | --- | --- |
| A-01 | US-09-01 | 新安装与 v0.6/v0.7/v0.8 数据迁移 | 真实 transport 默认 off；旧 Preview 同意不迁移；无 key 的旧 trust 变为 `needsRePairing`；迁移原子 | 通过（检查点适用边界） |
| A-02 | US-09-01 | Feature 状态与资源生命周期 | 只有显式 enable+权限允许进入 `enabled`；任一启动失败回滚 listener/browse/publish/session；disable 清理全部 | 部分 / production Deferred |
| A-03 | US-09-01 | Core 与权限边界 | 启动、History、Quick Paste、Settings、Labs 不请求 Local Network/Notification/Accessibility，不启动 Local Link | 通过（fail-closed 检查点） |
| A-04 | US-09-01/02 | Bonjour metadata builder | service type 和 TXT 仅含 allow-list；随机 instance 不含设备名/正文/指纹；endpoint 不持久化 | 部分 / runtime Deferred |
| A-05 | US-09-02 | Identity store | 生成、重启复用、并发访问、格式校验；missing/corrupt/denied fail closed；private key 不进 SQLite/IPC/log | 部分通过；签名 Keychain Deferred |
| A-06 | US-09-02 | 配对状态机 | 同一 transcript/SAS/双方确认才创建 trust；单侧、取消、超时、不匹配、重复、身份变化均为零 trust | 部分 / 真实双方 Deferred |
| A-07 | US-09-02/03 | Noise/framing parser | 正确 peer 成功；tamper、unknown peer、replaced key、downgrade、truncated/oversized、长度欺骗、unknown version 全部拒绝 | 部分通过；production adapter Deferred |
| A-08 | US-09-03 | 发送资格与边界值 | 仅 1 B、普通文本、256 KiB UTF-8 可构造 payload；0 B、256 KiB+1、invalid UTF-8、非文本、policy blocked 在 transport 前阻止 | 通过（纯逻辑/fixture） |
| A-09 | US-09-03 | Offer/body 次序与压力 | locked/busy/incompatible/rate-limited/replay 在接收 Body 前拒绝；每 peer 1、全局 3、每分钟 5 的上限不超出 | 通过（service seam） |
| A-10 | US-09-03/04 | Canonical transfer 状态 | 只允许状态矩阵定义的迁移；终态不可变；`encrypted` 不等于 Delivered；前端不由零散 boolean 拼状态 | 通过（状态/展示） |
| A-11 | US-09-04/05 | 原子赢家竞态 | 100 组并发 Copy/Save/Reject/Cancel/Revoke/Timeout/Lock 只有一个赢家和最多一个副作用 | 部分；外部副作用/终态非原子，No-Go |
| A-12 | US-09-04 | Native reveal API schema | WebView 只发送 opaque transfer ID；响应和 event 不含正文/preview/digest；Reveal 不改终态、不延长 TTL | 通过（opaque seam） |
| A-13 | US-09-04/05 | Reveal 生命周期 | Close/Lock/User Switch/Timeout/Cancel/Revoke/Disconnect/Disable/Exit 全部关闭原生视图并清理 buffer；不能恢复 | Deferred / No-Go |
| A-14 | US-09-04 | DOM/IPC sentinel | 唯一合成 marker 在 DOM、rendered HTML、Tauri IPC/event、WebView logs/devtools fixture 中出现 0 次 | 部分；静态 DTO/IPC 通过，native scan Deferred |
| A-15 | US-09-04 | Copy | 系统剪贴板精确写一次；History 不变；self-capture suppression 消费一次；Receipt `copied` | 部分 / 系统与原子性 Deferred |
| A-16 | US-09-04 | Save | History 精确写一次；系统剪贴板不变；来源按冻结策略；策略阻止时双方得到同一有限终态 | 部分 / 外部副作用原子性 No-Go |
| A-17 | US-09-04 | Reject / Escape | Clipboard/History 均不写；正文清理；Receipt `rejected`；Escape 与按钮语义相同 | 通过（service + UI fixture） |
| A-18 | US-09-03/05 | Cancel、过期与重启 | 每个非终态可取消；60 秒过期；restart 不恢复正文；late frame/ack 不复活终态 | 部分通过；remote late frame Deferred |
| A-19 | US-09-05 | 撤销 | 撤销与接收动作线性化；peer binding/session/pending 清理；旧 key 新认证失败；History 不删除 | 部分通过；真实 session Deferred |
| A-20 | US-09-03/06 | Receipt 丢失、重复、乱序 | receiver 终态幂等查询收敛；不重发 Body、不重复副作用；未收敛不猜测 Copied/Saved/Delivered | 部分；状态机/TCP pre-gate 通过，production query Deferred |
| A-21 | US-09-06 | Transfer schema/retention | 最多 20 条且最长 24 小时；无 body/preview/digest/source/path/endpoint/key/old payload；startup/write 均清理 | 通过（检查点 schema/retention） |
| A-22 | US-09-06/07 | 诊断编号与导出 allow-list | 编号不可还原内容/跨安装关联；导出仅固定字段；sentinel、设备名、指纹、IP/port、路径和 raw error 为 0 | 部分；编号通过，导出 Deferred |
| A-23 | US-09-01/02 | Devices/Pairing UI | 默认关闭、四类设备状态、一个主动作、60 秒倒计时、双方确认、用户化文案、撤销显示短指纹 | 通过（browser fixture；非 trust 证据） |
| A-24 | US-09-03/04 | Send/Incoming UI | 四个用户阶段、Cancel、六种 Receipt、无正文通知、原生 Reveal 意图、Copy/Save/Reject 副作用说明 | 部分通过；native/notification Deferred |
| A-25 | US-09-06 | Transfers UI | All/Received/Sent/Failed 过滤；状态文字+图标；无正文与 Retry；清除不影响 History/trust | 通过（UI + service seam） |
| A-26 | 全部 | 焦点与 a11y | 单一主表面、稳定 Tab/Escape、焦点恢复、44 px、Reduce Motion、状态不只靠颜色、VoiceOver label 完整 | 部分；自动化/视觉通过，VoiceOver 等 Deferred |
| A-27 | Core | Core 回归 | Link off/start/failure/revoke/异常输入不改变 History、Capture、Quick Paste、Recycle Bin、Direct Paste、Labs | 通过（完整自动回归） |
| A-28 | 全部 | 边界文档静态契约 | Privacy/data-flow/module-boundary/Security/protocol/threat/docs 与当前代码一致；不保留“激活后仍不监听”的旧假设 | 通过（当前 fail-closed 契约） |
| A-29 | US-09-01/07 | Bundle 静态检查 | Info.plist 的 Local Network/Bonjour/Notification 声明准确最小；capabilities 不给 WebView socket/Keychain/raw endpoint | Deferred；bundle/权限未激活 |
| A-30 | 全部 | 失败文案映射 | 每个有限 failure 都说明原因、是否送达、下一步；raw Noise/socket/SQL/Keychain error 不进入 UI/log/diagnostic | 部分通过；生产 raw error scan Deferred |

### 自动化执行建议

针对性前端：

```bash
pnpm exec vitest run \
  src/features/clipboard/LocalLink.test.tsx \
  src/features/clipboard/LocalLinkPresentation.test.ts \
  src/features/clipboard/LocalLink.browserRepository.test.ts \
  src/app/App.test.tsx \
  src/test/privacyBoundary.test.ts
```

针对性 Rust：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked local_link
```

完整自动门：

```bash
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm lint:rust
pnpm test:rust
pnpm release:verify
git diff --check
```

`pnpm release:verify` 当前内部 Rust test 未显式 `--locked`，因此仍需单独运行 `pnpm test:rust`。
unsigned bundle 是后续安装预检，不是签名或双真机证据：

```bash
pnpm release:bundle:unsigned
```

## I · 同机双进程集成矩阵

本层需新增可重复 harness。两个进程必须使用独立数据库目录、独立测试 identity、独立 listener
port 和可控时钟/网络故障注入。不得使用真实用户 Keychain、History 或剪贴板。

当前 `protocol.rs` 的 pre-gate 使用一个测试进程、两套独立合成 identity 和真实 localhost TCP，
覆盖 200 条 payload 与 Receipt 故障收敛。它证明 frame I/O 比 `UnixStream::pair()` 更接近 socket
边界，但不是两个 OS 进程，因此只记 A 层预检；I-01~I-10 继续“harness 不存在 / 未执行”。

若本次实现新增了真实 TCP loopback/pre-gate harness，执行人应把实际配对数、传输数、失败注入
数和命令回填到下表。即使使用两个原生进程和真实 TCP，本层仍明确标记 **非双真机证据**。

| ID | 场景 | 样本 | 必须断言 | 当前状态 |
| --- | --- | ---: | --- | --- |
| I-01 | 两独立进程 discovery/listener 生命周期 | 各 20 次 | enable 后可发现；disable/exit 后端口和服务消失；Core 进程不受影响 | 待实现/待回填实际数 |
| I-02 | 双端配对与 trust persistence | 50 次正常 + 负例 | 50/50 正常；双方绑定对方 static key；取消/超时/不匹配/重放为 0 trust | 待实现/待回填实际数 |
| I-03 | UTF-8 完整性与边界 | 200 条 | 中英文、代码、URL、emoji、多行、1 B、256 KiB 逐 byte 相等；零错设备、损坏、重复 | 待实现/待回填实际数 |
| I-04 | Copy/Save/Reject effect adapters | 各 100 次 | 注入的 Clipboard/History writer 只有一个精确调用；Receipt 一致 | 待实现/待回填实际数 |
| I-05 | Cancel/Timeout/Revoke/Disconnect race | 每组合 100 次 | 一个终态；终态后无 writer 调用；buffer 归零 | 待实现/待回填实际数 |
| I-06 | Receipt drop/delay/reorder/duplicate | 各 50 次 | 幂等查询最终收敛；不重发 Body、不重复 effect、不猜测 Delivered | 待实现/待回填实际数 |
| I-07 | Malformed/abuse | 各 50 次 | frame、version、rate、capacity、replay 在边界拒绝；进程/内存保持有界 | 待实现/待回填实际数 |
| I-08 | 进程 crash/restart | 两端各 20 次 | 未决正文不恢复；旧 session/nonce 失效；content-free 终态一致 | 待实现/待回填实际数 |
| I-09 | 数据边界扫描 | 每条终态 ≥20 | unique marker 不进 SQLite/WAL/files/logs/Transfers/diagnostic；Save 只在测试 History 目标出现 | 待实现/待回填实际数 |
| I-10 | 协议性能预检 | ≥200 样本 | 记录 handshake、offer、body、receipt 各阶段；只作性能预警，不填真实 p50/p95 Go | 待实现/待回填实际数 |

即使 I-01 至 I-10 全部通过，发布结论仍是 Preview/Alpha No-Go，直到 P 层完成。

## P · 双真机物理矩阵

### 候选与环境前提

- 两台不同真实 Mac，独立用户数据目录和 Keychain identity。
- 安装同一冻结 Git SHA 构建的签名候选；记录 bundle version/build、macOS、芯片和网络。
- 只使用合成正文、设备名和测试数据库；证据按
  [`../release/local-link-beta-evidence.md`](../release/local-link-beta-evidence.md) 脱敏记录。
- 当前单机工作区不满足本前提，所以下表全部保持 **未执行 / No-Go**。

| ID | 场景 | 样本目标 | 通过条件 | 当前状态 |
| --- | --- | ---: | --- | --- |
| P-01 | 签名安装、首次启动、升级与重装 | 两台，各 5 次 | Gatekeeper/签名符合候选；版本/SHA 一致；无意外权限或数据迁移 | 未执行 / No-Go |
| P-02 | 新安装/升级默认关闭 | 两台 + 10 份旧数据副本 | 无 listener/mDNS/session/权限提示；旧 Preview 开关不继承；Core 正常 | 未执行 / No-Go |
| P-03 | Local Network 与 Notification 权限 | 允许/拒绝/撤销各 10 次 | 在上下文请求；拒绝不联网且有 fallback；签名身份下重启行为稳定 | 未执行 / No-Go |
| P-04 | Bonjour 发现与关闭 | ≥50 次发现窗口 | p95 ≤5 秒；10 秒空态准确；TXT 无敏感字段；关闭/退出后服务消失 | 未执行 / No-Go |
| P-05 | 正常双方配对 | 50 次 | 50/50 在窗口内完成；双方同一 SAS/static peer；错误 trust 为 0 | 未执行 / No-Go |
| P-06 | 配对负例 | 取消/超时/不匹配/同名/身份变化/MITM/重放各 20 次 | 创建 trust 为 0；无降级；临时状态清理；重试使用新 transcript | 未执行 / No-Go |
| P-07 | 文本传输 | 200 条 | 中英文、代码、URL、emoji、多行、1 B、256 KiB 为 200/200 精确；零错设备/损坏/重复 | 未执行 / No-Go |
| P-08 | Copy/Save/Reject | 各 ≥50 次 | 每请求仅一个目的地；Copy 不写 History；Save 不写 Clipboard；Reject 两者不写 | 未执行 / No-Go |
| P-09 | 原生按需显示与通知隐私 | 每路径 ≥20 | 通知无正文；正文仅 AppKit view；锁屏/关闭/超时后消失且不可恢复；DOM/IPC marker 为 0 | 未执行 / No-Go |
| P-10 | Receipt 故障与原子竞态 | drop/reorder/duplicate/race 各 ≥20 | 双方最终一致；零假送达；零重复副作用；无法收敛即 No-Go | 未执行 / No-Go |
| P-11 | Cancel、过期、退出与撤销 | 每阶段各 ≥20 | pending 清理；旧身份 ≤1 秒内无法新认证；无迟到 Clipboard/History 写入 | 未执行 / No-Go |
| P-12 | 锁屏与快速用户切换 | 各 ≥20 | 保留正文前拒绝或原子清理；解锁后旧请求不复活，新请求可用 | 未执行 / No-Go |
| P-13 | 网络与电源恢复 | 断网/AP 隔离/防火墙/Wi-Fi 切换/对端重启/sleep-wake 各 ≥20 | 准确失败、无离线队列/假送达；唤醒后目标 10 秒内恢复 listener | 未执行 / No-Go |
| P-14 | 端到端性能 | ≥200 样本 | 发现 p95 ≤5 秒；首次配对 median ≤45 秒、p95 ≤90 秒；Send→Incoming p50 ≤2 秒、p95 ≤5 秒 | 未执行 / No-Go |
| P-15 | 抓包与本机数据扫描 | 所有终态和失败路径 | payload 为密文；仅文档化 mDNS/peer TCP；marker 不进禁区；无云/明文 fallback | 未执行 / No-Go |
| P-16 | Keychain 生命周期 | first setup/restart/upgrade/reset/reinstall/denied 各 ≥5 | identity 稳定；reset 使全部重配；private key 不进 SQLite/IPC/log/diagnostic | 未执行 / No-Go |
| P-17 | Transfers/诊断导出 | >30 条、跨 24 小时 | 20/24 清理；导出固定字段；无正文/设备真实标识/endpoint/key；无自动上传 | 未执行 / No-Go |
| P-18 | Core 回归与攻击流量 | 全流程 | History/Capture/Quick Paste/Recycle Bin/Direct Paste 正确性和响应性不退化 | 未执行 / No-Go |
| P-19 | 键盘、VoiceOver、Reduce Motion、大字体 | 两台完整路径 | 全键盘可完成；焦点稳定；44 px；状态不只靠色；无裁切/重复动作 | 未执行 / No-Go |
| P-20 | 无指导可用性 | 至少 5 人 | 可完成首次配对和一次发送；能说清 Copy/Save/Reject 副作用和是否送达 | 未执行 / No-Go |

### 物理证据不能被当前环境自动完成的部分

当前没有第二台已配置的真实 Mac、远程测试目标、Apple 签名候选、可控 AP/防火墙测试台或人工
VoiceOver/可用性参与者，因此 P-01 至 P-20 不能填写通过。即便未来可以通过远程脚本辅助执行，
以下仍需人工或半人工观察：双方 SAS 对比、系统权限弹窗、锁屏/用户切换、睡眠/唤醒、AP 隔离、
VoiceOver 朗读、签名/Gatekeeper 行为和无指导可用性。

普通 SQLite/文件扫描也不能证明 macOS swap、hibernation 或 crash dump 永不包含内存；证据必须
保留该平台限定。

## Go / No-Go 门槛

| Gate | Go 条件 | 当前结论 |
| --- | --- | --- |
| G0 · Spec | 产品、状态、Receipt、安全边界和错误词典冻结并共同评审 | No-Go · 文档已形成，待维护者共同签字 |
| G1 · Automated | A-01 至 A-30 全部通过；P0/P1 缺陷为 0 | No-Go · 冻结检查点回归通过，完整 A 矩阵含 Deferred |
| G2 · Two-process | I-01 至 I-10 全部通过，证据明确标为非物理 | No-Go · 待实现/待回填实际数 |
| G3 · Dual Mac | P-04 至 P-14 达到 50 配对/200 传输/动作/性能门槛 | No-Go · 当前单机不可执行 |
| G4 · Privacy/Security | P-15 至 P-17 通过；绝对 No-Go 为 0 | No-Go · 未执行 |
| G5 · UX/Core | P-18 至 P-20 通过；P0/P1 为 0 | No-Go · fixture 视觉/自动回归通过，人工/物理未执行 |
| G6 · Build/Release | 同一 SHA 的签名/打包/升级/SBOM/权限/回滚证据齐全 | No-Go · 未执行 |
| G7 · Approval | 维护者在 Beta evidence 明确签字 | No-Go · 未批准 |

任何明文/秘密泄露、错误 trust、未认证/降级 transport、重复 Copy/Save、撤销后写入/认证、正文
落盘、假送达、双方终态矛盾、未同意即联网或物理证据缺失，均为绝对 No-Go。

## 每次执行记录格式

每个状态更新必须记录：

- Git SHA、bundle version/build、签名状态和是否有未提交改动。
- 测试层级 A/I/P、命令或人工步骤、机器/OS/架构、日期和执行人。
- 原始样本数、通过/失败数、p50/p95 的计算方法和脱敏证据路径。
- 跳过原因、Issue、修复 SHA；修复后必须在新候选上重跑受影响门槛。
- Privacy、Security、License 影响和故意未执行项。

不得把旧 SHA 的证据拼接成新候选 Go。最后一个 feature merge 后必须重新运行受影响的自动、
打包和物理门槛。
