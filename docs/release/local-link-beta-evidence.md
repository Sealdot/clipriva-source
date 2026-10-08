# Local Link v0.9 Beta 证据记录

> **历史证据说明（2026-07-30）：** 本文保留 v0.9 检查点，不能描述当前 v1.5 原生候选。
> 当前实现已包含默认关闭的 Bonjour/TCP/Noise/SAS/文本交付路径，但真实双 Mac、换网与
> 睡眠/唤醒、抓包/存储扫描、签名安装证据仍未完成，因此公开发布结论仍为 **No-Go**。
> v1.5 的需求、自动化和真实设备 Gate 以
> [`../clipriva-1.5-implementation-and-acceptance.md`](../clipriva-1.5-implementation-and-acceptance.md)
> 及 [`../beta-validation-matrix.md`](../beta-validation-matrix.md) 为准；不得沿用下方旧计数。

- **文档状态：** 证据模板
- **默认结论：** **No-Go**
- **当前原因：** v0.9 fail-closed 工程检查点不包含生产 listener/pairing/delivery；签名候选和
  双真机矩阵尚未完成
- **禁止用途：** 本文不授权 push、tag、签名、公证、发布或 Beta 文案

只有 [`../qa/local-link-v09-matrix.md`](../qa/local-link-v09-matrix.md) 的全部适用门槛在同一冻结
候选 SHA 上通过，并由维护者在本文签字后，结论才可改为 Go。空白、`TBD`、自动化、fixture、
loopback、同机双进程、unsigned bundle 或单 Mac 截图都按未通过处理。

## 1. 候选身份

| 字段 | 记录 |
| --- | --- |
| Git SHA | 未冻结；本轮本地工程提交不构成 Beta candidate freeze |
| Branch | `codex/v0-8-local-link-text-alpha`；仅工程检查点 |
| 工作树 | 单机工程验收已核对任务文件；存在维护者自有未跟踪文档，故不满足 Beta clean-checkout 前提 |
| Package / Cargo / Tauri version | 未记录；三者必须一致 |
| Bundle build number | 未记录 |
| 签名 identity 摘要 | 未记录；不得记录证书私钥或完整个人信息 |
| Notarization | 未执行 |
| Artifact SHA-256 | 未生成 |
| 构建日期 / 构建人 | 未记录 |
| Node / pnpm / Rust / Xcode | 未记录 |
| 自动化报告 | 未附 |
| License / SBOM 报告 | 未附 |

若工作树不干净、两台 Mac 安装的 artifact hash 不同，或 feature merge 后未重新构建，本次证据
批次立即作废。

## 2. 规格与边界签字

| 规格 | 冻结 SHA / 评审人 / 日期 | 状态 |
| --- | --- | --- |
| [`../product/local-link-v09.md`](../product/local-link-v09.md) | 未记录 | No-Go |
| [`../design/local-link-v09.md`](../design/local-link-v09.md) | 未记录 | No-Go |
| [`../design/local-link-v09-state-matrix.md`](../design/local-link-v09-state-matrix.md) | 未记录 | No-Go |
| [`../security/local-link-boundary.md`](../security/local-link-boundary.md) | 未记录 | No-Go |
| `docs/local-link/PROTOCOL.md` | v0.9 实现尚未同步评审 | No-Go |
| `docs/local-link/THREAT_MODEL.md` | v0.9 实现尚未同步评审 | No-Go |
| Privacy / data flow / module boundary | v0.9 实现尚未同步评审 | No-Go |

## 3. 测试环境

### Mac A

| 字段 | 记录 |
| --- | --- |
| 合成设备标签 | 未记录 |
| Mac 型号 / 芯片 | 未记录 |
| macOS 版本 / build | 未记录 |
| 用户类型 | 专用测试用户，未验证 |
| Local Network 状态 | 未验证 |
| Notification 状态 | 未验证 |
| Keychain identity 短摘要 | 未记录；只允许测试用不可逆短摘要 |
| Artifact SHA-256 | 未记录 |

### Mac B

| 字段 | 记录 |
| --- | --- |
| 合成设备标签 | 未记录 |
| Mac 型号 / 芯片 | 未记录 |
| macOS 版本 / build | 未记录 |
| 用户类型 | 专用测试用户，未验证 |
| Local Network 状态 | 未验证 |
| Notification 状态 | 未验证 |
| Keychain identity 短摘要 | 未记录；只允许测试用不可逆短摘要 |
| Artifact SHA-256 | 未记录 |

### 网络与工具

| 字段 | 记录 |
| --- | --- |
| AP / 网络类型 | 未记录；不得提交真实 SSID/公网信息 |
| AP isolation / firewall 控制 | 未验证 |
| Packet capture 工具 / 版本 | 未记录 |
| SQLite/WAL/log 扫描方法 | 未记录 |
| 时延采样与 p50/p95 方法 | 未记录 |
| VoiceOver / Reduce Motion / 字体设置 | 未记录 |

当前工作区只有单机上下文，Mac A/Mac B 双机前提未建立，因此后续物理表不得标记通过。

## 4. 自动化与同机双进程结果

| Gate | 目标 | 结果 | 证据 |
| --- | --- | --- | --- |
| Frontend lint/typecheck/test/build | 全部通过 | 通过；15 文件 109/109，lint/typecheck/build 通过 | 本地串行命令；非 Beta artifact |
| Rust fmt/clippy/test | 全部通过 | 通过；110 passed、0 failed、1 ignored，fmt/Clippy 通过 | 本地串行命令；非签名环境 |
| Privacy/security boundary tests | 全部通过 | 检查点静态契约通过 | Frontend 完整回归内 `privacyBoundary.test.ts` |
| State/Receipt/atomic race tests | 全部通过 | 状态/Receipt seam 通过；Copy/Save 外部副作用原子性未解决 | Local Link 33/33；生产仍 No-Go |
| Native secure view DOM/IPC sentinel | 0 泄露 | opaque-ID/`void` 静态 seam 通过；已打开视图 lifecycle 未执行 | 部分 / No-Go |
| Same-process localhost TCP pre-gate | 200 条 synthetic payload + Receipt 故障收敛 | 200/200 通过；仅 A 层、非双进程/双真机 | `protocol.rs` test |
| Two-process pairing | 50/50 + 负例 | 0；harness 未实现 | 未附 |
| Two-process transfer | 200/200 | 0；harness 未实现 | 未附 |
| Receipt fault injection | 全部收敛，0 重复副作用 | 纯状态机与同进程 TCP pre-gate 收敛；production query/effect 未验证 | 仅 A 层部分证据 |
| Unsigned bundle smoke | 可安装预检；不替代签名 | 故意未执行；8 GB 机器避免高占用，且本任务不授权 release | 未附 |

自动化与同机双进程即使全部通过，也不会改变双真机 Gate 的 No-Go。

当前真实 localhost TCP pre-gate 仍在一个测试进程内运行，必须回填为 A 层“单机工程预门槛”，
不要写成 two-process、Dual Mac 或 Beta evidence。单机工程验收可以独立通过，同时本文的 Beta
总门槛继续保持 No-Go。

本轮还故意未执行 `pnpm release:verify`（会重复已经逐项串行完成的门槛）和 release-mode 10k
Quick Paste benchmark（既有 ignored 性能项，非 Local Link 检查点）。依赖 manifest/lock 未变化，
因此没有触发新的 license/SBOM inventory 复审。

## 5. 双真机配对证据

| 指标 | 目标 | 实际 | 结论 |
| --- | ---: | ---: | --- |
| 正常首配 | 50 次 | 0 | No-Go |
| 正常成功率 | >98% 且错误 trust 为 0 | 未计算 | No-Go |
| 配对中位数 | ≤45 秒 | 未计算 | No-Go |
| 配对 p95 | ≤90 秒 | 未计算 | No-Go |
| 取消 | ≥20 次，0 trust | 0 | No-Go |
| 超时 | ≥20 次，0 trust | 0 | No-Go |
| SAS/指纹不匹配 | ≥20 次，0 trust | 0 | No-Go |
| 同名设备/身份变化/MITM/重放 | 各 ≥20 次，0 trust | 0 | No-Go |

原始样本与脱敏证据：未附。

## 6. 双真机传输与接收动作

| 指标 | 目标 | 实际 | 结论 |
| --- | ---: | ---: | --- |
| 合法 UTF-8 文本 | 200/200 精确 | 0 | No-Go |
| 中文 / 英文 / 代码 / URL / emoji / 多行 | 全覆盖 | 未执行 | No-Go |
| 1 B / 256 KiB | 精确；边界外发送前阻止 | 未执行 | No-Go |
| 错设备 / 损坏 / 丢失 / 重复 | 全部 0 | 未测 | No-Go |
| Copy | ≥50；只写 Clipboard 一次 | 0 | No-Go |
| Save | ≥50；只写 History 一次 | 0 | No-Go |
| Reject / Escape | ≥50；两者均不写 | 0 | No-Go |
| Notification 正文泄露 | 0 | 未测 | No-Go |
| Native reveal DOM/IPC 泄露 | 0 | 未测 | No-Go |
| Receipt 丢失/乱序/重复 | 最终一致，零假送达/重复副作用 | 未测 | No-Go |

原始样本与脱敏证据：未附。

## 7. 性能与恢复

| 场景 | 目标 | 实际 | 结论 |
| --- | --- | --- | --- |
| Bonjour 发现 | p95 ≤5 秒；10 秒进入准确空态 | 未测 | No-Go |
| Send → Incoming | p50 ≤2 秒；p95 ≤5 秒；≥200 样本 | 未测 | No-Go |
| Sender Cancel 各阶段 | 无迟到副作用；双方一致 | 未测 | No-Go |
| 60 秒 Timeout | 正文清理，不复活 | 未测 | No-Go |
| Revoke 各阶段 | 旧身份新认证失败；无迟到写入 | 未测 | No-Go |
| Receiver Lock / User Switch | 保留前拒绝或原子清理 | 未测 | No-Go |
| Wi-Fi 断开/切换/AP isolation/firewall | 准确失败，无队列/假送达 | 未测 | No-Go |
| Sleep / Wake / peer restart | 旧正文不复活；listener 目标 10 秒内恢复 | 未测 | No-Go |
| App exit / crash / restart | pending 不恢复，旧 session 失效 | 未测 | No-Go |

原始时序、网络和恢复证据：未附。

## 8. 隐私与安全证据

| 检查 | 目标 | 实际 | 结论 |
| --- | --- | --- | --- |
| Packet capture | payload 为密文；仅文档化 mDNS/peer TCP；无云/明文 fallback | 未测 | No-Go |
| SQLite / WAL | Copy/Reject/Cancel/Failure marker 为 0；Save 只在 History 目标出现 | 未测 | No-Go |
| 文件 / blobs / logs | 禁止 marker、key、endpoint 为 0 | 未测 | No-Go |
| Transfers | 最多 20 条/24 小时；无正文/preview/digest/path/endpoint/key | 未测 | No-Go |
| 诊断编号 / 包 | 固定 allow-list；无正文/设备真实标识/endpoint/key；无自动上传 | 未测 | No-Go |
| Keychain lifecycle | restart/upgrade/reset/reinstall/denied 符合边界 | 未测 | No-Go |
| Default-off / permission denied | listener/browse/publish/session 为 0 | 未测 | No-Go |
| WebView capability | 无 socket/Keychain/raw endpoint/trust-anchor/body | 未测 | No-Go |

磁盘扫描只证明检查范围内未主动序列化；不能声称 macOS swap、hibernation 或 crash dump 永不
含内存。脱敏检查摘要：未附。

## 9. UX、Accessibility 与 Core

| 检查 | 目标 | 实际 | 结论 |
| --- | --- | --- | --- |
| 全键盘路径 | Devices/Pairing/Send/Incoming/Transfers 全部可完成 | Vitest 与 browser fixture 部分通过；原生/VoiceOver 未测 | No-Go |
| VoiceOver | 标签、状态、剩余时间和动作副作用准确 | 未测 | No-Go |
| 44 px / 大字体 / 高对比度 | 无截断、重叠或仅靠颜色 | 44 px 与默认视口截图通过；大字体/高对比度未测 | No-Go |
| Reduce Motion | 无必要长动画或倒计时抢占朗读 | 未测 | No-Go |
| 单一主表面 / 焦点恢复 | 无 modal 竞争；返回安全触发器 | 自动化与 browser fixture 通过；原生窗口未测 | No-Go |
| 5 人无指导任务 | 全部完成首次配对和一次发送 | 0 人 | No-Go |
| Core 回归 | History/Capture/Quick Paste/Recycle Bin/Direct Paste 无退化 | Frontend 109/109、Rust 110 passed 的单机自动回归通过 | No-Go |
| P0/P1 缺陷 | 0 | 生产已知 No-Go：Copy/Save 原子性、reveal 强制关闭 lifecycle | No-Go |

录屏/截图必须使用合成内容并脱敏。证据：未附。

## 10. 打包、许可与发布准备

| 检查 | 目标 | 实际 | 结论 |
| --- | --- | --- | --- |
| 三处版本一致 | package/Cargo/Tauri 一致；版本由 release owner 决定 | 未验证 | No-Go |
| Info.plist / entitlements | Local Network/Bonjour/Notification 准确最小 | 未验证 | No-Go |
| 签名 / Gatekeeper / notarization | 同一候选全部通过 | 未执行 | No-Go |
| Clean checkout / install / upgrade | 可复现，无本地凭据/数据污染 | 未执行 | No-Go |
| Dependency / SBOM / vulnerability | 与最终 lockfiles 一致，无未处理发布阻断 | 本轮 lock/manifest 未变化，未触发重审；最终 release 仍须重跑 | No-Go |
| THIRD_PARTY_NOTICES / asset provenance | 所有新增资产可再分发且已记录 | 无新增仓库资产；使用既有 Lucide 依赖 | No-Go |
| README / Privacy / Security / protocol | 与实际候选一致，未过度宣称 | 边界文档与静态契约通过；维护者评审未签字 | No-Go |
| Rollback | 关闭可停止网络并保留 Core；无降级 transport | fail-closed service 自动化通过；真实 listener 不存在 | No-Go |

## 11. 绝对 No-Go 审核

下列每项必须有证据证明“未发生”，不能以空白视为通过：

- [ ] 明文或秘密未出现在 packet、SQLite/WAL、非 Save 文件、日志、诊断、DOM/IPC 或错误位置。
- [ ] 错误 peer trust 为 0；没有未认证、plaintext 或降级 transport。
- [ ] 重复 Copy/Save 为 0；Cancel/Revoke/Timeout/Lock 后写入为 0。
- [ ] 撤销后旧身份认证为 0；旧正文复活为 0。
- [ ] 失败误报 Copied/Saved/Delivered 为 0；双方矛盾终态为 0。
- [ ] 默认关闭、升级未重新同意或权限拒绝时的网络活动为 0。
- [ ] Receipt 故障均收敛；没有猜测终态。
- [ ] 两台真实 Mac 的 50 配对、200 传输和全部物理矩阵完成。

当前以上项目均未形成同一签名 SHA 的完整证据，因此总结果保持 **No-Go**。

## 12. 最终决定

| 决策 | 记录 |
| --- | --- |
| 当前建议 | 继续 Preview / Alpha；No-Go |
| Product | 未签字 |
| Design | 未签字 |
| Native/Security | 未签字 |
| QA | 未签字 |
| Release owner | 未签字 |
| 决策日期 | 未记录 |
| 已知风险 / 延期项 | Copy/Save 副作用与终态 DB 非原子；native reveal 强制关闭/registry/清零；production network、I/P 层均 Deferred |

只有 release owner 在所有前置签字和证据完整后，才能把“当前建议”改为 Go。更改本文结论不等于
授权发布；仍须完成仓库 `docs/release/release-checklist.md` 的独立门槛。
