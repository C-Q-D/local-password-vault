//! 密码库领域模型与业务规则。该 crate 不依赖 Tauri、存储或操作系统 API。

mod error;
mod vault;

pub use error::{
    AccountError, AccountUsageError, CategoryError, DeleteAccountError, MoveAccountError,
    SearchAccountError, UpdateAccountError, VaultValidationError,
};
pub use vault::{
    Account, AccountSearchResult, AccountSearchScope, CURRENT_SCHEMA_VERSION, Category,
    CreateAccount, UpdateAccount, VaultDocument,
};
