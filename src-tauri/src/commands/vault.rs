use serde::{Deserialize, Serialize};
use tauri::State;
use vault_app::{
    AccountEditDetail, AccountSummary, CategorySummary, CreateAccountInput, UpdateAccountInput,
    VaultApplication, VaultApplicationError, VaultSnapshot,
};

/// 分类安全摘要；账号数由 Rust 当前确认文档计算。
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CategoryDto {
    id: String,
    name: String,
    account_count: usize,
}

/// 账号列表 DTO 明确没有 password 字段，避免明文密码进入前端常驻状态。
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountDto {
    id: String,
    category_id: Option<String>,
    username: String,
    address: Option<String>,
    email: Option<String>,
}

/// 新增账号请求仅在 command 调用期间持有明文密码，不写日志也不放入响应。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateAccountDto {
    category_id: Option<String>,
    username: String,
    password: String,
    address: Option<String>,
    email: Option<String>,
    notes: Option<String>,
}

/// 单条编辑详情 DTO 只通过按 ID 的专用 command 返回。
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountEditDto {
    id: String,
    category_id: Option<String>,
    username: String,
    password: String,
    address: Option<String>,
    email: Option<String>,
    notes: Option<String>,
}

/// 编辑请求不包含账号 ID；字段和分类由应用层一次保存。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateAccountDto {
    category_id: Option<String>,
    username: String,
    password: String,
    address: Option<String>,
    email: Option<String>,
    notes: Option<String>,
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
    accounts: Vec<AccountDto>,
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
            accounts: snapshot
                .accounts
                .into_iter()
                .map(AccountDto::from)
                .collect(),
        }
    }
}

impl From<AccountSummary> for AccountDto {
    fn from(account: AccountSummary) -> Self {
        Self {
            id: account.id,
            category_id: account.category_id,
            username: account.username,
            address: account.address,
            email: account.email,
        }
    }
}

impl From<AccountEditDetail> for AccountEditDto {
    fn from(account: AccountEditDetail) -> Self {
        Self {
            id: account.id,
            category_id: account.category_id,
            username: account.username,
            password: account.password,
            address: account.address,
            email: account.email,
            notes: account.notes,
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
            VaultApplicationError::AccountUsernameRequired => Self {
                code: "ACCOUNT_USERNAME_REQUIRED",
                message: "账号名不能为空",
            },
            VaultApplicationError::AccountPasswordRequired => Self {
                code: "ACCOUNT_PASSWORD_REQUIRED",
                message: "密码不能为空",
            },
            VaultApplicationError::AccountNotFound => Self {
                code: "ACCOUNT_NOT_FOUND",
                message: "账号不存在，请重新加载",
            },
            VaultApplicationError::VaultSaveFailed => Self {
                code: "VAULT_SAVE_FAILED",
                message: "密码库保存失败，请重试",
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

#[tauri::command]
pub(crate) fn delete_category(
    id: String,
    application: State<'_, VaultApplication>,
) -> Result<VaultStatusDto, CommandErrorDto> {
    application
        .delete_category(&id)
        .map(VaultStatusDto::from)
        .map_err(CommandErrorDto::from)
}

#[tauri::command]
pub(crate) fn create_account(
    input: CreateAccountDto,
    application: State<'_, VaultApplication>,
) -> Result<VaultStatusDto, CommandErrorDto> {
    application
        .create_account(CreateAccountInput {
            category_id: input.category_id,
            username: input.username,
            password: input.password,
            address: input.address,
            email: input.email,
            notes: input.notes,
        })
        .map(VaultStatusDto::from)
        .map_err(CommandErrorDto::from)
}

#[tauri::command]
pub(crate) fn load_account_for_edit(
    id: String,
    application: State<'_, VaultApplication>,
) -> Result<AccountEditDto, CommandErrorDto> {
    application
        .account_for_edit(&id)
        .map(AccountEditDto::from)
        .map_err(CommandErrorDto::from)
}

#[tauri::command]
pub(crate) fn update_account(
    id: String,
    input: UpdateAccountDto,
    application: State<'_, VaultApplication>,
) -> Result<VaultStatusDto, CommandErrorDto> {
    application
        .update_account(
            &id,
            UpdateAccountInput {
                category_id: input.category_id,
                username: input.username,
                password: input.password,
                address: input.address,
                email: input.email,
                notes: input.notes,
            },
        )
        .map(VaultStatusDto::from)
        .map_err(CommandErrorDto::from)
}

#[tauri::command]
pub(crate) fn delete_account(
    id: String,
    application: State<'_, VaultApplication>,
) -> Result<VaultStatusDto, CommandErrorDto> {
    application
        .delete_account(&id)
        .map(VaultStatusDto::from)
        .map_err(CommandErrorDto::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_contains_only_safe_list_summaries() {
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
            accounts: vec![AccountSummary {
                id: "account-id".to_owned(),
                category_id: Some("safe-id".to_owned()),
                username: "测试账号".to_owned(),
                address: Some("https://example.invalid".to_owned()),
                email: None,
            }],
        });

        assert_eq!(dto.categories[0].name, "工作");
        assert_eq!(dto.categories[0].account_count, 0);
        assert_eq!(dto.revision, 2);
        assert_eq!(dto.accounts[0].username, "测试账号");
        assert_eq!(dto.accounts[0].category_id.as_deref(), Some("safe-id"));
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
        assert_eq!(
            CommandErrorDto::from(VaultApplicationError::AccountUsernameRequired).code,
            "ACCOUNT_USERNAME_REQUIRED"
        );
        assert_eq!(
            CommandErrorDto::from(VaultApplicationError::AccountPasswordRequired).code,
            "ACCOUNT_PASSWORD_REQUIRED"
        );
        assert_eq!(
            CommandErrorDto::from(VaultApplicationError::AccountNotFound).code,
            "ACCOUNT_NOT_FOUND"
        );
    }

    #[test]
    fn edit_detail_dto_maps_only_one_requested_record() {
        let dto = AccountEditDto::from(AccountEditDetail {
            id: "account-id".to_owned(),
            category_id: None,
            username: "账号".to_owned(),
            password: "密码".to_owned(),
            address: None,
            email: None,
            notes: Some("备注".to_owned()),
        });

        assert_eq!(dto.id, "account-id");
        assert_eq!(dto.password, "密码");
        assert_eq!(dto.notes.as_deref(), Some("备注"));
    }
}
