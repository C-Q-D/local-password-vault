# Windows 平台能力方案

## 1. 依赖边界

Windows 平台能力统一位于 `crates/vault-platform-windows`，使用 `windows = 0.62.2` 直接调用 Win32 API。当前只启用实际需要的 features：

```toml
windows = { version = "=0.62.2", features = [
  "Win32_Foundation",
  "Win32_Security_Cryptography_UI",
  "Win32_Storage_FileSystem",
  "Win32_System_DataExchange",
  "Win32_System_Memory",
] }
```

该 feature 集已在 Rust 1.96.1 下通过 `cargo check -p vault-platform-windows`。平台类型不得进入 `vault-core`，Tauri command 也不直接编写 Win32 调用。

## 2. DPAPI CurrentUser

- 使用 `CryptProtectData` 和 `CryptUnprotectData`。
- flags 只启用 `CRYPTPROTECT_UI_FORBIDDEN`；不启用 `CRYPTPROTECT_LOCAL_MACHINE`，因此采用默认 CurrentUser 边界。
- 不使用 optional entropy，避免引入没有独立保密价值的应用常量。
- 解包结果必须恰好为 32 字节，随后还必须成功通过 AES-GCM 认证，不能根据单个 DPAPI 错误码判断数据完整性。
- DPAPI 输出由局部 RAII guard 管理：包装后的缓冲复制完成后 `LocalFree`；解包后的完整实际 `cbData` 先使用 `zeroize` 做不会被编译器消除的清零，再 `LocalFree`。windows-rs 不提供 Windows 头文件内联 `SecureZeroMemory` 的可调用绑定，因此不伪造 DLL FFI。
- Win32 错误码只进入内部错误来源，前端只接收稳定且不含敏感信息的错误分类。

恢复边界保持为同一 Windows 用户配置；系统重装、换机或管理员重置用户密码不属于第一版可恢复范围。

## 3. 同目录原子发布与备份

### 首次保存

1. 在主文件同目录使用 `create_new` 创建唯一 temp；
2. 写入完整密文并调用 `File::sync_all`；
3. 在单写者边界内再次确认 main 不存在；
4. 使用 `MoveFileExW(temp, main, MOVEFILE_WRITE_THROUGH)`，不允许覆盖；
5. 失败时 main 仍不存在，不能声明初始化成功。

该协议处理应用崩溃、重复初始化和普通 I/O 失败。MVP 不把能够在当前用户数据目录任意删改文件、读取当前进程内存并调用同用户 DPAPI 的恶意进程作为文件竞态防护对象，因此不为 temp 路径抢换引入额外句柄重命名协议；应用自身通过唯一 `create_new` temp 和单实例写者避免协作范围内的竞争。

### 后续保存

1. 同目录写入并 `sync_all` 唯一 temp；
2. 选择不存在的唯一 `backup-next`；
3. 调用 `ReplaceFileW(main, temp, backup-next, 0, null, null)`；
4. 成功后 main 是新版本，`backup-next` 是紧前一版 main；
5. 使用 `MoveFileExW` 的 `REPLACE_EXISTING | WRITE_THROUGH` 把 `backup-next` 晋升为正式 backup；
6. 备份晋升失败时 main 已提交，不能伪装成“保存未发生”；保留 `backup-next` 作为恢复现场。

`REPLACEFILE_WRITE_THROUGH` 在 Microsoft 文档中标为不支持，因此不把它写入保证。当前承诺是运行中不通过分步覆盖暴露半写 main，并保留恢复文件；不宣称数据库事务级断电持久性。

### 特殊失败分支

`ReplaceFileW` 返回 `ERROR_UNABLE_TO_MOVE_REPLACEMENT_2`（1177）时，temp 仍在原名，但旧 main 可能已经被移动到 `backup-next`，main 路径可能缺失。此时必须：

1. 立即尝试把 `backup-next` 恢复为 main；
2. 恢复成功后按保存失败处理，继续使用旧快照；
3. 恢复失败时停止编辑并进入恢复状态，保留 temp 与 backup 现场。

因此不能再使用“所有提交点前失败都保证旧 main 原位不变”的绝对表述。

## 4. 剪贴板写入与条件清理

不使用 `tauri-plugin-clipboard-manager` 处理密码，因为它没有本次写入 token 和 compare-and-clear 能力。

写入时在一次 `OpenClipboard(hwnd)` 会话内设置：

1. `CF_UNICODETEXT` 文本；
2. 应用注册格式 `LocalPasswordVault.ClipboardToken.v1`，保存进程内唯一 token；
3. Windows 注册格式 `ExcludeClipboardContentFromMonitorProcessing`，阻止系统 Clipboard History 和 Cloud Clipboard 收录该密码。

全部写入成功并关闭剪贴板后，记录 `GetClipboardSequenceNumber()`，形成 `(token, sequence, expected_text)` 清理凭证。密码包含内嵌 NUL 时拒绝复制，避免 UTF-16 文本被静默截断。

30 秒后先 `OpenClipboard(hwnd)`，在仍持有剪贴板锁时依次确认 sequence、token 和文本完全匹配，三项都成立才 `EmptyClipboard`。任一项变化或读取失败都不清除，以免覆盖用户后来复制的内容。正常退出时对当前凭证执行同一条件清理；崩溃或强制结束无法保证清除。

资源所有权：

- 每次成功 `OpenClipboard` 都由 guard 保证 `CloseClipboard`；
- `SetClipboardData` 成功后 HGLOBAL 归系统，应用不得释放；
- 尚未移交的 HGLOBAL 由应用 `GlobalFree`；
- `GetClipboardData` 返回的句柄只读、不释放；
- 不在日志中记录密码、token 或剪贴板文本。

## 5. 官方依据

- [CryptProtectData](https://learn.microsoft.com/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
- [CryptUnprotectData](https://learn.microsoft.com/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata)
- [ReplaceFileW](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-replacefilew)
- [MoveFileExW](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-movefileexw)
- [Clipboard Operations](https://learn.microsoft.com/windows/win32/dataxchg/clipboard-operations)
- [GetClipboardSequenceNumber](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-getclipboardsequencenumber)
- [Standard Clipboard Formats](https://learn.microsoft.com/windows/win32/dataxchg/clipboard-formats)
- [windows-rs](https://github.com/microsoft/windows-rs)
