use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackCheckResult {
    message: String,
    rust_version: String,
}

/// 只验证前端到 Rust 的真实 IPC 链路，不承载密码库业务逻辑。
#[tauri::command]
pub(crate) fn check_stack(name: String) -> StackCheckResult {
    println!("STACK_CHECK_COMMAND_INVOKED");

    StackCheckResult {
        message: format!("{name} 已成功调用 Rust command"),
        rust_version: format!("{}（项目固定版本）", env!("CARGO_PKG_RUST_VERSION")),
    }
}

#[cfg(test)]
mod tests {
    use super::check_stack;

    #[test]
    fn command_returns_the_requested_experiment_name() {
        let result = check_stack("测试密码库".to_owned());

        assert_eq!(result.message, "测试密码库 已成功调用 Rust command");
        assert!(result.rust_version.starts_with("1.96.1"));
    }
}
