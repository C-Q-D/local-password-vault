# LPV-12 保存失败保持旧数据原子开发计划

## 计划元数据

- 计划 ID：ATOMIC-LPV-12-001
- 类型：atomic-development
- 修订版本：1
- 状态：completed
- 父级 ID：LPV-12
- 创建基线：2ae6013

## 总体计划

- 产品结果：密码库保存失败时显示明确错误，不发布候选内存状态，也不丢失上一份已确认主库。
- 验收场景：账号编辑保存失败后表单和列表恢复旧值；新增、删除或分类保存失败不产生假成功；重启仍能读取旧数据。
- 明确排除：不实现自动重试、故障诊断页、备份恢复界面和 LPV-13 的用户确认流程。
- 已确认架构：`vault-storage::save_confirmed` 在替换前完成候选认证；`VaultApplication` 只在仓储返回确认文档后替换内存快照；Tauri 已把失败映射为 `VAULT_SAVE_FAILED`；Svelte 当前只在 command 成功后赋值 `vault`。
- 当前缺口：`WindowsVaultFiles::replace_main` 对 Win32 错误 1177 仅返回普通 I/O 错误，未恢复已被重命名的旧主库；账号编辑失败时表单仍保留未保存草稿，没有恢复已确认旧值。
- 数据不变量：失败路径不得把候选数据作为成功返回；1177 恢复只允许操作当前数据根中的固定 main、backup 和本次拥有的 temp；测试只使用 `tempfile` 隔离目录。
- 执行模式：连续执行；每个原子独立验证、提交并按项目规则同步远端。
- 原子顺序：`LPV-12-01` → `LPV-12-02`。
- 整体回归：目标平台/应用测试、`npm run check`、`cargo test --workspace`、运行中的 `tauri dev` Debug 冒烟；不执行 Release 构建。

## LPV-12-01 恢复 ReplaceFileW 1177 后的旧主库

- 状态：done
- 支持的验收场景：Windows 替换进入 1177 部分失败状态时，旧主库回到固定主文件名，候选不被发布。
- 唯一目标：为错误 1177 增加可验证的旧主库恢复分支。
- 当前行为与目标行为：当前所有 `ReplaceFileW` 错误都直接返回 `Io`；完成后调用时提供固定 backup 路径，1177 时将该备份移回 main，再返回保存失败。
- 前置条件与依赖：现有 `OwnedTemp` 只清理本次 `create_new` 取得的临时文件；应用层已有失败不发布快照测试。
- 代码定位依据：`crates/vault-platform-windows/src/atomic_file.rs` 的 `replace_main`、`MoveFileW` 和隔离测试；`crates/vault-app/src/lib.rs` 的保存失败测试。
- 允许修改：Windows 原子文件适配、其直接测试、必要计划状态。
- 明确不修改：加密信封、DPAPI、Tauri command、Svelte、启动恢复流程和 EFS 专用写入路径。
- 实现步骤：向 `ReplaceFileW` 提供同目录固定 backup；提取只接受错误码和固定路径的恢复辅助函数；1177 时确认 main 缺失且 backup 存在后用 `MoveFileW` 恢复；无论恢复是否成功都返回失败并由 `OwnedTemp` 清理候选。
- 接口、数据与错误契约：`VaultFiles::replace_main` 公共签名不变；成功仍返回 `Ok(())`；1177 恢复成功或失败都向上返回 `VaultFileError::Io`，不得把候选误报为已提交。
- 边界与异常：非 1177 不执行恢复；恢复目标存在或 backup 缺失时不覆盖任何文件；日志只含操作类别和系统码，不含路径或内容。
- 测试要求：隔离目录模拟 1177 文件布局，断言旧 backup 恢复为 main、候选保持临时名并可由所有者清理；非 1177 不移动；现有正常替换测试继续通过；应用层编辑/删除保存失败测试继续证明旧快照不变。
- 验证命令：`cargo test -p vault-platform-windows atomic_file`；`cargo test -p vault-app save_failure`；`cargo check -p vault-platform-windows`。
- 预期结果：Windows 部分替换失败不会让固定主库路径消失，也不会发布候选。
- 完成判定：隔离恢复测试、平台测试和相关应用失败测试通过，DDD 结论无未处理 blocker。
- 交付给下一原子的输出：后端所有当前保存失败路径都保持旧确认状态并返回稳定失败。
- 停止或重新规划条件：1177 的官方文件状态与当前固定 backup 方案冲突；恢复需要覆盖未知文件或操作测试目录外数据。
- 风险等级：L3。
- DDD 门禁：实现前隔离审查“1177 时只把由本次 ReplaceFileW 产生的固定 backup 移回缺失 main 能保住旧主库且不会发布候选”。
- 计划提交信息：`fix(storage): [LPV-12-01] 恢复替换失败后的旧主库`。

### 执行记录

- 实际验证：DDD 隔离审查返回 PASS；10 项 Windows 原子文件测试通过，覆盖 1177 恢复、已有 main 不覆盖、非 1177 不移动及重复替换保留上一版；4 项应用保存失败测试通过；平台 `cargo check` 和格式检查通过。

## LPV-12-02 编辑保存失败时恢复已确认表单

- 状态：done
- 支持的验收场景：账号编辑保存失败后看到明确错误，表单、列表和再次打开的详情都是旧值。
- 唯一目标：账号编辑失败时恢复打开弹框时的已确认字段。
- 当前行为与目标行为：当前列表不会提前变化，但弹框保留失败草稿；完成后仅 `VAULT_SAVE_FAILED` 恢复编辑基线并保留错误提示，创建失败仍保留输入便于重试。
- 前置条件与依赖：LPV-12-01 确认后端失败契约；`load_account_for_edit` 已提供单条已确认详情。
- 代码定位依据：`frontend/src/app/App.svelte` 的 `openEditAccountDialog`、`submitAccount`、`closeAccountDialog` 和账号 Draft。
- 允许修改：该 Svelte 组件、必要的前端类型/样式和计划状态。
- 明确不修改：Rust、分类交互、删除确认流程、备份恢复界面和使用概览。
- 实现步骤：加载编辑详情时保存一份独立确认基线；提交时使用草稿副本；收到 `VAULT_SAVE_FAILED` 时把 Draft 恢复为基线并显示固定明确提示；成功或关闭时清理基线。
- 接口、数据与错误契约：不新增 command；错误码沿用 `VAULT_SAVE_FAILED`；不得把密码写入日志、通知或列表状态。
- 边界与异常：必填校验错误不恢复；分类已删除等业务错误保留用户草稿；创建失败没有旧账号可恢复；删除失败保持确认框和旧列表。
- 测试要求：`svelte-check` 通过；Debug HMR 成功；人工注入失败由现有隔离后端测试证明数据不变量，正式用户密码库不用于故障注入。
- 验证命令：`npm run check`；`cargo test --workspace`；运行中的 `tauri dev` 冒烟。
- 预期结果：保存失败不再让编辑弹框看起来像新值已生效。
- 完成判定：前端检查和 Workspace 回归通过，程序可供用户验证正常成功路径。
- 交付给下一原子的输出：LPV-12 完整交付，下一单元为 LPV-13。
- 停止或重新规划条件：现有 command 在失败时会返回候选数据或覆盖前端 `vault`。
- 风险等级：L1。
- DDD 门禁：不适用。
- 计划提交信息：`fix(ui): [LPV-12-02] 保存失败时恢复已确认账号字段`。

### 执行记录

- 实际验证：`svelte-check` 通过（0 error、0 warning）；运行中的 Tauri Debug 程序收到 Svelte HMR 更新；`cargo test --workspace` 共 87 项测试通过，其中应用层覆盖新增、编辑、删除、分类和使用时间保存失败时保留旧确认快照，Windows 平台覆盖 1177 恢复；未对真实密码库注入故障。

## 变更记录

| 修订版本 | 变化 | 原因 |
|---|---|---|
| 1 | 初始计划 | 补齐 Windows 1177 恢复和账号编辑失败反馈 |
