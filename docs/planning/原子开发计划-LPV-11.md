# LPV-11 查看和复制账号凭据原子开发计划

## 计划元数据

- 计划 ID：ATOMIC-LPV-11-001
- 类型：atomic-development
- 修订版本：1
- 状态：active
- 父级 ID：LPV-11
- 创建基线：c757d8f

## 总体计划

- 产品结果：用户可从账号列表临时显示或隐藏单条密码，并按稳定账号 ID 复制账号名或密码。
- 验收场景：密码默认隐藏；显示只返回当前账号密码；复制账号名和密码可粘贴正确内容；密码写入排除 Windows 历史与云同步；30 秒后仅在剪贴板仍属于本次写入时清除；行内按钮不打开编辑弹框。
- 明确排除：自动填充、复制地址或邮箱、剪贴板历史管理、崩溃后的绝对清除保证、macOS 实现、使用统计和 Release 构建。
- 已确认架构：`VaultApplication` 持有已确认明文文档；Tauri command 负责按 ID 调用用例与平台边界；Windows 平台能力位于 `crates/vault-platform-windows`；列表摘要和搜索结果不含密码；正式前端入口位于 `frontend/src/app/App.svelte`。
- 全局安全约束：密码不得进入列表摘要、搜索结果、日志、通知文字或持久前端状态；复制 command 不返回明文；条件清理必须在同一次剪贴板锁内比较 sequence、token 和文本；测试不得读写用户真实密码库或真实剪贴板。
- 执行模式：连续执行；每个原子独立验证、提交并同步 `origin/master`，全部完成后停下交由项目所有者验收。
- 整体回归：目标 Crate 检查与测试、`cargo test --workspace`、`npm run check`、Tauri Debug 增量启动和真实窗口人工冒烟；不执行 Release 构建。

## 原子顺序

1. `LPV-11-01`：应用层按稳定账号 ID 读取单项凭据。
2. `LPV-11-02`：Windows 安全写入密码并条件清理剪贴板。
3. `LPV-11-03`：Tauri 暴露显示与复制凭据 command。
4. `LPV-11-04`：Svelte 账号列表接入显示、隐藏和复制操作。

## LPV-11-01 应用层按 ID 读取单项凭据

- 状态：done
- 支持的验收场景：显示或复制动作只取得指定账号的当前字段。
- 唯一目标：`VaultApplication` 能按稳定账号 ID 返回账号名或密码，目标不存在或 ID 非法时返回既有账号不存在错误。
- 当前行为与目标行为：当前只有编辑详情一次返回全部字段；完成后凭据操作可请求单一字段，不必复用完整编辑 DTO。
- 前置条件与依赖：LPV-05、LPV-07 已完成；应用层已有确认快照互斥锁和账号不存在错误。
- 代码定位依据：`crates/vault-app/src/lib.rs` 中 `account_for_edit`、`VaultApplicationError` 及合成文档测试夹具。
- 允许修改：`crates/vault-app/src/lib.rs` 及其同文件测试。
- 明确不修改：Tauri command、Windows 剪贴板、前端、磁盘存储和账号摘要结构。
- 实现步骤：增加最小凭据字段枚举或两个窄读取方法；在已确认状态锁内解析 ID、定位账号并只克隆所需字段；沿用安全错误映射。
- 接口、数据与错误契约：成功只返回请求字段；失败为 `AccountNotFound`；不改变 revision，不调用仓储。
- 边界与异常：非法 UUID、已删除账号均视为不存在；不记录 ID 对应的内容。
- 测试要求：合成账号分别读取账号名和密码；非法/未知 ID 失败；读取不保存、不增加 revision。
- 验证命令：`cargo test -p vault-app credential`；`cargo check -p vault-app`。
- 预期结果：窄测试和检查通过，现有编辑接口保持不变。
- 完成判定：后续 command 无需加载完整编辑 DTO 即可取得单项凭据。
- 交付给下一原子的输出：稳定的应用层凭据读取接口。
- 停止或重新规划条件：现有状态所有权无法保证读取来自确认快照。
- 风险等级：L2
- DDD 门禁：只有无法由窄测试证明“不返回额外字段”时执行隔离质疑。
- 计划提交信息：`feat(credentials): [LPV-11-01] 支持按账号 ID 读取单项凭据`

### 执行记录

- 实际验证：`cargo test -p vault-app credential` 通过（2 项）；`cargo check -p vault-app` 通过。

## LPV-11-02 Windows 安全写入和条件清理

- 状态：done
- 支持的验收场景：复制密码可粘贴，并在未被用户覆盖时于 30 秒后安全清理。
- 唯一目标：Windows 平台模块能写入普通文本或受保护密码，并使用写入凭证进行 compare-and-clear。
- 当前行为与目标行为：`clipboard.rs` 目前只有模块说明；完成后实现 `CF_UNICODETEXT`、应用 token、系统监控排除格式和条件清理。
- 前置条件与依赖：Windows API features 已固定；技术设计已明确 sequence、token、文本三重比较。
- 代码定位依据：`crates/vault-platform-windows/src/clipboard.rs`、`Cargo.toml`，以及 `docs/architecture/Windows平台能力方案.md` 第 4 节。
- 允许修改：Windows 平台 Crate 的剪贴板模块、必要依赖/feature 和纯逻辑测试。
- 明确不修改：应用层、Tauri、前端、DPAPI、文件替换；自动测试不得打开系统真实剪贴板。
- 实现步骤：用 RAII 管理 `OpenClipboard/CloseClipboard` 和未移交的 `HGLOBAL`；写 Unicode 文本；密码额外写唯一 token 与 `ExcludeClipboardContentFromMonitorProcessing`；记录 sequence；清理时持锁比较三项后再 `EmptyClipboard`。
- 接口、数据与错误契约：拒绝内嵌 NUL；平台错误不包含文本或 token；普通文本无定时清理凭证；密码成功返回仅供 Rust 持有的清理凭证。
- 边界与异常：任何比较、读取或打开失败均不清空；`SetClipboardData` 成功后不得释放已移交内存；用户后来复制内容时返回未清理。
- 测试要求：以不触碰系统剪贴板的纯比较/后端接缝验证三项都相等才清除，sequence、token、文本任一变化均不清除；覆盖内嵌 NUL 和 UTF-16 文本编码。
- 验证命令：`cargo test -p vault-platform-windows clipboard`；`cargo check -p vault-platform-windows`。
- 预期结果：Windows 模块与隔离测试通过，不输出敏感数据。
- 完成判定：平台 API 可被 command 调用，且所有权与失败分支明确。
- 交付给下一原子的输出：普通文本写入、密码写入及条件清理 API。
- 停止或重新规划条件：Windows API 实际契约不能支持同锁比较或历史排除格式。
- 风险等级：L3
- DDD 门禁：实现前必须对“不会清除用户后来复制内容”的单一声明执行隔离质疑。
- 计划提交信息：`feat(clipboard): [LPV-11-02] 实现 Windows 密码剪贴板条件清理`

### 执行记录

- 实际验证：DDD 隔离质疑结果为 PASS；`cargo test -p vault-platform-windows clipboard` 通过（3 项）；`cargo check -p vault-platform-windows` 通过；`cargo fmt --package vault-platform-windows -- --check` 通过。

## LPV-11-03 Tauri 凭据 command

- 状态：done
- 支持的验收场景：前端可显示密码，复制操作不接收明文响应。
- 唯一目标：注册显示密码、复制账号名和复制密码三个窄 command，并映射安全错误。
- 当前行为与目标行为：当前 Tauri 只有完整编辑详情 command；完成后列表操作使用独立 IPC。
- 前置条件与依赖：`LPV-11-01`、`LPV-11-02` 完成。
- 代码定位依据：`src-tauri/src/commands/vault.rs` 的现有 command/DTO/错误映射、`src-tauri/src/lib.rs` 的 handler 注册。
- 允许修改：`src-tauri` command、注册、必要依赖和 command 级安全形状测试。
- 明确不修改：前端、列表摘要、搜索响应、存储格式和 macOS 实现。
- 实现步骤：显示 command 只返回 `{ password }`；两个复制 command 按 ID 从应用层取单项字段并直接调用平台 API；密码成功后启动 30 秒延迟条件清理，失败不记录敏感值。
- 接口、数据与错误契约：复制成功返回空结果；账号不存在和剪贴板失败使用稳定错误码；所有 DTO 使用 camelCase；不把 token/sequence/文本返回前端。
- 边界与异常：清理线程失败静默保留用户当前剪贴板且不记录内容；command 不持有状态锁等待 30 秒。
- 测试要求：DTO 只含密码字段；错误映射稳定；handler 注册和 Windows Debug 编译通过。
- 验证命令：`cargo test -p local-password-vault commands::vault`；`cargo check -p local-password-vault`。
- 预期结果：IPC 契约可供前端调用且无列表边界扩张。
- 完成判定：三个 command 已注册并能在 Windows Debug 构建中调用后端能力。
- 交付给下一原子的输出：固定的前端 command 名称和响应类型。
- 停止或重新规划条件：平台能力需要把密码或清理凭证返回 WebView。
- 风险等级：L2
- DDD 门禁：若 DTO 或错误无法通过直接测试证明安全形状则执行隔离质疑。
- 计划提交信息：`feat(commands): [LPV-11-03] 暴露凭据显示与复制接口`

### 执行记录

- 实际验证：`cargo test -p local-password-vault commands::vault` 通过（6 项）；`cargo check -p local-password-vault` 通过；运行中的 `tauri dev` 完成增量重编译并重新启动真实窗口。

## LPV-11-04 Svelte 列表凭据操作

- 状态：pending
- 支持的验收场景：密码默认隐藏，可显示/隐藏并复制账号名或密码，按钮不打开编辑弹框。
- 唯一目标：正式账号列表接入三个真实 command，并维持单条、短生命周期的密码显示状态。
- 当前行为与目标行为：列表目前只显示“已保存”和“编辑”提示；完成后提供掩码、显示/隐藏、复制账号和复制密码按钮。
- 前置条件与依赖：`LPV-11-03` 完成，现有搜索和分类列表已验收。
- 代码定位依据：`frontend/src/app/App.svelte` 的账号行和通知状态、`frontend/src/lib/tauri/commands.ts`、`frontend/src/styles/global.css`。
- 允许修改：上述三个正式前端文件。
- 明确不修改：原型、账号编辑表单契约、摘要 DTO、排序/最近使用和整体布局。
- 实现步骤：封装三个 IPC；仅保存一个已显示账号的密码和请求序号；隐藏、切换账号/分类、删除时清空；行内按钮隔离 click/keydown 冒泡；复用通知展示成功或安全错误。
- 接口、数据与错误契约：列表/搜索结果仍不含密码；复制响应无明文；迟到显示响应不能恢复已隐藏或已切换的密码。
- 边界与异常：显示失败保持掩码；复制失败不显示内容；按钮禁用只作用于对应账号操作；行空白仍可打开编辑。
- 测试要求：前端类型检查；编译检查搜索结果与密码状态保持响应式；真实 Debug 窗口人工验证显示、隐藏、复制、事件隔离和搜索结果内操作。
- 验证命令：`npm run check`；`cargo test --workspace`；保持 `npm run tauri dev` 做真实窗口冒烟。
- 预期结果：用户可直接完成 LPV-11 验收，密码不进入常驻账号列表。
- 完成判定：自动检查通过、Debug 应用可运行并交由项目所有者测试。
- 交付给下一原子的输出：LPV-11 完整端到端能力。
- 停止或重新规划条件：前端必须扩张摘要 DTO 才能工作，或行内操作无法与编辑入口隔离。
- 风险等级：L2
- DDD 门禁：若异步隐藏状态不能由代码检查和冒烟充分证明，则执行隔离质疑。
- 计划提交信息：`feat(ui): [LPV-11-04] 接入凭据显示与复制操作`

## 变更记录

| 修订版本 | 变化 | 原因 |
|---|---|---|
| 1 | 初始原子计划 | 开始 LPV-11 端到端实现 |
