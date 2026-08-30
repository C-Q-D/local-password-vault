# Local Password Vault

一个面向 Windows 10/11 的轻量本地密码库。账号和密码只保存在当前电脑，不需要在线账号，也不会同步到远程服务器。

## 主要功能

- 创建、重命名和删除账号分类；非空分类不会被误删。
- 新增、编辑和删除账号，保存账号名、密码、地址、邮箱与备注。
- 按账号名、地址、邮箱和备注即时搜索，不使用密码或分类名称作为搜索内容。
- 密码默认隐藏；账号名和密码可以分别复制。
- 复制到剪贴板的密码会在 30 秒后按所有权凭证安全清理，不会清除用户后来复制的其他内容。
- 记录账号复制和密码复制次数，按近 7 天、近 1 个月、近半年或近 1 年查看使用概览。
- 支持界面字体类型和字号调整，并内置项目架构及数据保护原理说明。

## 安装

1. 打开仓库的 [Releases](https://github.com/C-Q-D/local-password-vault/releases) 页面。
2. 下载当前版本的 Windows `Setup.exe` 安装包。
3. 运行安装程序，然后从开始菜单启动 **Local Password Vault**。

当前安装包尚未进行商业代码签名，Windows SmartScreen 可能显示来源提示。请只从本仓库的 GitHub Release 下载，并核对 Release 中提供的 SHA-256。

## 数据如何保护

- 密码库文档先使用 **AES-256-GCM** 整体加密，同时验证密文是否被修改。
- 每份密码库使用独立随机数据密钥；Windows **DPAPI** 再把该密钥绑定到当前 Windows 用户。
- 保存时使用同目录临时文件和原子替换，成功替换前不会把内存候选状态当成已保存状态。
- 文件替换时保留上一版加密备份；主文件损坏时由用户确认是否恢复。
- 前端账号列表不持有密码或备注，显示密码、复制密码等操作通过明确的 Tauri command 进入 Rust 边界。

这套设计没有主密码。加密文件只能在创建它的 Windows 用户环境中由 DPAPI 解开，因此仅复制密码库文件并不能用于跨电脑、重装系统或切换 Windows 用户恢复。请同时维护可靠的系统账户和整机备份。

账号管理功能离线可用。界面开发阶段通过小米官方 CDN 尝试加载 MiSans；离线时会自动回退到系统字体，不影响密码库功能。

## 技术栈与目录

- 桌面框架：Tauri 2
- 前端：Svelte 5、TypeScript、Vite
- 核心逻辑：Rust stable
- Windows 能力：DPAPI、Win32 原子文件替换、系统剪贴板

```text
frontend/                     Svelte 正式界面与 Tauri 调用
src-tauri/                    Tauri 程序入口、command 与打包配置
crates/vault-core/            领域模型和业务规则
crates/vault-app/             应用用例与安全摘要
crates/vault-storage/         AES-256-GCM 加密封装和保存恢复流程
crates/vault-platform-windows/ DPAPI、原子文件、进程锁和剪贴板
docs/                         产品、架构、开发与发布文档
```

## 本地开发

需要 Rust 1.96.1 或更高版本、Node.js 24 或更高版本，以及 npm。

```powershell
npm install
npm run tauri dev
```

常用检查：

```powershell
npm run check
cargo test --workspace
```

正式 Windows 安装包：

```powershell
npm run tauri build
```

Release 构建产物位于 `target/release/bundle/nsis/`。构建缓存、运行数据和测试密码库不会提交到 Git。

## 当前范围

当前版本只支持 Windows 10/11 x64。云同步、浏览器自动填充、手机端、多人共享、导入导出和 macOS 版本不在 v0.1.0 范围内。
