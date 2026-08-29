//! 用例编排与已确认内存快照的所有权边界。Tauri command 只调用这里暴露的用例。

use std::sync::Mutex;

use vault_core::{CategoryError, VaultDocument};
use vault_storage::{OpenedVault, VaultOpenError};

/// 分类在前端可见的安全摘要，不包含任何账号字段或密码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategorySummary {
    pub id: String,
    pub name: String,
    pub account_count: usize,
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
}

/// 应用层只依赖“保存后返回磁盘确认文档”这一条最小持久化能力。
pub trait VaultRepository: Send + Sync {
    fn save_confirmed(&self, candidate: &VaultDocument) -> Result<VaultDocument, VaultOpenError>;
}

/// 分类用例向 Tauri 暴露的稳定错误，不携带用户输入或本地路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultApplicationError {
    CategoryNameEmpty,
    CategoryNameDuplicate,
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

    /// 生成不包含路径、密钥和账号字段的当前确认快照。
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
}

fn map_category_error(error: CategoryError) -> VaultApplicationError {
    match error {
        CategoryError::BlankName => VaultApplicationError::CategoryNameEmpty,
        CategoryError::DuplicateName => VaultApplicationError::CategoryNameDuplicate,
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
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

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
        let dropped = Arc::new(AtomicBool::new(false));
        let save_count = Arc::new(AtomicUsize::new(0));
        let application = VaultApplication::new(
            TestGuard {
                dropped: Arc::clone(&dropped),
            },
            OpenedVault {
                document: VaultDocument::empty(),
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
}
