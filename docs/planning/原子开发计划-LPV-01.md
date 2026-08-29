# LPV-01 首次启动并进入加密空库原子开发计划

## 计划元数据

- 计划 ID：ATOMIC-LPV-01-001
- 类型：atomic-development
- 修订版本：2
- 状态：active
- 父级 ID：LPV-01
- 创建基线：9a19d6e67c21afe49a8fcfdd10a829468877cd7b

## 总体计划

- 产品结果：第一次启动 App 时创建真实加密空库并进入主界面；再次启动时读取同一空库。
- 当前执行边界：连续完成 Rust 后端和 Tauri command；开始整体前端主界面前停止，等待项目所有者提供参考图。
- 验收场景：在隔离数据目录首次初始化得到加密 `vault.dat`；文件中没有明文 JSON；重新创建应用并加载时得到同一个空库状态；后续前端根据参考图展示分类区、账号区、搜索入口和空状态。
- 明确排除：分类和账号增删改查、搜索、剪贴板、备份恢复、Release 构建、性能优化，以及未经参考图确认的整体界面设计。
- 当前代码证据：Rust Workspace 已有 `vault-core`、`vault-storage`、`vault-app`、`vault-platform-windows` 与 `src-tauri`；当前仅有技术实验 command，没有密码库业务实现。
- 依赖版本：`aes-gcm 0.11.1`、`zeroize 1.9.0`、`uuid 1.26.0`、`thiserror 2.0.20`、`tempfile 3.27.0`；Windows API 使用已固定的 `windows 0.62.2` 精确 features。
- 执行模式：连续执行 ATOM-01 至 ATOM-07；ATOM-08 在修改整体前端前停止。
- Git 策略：每个原子验证通过后独立中文提交，标题包含原子 ID；按仓库规则推送当前 `origin/master`。

## 跨原子约束

- 新增说明性注释使用中文，重点解释公共契约、加密、资源所有权、失败状态和不可见副作用；不为直观代码堆叠重复注释。
- 不接触用户正式密码库；所有自动验证只使用内存数据或 `tempfile` 创建的隔离目录。
- 普通日志不得包含账号字段、数据密钥、明文 JSON、DPAPI blob 或密码库内容。
- `vault-core` 不依赖 Tauri、存储或操作系统；`vault-storage` 通过消费方接口使用密钥保护和文件边界；Windows API 只出现在 `vault-platform-windows`。
- 前端不直接读写文件；Tauri command 只负责状态转换和安全 DTO。
- 日常验证遵守编译策略，不执行 Release，不清空 `target`。

## 原子顺序

| 顺序 | 原子 | 依赖 | 风险 | 状态 |
|---|---|---|---|---|
| 1 | LPV-01-01 定义空密码库文档 | 无 | L1 | done |
| 2 | LPV-01-02 加密和认证空密码库信封 | LPV-01-01 | L3 | in_progress |
| 3 | LPV-01-03 使用 DPAPI 保护数据密钥 | LPV-01-02 | L3 | pending |
| 4 | LPV-01-04 首次原子发布和读取主库 | LPV-01-02 | L3 | pending |
| 5 | LPV-01-05 初始化或加载空密码库 | LPV-01-03、LPV-01-04 | L3 | pending |
| 6 | LPV-01-06 持有 Windows 单实例锁 | LPV-01-04 | L3 | pending |
| 7 | LPV-01-07 暴露启动状态 command | LPV-01-05、LPV-01-06 | L2 | pending |
| 8 | LPV-01-08 按参考图实现整体主界面 | LPV-01-07、项目所有者参考图 | L1 | pending |

## LPV-01-01 定义空密码库文档

- 状态：done
- 支持的验收场景：首次启动得到结构有效的空密码库。
- 唯一目标：`vault-core` 能构造、序列化、反序列化并校验 schema version 为 1、revision 为 0、分类和账号均为空的文档。
- 当前行为与目标行为：当前 crate 为空；完成后存在稳定的 `VaultDocument`、`Category` 和 `Account` 数据结构以及文档校验错误。
- 前置条件与依赖：无代码原子依赖。
- 代码定位依据：`crates/vault-core/src/lib.rs` 当前只有模块说明；在该 crate 内按模型与校验职责分模块。
- 允许修改：`crates/vault-core` 的 manifest、源码和单元测试；本原子状态记录。
- 明确不修改：加密、DPAPI、文件、Tauri command 和前端。
- 实现步骤：增加版本常量和完整数据结构；提供 `empty()` 与 `validate()`；使用 serde 保持稳定字段名；为未来记录 ID 使用 UUID，但不实现 CRUD。
- 接口、数据与错误契约：schema version 只接受 1；非空账号的分类引用必须存在；账号名和密码不得全为空白；错误不得携带字段内容。
- 边界与异常：本原子只验证内存文档，不生成密钥或文件。
- 测试要求：空文档默认值；JSON 往返；错误 schema 被拒绝；悬空分类引用被拒绝；空白账号名或密码被拒绝。
- 验证命令：`cargo test -p vault-core`；`cargo check -p vault-core`；`cargo fmt --all -- --check`。
- 预期结果：相关测试全部通过，crate 不依赖 Tauri 或 Windows。
- 完成判定：空文档及不变量有可重复单元测试，仓库仍可编译。
- 交付给下一原子的输出：可安全序列化并在解密后重新校验的 `VaultDocument`。
- 停止或重新规划条件：现有产品需求与架构中的字段或 schema 规则冲突。
- 风险等级：L1。
- DDD 门禁：不触发，直接测试可以证明行为。
- 计划提交信息：`feat(core): [LPV-01-01] 定义空密码库文档`。

## LPV-01-02 加密和认证空密码库信封

- 状态：in_progress
- 支持的验收场景：磁盘候选数据不包含明文 JSON，篡改后无法加载。
- 唯一目标：`vault-storage` 能把经过校验的 `VaultDocument` 编码为版本化 AES-256-GCM 信封，并从信封认证解码。
- 当前行为与目标行为：当前 crate 为空；完成后提供内存中的 `seal` 和 `open`，但不调用 DPAPI 或文件系统。
- 前置条件与依赖：LPV-01-01。
- 代码定位依据：`crates/vault-storage/src/lib.rs`；依赖 `vault-core`。
- 允许修改：`vault-storage` manifest、信封源码、错误类型和单元测试；本原子状态记录。
- 明确不修改：Windows DPAPI、文件发布、应用状态、Tauri 和前端。
- 实现步骤：定义有界二进制信封；使用随机 32 字节数据密钥和 12 字节 nonce；采用 AES-256-GCM detached tag；将从 magic 起至 wrapped key 末尾的原始规范头字节作为 AAD；认证后才反序列化和校验文档；敏感密钥使用 `zeroize`。
- 接口、数据与错误契约：magic、format version、header length、wrapped key length、ciphertext length、wrapped key、nonce、tag 和 ciphertext 均有明确上限；seal/open 必须使用完全相同的原始头字节作为 AAD，任何影响解析或密码学语义的新增持久化元数据也必须纳入 AAD；对外只返回稳定错误类别，不包含明文或密钥。
- 边界与异常：文件总大小上限 64 MiB；nonce 每次封装重新生成；错误长度、版本或认证失败均拒绝。
- 测试要求：空库往返；相同文档两次封装不同；ciphertext、nonce、tag 和长度篡改被拒绝；信封中不出现明文 JSON 字段。
- 验证命令：`cargo test -p vault-storage`；`cargo check -p vault-storage`；格式检查。
- 预期结果：内存往返及篡改测试通过，没有操作真实文件。
- 完成判定：加密信封可以独立验证且错误信息安全。
- 交付给下一原子的输出：需要平台包装的 32 字节数据密钥和可持久化信封。
- 停止或重新规划条件：`aes-gcm 0.11.1` API 或当前信封布局无法满足 detached tag 与有界解析。
- 风险等级：L3。
- DDD 门禁：实现前只审“当前信封布局能在不泄露明文的情况下拒绝可达篡改”这一声明；提交前以测试和 diff 复核。
- 计划提交信息：`feat(storage): [LPV-01-02] 增加认证加密信封`。

## LPV-01-03 使用 DPAPI 保护数据密钥

- 状态：pending
- 支持的验收场景：数据密钥不以明文存盘，同一 Windows 用户可重新解包。
- 唯一目标：`vault-platform-windows` 使用 CurrentUser DPAPI 包装和解包恰好 32 字节的数据密钥。
- 当前行为与目标行为：当前 `dpapi.rs` 只有模块说明；完成后实现 `vault-storage` 定义的密钥保护接口。
- 前置条件与依赖：LPV-01-02 定义密钥保护接口。
- 代码定位依据：`crates/vault-platform-windows/src/dpapi.rs` 和已固定的 `windows 0.62.2` features。
- 允许修改：Windows 平台 crate 的 DPAPI 模块、所需精确 features、隔离集成测试；本原子状态记录。
- 明确不修改：加密算法、文件发布、Tauri、前端和 macOS。
- 实现步骤：调用 `CryptProtectData` / `CryptUnprotectData`；只用 `CRYPTPROTECT_UI_FORBIDDEN`；使用 RAII 管理 `LocalFree`；明文输出在释放前调用 `SecureZeroMemory`；校验解包长度为 32。
- 接口、数据与错误契约：错误只返回安全分类和内部 Win32 code；不记录输入或输出 blob；不启用 LocalMachine。
- 边界与异常：测试仅使用随机合成密钥；非 32 字节解包结果被拒绝。
- 测试要求：同用户往返；两次包装不要求字节相同但都能解包；篡改 blob 被拒绝；错误不包含密钥内容。
- 验证命令：`cargo test -p vault-platform-windows dpapi`；`cargo check -p vault-platform-windows`；格式检查。
- 预期结果：真实 DPAPI 往返在当前 Windows 用户下通过。
- 完成判定：资源释放和长度校验由代码与测试覆盖。
- 交付给下一原子的输出：可注入信封编码器的 Windows 数据密钥保护器。
- 停止或重新规划条件：windows-rs 0.62 API 与已确认文档不一致，或测试环境无法调用 DPAPI。
- 风险等级：L3。
- DDD 门禁：实现前只审“所有 DPAPI 分配的明文缓冲在退出路径上先清零再释放”这一声明。
- 计划提交信息：`feat(windows): [LPV-01-03] 使用 DPAPI 保护数据密钥`。

## LPV-01-04 首次原子发布和读取主库

- 状态：pending
- 支持的验收场景：首次保存不会覆盖已有主库，失败不会产生半写 main。
- 唯一目标：Windows 文件实现能在指定隔离目录把完整候选字节首次发布为 `vault.dat`，并重新读取相同字节。
- 当前行为与目标行为：当前 `atomic_file.rs` 只有模块说明；完成后实现 `vault-storage` 定义的首次文件接口。
- 前置条件与依赖：LPV-01-02 定义文件接口。
- 代码定位依据：`crates/vault-platform-windows/src/atomic_file.rs`；官方 `MoveFileExW` 方案已记录。
- 允许修改：Windows 文件模块、精确 feature、临时目录集成测试；本原子状态记录。
- 明确不修改：后续 ReplaceFileW、backup、恢复、正式用户目录、Tauri 和前端。
- 实现步骤：同目录 `create_new` 唯一 temp；写入并 `sync_all`；使用不带覆盖标志的 `MoveFileExW` 发布；读取 main；失败时清理本次可确认拥有的 temp。
- 接口、数据与错误契约：主文件存在时返回 AlreadyExists，不覆盖；所有路径由调用方显式传入；错误不包含文件内容。
- 边界与异常：只允许当前存储实例生成的 temp 名；测试目录由 `tempfile` 创建。
- 测试要求：首次发布/读取往返；已有 main 时拒绝并保持旧字节；空候选被拒绝；失败后 main 不出现半写内容。
- 验证命令：`cargo test -p vault-platform-windows atomic_file`；目标 crate check；格式检查。
- 预期结果：隔离目录中的真实 Windows 文件测试通过。
- 完成判定：首次发布行为可独立重复验证且不覆盖现有 main。
- 交付给下一原子的输出：空库初始化可使用的 Windows 主库文件边界。
- 停止或重新规划条件：API 需要跨卷移动，或无法保证 temp 与 main 同目录。
- 风险等级：L3。
- DDD 门禁：实现前只审“首次发布失败不会覆盖调用前已存在的 main”这一声明。
- 计划提交信息：`feat(windows): [LPV-01-04] 支持首次原子发布主库`。

## LPV-01-05 初始化或加载空密码库

- 状态：pending
- 支持的验收场景：首次启动创建空库，第二次启动解密加载同一空库。
- 唯一目标：`vault-storage` 用一个存储入口在 main 不存在时初始化空库，在 main 存在时认证加载。
- 当前行为与目标行为：信封与平台接口彼此独立；完成后形成 `open_or_initialize` 用例。
- 前置条件与依赖：LPV-01-02、LPV-01-03、LPV-01-04。
- 代码定位依据：`vault-storage` 的信封、密钥保护和文件接口；Windows 实现由组合层注入。
- 允许修改：`vault-storage` 组合逻辑、测试 fake、Windows 隔离集成测试；本原子状态记录。
- 明确不修改：备份恢复、后续替换、应用 UI、分类和账号写操作。
- 实现步骤：先读 main；不存在时生成 `VaultDocument::empty()`、封装并首次发布；发布成功后重新读取、认证和校验；存在时直接认证加载。
- 接口、数据与错误契约：返回已确认 `VaultDocument` 和初始化/已加载状态；任何失败都不返回未确认快照。
- 边界与异常：并发创建由 AlreadyExists 返回后重新读取 main；损坏 main 返回不可用错误，不创建新空库覆盖。
- 测试要求：首次初始化；第二次加载；信封不含明文；损坏 main 被拒绝；已有 main 不被覆盖；隔离 Windows DPAPI + 文件全链路。
- 验证命令：`cargo test -p vault-storage`；Windows 相关集成测试；目标 crate checks；格式检查。
- 预期结果：隔离目录真实初始化和重开通过。
- 完成判定：AC-09 的空库持久化后端路径成立。
- 交付给下一原子的输出：应用层可调用的已确认空库加载结果。
- 停止或重新规划条件：并发首次创建不能在现有接口内安全处理，或损坏 main 会误判为从未初始化。
- 风险等级：L3。
- DDD 门禁：实现前只审“损坏或不可认证的现有 main 永远不会触发新空库覆盖”这一声明。
- 计划提交信息：`feat(storage): [LPV-01-05] 初始化或加载空密码库`。

## LPV-01-06 持有 Windows 单实例锁

- 状态：pending
- 支持的验收场景：第二实例不会同时读取或写入同一密码库。
- 唯一目标：Windows 平台在指定数据目录获取并持有 `vault.lock` 独占锁，直到 guard 释放。
- 当前行为与目标行为：`process_lock.rs` 只有模块说明；完成后同目录第二次获取明确返回 Busy。
- 前置条件与依赖：LPV-01-04 已确定数据目录文件边界。
- 代码定位依据：`crates/vault-platform-windows/src/process_lock.rs` 和 `Win32_Storage_FileSystem`。
- 允许修改：Windows 锁模块、隔离测试、本原子状态记录。
- 明确不修改：进程发现、强制结束第二实例、Tauri UI 和密码库内容。
- 实现步骤：打开或创建锁文件；使用 Windows 独占文件锁；guard Drop 解锁并关闭句柄；Busy 与系统错误分开映射。
- 接口、数据与错误契约：锁 guard 不可克隆；锁文件内容无业务数据；第二获取只返回 Busy。
- 边界与异常：测试只在 `tempfile` 目录；崩溃后的 OS 句柄释放由 Windows 保证。
- 测试要求：第一次成功；guard 存活时第二次 Busy；释放后可重新获取。
- 验证命令：`cargo test -p vault-platform-windows process_lock`；目标 crate check；格式检查。
- 预期结果：锁生命周期测试通过。
- 完成判定：同一数据根只有一个活跃 guard。
- 交付给下一原子的输出：组合层可持有的单实例 guard。
- 停止或重新规划条件：当前 API 无法在 Drop 中可靠解锁或 Busy 无法区分。
- 风险等级：L3。
- DDD 门禁：实现前只审“guard 存活期间第二个调用者无法获得同一锁”这一声明。
- 计划提交信息：`feat(windows): [LPV-01-06] 增加密码库单实例锁`。

## LPV-01-07 暴露启动状态 command

- 状态：pending
- 支持的验收场景：前端可以调用后端获得空库状态，不直接接触文件或密钥。
- 唯一目标：`vault-app` 持有锁和已确认空库快照，Tauri 暴露 `load_vault` 返回安全启动 DTO。
- 当前行为与目标行为：当前只有实验 command；完成后组合真实 Windows DPAPI、文件实现和应用状态，并注册 `load_vault`。
- 前置条件与依赖：LPV-01-05、LPV-01-06。
- 代码定位依据：`crates/vault-app`、`src-tauri/src/lib.rs`、`src-tauri/src/commands/`。
- 允许修改：应用 crate、Tauri 组合与 command、command/应用测试、本原子状态记录。
- 明确不修改：整体 Svelte 主界面、分类和账号 command、正式数据文件测试、Release。
- 实现步骤：定义只读启动摘要；应用构造时先持锁再初始化/加载；Tauri state 持有应用；`load_vault` 只转换安全 DTO 和稳定错误。
- 接口、数据与错误契约：DTO 只包含 schemaVersion、revision、categoryCount、accountCount 和 initialized；不包含密钥、路径、wrapped key 或密码字段。
- 边界与异常：组合失败时返回可理解安全错误；不能降级为空内存假数据。
- 测试要求：应用 fake 的初始化/加载摘要；DTO 字段；错误不泄露路径或内容；Tauri command 可编译。
- 验证命令：`cargo test -p vault-app`；`cargo test -p local-password-vault`；相关 checks；格式检查。
- 预期结果：前端调用契约已存在并由测试证明，但不开始整体 UI。
- 完成判定：真实后端依赖已在 Tauri 组合层连通，整体前端可只消费 `load_vault`。
- 交付给下一原子的输出：整体主界面需要的安全启动状态接口。
- 停止或重新规划条件：Tauri 生命周期无法安全持有锁 guard，或 command 必须暴露敏感内部状态。
- 风险等级：L2。
- DDD 门禁：若测试无法证明锁和状态的生命周期所有权，提交前对该单一声明做隔离审查。
- 计划提交信息：`feat(app): [LPV-01-07] 暴露空库启动状态`。

## LPV-01-08 按参考图实现整体主界面

- 状态：pending
- 支持的验收场景：用户启动后直接看到分类区、账号列表、详情、搜索入口和空状态。
- 唯一目标：依据项目所有者提供的参考图实现 LPV-01 所需整体主界面，并调用 `load_vault` 展示真实空状态。
- 当前行为与目标行为：当前是技术实验页面；目标布局和视觉风格等待参考图确定。
- 前置条件与依赖：LPV-01-07；项目所有者提供参考图。
- 代码定位依据：`frontend/src/app/App.svelte`、`frontend/src/lib/tauri/commands.ts`、`frontend/src/styles/`。
- 允许修改：参考图确认后的前端组件、样式、调用封装和前端测试；本原子状态记录。
- 明确不修改：未确认的整体视觉设计、分类和账号真实操作、Release。
- 实现步骤：收到参考图后重新确认界面区域和视觉规则，再拆组件并接入真实空库状态。
- 接口、数据与错误契约：只消费 LPV-01-07 的安全启动 DTO；加载失败展示明确错误，不伪装为空库。
- 边界与异常：当前必须在写整体界面之前停止，不使用现有技术实验页作为最终设计依据。
- 测试要求：前端类型检查；空状态和失败状态；Tauri Debug 真实路径人工验证。
- 验证命令：收到参考图后确定；不执行 Release。
- 预期结果：依据参考图完成可直接人工验证的主界面空状态。
- 完成判定：项目所有者按参考图验收通过后，LPV-01 才能标记 done。
- 交付给下一原子的输出：LPV-02 可复用的分类侧边栏入口。
- 停止或重新规划条件：参考图未提供或与已确认产品区域冲突。
- 风险等级：L1。
- DDD 门禁：不触发。
- 计划提交信息：`feat(frontend): [LPV-01-08] 实现主界面空状态`。

## 整体回归

- 后端每个原子使用目标 crate 的 `cargo check` 和相关测试。
- LPV-01-07 后运行 `cargo test --workspace`、前端类型检查和 Tauri Debug command 冒烟；不运行 Release。
- LPV-01-08 收到参考图后再执行整体前端人工验收。

## 计划变更记录

| 修订版本 | 变化 | 原因 |
|---|---|---|
| 1 | 初始计划 | 首次拆分 LPV-01，并按用户要求在整体前端前设置停止点 |
| 2 | 明确加密信封 AAD 边界 | DDD 反证指出仅靠结构校验不能证明头部元数据与密文绑定 |

## 执行记录

### LPV-01-01

- 完成结果：定义 `VaultDocument`、`Category`、`Account` 和稳定校验错误；空库固定为 schema version 1、revision 0。
- 不变量证据：统一校验结构版本、ID 唯一性、分类名称、账号必填字段和分类引用；错误不携带用户字段内容。
- 验证证据：`cargo test -p vault-core`（7 项通过）、`cargo check -p vault-core`、`cargo fmt --all -- --check` 和 `git diff --check` 均通过。
- 范围确认：未修改加密、平台文件、Tauri command 或前端。
