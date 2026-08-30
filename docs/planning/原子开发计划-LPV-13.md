# LPV-13 从有效备份恢复原子开发计划

## 计划元数据

- 计划 ID：ATOMIC-LPV-13-001
- 类型：atomic-development
- 修订版本：1
- 状态：active
- 父级 ID：LPV-13
- 创建基线：c5427a9

## 总体计划

- 产品结果：主库损坏或缺失且上一版备份有效时，应用进入不可编辑的待恢复状态；只有用户明确确认后才恢复并进入正常主界面。
- 验收场景：main 有效时正常载入；main 无效或缺失、backup 有效时不写盘并提示确认；确认后显示备份中的账号；取消不写盘；main 与 backup 都无效时只显示不可编辑错误。
- 明确排除：自动静默恢复、任意路径选择、跨电脑或系统重装恢复、导入其他密码管理器、诊断页、自动重试和 Release 构建。
- 已确认架构：`vault-storage::open_or_initialize` 当前会自动恢复有效 backup，必须改为只探测；`VaultFiles` 已提供固定 main/backup 读取和恢复入口；`VaultApplication` 当前只持有 Ready 状态；Tauri setup 当前在密码库打开失败时终止；Svelte 当前只有 loading/ready/error。
- 数据不变量：只有 main 与 backup 都不存在时才允许初始化空库；待恢复、取消和不可用状态不得写 main/backup；恢复源 backup 不得移动、删除或覆盖；只有重新认证成功的 main 才能进入 Ready；测试只使用内存 Fake 或 `tempfile` 隔离目录。
- 执行模式：连续执行；每个原子独立验证、提交并按项目 Git 规则推送 `origin/master`。
- 原子顺序：`LPV-13-01` → `LPV-13-02` → `LPV-13-03` → `LPV-13-04`。
- 整体回归：受影响 Crate 聚焦测试与 check、`npm run check`、`cargo test --workspace`、运行中的 Tauri Debug HMR/增量编译；不执行 Release。

## LPV-13-01 启动探测主库与备份状态

- 状态：done
- 支持的验收场景：main 有效正常载入；main 无效或缺失且 backup 有效进入待恢复；两者无效进入不可用；两者都不存在才初始化空库。
- 唯一目标：`vault-storage` 在启动时返回明确状态且不自动恢复 backup。
- 当前行为与目标行为：当前有效 backup 会立即写回 main；完成后启动只读取和认证，RecoveryRequired 与 Unavailable 均不写盘。
- 前置条件与依赖：现有加密信封认证、`VaultFiles::read_main/read_backup` 和首次不覆盖发布。
- 代码定位依据：`crates/vault-storage/src/initialization.rs` 的 `open_or_initialize`、`recover_backup` 与内存文件测试；`crates/vault-storage/src/lib.rs` 导出。
- 允许修改：存储启动状态类型、启动判定、直接测试、计划状态。
- 明确不修改：Windows 发布恢复、应用状态、Tauri、TypeScript 和 Svelte。
- 实现步骤：增加 Ready/RecoveryRequired/Unavailable 启动结果；分别读取并认证 main/backup；main Ready 优先；只有两份都 NotFound 才执行原有初始化；其余组合不写盘并返回对应状态。
- 接口、数据与错误契约：RecoveryRequired 只表明存在已认证 backup，不向上返回备份文档或字节；Unavailable 不携带路径、密码或内部错误；Ready 继续携带 `OpenedVault`。
- 边界与异常：backup 缺失与 backup 存在但无效必须区分；main 读取 I/O 失败不能被当成未初始化；有效 main 不要求读取 backup。
- 测试要求：覆盖 main 有效、两份缺失、main 损坏+backup 有效、main 缺失+backup 有效、两者损坏、main 损坏+backup 缺失，并断言非 Ready 状态不写 main/backup。
- 验证命令：`cargo test -p vault-storage initialization`；`cargo check -p vault-storage`；`cargo fmt --all -- --check`。
- 预期结果：任何损坏或认证失败痕迹都不会触发空库初始化或自动恢复。
- 完成判定：组合测试通过，旧自动恢复函数和测试被当前状态契约替代。
- 交付给下一原子的输出：调用方可以识别 RecoveryRequired，但尚不能执行恢复。
- 停止或重新规划条件：现有文件错误无法区分 NotFound 与其他失败，或检测必须修改真实用户文件。
- 风险等级：L3。
- DDD 门禁：实现前隔离审查“任何损坏或不可认证痕迹都不会进入空库初始化或写盘”。
- 计划提交信息：`feat(storage): [LPV-13-01] 判定主库与备份启动状态`。

### 执行记录

- 实际验证：隔离反证定位到旧自动恢复路径并裁决为 FIX_CURRENT；修正后 10 项 `vault-storage` 初始化测试通过，覆盖有效 main、全新初始化、损坏/缺失 main + 有效 backup、双损坏和无 backup，非 Ready 路径写调用为 0；Crate check、格式检查和 diff 检查通过。

## LPV-13-02 显式恢复已认证备份

- 状态：done
- 支持的验收场景：用户确认后从有效 backup 恢复缺失或损坏 main；失败保留 backup 和不可编辑现场。
- 唯一目标：提供一个只在显式调用时执行的安全恢复存储用例。
- 当前行为与目标行为：LPV-13-01 只能探测；完成后恢复会重新读取并认证 backup，通过同目录唯一 temp 发布 main，再重新读取认证后返回文档。
- 前置条件与依赖：LPV-13-01 启动状态；Windows 已有唯一 temp、同步写和 ReplaceFileW 边界。
- 代码定位依据：`crates/vault-storage/src/initialization.rs`、`file_access.rs`；`crates/vault-platform-windows/src/atomic_file.rs` 的 `OwnedTemp`、`replace_main`、`restore_main` 与隔离测试。
- 允许修改：恢复存储用例、`VaultFiles` 恢复契约、Windows 恢复发布和直接测试、计划状态。
- 明确不修改：应用/Tauri/前端、固定 backup 内容、自动调用恢复和任意外部路径。
- 实现步骤：重新读取并认证 backup；Windows 把候选复制到本次拥有的唯一 temp、同步并核对完整字节；main 缺失时不覆盖发布，main 存在时用 ReplaceFileW 发布并把损坏 main 留在唯一现场文件；成功后重读 main 并认证；任一步失败均返回错误。
- 接口、数据与错误契约：`recover_from_backup` 不接受路径；成功返回 `OpenedVault`；失败返回现有安全存储错误；backup 始终只读且不作为移动源。
- 边界与异常：main 在探测后可能变化，发布时重新判断；恢复失败不初始化空库；发布后重验失败也不报告成功；日志不包含路径或内容。
- 测试要求：内存层覆盖 backup 重验、发布失败和回读失败；Windows 隔离目录覆盖 main 缺失/存在恢复、backup 字节不变、损坏 backup 不发布及失败现场。
- 验证命令：相关 storage recovery 测试；`cargo test -p vault-platform-windows atomic_file`；两个 Crate check；格式检查。
- 预期结果：未经认证或未经 temp 核对的内容无法发布，且唯一有效 backup 不被消耗。
- 完成判定：缺失/损坏 main 均可从隔离有效 backup 恢复，失败断言证明 backup 原样保留。
- 交付给下一原子的输出：应用层可以在用户确认后调用固定恢复用例。
- 停止或重新规划条件：恢复发布会覆盖或移动固定 backup，或无法区分发布是否已经提交。
- 风险等级：L3。
- DDD 门禁：实现前隔离审查“恢复所有失败路径都不移动、删除或覆盖固定 backup”。
- 计划提交信息：`feat(storage): [LPV-13-02] 显式恢复已认证备份`。

### 执行记录

- 实际验证：DDD 隔离审查 PASS；4 项存储恢复测试通过，覆盖成功、备份无效、发布失败和发布后回读失败；13 项 Windows 原子文件测试通过，覆盖 main 缺失/存在、固定 backup 字节不变、损坏 main 现场保留和无效候选不写入；两个 Crate check、格式和 diff 检查通过。

## LPV-13-03 暴露恢复状态与确认命令

- 状态：done
- 支持的验收场景：应用启动后能返回 Ready/RecoveryRequired/Unavailable；只有恢复确认 command 能从待恢复转为 Ready。
- 唯一目标：建立 Rust 应用状态、Tauri command 与 TypeScript 的恢复协议。
- 当前行为与目标行为：应用和 IPC 只能表达 Ready 快照；完成后启动失败不会退出进程，所有非 Ready 业务操作被拒绝，显式恢复成功才发布快照。
- 前置条件与依赖：LPV-13-01、LPV-13-02。
- 代码定位依据：`crates/vault-app/src/lib.rs` 的状态与用例；`src-tauri/src/lib.rs` setup；`src-tauri/src/commands/vault.rs`；`frontend/src/lib/tauri/commands.ts`。
- 允许修改：应用状态机及测试、Windows repository 组合、Tauri DTO/command 及测试、TS 类型和调用、计划状态。
- 明确不修改：Svelte 展示、文件路径参数、自动恢复、跨电脑行为和其他业务规则。
- 实现步骤：应用状态支持 Ready/RecoveryRequired/Unavailable；快照接口返回判别状态；所有既有读写用例只允许 Ready；repository 增加固定恢复能力；新增无参数 `recover_from_backup` command；恢复成功替换确认状态，失败保持 RecoveryRequired；setup 始终管理应用状态而不因库不可用退出。
- 接口、数据与错误契约：前端启动响应使用判别联合；只有 Ready 携带安全 VaultStatus；恢复 command 无路径和备份内容参数；稳定错误不泄露本地信息。
- 边界与异常：重复恢复调用串行；Unavailable 禁止恢复；取消等同于不调用 command；恢复期间没有其他写入口。
- 测试要求：应用状态转换、失败保持待恢复、非 Ready 拒绝读写；Tauri DTO 字段安全和错误映射；TypeScript 静态检查。
- 验证命令：`cargo test -p vault-app recovery`；Tauri command 相关测试；受影响 Crate check；`npm run check`；格式检查。
- 预期结果：前端无需接触路径或备份内容即可可靠驱动恢复。
- 完成判定：协议测试与类型检查通过，Tauri Debug 增量编译成功。
- 交付给下一原子的输出：Svelte 可显示并操作真实恢复状态。
- 停止或重新规划条件：恢复必须由前端传递任意路径/明文文档，或非 Ready 状态仍能调用写用例。
- 风险等级：L3。
- DDD 门禁：实现前隔离审查“没有显式 recover command 时 RecoveryRequired 状态绝不写 main/backup”。
- 计划提交信息：`feat(app): [LPV-13-03] 暴露备份恢复命令`。

### 执行记录

- 实际验证：DDD 提交前隔离审查 PASS；26 项 `vault-app` 测试全部通过，其中 4 项覆盖待恢复只读、显式成功、失败保持和不可用拒绝；8 项 Tauri command/DTO 测试通过；受影响 Crate check、`svelte-check`（0 error、0 warning）、格式和 diff 检查通过；Tauri Debug 增量编译后重新运行成功。

## LPV-13-04 显示恢复确认与不可编辑状态

- 状态：pending
- 支持的验收场景：待恢复时显示确认；取消不写盘；确认显示恢复中并成功进入主界面；失败和两份无效时保持不可编辑。
- 唯一目标：Svelte 根据真实启动状态完成最小备份恢复交互。
- 当前行为与目标行为：当前只有通用读取失败；完成后 RecoveryRequired 与 Unavailable 使用专用工作区状态且隐藏/禁用所有编辑入口。
- 前置条件与依赖：LPV-13-03 IPC 判别联合和恢复 command。
- 代码定位依据：`frontend/src/app/App.svelte` 的 `refreshVault`、顶层 status、错误工作区和现有按钮样式。
- 允许修改：该 Svelte 组件及必要局部样式、计划状态。
- 明确不修改：整体界面重做、文件选择器、导入、自动静默恢复、发布构建和真实密码库故障注入。
- 实现步骤：加载判别状态；RecoveryRequired 展示说明与“恢复上一版”按钮；取消只保持当前不可编辑提示；恢复时禁重复；成功接收 Ready 快照并进入主界面；失败显示安全错误并保持恢复入口；Unavailable 只显示不可编辑说明。
- 接口、数据与错误契约：界面不显示路径、密钥、密码或内部诊断；主业务 UI 只在 Ready 渲染。
- 边界与异常：刷新/重启重新探测；取消不调用后端；恢复失败不显示成功通知；两份无效时不出现初始化或恢复按钮。
- 测试要求：`svelte-check`；运行中 Tauri Debug HMR；后端隔离矩阵作为真实数据不变量证据；不对正式 AppData 注入故障。
- 验证命令：`npm run check`；`cargo test --workspace`；运行中的 Tauri Debug 增量冒烟。
- 预期结果：用户能明确决定是否恢复，并且任何非 Ready 状态都无法编辑密码库。
- 完成判定：前端检查、Workspace 回归和 Debug 编译通过，可交给用户验证正常界面；故障路径由隔离自动测试证明。
- 交付给下一原子的输出：LPV-13 与 MVP 功能范围全部完成，后续进入用户另行授权的整体测试或 Release 阶段。
- 停止或重新规划条件：必须操作正式密码库才能验证，或恢复界面会在未确认时调用写 command。
- 风险等级：L1。
- DDD 门禁：不适用；高风险写入已由前三个原子隔离验证。
- 计划提交信息：`feat(frontend): [LPV-13-04] 增加备份恢复流程`。

## 变更记录

| 修订版本 | 变化 | 原因 |
|---|---|---|
| 1 | 初始计划 | 按当前代码把启动探测、显式恢复、IPC 和界面拆为四个可验证原子 |
