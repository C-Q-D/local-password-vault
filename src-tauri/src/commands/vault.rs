use serde::Serialize;
use tauri::State;
use vault_app::{CategorySummary, VaultApplication, VaultApplicationError, VaultSnapshot};

/// 分类安全摘要；账号数由 Rust 当前确认文档计算。
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CategoryDto {
    id: String,
    name: String,
    account_count: usize,
}

/// Tauri IPC 只返回界面需要的计数、版本和分类，不暴露账号内容。
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VaultStatusDto {
    schema_version: u32,
    revision: u64,
    category_count: usize,
    account_count: usize,
    initialized: bool,
    categories: Vec<CategoryDto>,
    uncategorized_count: usize,
}

impl From<VaultSnapshot> for VaultStatusDto {
    fn from(snapshot: VaultSnapshot) -> Self {
        Self {
            schema_version: snapshot.schema_version,
            revision: snapshot.revision,
            category_count: snapshot.category_count,
            account_count: snapshot.account_count,
            initialized: snapshot.initialized,
            categories: snapshot
                .categories
                .into_iter()
                .map(CategoryDto::from)
                .collect(),
            uncategorized_count: snapshot.uncategorized_count,
        }
    }
}

impl From<CategorySummary> for CategoryDto {
    fn from(category: CategorySummary) -> Self {
        Self {
            id: category.id,
            name: category.name,
            account_count: category.account_count,
        }
    }
}

/// IPC 错误使用固定代码和固定信息，避免把内部错误或用户输入泄露给前端。
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct CommandErrorDto {
    code: &'static str,
    message: &'static str,
}

impl From<VaultApplicationError> for CommandErrorDto {
    fn from(error: VaultApplicationError) -> Self {
        match error {
            VaultApplicationError::CategoryNotFound => Self {
                code: "CATEGORY_NOT_FOUND",
                message: "分类不存在，请重新加载",
            },
            VaultApplicationError::CategoryNotEmpty => Self {
                code: "CATEGORY_NOT_EMPTY",
                message: "分类中仍有账号，不能删除",
            },
            VaultApplicationError::CategoryNameEmpty => Self {
                code: "CATEGORY_NAME_EMPTY",
                message: "分类名称不能为空",
            },
            VaultApplicationError::CategoryNameDuplicate => Self {
                code: "CATEGORY_NAME_DUPLICATE",
                message: "分类名称已经存在",
            },
            VaultApplicationError::VaultSaveFailed => Self {
                code: "VAULT_SAVE_FAILED",
                message: "分类保存失败，请重试",
            },
            VaultApplicationError::StateUnavailable => Self {
                code: "VAULT_STATE_UNAVAILABLE",
                message: "密码库状态暂时不可用",
            },
        }
    }
}

#[tauri::command]
pub(crate) fn load_vault(
    application: State<'_, VaultApplication>,
) -> Result<VaultStatusDto, CommandErrorDto> {
    application
        .snapshot()
        .map(VaultStatusDto::from)
        .map_err(CommandErrorDto::from)
}

#[tauri::command]
pub(crate) fn create_category(
    name: String,
    application: State<'_, VaultApplication>,
) -> Result<VaultStatusDto, CommandErrorDto> {
    application
        .create_category(&name)
        .map(VaultStatusDto::from)
        .map_err(CommandErrorDto::from)
}

#[tauri::command]
pub(crate) fn rename_category(
    id: String,
    name: String,
    application: State<'_, VaultApplication>,
) -> Result<VaultStatusDto, CommandErrorDto> {
    application
        .rename_category(&id, &name)
        .map(VaultStatusDto::from)
        .map_err(CommandErrorDto::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_contains_only_safe_category_summary() {
        let dto = VaultStatusDto::from(VaultSnapshot {
            schema_version: 1,
            revision: 2,
            category_count: 1,
            account_count: 0,
            initialized: true,
            categories: vec![CategorySummary {
                id: "safe-id".to_owned(),
                name: "工作".to_owned(),
                account_count: 0,
            }],
            uncategorized_count: 0,
        });

        assert_eq!(dto.categories[0].name, "工作");
        assert_eq!(dto.categories[0].account_count, 0);
        assert_eq!(dto.revision, 2);
    }

    #[test]
    fn application_errors_map_to_stable_command_codes() {
        assert_eq!(
            CommandErrorDto::from(VaultApplicationError::CategoryNameEmpty).code,
            "CATEGORY_NAME_EMPTY"
        );
        assert_eq!(
            CommandErrorDto::from(VaultApplicationError::CategoryNameDuplicate).code,
            "CATEGORY_NAME_DUPLICATE"
        );
        assert_eq!(
            CommandErrorDto::from(VaultApplicationError::VaultSaveFailed).code,
            "VAULT_SAVE_FAILED"
        );
    }
}
