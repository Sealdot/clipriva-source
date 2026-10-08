# Local Link v0.9 状态与 Receipt 矩阵

- **状态：** Preview / Alpha 规范
- **权威范围：** 设备、配对、发送、接收、Transfer 终态、Receipt 与用户文案
- **原则：** 原生状态机是唯一事实来源；前端只做有限投影

本文是 v0.9 状态名称、合法迁移和 Receipt 映射的唯一来源。产品或页面文档不得定义另一套状态；
代码中的枚举值如需与本文不同，必须提供一对一序列化映射并由测试锁定。

本轮工程候选沿用已经持久化并进入 Tauri IPC 的 v0.8 Transfer 枚举：`connecting`、
`encrypted`、`awaitingReceiver`、`copied`、`saved`、`rejected`、`cancelled`、`expired`、`failed`。
下表直接使用这些 wire/storage 名称；“检查发送策略”是创建 Transfer 前的 UI 阶段，不是可持久化
状态。这样升级数据库、Rust DTO 与 TypeScript DTO 不需要维护第二套近义枚举。

当前只有 Transfer 表是已经实现的 canonical native/IPC 状态。Feature、Pairing 与 Incoming 小节
描述完整目标模型；生产 pairing/reveal lifecycle 尚未接成这些状态。Device 的当前映射为：Rust
持久 trust `pairing | trusted | needsRePairing | revoked` 加瞬时 `online`，前端再投影为
`pairing | trustedOnline | trustedOffline | needsRePairing | blocked`。`discovering`/`candidate` 是
目标 discovery 状态，当前只存在于显著标注的 browser fixture。生产 pairing 命令继续 fail-closed，
不得把 fixture 的确认布尔值当作 native canonical state。

## 建模规则

1. 每个对象在一个时刻只有一个 canonical state，不能用多个布尔值推导互相矛盾的组合。
2. `reveal` 是 Incoming 的短暂原生呈现状态，不是正文副本，也不是 Transfer 终态。
3. 每个 transfer 只有一个不可变终态和最多一个副作用。
4. Receipt 由接收端原子决策或双方确认的生命周期终止生成；发送端不得自行猜测终态。
5. 网络/认证失败使用有限 `failure_reason`，正文、raw error、端点或密钥不能成为状态字段。
6. UI 文案必须同时说明是否送达以及下一步；内部枚举不得直接出现在主文案。

## Feature 状态

| Canonical state | 含义 | 用户可见状态 | 允许动作 | 退出条件 |
| --- | --- | --- | --- | --- |
| `disabled` | 用户未同意或已关闭；无 listener/browse/publish/session | Local Link 已关闭 | 开启、查看/撤销已保存信任 | 用户明确开启 |
| `permissionRequired` | 用户已表达开启意图，系统局域网权限未决定 | 需要允许局域网访问 | 请求权限、取消 | 系统允许/拒绝 |
| `permissionDenied` | 系统拒绝局域网访问；无网络生命周期 | 局域网访问已关闭，未开始发现 | 打开系统设置、关闭 | 权限变更或关闭 |
| `starting` | 原生层准备 identity、listener、browse 和 publish | 正在开启 Local Link | 取消 | 全部成功或有限失败 |
| `enabled` | 已显式同意且原生网络生命周期运行 | Local Link 已开启 | 管理设备、关闭 | 关闭、权限撤销、运行失败 |
| `unavailable` | Keychain、listener 或运行依赖失败且已 fail closed | Local Link 暂时不可用，未开始传输 | 重试、关闭、查看原因 | 重试成功或关闭 |

任何失败都必须先停止未完整启动的网络资源，再进入 `unavailable`；不得以“部分开启”继续运行。

## Device 状态

| Canonical state | 用户可见信息 | 允许动作 | 退出条件 |
| --- | --- | --- | --- |
| `discovering` | 正在查找附近设备 | 停止 | 发现候选或 10 秒空态 |
| `candidate` | 附近设备，尚未信任 | 配对、忽略 | 开始配对、消失或超时 |
| `trustedOnline` | 在线 · 已信任 | 发送、查看安全详情、撤销 | 离线、撤销、身份/版本变化 |
| `trustedOffline` | 离线 · 上次在线… | 查看、撤销 | 重新上线或撤销 |
| `needsRePairing` | 设备身份已变化，需要重新配对 | 重新配对、移除 | 成功配对或移除 |
| `blocked` | 已阻止 | 解除阻止、移除 | 用户明确操作 |
| `revoked` | 旧信任已撤销；仅作为原生审计瞬时状态 | 已撤销 | 无 | 从活动列表移除；旧身份永远不能认证 |

设备显示名、IP、端口、Bonjour instance 或在线状态不能把 `candidate` 直接提升为
`trustedOnline`。

## Pairing 状态

| Canonical state | 用户可见信息 | 允许动作 | 合法下一状态 |
| --- | --- | --- | --- |
| `openingWindow` | 正在开放 60 秒配对窗口 | 取消 | `negotiating`、`failed`、`cancelled` |
| `negotiating` | 正在建立安全配对 | 取消 | `waitingBoth`、`mismatched`、`failed`、`expired`、`cancelled` |
| `waitingBoth` | 核对同一配对码；等待双方确认 | 本机确认、取消、展开安全详情 | `waitingPeer`、`confirmed`、`mismatched`、`expired`、`cancelled` |
| `waitingPeer` | 本机已确认，等待对方确认 | 取消、展开安全详情 | `confirmed`、`mismatched`、`expired`、`cancelled` |
| `confirmed` | 已建立可信连接 | 完成 | 无；创建 `trustedOnline` 设备 |
| `mismatched` | 配对信息不一致，未建立信任 | 关闭、重新开始 | 无；必须清理临时状态 |
| `cancelled` | 配对已取消，未建立信任 | 关闭、重新开始 | 无 |
| `expired` | 配对窗口已过期，未建立信任 | 关闭、重新开始 | 无 |
| `failed` | 配对失败，未建立信任 | 关闭、按建议重试 | 无 |

`confirmed` 只能由同一握手转录、同一 SAS、固定双方身份和双方确认共同触发。单侧点击、重复点击
或 WebView 提交的六码不能直接改变信任。

## Transfer 状态

Transfer 状态同时服务发送方和接收方。`direction` 是元数据，不产生第二套状态。

| Canonical state | 阶段 | 发送方投影 | 接收方投影 | 允许动作 |
| --- | --- | --- | --- | --- |
| `connecting` | 发现已信任 endpoint 并建立连接 | 正在建立安全连接 | 不可见 | Cancel |
| `encrypted` | 已认证 peer，Offer 已接受，正在发送加密 Body | 安全连接已建立 | 正在接收请求 | Cancel / 生命周期取消 |
| `awaitingReceiver` | 接收端 native memory 持有正文，等待原子决定 | 等待对方决定 | 收到文本，正文未显示 | Sender Cancel；接收端 Reveal/Copy/Save/Reject |
| `copied` | 终态 | 对方已复制到剪贴板 | 已复制 | 查看结果 |
| `saved` | 终态 | 对方已保存到 History | 已保存 | 查看结果 |
| `rejected` | 终态 | 对方已拒绝 | 已拒绝 | 查看结果 |
| `cancelled` | 终态 | 已取消，正文已清除 | 发送方已取消 | 查看结果 |
| `expired` | 终态 | 对方未在时限内处理 | 请求已过期 | 查看结果 |
| `failed` | 终态 | 文本未送达；显示有限原因和下一步 | 请求未送达或已清除 | 查看原因、重新选择原 Item |

`encrypted` 不等于 Delivered；`awaitingReceiver` 只表示正文可能存在于接收端受控 native memory。
`failed` 是内容无关的持久化枚举，UI 必须把它与有限 `failureReason` 投影为 “Not delivered” 及
具体下一步；禁止把 `final`、`success`、`delivered` 或裸 `failed` 直接当作用户文案。

## Incoming 目标呈现状态

Incoming 的正文呈现完全由原生层维护，只能附着于 `awaitingReceiver`。

当前检查点只实现 opaque-ID/`void` reveal seam；尚无持久或可观察的 `pendingRevealed` native
状态，也没有可被锁屏、用户切换、超时、撤销或 disable 强制关闭的视图 handle。所以下表是网络
激活前必须完成的目标，不能作为当前验收结果。

| Presentation state | 正文位置 | React/WebView 可见内容 | 退出条件 |
| --- | --- | --- | --- |
| `pendingHidden` | native pending buffer | 设备、类型、字节数、剩余时间、动作；无正文 | Reveal、终态、锁屏、超时、关闭 |
| `pendingRevealed` | 同一 native pending buffer + AppKit 安全视图 | 仍只有元数据；无正文或预览 | 隐藏、Copy、Save、Reject、任何生命周期取消 |

Reveal 不复制正文，不重置 TTL，不产生 Receipt，不增加 Transfer 记录，也不允许正文进入 IPC 回包。

## Receipt 枚举与唯一映射

| Receipt | Canonical terminal state | 唯一副作用 | 发送方文案 | 接收方文案 |
| --- | --- | --- | --- | --- |
| `copied` | `copied` | 系统剪贴板写入一次；History 不变 | 对方已复制到剪贴板 | 已复制 |
| `saved` | `saved` | 本机 History 写入一次；系统剪贴板不变 | 对方已保存到 History | 已保存 |
| `rejected` | `rejected` | 剪贴板和 History 均不写 | 对方已拒绝 | 已拒绝 |
| `cancelled` | `cancelled` | 清除正文和会话；无内容副作用 | 已取消 | 发送方已取消 |
| `expired` | `expired` | 清除正文和会话；无内容副作用 | 对方未在时限内处理 | 请求已过期 |
| `notDelivered` | `failed` + 有限 `failureReason` | 无内容副作用；如正文曾进入 pending memory，必须先原子清除 | 文本未送达 | 请求未送达 |

Receipt 只允许 transfer ID 关联、终态、有限失败原因和认证信息；不得含正文、preview、digest、
来源 App、路径、endpoint 或 key。

## 有限失败原因与用户投影

| `failure_reason` | 可进入阶段 | 用户文案模板 | 下一步 |
| --- | --- | --- | --- |
| `transportUnavailable` | `starting`/`connecting` | Local Link 暂时不可用，文本未送达 | 稍后重试；保持 Core 可用 |
| `deviceUnavailable` | `connecting` | 设备已离线，文本未送达 | 检查两台 Mac 的网络和 Local Link |
| `authFailed` | `connecting`/`encrypted` | 无法验证设备身份，文本未送达 | 移除后重新配对 |
| `versionMismatch` | pairing/`connecting` | 对方版本不兼容，文本未送达 | 更新两台 Mac 上的 ClipRiva |
| `deviceNotTrusted` | preflight/`connecting` | 设备未受信任或身份已变化，文本未送达 | 重新配对 |
| `receiverLocked` | Offer | 对方 Mac 已锁定，文本未送达 | 解锁后重新选择原条目发送 |
| `receiverBusy` | Offer | 对方正在处理另一个请求，文本未送达 | 稍后重新选择原条目发送 |
| `rateLimited` | Offer | 请求过于频繁，文本未送达 | 稍后重试 |
| `captureBlocked` | preflight | 此条内容受到隐私保护，无法发送 | 选择允许发送的文本；不要提供绕过 |
| `itemUnavailable` | preflight | 当前 Item 已不可用，未发送 | 重新选择一个仍存在的 Item |
| `unsupportedItem` | preflight | 仅支持 1 B–256 KiB UTF-8 文本，未发送 | 选择符合范围的单条文本 |
| `protocolViolation` | any network stage | 连接不符合安全协议，文本未送达 | 关闭连接；必要时重新配对 |
| `deviceRevoked` | any nonterminal stage | 设备信任已撤销，请求已停止 | 重新配对后重新选择原条目 |
| `duplicate` | any network stage | 此请求已处理，不会重复写入 | 查看 Transfers；需要时从原 Item 新建请求 |
| `payloadUnavailable` | lifecycle/restart | 请求内容已清除，不能恢复 | 回到原 Item 新建请求 |

工程错误必须在进入 UI/Transfers/日志前映射到该白名单。raw socket、Noise、SQL 或 Keychain
错误字符串不得直通。

## 合法迁移

```text
preflight (not persisted) -> connecting -> encrypted -> awaitingReceiver

connecting | encrypted | awaitingReceiver
  -> cancelled | expired | failed

awaitingReceiver
  -> copied | saved | rejected
```

以下迁移非法且必须被原生层拒绝并测试：

- 任一终态回到非终态或变成另一终态。
- `connecting` 直接到 `copied`/`saved`。
- `pendingHidden`/`pendingRevealed` 存在于 `awaitingReceiver` 之外。
- 同一 transfer 产生两个 Receipt 或两个副作用。
- `failed` 之后收到迟到 Receipt 并改写 UI。
- 被撤销的旧 peer identity 新建 `connecting`。

## 原子赢家

Copy、Save、Reject、Sender Cancel、Timeout、Revoke、Disable、Disconnect、Lock、User Switch 和
Exit 竞争同一个 native atomic decision claim。

1. 第一个成功提交的动作确定唯一终态。
2. 其他动作收到 content-free “already final” 结果，不执行副作用。
3. Clipboard/History 写入与终态提交必须在同一线性化边界内，或使用可证明的事务/补偿设计。
4. `Reveal` 不参与赢家竞争，但任何终态必须同步关闭原生安全视图。
5. 崩溃/重启不恢复正文；未完成记录变为 `failed` + `payloadUnavailable`，不能重试旧正文。

## Receipt 丢失、乱序与收敛

- 接收端为每个 transfer 保存内容无关、幂等的终态记录；重复决策不重复副作用。
- 发送端收到乱序终态时只接受经过认证、匹配 transfer/session/peer 且版本兼容的 Receipt。
- Receipt 丢失时，发送端以有界退避查询同一 transfer 的终态；接收端返回同一内容无关结果。
- 查询重试不重发 Body，也不延长接收端正文 TTL。
- 在认证会话内无法收敛时，发送端不得显示 Copied/Saved/Rejected 或笼统 Delivered。该情况在
  Preview 中保持“正在确认结果/无法确认结果”并使 Beta gate No-Go；不得伪造 `not_delivered`。

`outcomeUnknown` 不是允许发布的终态，只是候选构建的阻断诊断状态。Beta 候选必须证明所有规定
故障注入最终收敛到上表六种 Receipt 之一。

## 前端投影约束

- 当前前端接收 `{status, failureReason?, receiverAction?, expiresAt, peerDisplayName, byteSize}`
  之类的 UI-safe 结构；未来如增加 `receipt` 字段必须保持一一映射。不得接收正文、digest、nonce、
  endpoint、fingerprint 或 raw error。
- 同一 canonical state 在 Devices、Send、Incoming 和 Transfers 的标签、图标、色彩语义一致。
- 倒计时由原生 `expiresAt` 投影，前端时钟不能延长 TTL 或改变终态。
- Fixture 必须显著标记 `fixture`，使用合成设备与文本，不得产出 Beta evidence。

## 状态契约测试要求

最低自动化覆盖：

1. 每个非终态的全部允许和禁止迁移。
2. 每个 Receipt 到双方文案和副作用的一对一映射。
3. 100 组并发 Copy/Save/Reject/Cancel/Revoke/Timeout，只产生一个赢家。
4. 重复/迟到/乱序 Receipt 和终态查询收敛，不重发 Body、不改变终态。
5. 每个 `failure_reason` 都说明是否送达和下一步，且 raw error 不进入 UI。
6. `pendingRevealed` 的正文不进入 DOM、IPC、Transfers、日志或诊断，生命周期结束后不可恢复。
