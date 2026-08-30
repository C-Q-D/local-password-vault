# Local Password Vault

> 把散落在便签、文档和浏览器之外的账号，收进一个属于当前 Windows 用户的本地密码库。

网站账号越来越多，同一个网站还可能有工作、个人等多个身份。把密码写在便签或普通文件里不够妥当，分散保存在不同地方又很难查找；而云端密码管理器，也并不是每个人都愿意使用。

**Local Password Vault** 提供一个简单的本地选择：把日常账号集中保存在自己的 Windows 电脑上，按用途分类，需要时快速搜索、查看或复制。它不要求注册在线账号，也不会把密码库同步到远程服务器。

[下载 Windows 安装版](https://github.com/C-Q-D/local-password-vault/releases/latest) · [查看 v0.1.0 发布说明](https://github.com/C-Q-D/local-password-vault/releases/tag/v0.1.0)

![Local Password Vault 账号管理界面](docs/assets/readme/account-dashboard.png)

> 界面预览使用模拟账号，不包含真实凭据。

## 它解决什么问题

| 日常困扰 | Local Password Vault 的处理方式 |
| --- | --- |
| 账号散落在不同位置，需要时找不到 | 集中保存在一个本地密码库中，可按账号名、地址、邮箱和备注即时搜索 |
| 同一网站有多个身份，容易混淆 | 每个账号独立记录，并可按工作、个人等用途分类 |
| 不希望为了管理密码再注册一个云端账号 | 不需要登录或订阅，密码库数据保存在当前电脑 |
| 经常要复制账号和密码，手工操作麻烦 | 账号名和密码可分别复制，密码默认隐藏 |

它不试图替代功能庞大的密码管理平台，只专注于一件具体的事：让个人用户在自己的电脑上，更清楚、更方便地保管和取用日常账号。

## 适合谁

- 主要使用 Windows，希望在本机集中管理个人账号；
- 不需要云同步、团队共享或浏览器自动填充；
- 希望软件保持简单，打开后可以直接分类、搜索和复制。

如果你需要多设备同步、多人协作、手机端或自动填充，当前版本并不适合这些场景。

## 现在可以做什么

- 创建、重命名和删除分类，按分类筛选账号；
- 新增、编辑和删除账号，保存账号名、密码、地址、邮箱和备注；
- 按账号名、地址、邮箱和备注搜索，不检索密码内容；
- 单独显示、隐藏或复制密码，单独复制账号名；
- 按近 7 天、近 1 个月、近半年或近 1 年查看账号与密码的复制次数；
- 调整界面字体类型和字号；
- 在软件内直接查看项目架构与数据保护原理。

## 下载与安装

1. 前往 [Releases](https://github.com/C-Q-D/local-password-vault/releases/latest)；
2. 下载文件名以 `_x64-setup.exe` 结尾的 Windows 安装包；
3. 运行安装程序，再从开始菜单打开 **Local Password Vault**。

当前安装包尚未进行商业代码签名，Windows SmartScreen 可能显示来源提示。请只从本仓库的 GitHub Release 下载，并核对 Release 页面提供的 SHA-256。

当前版本支持 **Windows 10/11 x64**。

## 密码库如何保护数据

- 密码库使用 **AES-256-GCM** 整体加密。它既隐藏数据内容，也能检查加密文件是否被修改；
- 每份密码库使用独立的随机密钥，再由 Windows **DPAPI** 将密钥保护在当前 Windows 用户环境中；
- 保存时先写入临时文件并执行原子替换，同时保留上一版加密备份，降低写入中断造成文件损坏的风险；
- 账号列表不会预先取得全部明文密码；显示或复制密码时，前端通过明确的 Tauri command 请求 Rust 后端处理。

这不是“绝对安全”的承诺。如果当前 Windows 用户或电脑已经被恶意程序控制，本地加密无法代替系统安全防护。本项目目前也没有主密码：单独复制密码库文件，不能用于在另一台电脑、重装后的系统或其他 Windows 用户下恢复数据。

账号管理功能可以离线使用。界面会尝试从小米官方 CDN 加载 MiSans 字体；网络不可用时自动回退到系统字体，不影响密码库功能，密码库数据也不会因此上传。

<details>
<summary><strong>开发者信息</strong></summary>

### 技术栈

- Tauri 2
- Svelte 5、TypeScript、Vite
- Rust stable
- Windows DPAPI、Win32 原子文件替换与系统剪贴板

### 本地开发

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

更详细的实现说明见 [架构设计](docs/architecture/架构设计.md) 与 [Windows 平台能力方案](docs/architecture/Windows平台能力方案.md)。

</details>
