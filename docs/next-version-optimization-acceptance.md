# ClipRiva 0.2 反馈消化、优化方案与验收用例

## 反馈结论

`ClipRiva-next-version-optimization-plan.docx` 的核心结论是：产品的本地优先与隐私边界已经足够清楚；下一版应减少默认界面的管理感，让用户在第一次使用和高频复用时更快获得价值。

本轮实现聚焦以下可交付范围：

- **P0：首次价值闭环。** 首次说明确认后只保留“复制两条内容，再用 Quick Paste 找回前一条”的下一步；成功恢复一次后不再重复提示。
- **P0：Quick Paste 成为主路径。** 打开时搜索框自动聚焦，连续紧凑列表最多显示 7 条，支持上下键、Enter 恢复、Space 临时预览和 Escape 关闭。
- **P0：History 减重。** 移除永久侧栏，以 History / Pinned 分段切换；详情默认关闭并按需打开；清除未置顶记录进入溢出菜单并先显示影响数量。
- **P0：敏感内容分级。** 默认保护只拒绝当前疑似敏感条目；Strict 才暂停 Capture。两种模式都不能把内容送入 Labs 派生数据。
- **P1：Settings 与日用控制。** 将配置拆成 General / Privacy / Labs；保留键盘焦点锁、Escape 关闭和焦点回归；Direct Paste 以结果语言说明权限与安全降级。
- **P1：低摩擦整理。** 单条删除采用立即删除 + 临时 Undo；批量清理仍需明确确认，并默认 Cancel。

本轮不包含账户、同步、云端 AI、跨平台、Labs 独立工作台、真实机器签名/公证和完整性能压测。后两项作为 Beta 实机验证项保留在 `beta-validation-matrix.md`。

## 产品与实现决策

| 反馈 | 本轮决策 | 验收信号 |
| --- | --- | --- |
| 首次隐私说明后价值不明显 | 首次使用引导只给出“复制两条 → Quick Paste”的短路径；首次成功恢复后自动结束。 | 页面不要求先进入 History / Settings；成功恢复后引导消失。 |
| Quick Paste 仍像次级弹窗 | 保持搜索自动聚焦，收敛为连续结果行和固定键盘提示；Space 控制按需预览。 | 打开后可直接输入；键盘可完成选择与恢复。 |
| 三栏 History 过重 | 用分段控件替代侧栏；详情抽屉默认关闭；危险清理收进溢出菜单。 | 默认只见紧凑历史列表；Pinned 不占固定栏位。 |
| 敏感误判会长期漏记录 | `default` 只拒绝当条，`strict` 才写入暂停状态；保留已存历史。 | 默认模式之后可继续捕获普通内容；Strict 显示可恢复原因。 |
| Settings 信息密度过高 | General / Privacy / Labs 轻量分区；每次只显示一个分区。 | 分区可键盘切换；原有焦点锁与保存行为不退化。 |
| 删除二次确认影响整理 | 单条删除改为删除后 Undo；批量清除保持二次确认。 | 5–8 秒可 Undo / Cmd-Z；批量默认焦点是 Cancel。 |
| Labs 抢占 Core 导航 | 删除全局 Labs 导航，只在选中文本详情中提供上下文 Actions。 | Labs 关闭时无导航、搜索模式、详情派生内容或 Actions。 |

## 自动化验收用例

| ID | 场景 | 操作 | 预期 |
| --- | --- | --- | --- |
| V-00 | 首次价值 | 完成首次说明；模拟复制两条内容；恢复前一条。 | 仅显示一个下一步提示；首次恢复后提示不再出现。 |
| Q-01 | Quick Paste 主路径 | 打开面板、输入关键词、上下键、Enter。 | 搜索框自动聚焦；选择可见；恢复调用正确条目。 |
| Q-02 | Quick Paste 预览 | 选中条目后按 Space 两次。 | 首次打开临时预览，第二次关闭；不常驻详情栏。 |
| H-01 | History 结构 | 打开 History、切换 Pinned、打开/关闭详情。 | 无永久侧栏；分段切换可访问；详情默认关闭。 |
| H-02 | 整理恢复 | 删除单条，然后点击 Undo 或按 Cmd-Z。 | 条目回到原列表；失效后不会意外恢复。 |
| H-03 | 批量危险操作 | 从溢出菜单选择清除未置顶。 | 显示数量；默认焦点为 Cancel；置顶内容不受影响。 |
| S-01 | Settings 分区与键盘 | 打开设置、Tab 循环、Escape 关闭。 | 三个分区存在；焦点不穿透；关闭后回到入口。 |
| S-02 | 权限降级文案 | 选择 Direct Paste。 | 说明“返回原应用并发送 Command-V”；权限不可用时仍可 Restore。 |
| P-01 | 默认敏感策略 | 写入疑似 token，随后写入普通文本。 | token 不入历史 / Labs；普通文本仍被捕获，Capture 未暂停。 |
| P-02 | Strict 敏感策略 | Strict 下写入疑似 token。 | token 不入历史；Capture 暂停并记录可 Resume 原因。 |
| L-01 | Labs 边界 | 关闭 Labs 后检查导航、检索和详情。 | Labs 入口、派生内容和 Actions 均不可用。 |

## 发布门槛

1. `pnpm check`、前端 Vitest、`cargo fmt --check`、`cargo clippy -- -D warnings` 和 `cargo test` 全部通过。
2. 浏览器验收覆盖 Quick Paste、History、Settings、首次引导、默认/Strict 敏感保护和 Labs 关闭状态。
3. 关键窗口无布局回归：Quick Paste 约 600–640 × 400–440，History 默认紧凑，Settings 使用单一分区内容。
4. 实机 Beta 前仍需按 `docs/beta-validation-matrix.md` 覆盖快捷键冲突、辅助功能权限、不同 Space / 全屏和真实应用粘贴。
