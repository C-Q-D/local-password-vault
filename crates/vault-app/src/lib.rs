//! 用例编排与已确认内存快照的所有权边界。Tauri command 只调用这里暴露的用例。

use std::sync::Mutex;

use uuid::Uuid;
use vault_core::{
    AccountError, CategoryError, CreateAccount, DeleteAccountError, MoveAccountError,
    UpdateAccount, UpdateAccountError, VaultDocument,
};
use vault_storage::{OpenedVault, VaultOpenError};

/// 分类在前端可见的安全摘要，不包含任何账号字段或密码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategorySummary {
    pub id: String,
    pub name: String,
    pub account_count: usize,
}

/// 账号列表只返回定位和展示所需字段，明文密码始终留在 Rust 内存边界内。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    pub id: String,
    pub category_id: Option<String>,
    pub username: String,
    pub address: Option<String>,
    pub email: Option<String>,
}

/// Tauri 传入应用层的新增账号数据；分类 ID 在当前确认快照内重新验证。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAccountInput {
    pub category_id: Option<String>,
    pub username: String,
    pub password: String,
    pub address: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
}

/// 单条账号编辑详情；只由按 ID 的专用读取用例返回，不进入全量列表快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountEditDetail {
    pub id: String,
    pub category_id: Option<String>,
    pub username: String,
    pub password: String,
    pub address: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
}

/// 账号编辑输入不包含账号 ID；字段和分类归属在同一候选保存中更新。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAccountInput {
    pub category_id: Option<String>,
    pub username: String,
    pub password: String,
    pub address: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
}

/// 前端启动和分类写操作共同返回的非敏感密码库快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub category_count: usize,
    pub account_count: usize,
    pub initialized: bool,
    pub categories: Vec<CategorySummary>,
    pub uncategorized_count: usize,
    pub accounts: Vec<AccountSummary>,
}

/// 应用层只依赖“保存后返回磁盘确认文档”这一条最小持久化能力。
pub trait VaultRepository: Send + Sync {
    fn save_confirmed(&self, candidate: &VaultDocument) -> Result<VaultDocument, VaultOpenError>;
}

/// 分类用例向 Tauri 暴露的稳定错误，不携带用户输入或本地路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultApplicationError {
    CategoryNotFound,
    CategoryNotEmpty,
    CategoryNameEmpty,
    CategoryNameDuplicate,
    AccountUsernameRequired,
    AccountPasswordRequired,
    AccountNotFound,
    VaultSaveFailed,
    StateUnavailable,
}

struct ConfirmedState {
    document: VaultDocument,
    initialized: bool,
}

/// 持有单实例 guard、持久化边界和最近一次已确认密码库快照的应用状态。
pub struct VaultApplication {
    // 类型擦除让应用层不依赖 Windows，未来 macOS guard 可复用同一所有权模型。
    _instance_guard: Box<dyn Send + Sync>,
    repository: Box<dyn VaultRepository>,
    state: Mutex<ConfirmedState>,
}

impl VaultApplication {
    /// 只有已经取得 guard 并完成磁盘认证后才能构造应用状态。
    #[must_use]
    pub fn new(
        instance_guard: impl Send + Sync + 'static,
        opened_vault: OpenedVault,
        repository: impl VaultRepository + 'static,
    ) -> Self {
        Self {
            _instance_guard: Box::new(instance_guard),
            repository: Box::new(repository),
            state: Mutex::new(ConfirmedState {
                document: opened_vault.document,
                initialized: opened_vault.initialized,
            }),
        }
    }

    /// 生成不包含路径、密钥、密码和备注的当前确认快照。
    pub fn snapshot(&self) -> Result<VaultSnapshot, VaultApplicationError> {
        let state = self
            .state
            .lock()
            .map_err(|_| VaultApplicationError::StateUnavailable)?;
        Ok(snapshot_from(&state))
    }

    /// 创建分类并持久化；只有磁盘回读认证成功后才发布新内存快照。
    pub fn create_category(&self, name: &str) -> Result<VaultSnapshot, VaultApplicationError> {
        // 写入期间持有互斥锁，使 revision 和候选文档始终来自同一确认版本。
        let mut state = self
            .state
            .lock()
            .map_err(|_| VaultApplicationError::StateUnavailable)?;
        let mut candidate = state.document.clone();
        candidate
            .create_category(name)
            .map_err(map_category_error)?;
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or(VaultApplicationError::VaultSaveFailed)?;

        let confirmed = self
            .repository
            .save_confirmed(&candidate)
            .map_err(|_| VaultApplicationError::VaultSaveFailed)?;
        state.document = confirmed;
        Ok(snapshot_from(&state))
    }

    /// 重命名分类并持久化；ID 无效与分类不存在使用同一安全错误。
    pub fn rename_category(
        &self,
        id: &str,
        name: &str,
    ) -> Result<VaultSnapshot, VaultApplicationError> {
        let category_id =
            Uuid::parse_str(id).map_err(|_| VaultApplicationError::CategoryNotFound)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| VaultApplicationError::StateUnavailable)?;
        let mut candidate = state.document.clone();
        candidate
            .rename_category(category_id, name)
            .map_err(map_category_error)?;
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or(VaultApplicationError::VaultSaveFailed)?;

        let confirmed = self
            .repository
            .save_confirmed(&candidate)
            .map_err(|_| VaultApplicationError::VaultSaveFailed)?;
        state.document = confirmed;
        Ok(snapshot_from(&state))
    }

    /// 删除空分类并持久化；核心层会在紧邻写入前重新检查账号引用。
    pub fn delete_category(&self, id: &str) -> Result<VaultSnapshot, VaultApplicationError> {
        let category_id =
            Uuid::parse_str(id).map_err(|_| VaultApplicationError::CategoryNotFound)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| VaultApplicationError::StateUnavailable)?;
        let mut candidate = state.document.clone();
        candidate
            .delete_category(category_id)
            .map_err(map_category_error)?;
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or(VaultApplicationError::VaultSaveFailed)?;

        let confirmed = self
            .repository
            .save_confirmed(&candidate)
            .map_err(|_| VaultApplicationError::VaultSaveFailed)?;
        state.document = confirmed;
        Ok(snapshot_from(&state))
    }

    /// 新增账号并持久化；只有磁盘确认成功后才发布不含密码的列表快照。
    pub fn create_account(
        &self,
        input: CreateAccountInput,
    ) -> Result<VaultSnapshot, VaultApplicationError> {
        let category_id = input
            .category_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| VaultApplicationError::CategoryNotFound)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| VaultApplicationError::StateUnavailable)?;
        let mut candidate = state.document.clone();
        candidate
            .create_account(CreateAccount {
                category_id,
                username: input.username,
                password: input.password,
                address: input.address,
                email: input.email,
                notes: input.notes,
            })
            .map_err(map_account_error)?;
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or(VaultApplicationError::VaultSaveFailed)?;

        let confirmed = self
            .repository
            .save_confirmed(&candidate)
            .map_err(|_| VaultApplicationError::VaultSaveFailed)?;
        state.document = confirmed;
        Ok(snapshot_from(&state))
    }

    /// 按稳定 ID 读取一条编辑详情；不存在批量返回明文密码的接口。
    pub fn account_for_edit(&self, id: &str) -> Result<AccountEditDetail, VaultApplicationError> {
        let account_id = Uuid::parse_str(id).map_err(|_| VaultApplicationError::AccountNotFound)?;
        let state = self
            .state
            .lock()
            .map_err(|_| VaultApplicationError::StateUnavailable)?;
        let account = state
            .document
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .ok_or(VaultApplicationError::AccountNotFound)?;
        Ok(AccountEditDetail {
            id: account.id.to_string(),
            category_id: account.category_id.map(|id| id.to_string()),
            username: account.username.clone(),
            password: account.password.clone(),
            address: account.address.clone(),
            email: account.email.clone(),
            notes: account.notes.clone(),
        })
    }

    /// 按稳定 ID 提交字段和分类编辑；只有整份候选经磁盘确认后才替换应用快照。
    pub fn update_account(
        &self,
        id: &str,
        input: UpdateAccountInput,
    ) -> Result<VaultSnapshot, VaultApplicationError> {
        let account_id = Uuid::parse_str(id).map_err(|_| VaultApplicationError::AccountNotFound)?;
        let category_id = input
            .category_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| VaultApplicationError::CategoryNotFound)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| VaultApplicationError::StateUnavailable)?;
        let mut candidate = state.document.clone();
        candidate
            .update_account(
                account_id,
                UpdateAccount {
                    username: input.username,
                    password: input.password,
                    address: input.address,
                    email: input.email,
                    notes: input.notes,
                },
            )
            .map_err(map_update_account_error)?;
        // 分类存在性在持锁后的当前候选上重新验证，不能相信前端较早取得的分类列表。
        candidate
            .move_account(account_id, category_id)
            .map_err(map_move_account_error)?;
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or(VaultApplicationError::VaultSaveFailed)?;

        let confirmed = self
            .repository
            .save_confirmed(&candidate)
            .map_err(|_| VaultApplicationError::VaultSaveFailed)?;
        state.document = confirmed;
        Ok(snapshot_from(&state))
    }

    /// 按稳定 ID 删除账号；只有磁盘确认成功后才发布不含目标账号的新快照。
    pub fn delete_account(&self, id: &str) -> Result<VaultSnapshot, VaultApplicationError> {
        let account_id = Uuid::parse_str(id).map_err(|_| VaultApplicationError::AccountNotFound)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| VaultApplicationError::StateUnavailable)?;
        let mut candidate = state.document.clone();
        candidate
            .delete_account(account_id)
            .map_err(map_delete_account_error)?;
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or(VaultApplicationError::VaultSaveFailed)?;

        let confirmed = self
            .repository
            .save_confirmed(&candidate)
            .map_err(|_| VaultApplicationError::VaultSaveFailed)?;
        state.document = confirmed;
        Ok(snapshot_from(&state))
    }
}

fn map_category_error(error: CategoryError) -> VaultApplicationError {
    match error {
        CategoryError::NotFound => VaultApplicationError::CategoryNotFound,
        CategoryError::NotEmpty => VaultApplicationError::CategoryNotEmpty,
        CategoryError::BlankName => VaultApplicationError::CategoryNameEmpty,
        CategoryError::DuplicateName => VaultApplicationError::CategoryNameDuplicate,
    }
}

fn map_account_error(error: AccountError) -> VaultApplicationError {
    match error {
        AccountError::UsernameRequired => VaultApplicationError::AccountUsernameRequired,
        AccountError::PasswordRequired => VaultApplicationError::AccountPasswordRequired,
        AccountError::CategoryNotFound => VaultApplicationError::CategoryNotFound,
    }
}

fn map_update_account_error(error: UpdateAccountError) -> VaultApplicationError {
    match error {
        UpdateAccountError::NotFound => VaultApplicationError::AccountNotFound,
        UpdateAccountError::UsernameRequired => VaultApplicationError::AccountUsernameRequired,
        UpdateAccountError::PasswordRequired => VaultApplicationError::AccountPasswordRequired,
    }
}

fn map_move_account_error(error: MoveAccountError) -> VaultApplicationError {
    match error {
        MoveAccountError::AccountNotFound => VaultApplicationError::AccountNotFound,
        MoveAccountError::CategoryNotFound => VaultApplicationError::CategoryNotFound,
    }
}

fn map_delete_account_error(error: DeleteAccountError) -> VaultApplicationError {
    match error {
        DeleteAccountError::NotFound => VaultApplicationError::AccountNotFound,
    }
}

fn snapshot_from(state: &ConfirmedState) -> VaultSnapshot {
    let categories = state
        .document
        .categories
        .iter()
        .map(|category| CategorySummary {
            id: category.id.to_string(),
            name: category.name.clone(),
            account_count: state
                .document
                .accounts
                .iter()
                .filter(|account| account.category_id == Some(category.id))
                .count(),
        })
        .collect::<Vec<_>>();

    VaultSnapshot {
        schema_version: state.document.schema_version,
        revision: state.document.revision,
        category_count: categories.len(),
        account_count: state.document.accounts.len(),
        initialized: state.initialized,
        categories,
        uncategorized_count: state
            .document
            .accounts
            .iter()
            .filter(|account| account.category_id.is_none())
            .count(),
        accounts: state
            .document
            .accounts
            .iter()
            .map(|account| AccountSummary {
                id: account.id.to_string(),
                category_id: account.category_id.map(|id| id.to_string()),
                username: account.username.clone(),
                address: account.address.clone(),
                email: account.email.clone(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    use vault_core::Account;
    use vault_storage::{VaultFileError, VaultOpenError};

    use super::*;

    struct TestGuard {
        dropped: Arc<AtomicBool>,
    }

    impl Drop for TestGuard {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    struct TestRepository {
        save_count: Arc<AtomicUsize>,
        fail: bool,
    }

    impl VaultRepository for TestRepository {
        fn save_confirmed(
            &self,
            candidate: &VaultDocument,
        ) -> Result<VaultDocument, VaultOpenError> {
            self.save_count.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(VaultOpenError::File(VaultFileError::Io))
            } else {
                Ok(candidate.clone())
            }
        }
    }

    fn application(fail_save: bool) -> (VaultApplication, Arc<AtomicBool>, Arc<AtomicUsize>) {
        application_with_document(fail_save, VaultDocument::empty())
    }

    fn application_with_document(
        fail_save: bool,
        document: VaultDocument,
    ) -> (VaultApplication, Arc<AtomicBool>, Arc<AtomicUsize>) {
        let dropped = Arc::new(AtomicBool::new(false));
        let save_count = Arc::new(AtomicUsize::new(0));
        let application = VaultApplication::new(
            TestGuard {
                dropped: Arc::clone(&dropped),
            },
            OpenedVault {
                document,
                initialized: true,
            },
            TestRepository {
                save_count: Arc::clone(&save_count),
                fail: fail_save,
            },
        );
        (application, dropped, save_count)
    }

    #[test]
    fn application_holds_guard_and_returns_safe_empty_snapshot() {
        let (application, dropped, _) = application(false);

        assert_eq!(application.snapshot().expect("快照应可读取").revision, 0);
        assert!(
            application
                .snapshot()
                .expect("快照应可读取")
                .categories
                .is_empty()
        );
        assert!(!dropped.load(Ordering::SeqCst));

        drop(application);
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn create_category_publishes_confirmed_snapshot_and_increments_once() {
        let (application, _, save_count) = application(false);

        let snapshot = application
            .create_category("  工作  ")
            .expect("有效分类应创建成功");

        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.category_count, 1);
        assert_eq!(snapshot.categories[0].name, "工作");
        assert_eq!(snapshot.categories[0].account_count, 0);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
        assert_eq!(application.snapshot().expect("快照应可读取"), snapshot);
    }

    #[test]
    fn invalid_name_does_not_save_or_change_confirmed_snapshot() {
        let (application, _, save_count) = application(false);
        let before = application.snapshot().expect("快照应可读取");

        assert_eq!(
            application.create_category("   "),
            Err(VaultApplicationError::CategoryNameEmpty)
        );
        application
            .create_category("Work")
            .expect("首个分类应创建成功");
        let confirmed = application.snapshot().expect("快照应可读取");
        assert_eq!(
            application.create_category(" work "),
            Err(VaultApplicationError::CategoryNameDuplicate)
        );

        assert_eq!(before.revision, 0);
        assert_eq!(application.snapshot().expect("快照应可读取"), confirmed);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn save_failure_keeps_previous_confirmed_snapshot() {
        let (application, _, save_count) = application(true);
        let before = application.snapshot().expect("快照应可读取");

        assert_eq!(
            application.create_category("工作"),
            Err(VaultApplicationError::VaultSaveFailed)
        );

        assert_eq!(application.snapshot().expect("快照应可读取"), before);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn rename_category_publishes_once_and_rejects_unknown_id() {
        let (application, _, save_count) = application(false);
        let created = application.create_category("工作").expect("分类应创建成功");
        let category_id = created.categories[0].id.clone();

        let renamed = application
            .rename_category(&category_id, "开发")
            .expect("分类应重命名成功");

        assert_eq!(renamed.revision, 2);
        assert_eq!(renamed.categories[0].name, "开发");
        assert_eq!(save_count.load(Ordering::SeqCst), 2);
        let confirmed = application.snapshot().expect("快照应可读取");
        assert_eq!(
            application.rename_category("invalid-id", "其他"),
            Err(VaultApplicationError::CategoryNotFound)
        );
        assert_eq!(application.snapshot().expect("快照应可读取"), confirmed);
        assert_eq!(save_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn rename_save_failure_keeps_previous_confirmed_snapshot() {
        let mut document = VaultDocument::empty();
        let category = document.create_category("工作").expect("分类应创建成功");
        let (application, _, save_count) = application_with_document(true, document);
        let before = application.snapshot().expect("快照应可读取");

        assert_eq!(
            application.rename_category(&category.id.to_string(), "开发"),
            Err(VaultApplicationError::VaultSaveFailed)
        );

        assert_eq!(application.snapshot().expect("快照应可读取"), before);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn delete_category_publishes_once_and_removes_target() {
        let (application, _, save_count) = application(false);
        let created = application.create_category("工作").expect("分类应创建成功");

        let deleted = application
            .delete_category(&created.categories[0].id)
            .expect("空分类应删除成功");

        assert_eq!(deleted.revision, 2);
        assert!(deleted.categories.is_empty());
        assert_eq!(save_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn delete_save_failure_keeps_previous_confirmed_snapshot() {
        let mut document = VaultDocument::empty();
        let category = document.create_category("工作").expect("分类应创建成功");
        let (application, _, save_count) = application_with_document(true, document);
        let before = application.snapshot().expect("快照应可读取");

        assert_eq!(
            application.delete_category(&category.id.to_string()),
            Err(VaultApplicationError::VaultSaveFailed)
        );

        assert_eq!(application.snapshot().expect("快照应可读取"), before);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn delete_non_empty_category_is_rejected_before_save() {
        let mut document = VaultDocument::empty();
        let category = document.create_category("工作").expect("分类应创建成功");
        document.accounts.push(Account {
            id: Uuid::new_v4(),
            category_id: Some(category.id),
            username: "测试账号".to_owned(),
            password: "仅用于隔离测试".to_owned(),
            address: None,
            email: None,
            notes: None,
        });
        let (application, _, save_count) = application_with_document(false, document);
        let before = application.snapshot().expect("快照应可读取");

        assert_eq!(
            application.delete_category(&category.id.to_string()),
            Err(VaultApplicationError::CategoryNotEmpty)
        );

        assert_eq!(application.snapshot().expect("快照应可读取"), before);
        assert_eq!(save_count.load(Ordering::SeqCst), 0);
    }

    fn account_input(category_id: Option<String>, username: &str) -> CreateAccountInput {
        CreateAccountInput {
            category_id,
            username: username.to_owned(),
            password: "仅用于隔离测试的密码".to_owned(),
            address: Some("https://example.invalid".to_owned()),
            email: None,
            notes: None,
        }
    }

    #[test]
    fn create_account_publishes_safe_confirmed_snapshot_once() {
        let mut document = VaultDocument::empty();
        let category = document.create_category("工作").expect("分类应创建成功");
        let (application, _, save_count) = application_with_document(false, document);

        let first = application
            .create_account(account_input(Some(category.id.to_string()), "工作账号"))
            .expect("首个账号应创建成功");
        let second = application
            .create_account(account_input(None, "个人账号"))
            .expect("同地址账号应独立创建");

        assert_eq!(first.revision, 1);
        assert_eq!(second.revision, 2);
        assert_eq!(second.accounts.len(), 2);
        assert_ne!(second.accounts[0].id, second.accounts[1].id);
        assert_eq!(second.categories[0].account_count, 1);
        assert_eq!(second.uncategorized_count, 1);
        assert_eq!(save_count.load(Ordering::SeqCst), 2);
        assert_eq!(application.snapshot().expect("快照应可读取"), second);
    }

    #[test]
    fn create_account_failure_keeps_previous_confirmed_snapshot() {
        let (application, _, save_count) = application(true);
        let before = application.snapshot().expect("快照应可读取");

        assert_eq!(
            application.create_account(account_input(None, "有效账号")),
            Err(VaultApplicationError::VaultSaveFailed)
        );
        assert_eq!(application.snapshot().expect("快照应可读取"), before);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);

        let mut invalid = account_input(None, "  ");
        invalid.password = " ".to_owned();
        assert_eq!(
            application.create_account(invalid),
            Err(VaultApplicationError::AccountUsernameRequired)
        );
        assert_eq!(application.snapshot().expect("快照应可读取"), before);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn account_for_edit_returns_only_requested_full_record() {
        let mut document = VaultDocument::empty();
        let first = document
            .create_account(CreateAccount {
                category_id: None,
                username: "目标账号".to_owned(),
                password: "目标密码".to_owned(),
                address: Some("https://target.invalid".to_owned()),
                email: Some("target@example.invalid".to_owned()),
                notes: Some("目标备注".to_owned()),
            })
            .expect("目标账号应创建成功");
        document
            .create_account(CreateAccount {
                category_id: None,
                username: "其他账号".to_owned(),
                password: "其他密码".to_owned(),
                address: None,
                email: None,
                notes: None,
            })
            .expect("其他账号应创建成功");
        let (application, _, _) = application_with_document(false, document);

        let detail = application
            .account_for_edit(&first.id.to_string())
            .expect("目标详情应可读取");

        assert_eq!(detail.id, first.id.to_string());
        assert_eq!(detail.username, "目标账号");
        assert_eq!(detail.password, "目标密码");
        assert_eq!(detail.notes.as_deref(), Some("目标备注"));
        assert_eq!(
            application.account_for_edit("invalid-id"),
            Err(VaultApplicationError::AccountNotFound)
        );
    }

    #[test]
    fn update_account_publishes_once_and_failure_keeps_confirmed_snapshot() {
        let mut document = VaultDocument::empty();
        let source = document.create_category("工作").expect("源分类应创建成功");
        let target_category = document
            .create_category("个人")
            .expect("目标分类应创建成功");
        let target = document
            .create_account(CreateAccount {
                category_id: Some(source.id),
                username: "旧账号".to_owned(),
                password: "旧密码".to_owned(),
                address: None,
                email: None,
                notes: None,
            })
            .expect("目标账号应创建成功");
        let (application, _, save_count) = application_with_document(false, document.clone());
        let updated = application
            .update_account(
                &target.id.to_string(),
                UpdateAccountInput {
                    category_id: Some(target_category.id.to_string()),
                    username: "新账号".to_owned(),
                    password: "新密码".to_owned(),
                    address: Some("https://new.invalid".to_owned()),
                    email: None,
                    notes: Some("新备注".to_owned()),
                },
            )
            .expect("账号编辑应保存成功");

        assert_eq!(updated.revision, 1);
        assert_eq!(updated.accounts[0].username, "新账号");
        assert_eq!(
            updated.accounts[0].category_id,
            Some(target_category.id.to_string())
        );
        assert_eq!(updated.categories[0].account_count, 0);
        assert_eq!(updated.categories[1].account_count, 1);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            application
                .account_for_edit(&target.id.to_string())
                .expect("新详情应可读取")
                .password,
            "新密码"
        );

        let (failing, _, failing_count) = application_with_document(true, document);
        let before = failing.snapshot().expect("旧快照应可读取");
        assert_eq!(
            failing.update_account(
                &target.id.to_string(),
                UpdateAccountInput {
                    category_id: None,
                    username: "不会保存".to_owned(),
                    password: "不会保存".to_owned(),
                    address: None,
                    email: None,
                    notes: None,
                },
            ),
            Err(VaultApplicationError::VaultSaveFailed)
        );
        assert_eq!(failing.snapshot().expect("快照应可读取"), before);
        assert_eq!(failing_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn update_account_rejects_unknown_category_before_save_and_keeps_all_fields() {
        let mut document = VaultDocument::empty();
        let target = document
            .create_account(CreateAccount {
                category_id: None,
                username: "旧账号".to_owned(),
                password: "旧密码".to_owned(),
                address: None,
                email: None,
                notes: None,
            })
            .expect("目标账号应创建成功");
        let (application, _, save_count) = application_with_document(false, document);
        let before = application.snapshot().expect("旧快照应可读取");

        assert_eq!(
            application.update_account(
                &target.id.to_string(),
                UpdateAccountInput {
                    category_id: Some(Uuid::new_v4().to_string()),
                    username: "不能部分保存".to_owned(),
                    password: "不能部分保存".to_owned(),
                    address: Some("https://new.invalid".to_owned()),
                    email: None,
                    notes: None,
                },
            ),
            Err(VaultApplicationError::CategoryNotFound)
        );
        assert_eq!(application.snapshot().expect("快照应可读取"), before);
        assert_eq!(save_count.load(Ordering::SeqCst), 0);
        let detail = application
            .account_for_edit(&target.id.to_string())
            .expect("旧详情应可读取");
        assert_eq!(detail.username, "旧账号");
        assert_eq!(detail.password, "旧密码");
        assert_eq!(detail.category_id, None);
    }

    #[test]
    fn update_account_can_move_to_uncategorized_and_updates_counts() {
        let mut document = VaultDocument::empty();
        let category = document.create_category("工作").expect("分类应创建成功");
        let target = document
            .create_account(CreateAccount {
                category_id: Some(category.id),
                username: "工作账号".to_owned(),
                password: "测试密码".to_owned(),
                address: None,
                email: None,
                notes: None,
            })
            .expect("目标账号应创建成功");
        let (application, _, save_count) = application_with_document(false, document);

        let updated = application
            .update_account(
                &target.id.to_string(),
                UpdateAccountInput {
                    category_id: None,
                    username: "工作账号".to_owned(),
                    password: "测试密码".to_owned(),
                    address: None,
                    email: None,
                    notes: None,
                },
            )
            .expect("账号应移动到未分类");

        assert_eq!(updated.categories[0].account_count, 0);
        assert_eq!(updated.uncategorized_count, 1);
        assert_eq!(updated.accounts[0].category_id, None);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn delete_account_publishes_once_and_updates_category_count() {
        let mut document = VaultDocument::empty();
        let category = document.create_category("工作").expect("分类应创建成功");
        let target = document
            .create_account(CreateAccount {
                category_id: Some(category.id),
                username: "待删除账号".to_owned(),
                password: "测试密码".to_owned(),
                address: Some("https://same.invalid".to_owned()),
                email: None,
                notes: None,
            })
            .expect("目标账号应创建成功");
        let retained = document
            .create_account(CreateAccount {
                category_id: Some(category.id),
                username: "保留账号".to_owned(),
                password: "测试密码".to_owned(),
                address: Some("https://same.invalid".to_owned()),
                email: None,
                notes: None,
            })
            .expect("同地址账号应创建成功");
        let (application, _, save_count) = application_with_document(false, document);

        let deleted = application
            .delete_account(&target.id.to_string())
            .expect("目标账号应删除成功");

        assert_eq!(deleted.revision, 1);
        assert_eq!(deleted.account_count, 1);
        assert_eq!(deleted.categories[0].account_count, 1);
        assert_eq!(deleted.accounts[0].id, retained.id.to_string());
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn delete_account_failure_keeps_confirmed_snapshot() {
        let mut document = VaultDocument::empty();
        let target = document
            .create_account(CreateAccount {
                category_id: None,
                username: "保留账号".to_owned(),
                password: "测试密码".to_owned(),
                address: None,
                email: None,
                notes: None,
            })
            .expect("账号应创建成功");
        let (application, _, save_count) = application_with_document(true, document.clone());
        let before = application.snapshot().expect("旧快照应可读取");

        assert_eq!(
            application.delete_account(&target.id.to_string()),
            Err(VaultApplicationError::VaultSaveFailed)
        );
        assert_eq!(application.snapshot().expect("旧快照应可读取"), before);
        assert_eq!(save_count.load(Ordering::SeqCst), 1);

        let (valid, _, valid_save_count) = application_with_document(false, document);
        assert_eq!(
            valid.delete_account("invalid-id"),
            Err(VaultApplicationError::AccountNotFound)
        );
        assert_eq!(valid_save_count.load(Ordering::SeqCst), 0);
    }
}
