use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CategoryError, VaultValidationError};

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
