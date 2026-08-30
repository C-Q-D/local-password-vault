mod commands;
#[cfg(target_os = "windows")]
mod password_clear_scheduler;

pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "windows")]
    let builder = builder.setup(setup_windows);

    builder
        .invoke_handler(tauri::generate_handler![
            commands::experiment::check_stack,
            commands::vault::load_vault,
            commands::vault::recover_from_backup,
            commands::vault::create_category,
            commands::vault::rename_category,
            commands::vault::delete_category,
            commands::vault::create_account,
            commands::vault::load_account_for_edit,
            commands::vault::reveal_account_password,
            commands::vault::copy_account_username,
            commands::vault::copy_account_password,
            commands::vault::search_accounts,
            commands::vault::update_account,
            commands::vault::delete_account
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

#[cfg(target_os = "windows")]
fn setup_windows(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use crate::password_clear_scheduler::PasswordClearScheduler;
    use tauri::Manager;
    use vault_app::{VaultApplication, VaultRepository};
    use vault_core::VaultDocument;
    use vault_platform_windows::{
        atomic_file::WindowsVaultFiles, clipboard::WindowsClipboard, dpapi::DpapiKeyProtector,
        process_lock::ProcessLockGuard,
    };
    use vault_storage::{
        OpenedVault, VaultOpenError, VaultStartup, probe_or_initialize, recover_from_backup,
        save_confirmed,
    };

    /// 把 Windows 文件与 DPAPI 组合成应用层所需的最小持久化端口。
    struct WindowsVaultRepository {
        files: WindowsVaultFiles,
    }

    impl VaultRepository for WindowsVaultRepository {
        fn save_confirmed(
            &self,
            candidate: &VaultDocument,
        ) -> Result<VaultDocument, VaultOpenError> {
            save_confirmed(&self.files, &DpapiKeyProtector, candidate)
        }

        fn recover_from_backup(&self) -> Result<OpenedVault, VaultOpenError> {
            recover_from_backup(&self.files, &DpapiKeyProtector)
        }
    }

    let data_root = app.path().app_data_dir()?;
    // 锁必须先于任何密码库读取取得，并由下方应用状态持有到进程退出。
    let instance_guard = ProcessLockGuard::acquire(&data_root)?;
    let files = WindowsVaultFiles::new(&data_root);
    let startup = probe_or_initialize(&files, &DpapiKeyProtector).unwrap_or_else(|_| {
        eprintln!("[local-password-vault] operation=probe_vault result=unavailable");
        VaultStartup::Unavailable
    });
    let main_window = app
        .get_webview_window("main")
        .ok_or("找不到主窗口，无法初始化剪贴板")?;
    let owner_hwnd = main_window.hwnd()?.0 as isize;
    app.manage(WindowsClipboard::new(owner_hwnd)?);
    // 全程序只创建一个密码清理工作线程，连续复制不会线性增加休眠线程。
    app.manage(PasswordClearScheduler::new()?);
    app.manage(VaultApplication::from_startup(
        instance_guard,
        startup,
        WindowsVaultRepository { files },
    ));
    Ok(())
}
