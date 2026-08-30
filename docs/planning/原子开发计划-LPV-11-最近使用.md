# LPV-11 复制后更新最近使用时间原子开发计划

## 计划元数据

- 计划 ID：ATOMIC-LPV-11-USAGE-001
- 类型：atomic-development
- 修订版本：1
- 状态：active
- 父级 ID：LPV-11
- 创建基线：c871210

## 总体计划

- 产品结果：用户成功复制账号名或密码后，对应账号立即显示并持久化最近使用时间。
- 验收场景：复制成功后列表时间更新；重启后时间仍存在；最近/最早排序可切换；搜索和分类筛选继续生效。
- 明确排除：不实现使用概览图、不统计复制次数、不改变显示密码行为和剪贴板条件清理规则。
- 已确认调用链：`src-tauri/src/commands/vault.rs` 复制 command 读取凭据并写入平台剪贴板；`vault-app` 管理确认快照；`vault-core` 保存账号字段；`App.svelte` 当前固定渲染“未使用”。
- 数据约束：旧版账号缺少时间字段时必须兼容读取为空；时间使用 Unix 毫秒；列表 DTO 仍不得包含密码或备注。
- 执行模式：连续执行；每个原子独立验证、提交并按项目规则同步远端。
- 原子顺序：`LPV-11-U01` → `LPV-11-U02`。
- 整体回归：目标 Rust 测试、`cargo test --workspace`、`npm run check`、运行中的 `tauri dev` Debug 冒烟；不执行 Release 构建。

## LPV-11-U01 Rust 持久化最近使用时间

- 状态：done
- 支持的验收场景：复制成功后获得包含最新使用时间的安全确认快照，重载后数据不丢失。
- 唯一目标：为成功的复制操作记录并持久化账号最近使用时间。
- 当前行为与目标行为：当前账号模型没有时间字段且复制 command 返回空响应；完成后旧数据兼容为空，新复制记录 Unix 毫秒并返回 `VaultStatusDto`。
- 前置条件与依赖：LPV-11 的凭据读取和 Windows 剪贴板写入已完成。
- 代码定位依据：`vault-core::Account`、`VaultDocument`；`vault-app::AccountSummary`、`snapshot_from`；`commands/vault.rs` 的两个复制 command。
- 允许修改：上述 Rust 模块、直接受影响测试和计划状态。
- 明确不修改：Svelte 渲染、使用概览、剪贴板 token/sequence/30 秒清理。
- 实现步骤：账号增加兼容反序列化的可选 Unix 毫秒字段；核心按稳定 ID 更新时间；应用层保存候选并返回确认快照；复制成功后记录时间并返回 DTO。
- 接口、数据与错误契约：`lastUsedAt: number | null`；账号不存在沿用 `ACCOUNT_NOT_FOUND`；保存失败沿用 `VAULT_SAVE_FAILED`；响应不含密码。
- 边界与异常：只有平台剪贴板写入成功后记录；旧账号缺失字段解析为 `None`；保存失败不得替换应用确认快照。
- 测试要求：固定时间戳验证保存、revision 和快照；验证旧 JSON 兼容；验证 DTO 包含时间且无密码。
- 验证命令：`cargo test -p vault-app recording_credential_use_persists_timestamp_and_returns_updated_snapshot`；`cargo test -p vault-core`；`cargo test -p local-password-vault commands::vault`。
- 预期结果：复制 command 可返回持久化确认后的最新安全快照。
- 完成判定：目标测试通过，相关 Crate 检查通过，完整 diff 不改变剪贴板安全逻辑。
- 交付给下一原子的输出：前端可从复制响应读取 `lastUsedAt`。
- 停止或重新规划条件：需要升级 schema 或无法兼容现有密码库格式。
- 风险等级：L2。
- DDD 门禁：旧数据兼容和复制成功/记录失败语义可由针对性测试证明时不额外质疑。
- 计划提交信息：`fix(usage): [LPV-11-U01] 复制后持久化最近使用时间`。

### 执行记录

- 实际验证：失败测试先确认缺少 `record_account_used`；实现后目标应用层测试、25 项核心测试、6 项 Tauri command 测试及两个相关 Crate 的 `cargo check` 全部通过，格式检查通过。

## LPV-11-U02 前端显示并排序最近使用时间

- 状态：in_progress
- 支持的验收场景：复制后当前行显示时间并按所选顺序重排。
- 唯一目标：前端消费最近使用时间并提供已确认的两种排序。
- 当前行为与目标行为：当前复制包装返回 `void`、列表固定“未使用”、排序框禁用；完成后复制响应刷新列表，显示本地时间并支持最近/最早优先。
- 前置条件与依赖：`LPV-11-U01` 已完成并提供 `lastUsedAt`。
- 代码定位依据：`frontend/src/lib/tauri/commands.ts` 的账号摘要与复制包装；`App.svelte` 的 `copyCredential`、`visibleAccounts`、排序控件和最近使用单元格。
- 允许修改：上述前端文件、直接样式和计划状态。
- 明确不修改：Rust、使用概览、账号编辑表单及剪贴板文案。
- 实现步骤：扩展安全摘要字段；复制成功接收确认快照并在搜索时刷新结果；按时间稳定排序；格式化显示本地日期时间；启用排序下拉框。
- 接口、数据与错误契约：空时间显示“未使用”；有时间按本地环境格式显示；排序不得改变筛选范围。
- 边界与异常：未使用账号在最近优先时排后、最早优先时排前；相同时间保持稳定 ID/原顺序，不制造随机跳动。
- 测试要求：前端类型检查通过，Debug HMR 成功；人工验收复制后的时间和排序。
- 验证命令：`npm run check`；`cargo test --workspace`；运行中的 `tauri dev` 冒烟。
- 预期结果：用户点击复制后无需手动刷新即可看到最近使用时间。
- 完成判定：前端检查和 Workspace 回归通过，程序已热更新供用户测试。
- 交付给下一原子的输出：LPV-11 最近使用缺口修复完成。
- 停止或重新规划条件：后端 DTO 契约与计划不一致。
- 风险等级：L1。
- DDD 门禁：不适用。
- 计划提交信息：`fix(ui): [LPV-11-U02] 显示并排序最近使用时间`。

## 变更记录

| 修订版本 | 变化 | 原因 |
|---|---|---|
| 1 | 初始计划 | 修复复制后最近使用时间未更新 |
