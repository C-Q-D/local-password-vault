use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::VaultValidationError;

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

    /// 校验载入和保存共同依赖的领域不变量。
    ///
    /// 校验集中在这里，避免不同读写路径对损坏数据作出不一致判断。
    pub fn validate(&self) -> Result<(), VaultValidationError> {
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(VaultValidationError::UnsupportedSchemaVersion);
        }

        let mut category_ids = HashSet::with_capacity(self.categories.len());
        for category in &self.categories {
            if !category_ids.insert(category.id) {
                return Err(VaultValidationError::DuplicateCategoryId);
            }
            if category.name.trim().is_empty() {
                return Err(VaultValidationError::BlankCategoryName);
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
        }
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
