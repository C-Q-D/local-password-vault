mod commands;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::experiment::check_stack])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
