# ClipRiva v1.2 P0 迭代设计与验收用例

- **输入：** `ClipRiva-v1.2-产品优化方案（含配图）.docx`（2026-07-29）及 v1.1 实现审计。
- **本提交范围：** 把已存在的 Local Link Preview / Alpha 能力收口为更可发现、可理解、可核验的文本传递体验。
- **明确不做：** 账号、云同步、远程中继、自动同步、自动粘贴、图片/文件传递、离线正文队列、通知权限或新的遥测。

## 设计结论

v1.2 的主路径是：**History 找回 → Command+Enter 选择设备 → 核对目标/类型/大小/接收方式 → 再次确认发送 → 观察内容无关的回执或恢复动作**。

这不是同步协议。Local Link 继续只允许由用户开启后，向同一局域网内、双向验证过的 Mac 显式发送一个当前文本项；接收方仍必须选择 Copy、Save 或 Reject。Quick Paste 的 Enter 始终只恢复系统剪贴板并关闭浮层。

| 用户故事 | v1.2 落地设计 | 自动化证据 |
| --- | --- | --- |
| US-310 Quick Paste 显式发送 | `Command+Enter` 只进入设备选择；选定目标后出现确认页，第二次确认才创建请求。Enter 不会发送。 | `QuickPasteOverlay.test.tsx`、`LocalLink.test.tsx` |
| US-311 设备中心 | 主导航收为 History / Pinned / Devices；Settings 移到右上角。Devices 显示本地开关、受信在线数量、需要处理的信任状态、最近发送元数据和配对管理。 | `App.test.tsx`、`LocalLink.test.tsx` |
| US-330 配对与信任可理解 | 保留既有双向 6 位短码、短指纹、仅本次 / 30 天 / 长期信任、撤销与重配；设备项明确为 Mac，不以名称或发现记录作为信任依据。 | `LocalLink.test.tsx`、`src-tauri/src/local_link.rs` |
| US-331 发送回执可观察 | 发送确认展示目标、Text、字节数与接收方 Copy / Save / Reject 边界；状态面板不展示正文。可信在线状态仅由已认证会话投影，Copy / Save 成功才更新最近成功发送时间。 | `LocalLink.test.tsx`、`local_link.rs` 单元测试 |
| US-340 隐私解释 | 条目详情显示为何保留、是否固定、保留到期日和来源；被排除的内容继续不落库，只能通过既有设置中的内容无关原因排查。 | `ClipboardInspector.test.tsx`、`CaptureSettingsDialog.test.tsx` |

## 状态与隐私约束

| 数据 / 状态 | 显示或存储规则 | 禁止内容 |
| --- | --- | --- |
| 设备在线 | 仅已完成配对或已认证会话的进程内状态；90 秒后自然回到离线。 | 持久化端点、mDNS 记录、正文或设备跟踪日志。 |
| 最近成功发送 | 仅收据为 Copy 或 Save 时更新 `last_transfer_at`。 | 将失败、拒绝、取消或过期误报成送达。 |
| Recent sends | 仅显示目标显示名、Text 类型、时间和结果。 | 正文、预览、旧 payload 重试、端点、密钥、摘要或文件名。 |
| 留存说明 | 固定项说明“直到移除”；非固定项显示本地到期日期。 | 对敏感规则展示匹配片段，或暗示被排除内容已落库。 |
| 失败恢复 | 关闭或返回原始条目后由用户重新发起一次新请求。 | 自动重放旧正文、隐式离线排队或自动同步。 |

## 自动化验收矩阵

| ID | 场景 / 操作 | 通过条件 |
| --- | --- | --- |
| IA-01 | 打开主窗口，访问 History、Pinned、Devices 和右上 Settings。 | Devices 为一级页面；Settings 不含 Devices tab；页面可显示在线数、信任待处理数与内容无关的最近发送。 |
| IA-02 | 打开 Devices，检查浏览器预览的收件记录。 | 页面不显示请求正文；“Recent sends”只显示结果元数据，Transfers 可按需打开。 |
| QP-01 | 在 Quick Paste 用 ↑/↓ 选中条目后按 Enter。 | 仅复制选中项、关闭浮层；不打开设备流程、不发送、不自动粘贴。 |
| QP-02 | 在 Quick Paste 按 Command+Enter，选目标后按确认。 | 第一次操作只进入选择/确认；确认页显示目标、Text、字节数和接收方决定；第二次明确操作恰好创建一次发送。 |
| QP-03 | 在预览、发送确认、目标选择和普通列表连续按 Esc。 | 预览先返回列表；确认先返回设备选择；设备选择返回 Quick Paste；搜索词和选中项保持；最后 Esc 关闭浮层。 |
| QP-04 | 在 Quick Paste 按 Delete 或 Backspace。 | 不删除本地历史，不出现删除/Undo 副作用。 |
| LL-01 | 对可信 peer 完成认证，再查询设备；模拟 90 秒过期。 | 在线与 last seen 只在内存投影；到期显示离线；SQLite 中原始 last seen 不被认证轮询改写。 |
| LL-02 | 分别提交 Copy、Save、Reject、Cancelled、Expired、NotDelivered 收据。 | 仅 Copy / Save 更新最近成功发送；其余状态不更新且不被报告为送达。 |
| PR-01 | 查看条目详情（普通项与固定项）。 | 普通项显示本地保留日期；固定项显示长期保留原因；不展示敏感匹配或被排除的正文。 |
| PR-02 | 检查 Devices、Transfers、诊断和本地数据库现有测试夹具。 | 不出现 Local Link 正文、预览、端点、密钥、配对码、摘要或旧 payload。 |

## 发布与人工 Gate

自动化通过只完成工程验收，不把 Local Link 升级为 Beta 或 Stable。候选版本仍必须在**同一构建**完成：50 次真实双 Mac 配对、200 条真实文本传输、Copy/Save/Reject 幂等性、网络切换、休眠唤醒、撤销、AP/VPN 隔离，以及抓包、SQLite/WAL、日志和崩溃文件的正文扫描。

若要实现方案中“网络断开后自动恢复回执”或“Local Link 专项诊断导出（错误码、耗时、网络类型）”，需要新增协议会话或诊断存储类型。这两项不包含在本提交；开始前必须同步更新 `PRIVACY.md`、`docs/privacy/data-flow.md`、`docs/open-source/module-boundary.md` 和边界测试，且不得引入正文队列。

## 影响结论

- **隐私：** 未新增网络目的地、账号、遥测、导出类型、权限或正文存储；在线状态为短期内存投影，最近发送只在已经成功的内容无关收据后更新。
- **安全：** 未改变 Noise、配对、Keychain、接收端决定或传输协议；失败恢复始终从原始当前条目重新开始，不能重放旧正文。
- **许可证：** 未新增依赖、第三方代码或资产；不需要 SBOM / 许可证审查更新。
