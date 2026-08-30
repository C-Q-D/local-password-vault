# Local Password Vault Agent 规则

<!-- codex-plan-index:start -->
## 当前计划索引

| 计划 ID | 类型 | 文档 |
|---|---|---|
| SCOPE-LPV-MVP-001 | 产品拆分 | [本地密码库 MVP 开发计划](docs/planning/开发计划.md) |
| ATOMIC-LPV-11-001 | 原子开发 | [LPV-11 查看和复制账号凭据原子计划](docs/planning/原子开发计划-LPV-11.md) |

<!-- codex-plan-index:end -->

本文件补充用户级全局规则，只规定当前仓库的项目约束。若与更高优先级指令冲突，以更高优先级指令为准。

## 当前技术栈

- 桌面框架：Tauri 2。
- 前端：Svelte、TypeScript、Vite。
- 核心逻辑：Rust stable。
- 当前平台：Windows 10/11 x64；后续增加 macOS。

## 编译与测试前置规则

凡任务涉及以下任一操作，执行检查、测试、启动或编译前，必须先完整阅读 [Rust/Tauri 编译策略](docs/development/编译策略.md)：

- 新增、修改或删除 Rust 源码；
- 新增、修改或删除 Svelte、TypeScript、JavaScript 或 CSS；
- 修改 `Cargo.toml`、`Cargo.lock`、`tauri.conf.json`、`package.json` 或前端锁文件；
- 修改 Rust 或前端测试；
- 启动 Tauri 开发程序；
- 执行 `cargo check`、`cargo test`、`cargo build`、`tauri dev` 或 `tauri build`；
- 在 Git worktree 或多个 Agent 中并发开发。

不得凭习惯把 Release 全量编译作为每次改动后的默认验证。验证范围必须与本次改动范围匹配。

## 实现约束

- 第一优先级是功能正确，第二是基础界面，第三是性能优化。
- 前端不得直接读写密码库文件；通过 Tauri command 调用 Rust。
- Tauri command 保持薄，只做参数转换、授权边界和结果返回；业务规则放在 Rust 核心模块。
- Windows 和 macOS 平台代码隔离，通用核心不得依赖 DPAPI、Keychain 或具体系统路径。
- 新增说明性注释使用中文，代码标识符和标准术语保留英文。
- 不引入本阶段不需要的数据库、HTTP 服务、DI 容器或通用抽象层。

## 进度汇报

开发阶段每完成一个可独立验收的任务，必须说明该任务的验证结果和剩余任务。
