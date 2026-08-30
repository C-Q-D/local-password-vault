use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AccountError, AccountUsageError, CategoryError, DeleteAccountError, MoveAccountError,
    SearchAccountError, UpdateAccountError, VaultValidationError,
};

/// 当前代码能够完整理解和校验的密码库结构版本。
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// 一份完整的密码库明文文档。
///
/// 该结构只在内存和加密边界内使用，写入磁盘前必须由存储层加密。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultDocument {
    pub schema_version: u32,
    pub revision: u64,
    pub categories: Vec<Category>,
    pub accounts: Vec<Account>,
}

impl VaultDocument {
    /// 构造第一次启动时使用的、结构有效的空密码库。
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            revision: 0,
            categories: Vec::new(),
            accounts: Vec::new(),
        }
    }

    /// 在当前候选文档中创建一个名称唯一的用户分类。
    pub fn create_category(&mut self, name: &str) -> Result<Category, CategoryError> {
        let display_name = name.trim();
        if display_name.is_empty() {
            return Err(CategoryError::BlankName);
        }

        let normalized_name = normalize_category_name(display_name);
        if self
            .categories
            .iter()
            .any(|category| normalize_category_name(&category.name) == normalized_name)
        {
            return Err(CategoryError::DuplicateName);
        }

        let category = Category {
            id: Uuid::new_v4(),
            name: display_name.to_owned(),
        };
        self.categories.push(category.clone());
        Ok(category)
    }

    /// 按稳定 ID 重命名一个用户分类，失败时不修改候选文档。
    pub fn rename_category(&mut self, id: Uuid, name: &str) -> Result<(), CategoryError> {
        let display_name = name.trim();
        if display_name.is_empty() {
            return Err(CategoryError::BlankName);
        }

        let target_index = self
            .categories
            .iter()
            .position(|category| category.id == id)
            .ok_or(CategoryError::NotFound)?;
        let normalized_name = normalize_category_name(display_name);
        if self.categories.iter().enumerate().any(|(index, category)| {
            index != target_index && normalize_category_name(&category.name) == normalized_name
        }) {
            return Err(CategoryError::DuplicateName);
        }

        self.categories[target_index].name = display_name.to_owned();
        Ok(())
    }

    /// 只删除没有账号引用的分类，绝不级联删除或移动账号。
    pub fn delete_category(&mut self, id: Uuid) -> Result<(), CategoryError> {
        let target_index = self
            .categories
            .iter()
            .position(|category| category.id == id)
            .ok_or(CategoryError::NotFound)?;
        if self
            .accounts
            .iter()
            .any(|account| account.category_id == Some(id))
        {
            return Err(CategoryError::NotEmpty);
        }

        self.categories.remove(target_index);
        Ok(())
    }

    /// 创建一条独立账号；只校验当前产品要求的必填字段和分类引用。
    pub fn create_account(&mut self, input: CreateAccount) -> Result<Account, AccountError> {
        if input.username.trim().is_empty() {
            return Err(AccountError::UsernameRequired);
        }
        if input.password.trim().is_empty() {
            return Err(AccountError::PasswordRequired);
        }
        if input.category_id.is_some_and(|category_id| {
            !self
                .categories
                .iter()
                .any(|category| category.id == category_id)
        }) {
            return Err(AccountError::CategoryNotFound);
        }

        let account = Account {
            id: Uuid::new_v4(),
            category_id: input.category_id,
            username: input.username,
            password: input.password,
            address: input.address,
            email: input.email,
            notes: input.notes,
            last_used_at: None,
        };
        self.accounts.push(account.clone());
        Ok(account)
    }

    /// 按稳定 ID 更新账号字段；分类归属由后续独立用例处理。
    pub fn update_account(
        &mut self,
        id: Uuid,
        input: UpdateAccount,
    ) -> Result<(), UpdateAccountError> {
        if input.username.trim().is_empty() {
            return Err(UpdateAccountError::UsernameRequired);
        }
        if input.password.trim().is_empty() {
            return Err(UpdateAccountError::PasswordRequired);
        }
        let account = self
            .accounts
            .iter_mut()
            .find(|account| account.id == id)
            .ok_or(UpdateAccountError::NotFound)?;

        account.username = input.username;
        account.password = input.password;
        account.address = input.address;
        account.email = input.email;
        account.notes = input.notes;
        Ok(())
    }

    /// 按稳定 ID 更新账号分类归属；目标为 None 时表示移动到“未分类”。
    pub fn move_account(
        &mut self,
        id: Uuid,
        category_id: Option<Uuid>,
    ) -> Result<(), MoveAccountError> {
        let account_index = self
            .accounts
            .iter()
            .position(|account| account.id == id)
            .ok_or(MoveAccountError::AccountNotFound)?;
        if category_id.is_some_and(|target_id| {
            !self
                .categories
                .iter()
                .any(|category| category.id == target_id)
        }) {
            return Err(MoveAccountError::CategoryNotFound);
        }

        self.accounts[account_index].category_id = category_id;
        Ok(())
    }

    /// 只按稳定 ID 删除一条账号记录，不使用列表位置或可重复的业务字段定位。
    pub fn delete_account(&mut self, id: Uuid) -> Result<(), DeleteAccountError> {
        let account_index = self
            .accounts
            .iter()
            .position(|account| account.id == id)
            .ok_or(DeleteAccountError::NotFound)?;

        self.accounts.remove(account_index);
        Ok(())
    }

    /// 只有凭据操作成功后才记录时间；时间由调用边界提供，便于稳定测试。
    pub fn record_account_used(&mut self, id: Uuid, used_at: u64) -> Result<(), AccountUsageError> {
        let account = self
            .accounts
            .iter_mut()
            .find(|account| account.id == id)
            .ok_or(AccountUsageError::NotFound)?;
        account.last_used_at = Some(used_at);
        Ok(())
    }

    /// 在当前确认文档中搜索账号，并只返回列表展示所需的安全字段。
    pub fn search_accounts(
        &self,
        query: &str,
        scope: AccountSearchScope,
    ) -> Result<Vec<AccountSearchResult>, SearchAccountError> {
        if let AccountSearchScope::Category(category_id) = scope
            && !self
                .categories
                .iter()
                .any(|category| category.id == category_id)
        {
            return Err(SearchAccountError::CategoryNotFound);
        }

        let normalized_query = query.trim().to_lowercase();
        Ok(self
            .accounts
            .iter()
            .filter(|account| match scope {
                AccountSearchScope::All => true,
                AccountSearchScope::Uncategorized => account.category_id.is_none(),
                AccountSearchScope::Category(category_id) => {
                    account.category_id == Some(category_id)
                }
            })
            .filter(|account| account_matches_query(account, &normalized_query))
            .map(AccountSearchResult::from)
            .collect())
    }

    /// 校验载入和保存共同依赖的领域不变量。
    ///
    /// 校验集中在这里，避免不同读写路径对损坏数据作出不一致判断。
    pub fn validate(&self) -> Result<(), VaultValidationError> {
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(VaultValidationError::UnsupportedSchemaVersion);
        }

        let mut category_ids = HashSet::with_capacity(self.categories.len());
        let mut category_names = HashSet::with_capacity(self.categories.len());
        for category in &self.categories {
            if !category_ids.insert(category.id) {
                return Err(VaultValidationError::DuplicateCategoryId);
            }
            if category.name.trim().is_empty() {
                return Err(VaultValidationError::BlankCategoryName);
            }
            if !category_names.insert(normalize_category_name(&category.name)) {
                return Err(VaultValidationError::DuplicateCategoryName);
            }
        }

        let mut account_ids = HashSet::with_capacity(self.accounts.len());
        for account in &self.accounts {
            if !account_ids.insert(account.id) {
                return Err(VaultValidationError::DuplicateAccountId);
            }
            if account.username.trim().is_empty() {
                return Err(VaultValidationError::BlankUsername);
            }
            if account.password.trim().is_empty() {
                return Err(VaultValidationError::BlankPassword);
            }
            if account
                .category_id
                .is_some_and(|category_id| !category_ids.contains(&category_id))
            {
                return Err(VaultValidationError::MissingCategory);
            }
        }

        Ok(())
    }
}

fn normalize_category_name(name: &str) -> String {
    name.trim().to_lowercase()
}

impl Default for VaultDocument {
    fn default() -> Self {
        Self::empty()
    }
}

/// 用户创建的分类；“全部”和“未分类”不写入该结构。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
}

/// 一条完整账号记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub category_id: Option<Uuid>,
    pub username: String,
    pub password: String,
    pub address: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
    /// 旧密码库没有该字段时按“从未使用”读取，避免为本次增量字段升级 schema。
    #[serde(default)]
    pub last_used_at: Option<u64>,
}

/// 账号搜索的范围；用户分类始终使用稳定 UUID。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountSearchScope {
    All,
    Uncategorized,
    Category(Uuid),
}

/// 搜索结果只包含列表展示字段，密码和备注不会离开核心搜索边界。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSearchResult {
    pub id: Uuid,
    pub category_id: Option<Uuid>,
    pub username: String,
    pub address: Option<String>,
    pub email: Option<String>,
    pub last_used_at: Option<u64>,
}

impl From<&Account> for AccountSearchResult {
    fn from(account: &Account) -> Self {
        Self {
            id: account.id,
            category_id: account.category_id,
            username: account.username.clone(),
            address: account.address.clone(),
            email: account.email.clone(),
            last_used_at: account.last_used_at,
        }
    }
}

/// 新增账号所需的领域输入；由调用方决定空的可选字段使用 None 还是空字符串。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAccount {
    pub category_id: Option<Uuid>,
    pub username: String,
    pub password: String,
    pub address: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
}

/// 账号字段编辑输入不包含 ID 和分类，避免字段编辑意外改变记录归属。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAccount {
    pub username: String,
    pub password: String,
    pub address: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
}

fn account_matches_query(account: &Account, normalized_query: &str) -> bool {
    if normalized_query.is_empty() {
        return true;
    }

    // 明确列出允许搜索的四个字段，避免整个 Account 序列化或密码字段误入匹配范围。
    account.username.to_lowercase().contains(normalized_query)
        || account
            .address
            .as_deref()
            .is_some_and(|value| value.to_lowercase().contains(normalized_query))
        || account
            .email
            .as_deref()
            .is_some_and(|value| value.to_lowercase().contains(normalized_query))
        || account
            .notes
            .as_deref()
            .is_some_and(|value| value.to_lowercase().contains(normalized_query))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_account(category_id: Option<Uuid>) -> Account {
        Account {
            id: Uuid::new_v4(),
            category_id,
            username: "示例账号".to_owned(),
            password: "仅用于测试的密码".to_owned(),
            address: Some("https://example.invalid".to_owned()),
            email: None,
            notes: None,
            last_used_at: None,
        }
    }

    #[test]
    fn account_without_last_used_time_remains_compatible() {
        let mut document = VaultDocument::empty();
        document.accounts.push(valid_account(None));
        let mut value = serde_json::to_value(&document).expect("测试文档应可序列化");
        value["accounts"][0]
            .as_object_mut()
            .expect("账号应为对象")
            .remove("last_used_at");

        let loaded: VaultDocument = serde_json::from_value(value).expect("旧账号文档应继续可读取");

        assert_eq!(loaded.accounts[0].last_used_at, None);
    }

    #[test]
    fn empty_document_has_initial_version_and_revision() {
        let document = VaultDocument::empty();

        assert_eq!(document.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(document.revision, 0);
        assert!(document.categories.is_empty());
        assert!(document.accounts.is_empty());
        assert_eq!(document.validate(), Ok(()));
    }

    #[test]
    fn category_creation_trims_name_and_assigns_unique_id() {
        let mut document = VaultDocument::empty();

        let category = document
            .create_category("  工作  ")
            .expect("有效分类应创建成功");

        assert_eq!(category.name, "工作");
        assert_eq!(document.categories, vec![category]);
        assert_eq!(document.validate(), Ok(()));
    }

    #[test]
    fn category_creation_rejects_blank_and_normalized_duplicates_without_mutation() {
        let mut document = VaultDocument::empty();
        let original = document.clone();
        assert_eq!(
            document.create_category(" \n\t "),
            Err(CategoryError::BlankName)
        );
        assert_eq!(document, original);

        document
            .create_category("Development")
            .expect("首个分类应创建成功");
        let confirmed = document.clone();
        assert_eq!(
            document.create_category(" development "),
            Err(CategoryError::DuplicateName)
        );
        assert_eq!(document, confirmed);
    }

    #[test]
    fn rename_category_updates_only_the_target_by_id() {
        let mut document = VaultDocument::empty();
        let target = document.create_category("工作").expect("分类应创建成功");
        let untouched = document.create_category("个人").expect("分类应创建成功");

        document
            .rename_category(target.id, "  开发  ")
            .expect("目标分类应重命名成功");

        assert_eq!(document.categories[0].name, "开发");
        assert_eq!(document.categories[1], untouched);
    }

    #[test]
    fn rename_category_errors_do_not_mutate_the_document() {
        let mut document = VaultDocument::empty();
        let target = document.create_category("工作").expect("分类应创建成功");
        document.create_category("个人").expect("分类应创建成功");
        let confirmed = document.clone();

        assert_eq!(
            document.rename_category(Uuid::new_v4(), "其他"),
            Err(CategoryError::NotFound)
        );
        assert_eq!(document, confirmed);
        assert_eq!(
            document.rename_category(target.id, "  "),
            Err(CategoryError::BlankName)
        );
        assert_eq!(document, confirmed);
        assert_eq!(
            document.rename_category(target.id, " 个人 "),
            Err(CategoryError::DuplicateName)
        );
        assert_eq!(document, confirmed);
    }

    #[test]
    fn delete_category_removes_only_an_empty_target() {
        let mut document = VaultDocument::empty();
        let target = document.create_category("工作").expect("分类应创建成功");
        let untouched = document.create_category("个人").expect("分类应创建成功");

        document
            .delete_category(target.id)
            .expect("空分类应删除成功");

        assert_eq!(document.categories, vec![untouched]);
        assert!(document.accounts.is_empty());
    }

    #[test]
    fn delete_category_rejects_non_empty_or_unknown_target_without_mutation() {
        let mut document = VaultDocument::empty();
        let target = document.create_category("工作").expect("分类应创建成功");
        document.accounts.push(valid_account(Some(target.id)));
        let confirmed = document.clone();

        assert_eq!(
            document.delete_category(target.id),
            Err(CategoryError::NotEmpty)
        );
        assert_eq!(document, confirmed);
        assert_eq!(
            document.delete_category(Uuid::new_v4()),
            Err(CategoryError::NotFound)
        );
        assert_eq!(document, confirmed);
    }

    #[test]
    fn document_validation_rejects_normalized_duplicate_category_names() {
        let mut document = VaultDocument::empty();
        document.categories = vec![
            Category {
                id: Uuid::new_v4(),
                name: "Work".to_owned(),
            },
            Category {
                id: Uuid::new_v4(),
                name: " work ".to_owned(),
            },
        ];

        assert_eq!(
            document.validate(),
            Err(VaultValidationError::DuplicateCategoryName)
        );
    }

    #[test]
    fn json_round_trip_preserves_document() {
        let category = Category {
            id: Uuid::new_v4(),
            name: "工作".to_owned(),
        };
        let document = VaultDocument {
            schema_version: CURRENT_SCHEMA_VERSION,
            revision: 7,
            accounts: vec![valid_account(Some(category.id))],
            categories: vec![category],
        };

        let json = serde_json::to_vec(&document).expect("测试文档应能序列化");
        let decoded: VaultDocument = serde_json::from_slice(&json).expect("测试文档应能反序列化");

        assert_eq!(decoded, document);
        assert_eq!(decoded.validate(), Ok(()));
    }

    #[test]
    fn unsupported_schema_version_is_rejected() {
        let mut document = VaultDocument::empty();
        document.schema_version = CURRENT_SCHEMA_VERSION + 1;

        assert_eq!(
            document.validate(),
            Err(VaultValidationError::UnsupportedSchemaVersion)
        );
    }

    #[test]
    fn missing_category_reference_is_rejected() {
        let mut document = VaultDocument::empty();
        document.accounts.push(valid_account(Some(Uuid::new_v4())));

        assert_eq!(
            document.validate(),
            Err(VaultValidationError::MissingCategory)
        );
    }

    #[test]
    fn blank_username_is_rejected() {
        let mut account = valid_account(None);
        account.username = "  \n".to_owned();
        let mut document = VaultDocument::empty();
        document.accounts.push(account);

        assert_eq!(
            document.validate(),
            Err(VaultValidationError::BlankUsername)
        );
    }

    #[test]
    fn blank_password_is_rejected() {
        let mut account = valid_account(None);
        account.password = "\t".to_owned();
        let mut document = VaultDocument::empty();
        document.accounts.push(account);

        assert_eq!(
            document.validate(),
            Err(VaultValidationError::BlankPassword)
        );
    }

    #[test]
    fn account_creation_accepts_complete_and_minimal_independent_records() {
        let mut document = VaultDocument::empty();
        let category = document.create_category("工作").expect("分类应创建成功");
        let address = "https://example.invalid".to_owned();

        let complete = document
            .create_account(CreateAccount {
                category_id: Some(category.id),
                username: "  GitHub 工作账号  ".to_owned(),
                password: "  保留首尾空格的密码  ".to_owned(),
                address: Some(address.clone()),
                email: Some("person@example.invalid".to_owned()),
                notes: Some("测试备注".to_owned()),
            })
            .expect("完整账号应创建成功");
        let minimal = document
            .create_account(CreateAccount {
                category_id: None,
                username: "另一个账号".to_owned(),
                password: "测试密码".to_owned(),
                address: Some(address),
                email: None,
                notes: None,
            })
            .expect("最小账号应创建成功");

        assert_ne!(complete.id, minimal.id);
        assert_eq!(complete.username, "  GitHub 工作账号  ");
        assert_eq!(complete.password, "  保留首尾空格的密码  ");
        assert_eq!(document.accounts, vec![complete, minimal]);
        assert_eq!(document.validate(), Ok(()));
    }

    #[test]
    fn account_creation_rejects_invalid_input_without_mutation() {
        let mut document = VaultDocument::empty();
        let confirmed = document.clone();
        let input = |username: &str, password: &str, category_id| CreateAccount {
            category_id,
            username: username.to_owned(),
            password: password.to_owned(),
            address: None,
            email: None,
            notes: None,
        };

        assert_eq!(
            document.create_account(input("  ", "密码", None)),
            Err(AccountError::UsernameRequired)
        );
        assert_eq!(document, confirmed);
        assert_eq!(
            document.create_account(input("账号", "\n\t", None)),
            Err(AccountError::PasswordRequired)
        );
        assert_eq!(document, confirmed);
        assert_eq!(
            document.create_account(input("账号", "密码", Some(Uuid::new_v4()))),
            Err(AccountError::CategoryNotFound)
        );
        assert_eq!(document, confirmed);
    }

    #[test]
    fn update_account_changes_only_target_fields_and_keeps_identity() {
        let mut document = VaultDocument::empty();
        let category = document.create_category("工作").expect("分类应创建成功");
        let first = document
            .create_account(CreateAccount {
                category_id: Some(category.id),
                username: "旧账号".to_owned(),
                password: "旧密码".to_owned(),
                address: Some("https://same.invalid".to_owned()),
                email: None,
                notes: None,
            })
            .expect("首个账号应创建成功");
        let untouched = document
            .create_account(CreateAccount {
                category_id: None,
                username: "同地址账号".to_owned(),
                password: "另一个密码".to_owned(),
                address: Some("https://same.invalid".to_owned()),
                email: None,
                notes: None,
            })
            .expect("第二个账号应创建成功");

        document
            .update_account(
                first.id,
                UpdateAccount {
                    username: "新账号".to_owned(),
                    password: "新密码".to_owned(),
                    address: None,
                    email: Some("new@example.invalid".to_owned()),
                    notes: Some("新备注".to_owned()),
                },
            )
            .expect("目标账号应更新成功");

        let updated = &document.accounts[0];
        assert_eq!(updated.id, first.id);
        assert_eq!(updated.category_id, first.category_id);
        assert_eq!(updated.username, "新账号");
        assert_eq!(updated.password, "新密码");
        assert_eq!(updated.address, None);
        assert_eq!(updated.email.as_deref(), Some("new@example.invalid"));
        assert_eq!(updated.notes.as_deref(), Some("新备注"));
        assert_eq!(document.accounts[1], untouched);
    }

    #[test]
    fn update_account_rejects_invalid_target_or_required_fields_without_mutation() {
        let mut document = VaultDocument::empty();
        let account = document
            .create_account(CreateAccount {
                category_id: None,
                username: "账号".to_owned(),
                password: "密码".to_owned(),
                address: None,
                email: None,
                notes: None,
            })
            .expect("测试账号应创建成功");
        let confirmed = document.clone();
        let input = |username: &str, password: &str| UpdateAccount {
            username: username.to_owned(),
            password: password.to_owned(),
            address: None,
            email: None,
            notes: None,
        };

        assert_eq!(
            document.update_account(account.id, input(" ", "新密码")),
            Err(UpdateAccountError::UsernameRequired)
        );
        assert_eq!(document, confirmed);
        assert_eq!(
            document.update_account(account.id, input("新账号", "\t")),
            Err(UpdateAccountError::PasswordRequired)
        );
        assert_eq!(document, confirmed);
        assert_eq!(
            document.update_account(Uuid::new_v4(), input("新账号", "新密码")),
            Err(UpdateAccountError::NotFound)
        );
        assert_eq!(document, confirmed);
    }

    #[test]
    fn move_account_changes_only_target_category_and_accepts_uncategorized() {
        let mut document = VaultDocument::empty();
        let source = document.create_category("工作").expect("源分类应创建成功");
        let target = document
            .create_category("个人")
            .expect("目标分类应创建成功");
        let account = document
            .create_account(CreateAccount {
                category_id: Some(source.id),
                username: "测试账号".to_owned(),
                password: "测试密码".to_owned(),
                address: Some("https://example.invalid".to_owned()),
                email: Some("test@example.invalid".to_owned()),
                notes: Some("保持不变".to_owned()),
            })
            .expect("账号应创建成功");
        let original_fields = document.accounts[0].clone();

        document
            .move_account(account.id, Some(target.id))
            .expect("账号应移动到目标分类");
        assert_eq!(document.accounts[0].category_id, Some(target.id));
        assert_eq!(document.accounts[0].id, original_fields.id);
        assert_eq!(document.accounts[0].username, original_fields.username);
        assert_eq!(document.accounts[0].password, original_fields.password);
        assert_eq!(document.accounts[0].address, original_fields.address);
        assert_eq!(document.accounts[0].email, original_fields.email);
        assert_eq!(document.accounts[0].notes, original_fields.notes);

        document
            .move_account(account.id, None)
            .expect("账号应移动到未分类");
        assert_eq!(document.accounts[0].category_id, None);
        assert_eq!(document.validate(), Ok(()));
    }

    #[test]
    fn move_account_rejects_unknown_account_or_category_without_mutation() {
        let mut document = VaultDocument::empty();
        let category = document.create_category("工作").expect("分类应创建成功");
        let account = document
            .create_account(CreateAccount {
                category_id: Some(category.id),
                username: "测试账号".to_owned(),
                password: "测试密码".to_owned(),
                address: None,
                email: None,
                notes: None,
            })
            .expect("账号应创建成功");
        let confirmed = document.clone();

        assert_eq!(
            document.move_account(Uuid::new_v4(), None),
            Err(MoveAccountError::AccountNotFound)
        );
        assert_eq!(document, confirmed);
        assert_eq!(
            document.move_account(account.id, Some(Uuid::new_v4())),
            Err(MoveAccountError::CategoryNotFound)
        );
        assert_eq!(document, confirmed);
    }

    #[test]
    fn delete_account_removes_only_the_stable_id_target() {
        let mut document = VaultDocument::empty();
        let input = |username: &str| CreateAccount {
            category_id: None,
            username: username.to_owned(),
            password: "测试密码".to_owned(),
            address: Some("https://same.invalid".to_owned()),
            email: None,
            notes: None,
        };
        let target = document
            .create_account(input("待删除账号"))
            .expect("目标账号应创建成功");
        let retained = document
            .create_account(input("保留账号"))
            .expect("同地址账号应创建成功");

        document
            .delete_account(target.id)
            .expect("目标账号应删除成功");

        assert_eq!(document.accounts, vec![retained]);
        assert_eq!(document.validate(), Ok(()));
    }

    #[test]
    fn delete_account_rejects_unknown_id_without_mutation() {
        let mut document = VaultDocument::empty();
        document
            .create_account(CreateAccount {
                category_id: None,
                username: "保留账号".to_owned(),
                password: "测试密码".to_owned(),
                address: None,
                email: None,
                notes: None,
            })
            .expect("账号应创建成功");
        let confirmed = document.clone();

        assert_eq!(
            document.delete_account(Uuid::new_v4()),
            Err(DeleteAccountError::NotFound)
        );
        assert_eq!(document, confirmed);
    }

    #[test]
    fn search_accounts_matches_only_allowed_fields_case_insensitively() {
        let mut document = VaultDocument::empty();
        let category = document
            .create_category("工作分类")
            .expect("分类应创建成功");
        let accounts = [
            ("GitHub Account", None, None, None, "secret-one"),
            (
                "地址账号",
                Some("https://portal.example.invalid/path"),
                None,
                None,
                "secret-two",
            ),
            (
                "邮箱账号",
                None,
                Some("Person@Example.Invalid"),
                None,
                "secret-three",
            ),
            (
                "备注账号",
                None,
                None,
                Some("仅用于 Finance 项目"),
                "secret-four",
            ),
        ];
        for (username, address, email, notes, password) in accounts {
            document
                .create_account(CreateAccount {
                    category_id: Some(category.id),
                    username: username.to_owned(),
                    password: password.to_owned(),
                    address: address.map(str::to_owned),
                    email: email.map(str::to_owned),
                    notes: notes.map(str::to_owned),
                })
                .expect("测试账号应创建成功");
        }

        let usernames = |query: &str| {
            document
                .search_accounts(query, AccountSearchScope::All)
                .expect("搜索应成功")
                .into_iter()
                .map(|account| account.username)
                .collect::<Vec<_>>()
        };
        assert_eq!(usernames("github"), vec!["GitHub Account"]);
        assert_eq!(usernames("PORTAL.EXAMPLE"), vec!["地址账号"]);
        assert_eq!(usernames("person@example"), vec!["邮箱账号"]);
        assert_eq!(usernames("FINANCE"), vec!["备注账号"]);
        assert!(usernames("secret-three").is_empty());
        assert!(usernames("工作分类").is_empty());
        assert!(usernames("不存在").is_empty());
    }

    #[test]
    fn search_accounts_respects_all_category_and_uncategorized_scopes() {
        let mut document = VaultDocument::empty();
        let work = document
            .create_category("工作")
            .expect("工作分类应创建成功");
        let personal = document
            .create_category("个人")
            .expect("个人分类应创建成功");
        for (username, category_id) in [
            ("工作账号", Some(work.id)),
            ("个人账号", Some(personal.id)),
            ("未分类账号", None),
        ] {
            document
                .create_account(CreateAccount {
                    category_id,
                    username: username.to_owned(),
                    password: "测试密码".to_owned(),
                    address: None,
                    email: None,
                    notes: None,
                })
                .expect("测试账号应创建成功");
        }

        let all = document
            .search_accounts("  ", AccountSearchScope::All)
            .expect("空查询应返回全部账号");
        let work_only = document
            .search_accounts("账号", AccountSearchScope::Category(work.id))
            .expect("分类搜索应成功");
        let uncategorized = document
            .search_accounts("账号", AccountSearchScope::Uncategorized)
            .expect("未分类搜索应成功");

        assert_eq!(all.len(), 3);
        assert_eq!(work_only.len(), 1);
        assert_eq!(work_only[0].username, "工作账号");
        assert_eq!(uncategorized.len(), 1);
        assert_eq!(uncategorized[0].username, "未分类账号");
        assert_eq!(
            document.search_accounts("账号", AccountSearchScope::Category(Uuid::new_v4())),
            Err(SearchAccountError::CategoryNotFound)
        );
    }

    #[test]
    fn duplicate_ids_and_blank_category_names_are_rejected() {
        let repeated_id = Uuid::new_v4();
        let mut document = VaultDocument::empty();
        document.categories = vec![
            Category {
                id: repeated_id,
                name: "工作".to_owned(),
            },
            Category {
                id: repeated_id,
                name: "个人".to_owned(),
            },
        ];
        assert_eq!(
            document.validate(),
            Err(VaultValidationError::DuplicateCategoryId)
        );

        document.categories = vec![Category {
            id: Uuid::new_v4(),
            name: "  ".to_owned(),
        }];
        assert_eq!(
            document.validate(),
            Err(VaultValidationError::BlankCategoryName)
        );

        let first = valid_account(None);
        let mut second = valid_account(None);
        second.id = first.id;
        document.categories.clear();
        document.accounts = vec![first, second];
        assert_eq!(
            document.validate(),
            Err(VaultValidationError::DuplicateAccountId)
        );
    }
}
