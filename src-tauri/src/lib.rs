mod commands;

pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "windows")]
    let builder = builder.setup(setup_windows);

    builder
        .invoke_handler(tauri::generate_handler![
            commands::experiment::check_stack,
            commands::vault::load_vault
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

#[cfg(target_os = "windows")]
fn setup_windows(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;
    use vault_app::VaultApplication;
    use vault_platform_windows::{
        atomic_file::WindowsVaultFiles, dpapi::DpapiKeyProtector, process_lock::ProcessLockGuard,
    };
    use vault_storage::open_or_initialize;

    let data_root = app.path().app_data_dir()?;
    // 锁必须先于任何密码库读取取得，并由下方应用状态持有到进程退出。
    let instance_guard = ProcessLockGuard::acquire(&data_root)?;
    let files = WindowsVaultFiles::new(&data_root);
    let opened_vault = open_or_initialize(&files, &DpapiKeyProtector)?;
    app.manage(VaultApplication::new(instance_guard, opened_vault));
    Ok(())
}
