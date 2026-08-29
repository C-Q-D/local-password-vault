//! 用例编排与已确认内存快照的所有权边界。Tauri command 只调用这里暴露的用例。

use vault_core::VaultDocument;
use vault_storage::OpenedVault;

/// 前端启动时可见的非敏感密码库摘要。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultSummary {
    pub schema_version: u32,
    pub revision: u64,
    pub category_count: usize,
    pub account_count: usize,
    pub initialized: bool,
}

/// 持有单实例 guard 和最近一次已确认密码库快照的应用状态。
pub struct VaultApplication {
    // 类型擦除让应用层不依赖 Windows，未来 macOS guard 可复用同一所有权模型。
    _instance_guard: Box<dyn Send + Sync>,
    document: VaultDocument,
    initialized: bool,
}

impl VaultApplication {
    /// 只有已经取得 guard 并完成磁盘认证后才能构造应用状态。
    #[must_use]
    pub fn new(instance_guard: impl Send + Sync + 'static, opened_vault: OpenedVault) -> Self {
        Self {
            _instance_guard: Box::new(instance_guard),
            document: opened_vault.document,
            initialized: opened_vault.initialized,
        }
    }

    /// 生成不包含路径、密钥和账号字段的只读启动摘要。
    #[must_use]
    pub fn summary(&self) -> VaultSummary {
        VaultSummary {
            schema_version: self.document.schema_version,
            revision: self.document.revision,
            category_count: self.document.categories.len(),
            account_count: self.document.accounts.len(),
            initialized: self.initialized,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;

    struct TestGuard {
        dropped: Arc<AtomicBool>,
    }

    impl Drop for TestGuard {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn application_holds_guard_and_returns_safe_empty_summary() {
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = TestGuard {
            dropped: Arc::clone(&dropped),
        };
        let application = VaultApplication::new(
            guard,
            OpenedVault {
                document: VaultDocument::empty(),
                initialized: true,
            },
        );

        assert_eq!(
            application.summary(),
            VaultSummary {
                schema_version: 1,
                revision: 0,
                category_count: 0,
                account_count: 0,
                initialized: true,
            }
        );
        assert!(!dropped.load(Ordering::SeqCst));

        drop(application);
        assert!(dropped.load(Ordering::SeqCst));
    }
}
