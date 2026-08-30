# LPV-15 设置界面原子开发计划

## 计划元数据

- 计划 ID：ATOMIC-LPV-15-001
- 类型：atomic-development
- 修订版本：1
- 状态：completed
- 父级 ID：LPV-15
- 创建基线：bdec50e1bf7a16c7bf8591c9050e5b601cb70af6

## 总体计划

- 产品交付单元：LPV-15 设置界面。
- 产品结果：侧边栏提供设置入口；用户可以立即切换字体类型和字号，重启后保持选择；同一页面可直接阅读当前真实项目架构、数据存储与加密算法原理和选型理由。
- 验收场景：账号管理与设置可互相切换；字体类型、字号分别变化且无需重启；偏好损坏或缺失时使用默认值；说明内容覆盖 Svelte/Tauri/Rust 职责、文件位置和作用、AES-256-GCM、DPAPI、原子保存、备份、锁及安全边界。
- 明确排除：主题、语言、快捷键、密码库目录修改、设置同步、新 Rust command、设置数据库、HTTP 服务和 macOS迁移说明。
- 已确认架构：正式界面位于 `frontend/src/app/App.svelte`；全局字体与字号位于 `frontend/src/styles/global.css`；Tauri 组装位于 `src-tauri/src/lib.rs`；加密信封位于 `crates/vault-storage/src/envelope.rs`；Windows DPAPI、文件替换和锁位于 `crates/vault-platform-windows`。
- 数据不变量：字体偏好只含非敏感的预设 ID 和字号，保存在 WebView 本地偏好中，不进入 `VaultDocument`、不触发 Rust command、不增加密码库 revision；架构说明不得包含密码、密钥、真实数据目录绝对路径或未实现能力。
- 执行模式：连续执行；每个原子验证、提交、同步和汇报后自动进入下一原子。
- Git 策略：按仓库规则每个原子独立提交并推送当前 `master` 的上游 `origin/master`。
- 原子顺序：`LPV-15-01` → `LPV-15-02`。
- 整体回归：`npm run check`、Vite HMR、`git diff --check`；不执行 Release 构建和无关 Rust 全量编译。

## LPV-15-01 提供设置入口和字体偏好

- 状态：done
- 支持的验收场景：从侧边栏进入设置；字体类型和字号立即生效；再次启动恢复选择。
- 唯一目标：实现一个可持久化、可即时预览的全局字体偏好界面。
- 当前行为与目标行为：当前只有账号管理视图和固定字体栈/固定像素字号；完成后增加设置视图、字体预设、字号预设和本机恢复。
- 前置条件与依赖：现有 `App.svelte` 布局和 MiSans CDN 字体导入可用。
- 代码定位依据：`frontend/src/app/App.svelte` 的侧边栏、顶部栏和内容区；`frontend/src/styles/global.css` 的根字体和全部 `font-size`；新增职责明确的设置组件与字体偏好模块。
- 允许修改：上述前端入口和样式；新增 `frontend/src/app/settings/FontSettings.svelte`、`frontend/src/lib/preferences/font-preferences.ts`；本原子状态。
- 明确不修改：Rust、Tauri command、密码库文档、账号功能、架构说明正文和其他设置项。
- 实现步骤：增加账号/设置视图状态和侧边栏入口；定义默认、MiSans、Windows UI 等字体预设及标准字号预设；启动时容错读取本地偏好；切换时立即写入根 CSS 属性并保存；把现有字体像素值映射为 `rem` 字号令全局文字统一缩放；设置页只展示字体卡片和即时预览。
- 接口、数据与错误契约：本地键使用版本化固定名称；只接受白名单预设和有限字号；读取失败、未知值或存储不可用时回退默认值，仍允许当前会话即时应用。
- 边界与异常：字体文件不可用时由预设 fallback 字体继续显示；字号只改变文字，不缩放整个窗口或控件尺寸；返回账号管理时保留原分类状态。
- 测试要求：前端类型检查无错误警告；聚焦静态检查证明所有字体入口使用根变量且所有显式字号改为 `rem`；Vite HMR 成功。
- 验证命令：`npm run check`；字体偏好聚焦脚本；`git diff --check`；运行中 Tauri Debug HMR。
- 预期结果：不重启即可观察字体变化，刷新或重开时读取同一选择。
- 完成判定：前端检查通过，偏好读写边界可由代码和聚焦检查证明，设置入口可交给用户直接点击。
- 交付给下一原子的输出：可复用的设置页面布局和第二内容区接缝。
- 停止或重新规划条件：必须修改密码库格式或新增后端设置存储才能持久化偏好。
- 风险等级：L1。
- DDD 门禁：不适用；非敏感本地偏好且可由直接检查证明。
- 计划提交信息：`feat(frontend): [LPV-15-01] 增加字体设置`。

### 执行记录

- 实际验证：`svelte-check` 通过（0 error、0 warning）；聚焦检查确认 4 种字体预设、4 档字号、版本化本地存储键及全局字体变量链路完整，除可配置的根字号外所有全局显式文字字号均使用 `rem`；运行中的 Tauri Debug 收到 Svelte 和 CSS HMR 更新；`git diff --check` 通过。

## LPV-15-02 展示架构与加密原理

- 状态：done
- 支持的验收场景：用户在设置页内理解软件各层职责、数据落盘、加密组合、保存恢复和选择理由。
- 唯一目标：增加一份由当前真实代码支持的内置架构与数据保护说明。
- 当前行为与目标行为：当前原理只存在项目 Markdown 文档；完成后设置页以易读分区直接展示。
- 前置条件与依赖：LPV-15-01 已提供设置视图和内容布局。
- 代码定位依据：新增 `frontend/src/app/settings/ArchitectureInfo.svelte`；事实来源为 `src-tauri/src/lib.rs`、`crates/vault-storage/src/envelope.rs`、`crates/vault-platform-windows/src/{dpapi,atomic_file,process_lock}.rs` 和 `docs/architecture/架构设计.md`。
- 允许修改：架构说明组件、设置视图组合、局部样式、产品与计划完成状态。
- 明确不修改：Rust实现、Tauri command、密码库数据、字体行为、macOS迁移说明和外部文档跳转。
- 实现步骤：用“界面到磁盘”的流程概览解释模块职责；列出 `vault.dat`、`vault.backup.dat`、`vault.lock` 的作用；解释随机数据密钥、12字节 nonce、16字节 tag、AAD、认证后解析；解释 DPAPI CurrentUser只包装数据密钥；对比只用DPAPI、AES-CBC/CTR、口令派生密钥和另一种AEAD方案；解释临时文件、同步、ReplaceFileW、备份、重开认证和单实例锁；明确同用户恶意进程、内存和剪贴板不在静态加密防护范围内。
- 接口、数据与错误契约：说明为静态只读内容，不读取或展示任何真实账号、路径、密钥、密文或运行日志。
- 边界与异常：算法说明强调“当前实现”而不是绝对安全承诺；不得宣称DPAPI可抵御已取得当前用户权限的恶意进程。
- 测试要求：前端类型检查无错误警告；聚焦文本检查覆盖必要事实且不存在macOS迁移说明；Vite HMR成功。
- 验证命令：`npm run check`；说明内容聚焦脚本；`git diff --check`；运行中 Tauri Debug HMR。
- 预期结果：用户不离开软件即可理解当前底层原理和主要取舍。
- 完成判定：关键事实可映射到已检查源码，界面可滚动阅读，前端检查和HMR通过。
- 交付给下一原子的输出：LPV-15 完成，无后续原子。
- 停止或重新规划条件：说明所需事实与当前源码不一致，或必须暴露真实敏感信息才能解释。
- 风险等级：L1。
- DDD 门禁：不适用；只读说明由源码事实和文本检查直接证明。
- 计划提交信息：`feat(frontend): [LPV-15-02] 增加架构说明`。

### 执行记录

- 实际验证：`svelte-check` 通过（0 error、0 warning）；聚焦文本检查确认 17 项关键事实均存在，覆盖四层职责、三个数据文件、AES-256-GCM 参数和原理、DPAPI CurrentUser、算法替代方案、原子替换及安全边界；检查确认未包含迁移说明或真实绝对路径；运行中的 Tauri Debug 收到 App 与 CSS HMR 更新；`git diff --check` 通过。

## 变更记录

| 修订版本 | 变化 | 原因 |
|---|---|---|
| 1 | 初始计划 | 按字体偏好和只读架构说明拆分 |
