# ClipRiva v1.5 上线候选实施与验收方案

- **输入：** `ClipRiva_v1.5_上线版本产品方案.docx`（2026-07-30）与当前 v1.4 实现。
- **版本目标：** 从工程闭环进入可验证的 Release Candidate；本仓库任务完成自动化与同机原生测试，真实双 Mac、签名安装和公开发布仍由独立 Gate 决定。
- **核心边界：** 本机剪贴板管理；Local Link 仅传输用户显式选择的一条 UTF-8 文本；默认关闭；无账号、云同步、自动同步、离线正文队列、文件/图片传输或旧正文重放。
- **禁止误报：** 浏览器夹具、loopback、同机双进程和单元测试都不能作为真实双 Mac、睡眠唤醒、换网、签名安装或公证证据。

## 需求结论与本次实施包

| 包 | v1.5 目标 | 改造前差距 | 本次出口 |
| --- | --- | --- | --- |
| A | 智能分类与标签 | text/code/url/color 等已有；command 和用户标签缺失 | 增加 command 分类；提供有边界的本地标签增删、显示、筛选和搜索 |
| B | Quick Paste | 快捷键和键盘操作已有；WebView 零输入排序覆盖原生最近使用顺序 | 统一最近活动优先语义，查询态仍保持相关性优先 |
| C | Local Link 正式化 | SAS 配对、信任期、撤销已有；回执丢失、睡眠/换网和重启恢复不闭环 | 升级协议 v3；接入 metadata-only StatusQuery；增加 generation supervisor；正文永不自动重放 |
| D | 收件、设置与隐私 | Incoming Center、Copy/Save/Reject 已有；原生副作用与终态跨崩溃不原子 | Copy 持久化 effect claim + 私有 pasteboard marker；Save/Reject 使用事务/CAS；不确定结果显式标记 |
| E | 上线验证 | 自动化较完整；真实设备、换网/睡眠、签名安装证据缺失 | 新增故障注入与生命周期 reducer Gate；真实双 Mac/签名证据缺失时保持 No-Go |

## 测试用例设计

| ID | 场景 / 操作 | 通过条件 |
| --- | --- | --- |
| CL-15-01 | 输入 shell 命令、带参数命令、代码、URL 和普通句子 | command 只命中明确命令；既有 code/url/text 分类不回退 |
| CL-15-02 | 在 History 与 Quick Paste 选择 command 类型 | 两处均可筛选、搜索、复制；类型文案和图标一致 |
| TG-15-01 | 新增含大小写、首尾空白和重复值的标签 | 标签规范化、去重且顺序稳定；单条数量和长度有上限 |
| TG-15-02 | 重启数据库后读取、增删标签 | 标签本地持久化；删除标签不删除正文或 occurrence |
| TG-15-03 | 按标签筛选，并用标签文字搜索 | 只返回匹配条目；正文、来源应用与标签组合查询行为确定 |
| TG-15-04 | 条目进入回收站、恢复、永久删除 | 回收站期间标签保留；永久删除后标签级联删除 |
| QP-15-01 | 使用旧条目后重新打开零输入 Quick Paste | 该条目按 lastUsedAt/最近 activity 排在首位 |
| QP-15-02 | 固定但长期未使用条目与刚使用的普通条目并存 | 最近使用优先；有限窗口仍遵守原生固定项可见策略 |
| QP-15-03 | 输入查询并用方向键、Enter、Space、⌘Enter | 相关性排序优先；键盘选择和实际激活条目一致 |
| LL-15-01 | needsRePairing/revoked 设备点击重新配对 | 不再把稳定设备 ID 当匿名 discovery candidate；只有明确发现候选才开始 SAS |
| LL-15-02 | 无候选、单候选和多候选重新配对 | 无候选给检查项；单候选可进入 SAS；多候选要求用户明确选择 |
| LL-15-03 | 完整 payload 写入成功并持久化 no-replay boundary 后，在 `PayloadComplete`/Receipt 阶段注入断连或丢包 | 发送方只重试认证后的 StatusQuery；不重放正文；最终与接收方终态一致 |
| LL-15-04 | StatusQuery 返回 unknown/pending、进程重启或超过有界重试 | 使用持久 20 秒 active-time deadline；单轮最多 4 个 endpoint、单 query ≤1.5 秒、socket I/O ≤500 ms；重启不重置，已知生命周期暂停不消耗 active time；无法证明时显示 outcomeUnknown |
| LC-15-01 | sleep/wake、session inactive/active、网络路径变化与 listener worker death | 先清正文/关闭 reveal/停 transport；generation 防旧回调；旧 reconciliation worker 不得写 outcomeUnknown；恢复后沿用顺延 deadline；500 ms 防抖和有界重启；失败进入 degraded |
| FX-15-01 | Copy 在 claim、原生写入、DB 完成前后分别崩溃/重入 | 最多一次系统剪贴板写入；marker 可证明时完成，否则 outcomeUnknown；绝不重放正文/副作用 |
| FX-15-02 | Save/Reject 并发、DB 故障和重复命令 | Save 的 History+终态同一事务；Reject 先持久化 CAS 再清正文；仅一个终态胜出 |
| IN-15-01 | Incoming Copy / Save / Reject / Cancel / Expire | Copy 只写剪贴板；Save 只写 History；其余不写本机内容；终态唯一 |
| PR-15-01 | 序列化 transfer、diagnostics、日志/SQLite/WAL 哨兵扫描 | Local Link DTO/诊断无正文、标签、预览、路径、稳定 peer ID、endpoint 或 raw error |
| RG-15-01 | Local Link 关闭、失败、拒绝、撤回和过期 | Capture、History、Quick Paste、回收站和 Labs 默认行为不变 |
| PF-15-01 | release 模式搜索 10k 条目 | 执行已有 ignored benchmark 并记录耗时；不得以 debug 结果替代 |

## 本机工程 Gate

1. `pnpm lint`、`pnpm typecheck`、`pnpm test`、`pnpm build` 全绿。
2. `cargo fmt --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --locked` 全绿。
3. release 模式 10k Quick Paste 性能基准通过。
4. 新标签只存储在应用私有 SQLite；不进入 Local Link、诊断、遥测或远程服务；隐私边界测试覆盖该结论。
5. 直接依赖或锁文件变化时更新依赖清单并重跑 Node/Rust 许可证与 SBOM 元数据审查；不引入第三方资产、账号、遥测、远程模型、生产 endpoint、额外 OS 权限或签名材料。

### 执行结果（2026-08-01）

- `pnpm release:verify` 通过：版本一致性、Biome lint、TypeScript typecheck、生产构建、16 个
  Vitest 文件的 145/145 项测试均通过。
- Rust `fmt --check`、全 target / feature Clippy `-D warnings` 和全量测试通过：193 passed、
  0 failed、1 个 release 性能测试按默认规则 ignored。
- 已单独执行 ignored 的 release 基准：合成 10,000 条历史搜索耗时 60.995416 ms，低于
  100 ms 目标；该单次本机结果不替代冻结候选上的多轮 p50/p95。
- 本机 Gate 结论为 **通过**。真实双 Mac、换网/睡眠恢复、抓包/磁盘扫描、签名安装和
  公证证据均未在当前环境执行，因此公开发布 Gate 仍为 **No-Go**。

## 真实双 Mac 发布 Gate（环境外，缺证据即 No-Go）

1. 两台真实 Mac 使用同一候选构建，完成首次安装、权限拒绝/允许、升级、重启、卸载清理验证。
2. 完成至少 50 次双端 SAS 配对循环、200 条传输和 Copy/Save/Reject/Cancel/Expire，记录成功率与 P95；该门槛覆盖产品方案样本并服从仓库更严格的发布清单。
3. 覆盖 Wi-Fi 切换、VPN、AP 隔离、防火墙、睡眠/唤醒、锁屏、进程退出和双方决定竞态；确认不自动重发正文。
4. 使用唯一合成 marker 扫描应用日志、SQLite、WAL、临时目录与诊断导出；正文命中为 0（History 中用户明确 Save 的副本除外）。
5. 覆盖 macOS 支持矩阵、Intel/Apple Silicon、VoiceOver、Reduce Motion、多屏和中文输入法。
6. 签名、公证、Gatekeeper 和公开下载仅在 `docs/release/release-checklist.md` 全部满足且获得维护者授权后执行。

## 已知问题解决结果与剩余发布阻断

- **工程问题已解决：** macOS workspace + Network.framework 信号已接入 generation-scoped supervisor；覆盖重复/过期回调、500 ms 防抖、250 ms/1 s/2 s/5 s 重试、degraded 和 stale start 防护的 reducer/服务测试。
- **工程问题已解决：** 协议升级到 v3；生产 responder 按 Noise 认证 peer 处理 `StatusQuery`。完整 payload 写入后的 no-replay boundary、post-boundary provisional Cancel、持久 20 秒 active-time deadline、4 endpoint / 1.5 秒 query / 500 ms I/O 上限和 lifecycle pause/resume 均由实现强制；发送方只做 metadata-only 对账。真实 TCP 故障注入证明正文不重放且终态收敛。
- **工程问题已解决：** Copy 使用持久化 claim 和随机 pasteboard marker；Save 把 History 与终态置于同一事务；Reject 先 CAS 再清正文。崩溃后无法证明的 Copy 明确变为 `outcomeUnknown`，绝不重复副作用。
- **工程问题已解决：** Incoming native reveal 具备 60 秒关闭控制器，并在 sleep、session inactive、disable、termination 时清空和关闭；该结论是源代码/自动化证据，不是物理锁屏验收记录。
- **仍为发布阻断：** 两台真实 Mac 的 50 次配对/200 条传输、物理换网/睡眠/锁屏/用户切换、签名安装、升级/卸载、抓包及 SQLite/WAL/日志 marker 扫描尚未执行。以上任一证据缺失都不能标记 v1.5 可公开上线。

## 影响结论

- **隐私：** 新增的 `reconciling`/`outcomeUnknown`、effect claim、pasteboard marker 和生命周期事件均为内容无关元数据；claim 不含正文、digest、peer、endpoint 或 Item 引用，随 transfer 级联删除。正文仍不进入 SQLite/WAL、诊断或 WebView。
- **安全：** 不扩大 Local Link 的远程目标或信任边界；StatusQuery 必须经过 Noise 认证并匹配 peer；任何恢复都禁止正文/副作用重放。生命周期挂起先清正文和 reveal，再停止 transport。
- **许可证：** 新增 macOS 直接依赖 `block2 0.6.2`（MIT）承载 Network.framework 回调；它原已存在于锁定传递图中，因此不新增锁定 crate。已按仓库规则重跑 Node/Rust 元数据审查，未发现缺失或受限许可证。
