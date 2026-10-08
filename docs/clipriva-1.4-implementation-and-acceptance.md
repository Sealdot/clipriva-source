# ClipRiva v1.4 迭代设计、测试与验收方案

- **输入：** `ClipRiva-v1.4-产品优化方案（含配图）.docx`（2026-07-30）与 v1.3 实现。
- **版本定位：** `v1.4 Beta Candidate`。只有真实双 Mac Gate 的所有证据完成后才可以称为 Beta；本次工程验收不替代该 Gate。
- **产品目标：** 把一次本地文本交接收敛为“找回 → 发起 → 决定 → 结果 → 恢复”的明确闭环，同时保持显式发起、接收方决定、60 秒内存期限、无正文持久化与无旧请求重放。
- **不纳入本次实现：** 文件/图片/文件夹传输、账号或云同步、跨公网中继、自动接收/粘贴/发送、多目标发送、离线正文队列、跨重启正文恢复。系统本地通知不在本次 Candidate 实现：它会引入新的 OS 权限边界，待单独设计、测试与隐私审查后再启用；导航徽标和收件中心覆盖 P0 可发现性。

## 设计结论

### A. 单一状态模型与无正文边界（P0）

将面向用户的生命周期统一为：`created → delivered → viewed → copied | saved | rejected | expired | failed | cancelled`。

- `created` 仅表示本机创建了新的请求，不能显示为“已送达”。
- `delivered` 表示接收端已接受请求进入其原生内存；`viewed` 表示接收端明确打开收件中心。两者均不含正文、预览或接收操作细节。
- Copy、Save、Reject、Cancel 与 View 都必须按 transfer ID 幂等；最先确认的终态不可被后续决定覆盖。
- 取消仅在接收方尚未决定时可用；取消、拒绝、过期和应用退出立即清除 pending body，仅保留有限状态元数据。
- `Transfer` / `TransferEvent` 可以持久化内容无关元数据；`PendingPayload` 只存在原生进程内存，最多三条、最长 60 秒。重启绝不恢复正文或待处理队列。

### B. Handoff Sheet（P0，US-470 / US-471）

Quick Paste 和主窗口的“发送”动作共用同一 Handoff Sheet：展示当前条目的一行摘要、按“可发送 → 需处理 → 离线”排序的设备、一个阻塞原因和一个修复入口。

- `Enter` 始终只本机复制；`⌘Enter` 才打开 Sheet；选择设备不创建请求。
- 最近成功的可信在线设备可以预选，但确认按钮或 Sheet 内的 `Enter` 才创建一次新请求。
- 发送结果必须为“请求已创建”，并提供“查看进度”；不得将创建表述成送达。
- 失败只能引导“从当前条目重新创建请求”，并生成新的 transfer / session / nonce。

### C. Incoming Center 与 Timeline（P0，US-480 / US-481 / US-490）

主导航增加带数量的“收件”入口和 `⌘⇧I`；其页面是稳定的待处理请求入口，而非 Devices 的附属卡片。

- 打开收件中心会记录无正文的 `viewed` 事件；不自动读剪贴板、写 History 或展示通知正文。
- 每条待处理请求显示来源、固定 `TEXT` 类型、倒计时与 Copy / Save / Reject；待处理上限仍为 3 条。
- Copy 只写系统剪贴板；Save 只写本机 History，并保留不可逆的 Local Link 来源标记；Reject 不写两者。
- 传递详情统一显示状态、结果和唯一下一步。它是元数据页面，不显示正文、摘要、搜索词、路径或旧正文重试。

### D. Devices、History 与可访问性（P1）

- Devices 首屏只显示就绪结论、首个阻塞项、唯一修复动作和可用设备；配对、信任和诊断继续渐进披露。
- History 对 Save 结果显示“来自设备 · Local Link”，并可按 Local Link 来源筛选；Copy、Reject 与取消不生成 History。
- 所有抽屉/弹窗使用一致的 Tab 顺序、可见焦点环、`Esc` 关闭和不依赖颜色的状态标签；小窗口下信息面板改为抽屉。

## 实施包与依赖

| 包 | 变更 | 依赖 | 验收出口 |
| --- | --- | --- | --- |
| A | 传递状态投影、`viewed` / `cancelled` 协议与浏览器夹具、幂等终态测试 | 无 | 任一 transfer 只有一个终态；正文不进入 DTO 或持久化事件。 |
| B | Handoff Sheet、统一 readiness 排序、`⌘Enter`/`Enter` 规则与创建结果条 | A 的状态文案 | 不进入 Devices 即可从当前条目创建新请求。 |
| C | Incoming Center、导航徽标/快捷键、Copy/Save/Reject、Timeline | A | 接收方无需 Devices 就能发现、查看并决定。 |
| D | Devices 首屏收敛、History 来源标记与筛选、隐私边界文档/扫描 | B、C | 页面状态和持久化边界一致，正常本机功能不被 Local Link 改变。 |
| E | 工程、视觉、隐私验收及真实双 Mac Gate 记录 | A–D | 本地工程 Gate 通过；真实双 Mac Gate 如无设备证据则保持 No-Go。 |

## 自动化测试用例

| ID | 场景 / 操作 | 通过条件 |
| --- | --- | --- |
| ST-01 | 创建、送达、查看、Copy / Save / Reject / Cancel / Expire 事件 | 状态文案精确区分；终态没有歧义。 |
| ST-02 | 同一 transfer 重复 View、Copy、Save、Reject、Cancel | 无重复副作用；首个已确认终态保持不变。 |
| ST-03 | Cancel 与接收决定竞态 | 仅一个终态；pending body 在任一终态后清除。 |
| ST-04 | 进程重启、超时、清空终态记录 | 不恢复 pending body、待处理队列、旧 nonce 或正文。 |
| QP-14 | Quick Paste `Enter` | 仅本机复制；不打开 Sheet、不创建 transfer。 |
| QP-15 | Quick Paste `⌘Enter`、选择设备、再次 `Enter` | 首次只打开 Sheet；再次确认才创建一个新请求。 |
| QP-16 | 无设备、离线、失信、非文本、空文本、超限 | 保留可见设备；只显示一个阻塞原因和一个修复动作；不发送正文。 |
| IN-14 | 收件徽标、`⌘⇧I` 与空状态 | 待处理数量可访问；进入稳定收件中心；空状态可关闭。 |
| IN-15 | 打开收件中心与查看请求 | 记录内容无关 `viewed`；不读/写剪贴板、不写 History、不返回正文到 WebView。 |
| IN-16 | Copy / Save / Reject | Copy 仅剪贴板；Save 仅 History + 来源标记；Reject 无本机内容副作用。 |
| TL-14 | Timeline 的 created / delivered / viewed / terminal 路径 | 状态、设备引用、倒计时、失败分类和唯一恢复动作可读；无正文、摘要或 Retry old payload。 |
| DV-14 | 设备首屏与窄窗口 | 只有一个 readiness 结论和修复动作；状态不单靠颜色；无裁切、重叠或焦点陷阱。 |
| HS-14 | Local Link 来源筛选 | 仅 Save 结果可筛选；Copy / Reject / Cancel 不生成 History 条目。 |
| PR-14 | 传输 DTO、浏览器夹具、SQLite/WAL、日志、诊断包与临时文件哨兵扫描 | 正文、预览、搜索词、路径、网络标识、nonce 和 payload digest 命中为 0。 |
| RG-14 | Local Link 关闭、失败、拒绝、撤回或超时 | 不改变 Capture、普通 History、Quick Paste、回收站或 Labs 默认行为。 |

## 验收 Gate

### 本任务工程 Gate

1. Biome、TypeScript、Vitest 与 Vite build 全绿。
2. Rust format、Clippy 与 locked Rust tests 全绿。
3. 上述 v1.4 状态机、浏览器适配、UI、来源和隐私边界用例全绿。
4. 在 960–1180、900 和最小 760 px 宽度检查 Quick Paste、Handoff Sheet、Incoming Center、Timeline 与 Devices；无裁切、重叠、不可见焦点或正文泄露。
5. 不新增直接依赖、第三方资产、网络目的地、账号、遥测、远程模型、正文存储或 OS 权限。

### 真实双 Mac 发布 Gate（环境外；本次默认 No-Go）

必须分别留存 G1–G8 的可复核证据：权限与开关重启、20 次配对循环、200 次同候选文本请求及 P95、AP 隔离/VPN/换网/退出/睡眠/撤回竞态、磁盘隐私扫描、两小时性能、阻塞文案和安装升级回归。自动化、浏览器夹具、loopback 或同机多进程均不得表述为真实双 Mac Beta 证据。

## 影响结论

- **隐私：** `viewed` 只记录有限状态和时间；Save 的 History 来源为本机元数据。待处理正文继续只在原生内存，通知不在本次启用。相应更新 `PRIVACY.md`、`docs/privacy/data-flow.md`、`docs/open-source/module-boundary.md` 与边界测试。
- **安全：** 不改变双端配对、认证、信任、显式发起、接收方决定、60 秒期限、无旧正文重放或本地优先边界；所有新状态按 transfer ID 幂等。
- **许可证：** 不新增依赖、第三方代码或资产；无需变更直接依赖清单或重跑 SBOM/许可证审查。
