# ClipRiva 0.3 实施与验收方案

> 版本主题：**Daily Reliability / 可信找回与长期日用**。本方案依据 `ClipRiva-0.3-下一版本产品优化方案.doc`，保持 macOS、本地优先、无账户、无网络的 Core 边界。

## 1. 反馈消化与本轮边界

0.2 已经建立了 Quick Paste、轻量 History、首次价值路径及 Default / Strict 隐私保护。0.3 的问题不是功能不足，而是用户无法在异常与长期使用时确认“是否仍在记录、为什么未记录、能否可靠找回”。

本轮交付按风险优先：

| 优先级 | 模块 | 交付决策 | 不做什么 |
| --- | --- | --- | --- |
| P0 | Quick Paste | 稳定、可复现的本地排序；零输入建议；键盘 Restore / Direct Paste；底部状态提示 | 云端/语义黑盒排序、搜索词留存 |
| P0 | Capture Trust | 最近未捕获原因事件（仅原因、时间、来源）；状态可见、可 Resume；Strict/Default 行为可解释 | 保存被拒绝内容或将其送入 Labs |
| P0 | 韧性 | Direct Paste 的失败自动降级为 Restore；快捷键冲突和权限状态在 Settings 可见 | 向错误目标应用发送按键 |
| P1 | History | 按需筛选；临时批量删除；继续保持单条 Undo、批量确认 | 常驻复选框、复杂标签体系 |
| P1 | Settings | 从配置项升级为状态摘要；展示快捷键、Capture、保护模式和最近原因入口 | 大型偏好设置工作台 |
| 验证 | 性能 | 为 1k/10k/50k 检索与启动建立可运行基线和回归检查 | 以未验证的数值替代真实实机压测 |

Labs 继续只保留本地文本 Actions，默认关闭；账户、同步、跨平台、云端 AI、标签系统和插件平台不属于 0.3。

## 2. 设计方案

### 2.1 Quick Paste：可预测的高频找回

排序计算全部在本地完成，并在同一输入和同一数据集下保持稳定：

1. 规范化文本（大小写、连续空白与换行），并按词/域名/文件名边界匹配；
2. 依次比较完整匹配、前缀匹配、词边界/域名/文件名匹配和包含匹配；
3. 在相同匹配层内，以最近使用、当前来源相关、最近复制和使用频率作为稳定次级排序；
4. 零输入仅展示有限的近期/常用/置顶建议，置顶不永远占据搜索第一位。

面板默认聚焦搜索框、展示 7–9 个紧凑结果；上下键选择、Space 预览、Enter 按当前恢复设置执行、Command-Enter 强制 Direct Paste、Escape 关闭。任何 Direct Paste 失败都先恢复剪贴板，再说明手动粘贴下一步。

### 2.2 Capture Trust：未记录也必须可解释

新增本地 `CaptureStatusEvent`：`reason`、`occurredAt`、`sourceApp` 和可选的安全状态描述。禁止写入原始剪贴板内容、内容哈希、预览或 Labs 派生数据；最多保留 20 条，按保留策略清理过期事件。

事件覆盖：手动暂停、Default 拦截、Strict 拦截、排除应用、格式不支持和权限降级。Quick Paste 与 Settings 显示文字、图标及动作，而不是只用颜色；Strict 和手动暂停提供 Resume，Default 拦截明确“已阻止当前条目，后续捕获仍在继续”。

### 2.3 History / Settings：按需管理，状态优先

History 默认维持一个列表；点击筛选时才出现类型、来源应用、置顶、最近使用条件。批量模式为暂态，显示已选数量，Escape 退出；批量删除必须显示数量、二次确认，默认焦点为 Cancel。保留现有单条 6 秒 Undo / Command-Z。

Settings 顶部摘要 General 的 Quick Paste 快捷键与恢复模式、Privacy 的 Capture / Default-Strict / 排除应用状态、Labs 的开关。需要系统权限的功能以“可恢复到剪贴板”的降级结果说明，避免用户进入设置迷宫。

## 3. 测试用例与验收标准

| ID | 层级 | 场景与操作 | 预期 |
| --- | --- | --- | --- |
| QP-01 | 单元 | 同时存在完整、前缀、包含、不同大小写与空白的文本，执行本地搜索 | 排序符合匹配层级；同一输入重复结果一致；无网络调用 |
| QP-02 | 组件 | Quick Paste 零输入、上下键、Space、Enter、Command-Enter、Escape | 展示有限建议；焦点与选择可见；键盘操作调用正确恢复路径；关闭面板 |
| QP-03 | 集成 | Direct Paste 缺少辅助功能权限或目标应用不可用 | 内容先恢复到剪贴板；显示下一步；不向错误应用发送 Command-V |
| CT-01 | Rust | Default 模式拦截疑似敏感内容后再处理普通内容 | 被拒绝内容不进历史/Labs；Capture 持续；保存无内容的状态事件 |
| CT-02 | Rust | Strict 模式拦截疑似敏感内容 | 被拒绝内容不进历史/Labs；Capture 暂停；事件可查询；Resume 清除暂停并产生恢复状态 |
| CT-03 | Rust / 组件 | 连续产生 21 条原因事件及过期事件 | 仅返回最近 20 条且过期事件被清理；事件不含原文、预览或 hash |
| CT-04 | 组件 | Capture 暂停、Default/Strict 拦截、排除应用、格式不支持 | Quick Paste / Settings 同时有文字状态和可执行的 Resume 或管理入口 |
| H-01 | 组件 | 打开筛选，按类型、来源、置顶、最近使用组合筛选，随后清除 | 默认无常驻筛选栏；结果正确；清除后恢复普通单列表 |
| H-02 | 组件 | 进入批量管理、选择多条、Escape、再次进入并确认删除 | Escape 不删除；确认前默认焦点为 Cancel；确认后仅删除选中项；已置顶记录按选择规则保护 |
| H-03 | 组件 | 单条删除，点击 Undo 或 Command-Z | 6 秒内撤销恢复；超时后不会意外恢复 |
| ST-01 | 组件 | 打开 Settings，切换 General / Privacy / Labs，按 Tab / Escape | 摘要与设置一致；焦点不穿透；Escape 关闭并回到入口 |
| PF-01 | 基准 | 1k/10k/50k 混合记录的关键词检索与 Quick Paste 打开 | 输出可重复的耗时数据；10k 搜索目标为 ≤100 ms，超过阈值阻止发布 |
| RG-01 | 构建 | `pnpm check`、`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` | 全部通过；无 TypeScript、格式或警告回归 |
| UX-01 | 人工 | 首次找回、中文/URL/代码搜索、暂停后恢复、无权限 Direct Paste、深浅色窗口 | 60 秒首次找回不进 Settings；无静默漏记、无裁切、关键状态非仅颜色表达 |

## 4. 发布门槛

阻断发布的条件：数据丢失或敏感内容写入、Quick Paste 不可达/恢复失败、Capture 静默暂停或状态与实际不一致、权限降级后内容丢失、10k 历史明显卡顿、关键窗口裁切/焦点穿透。

合并前必须通过上述自动化门槛，并完成 Quick Paste、Capture 状态、History 批量、Settings 状态和权限降级的浏览器/实机验收。真实 macOS 快捷键冲突、多显示器、全屏与中文输入法组词仍需要在 Beta 机器上复核。
