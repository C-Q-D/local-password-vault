mod commands;

pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "windows")]
    let builder = builder.setup(setup_windows);

    builder
        .invoke_handler(tauri::generate_handler![
            commands::experiment::check_stack,
            commands::vault::load_vault,
            commands::vault::create_category,
            commands::vault::rename_category,
            commands::vault::delete_category,
            commands::vault::create_account,
            commands::vault::load_account_for_edit,
            commands::vault::update_account,
            commands::vault::delete_account
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

#[cfg(target_os = "windows")]
fn setup_windows(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;
    use vault_app::{VaultApplication, VaultRepository};
    use vault_core::VaultDocument;
    use vault_platform_windows::{
        atomic_file::WindowsVaultFiles, dpapi::DpapiKeyProtector, process_lock::ProcessLockGuard,
    };
    use vault_storage::{VaultOpenError, open_or_initialize, save_confirmed};

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
    }

    let data_root = app.path().app_data_dir()?;
    // 锁必须先于任何密码库读取取得，并由下方应用状态持有到进程退出。
    let instance_guard = ProcessLockGuard::acquire(&data_root)?;
    let files = WindowsVaultFiles::new(&data_root);
    let opened_vault = open_or_initialize(&files, &DpapiKeyProtector)?;
    app.manage(VaultApplication::new(
        instance_guard,
        opened_vault,
        WindowsVaultRepository { files },
    ));
    Ok(())
}
