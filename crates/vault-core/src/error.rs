use thiserror::Error;

/// 密码库文档不满足领域不变量时返回的稳定错误类别。
///
/// 错误不携带用户填写的字段内容，避免上层记录错误时意外泄露敏感数据。
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum VaultValidationError {
    #[error("不支持的密码库结构版本")]
    UnsupportedSchemaVersion,
    #[error("分类标识重复")]
    DuplicateCategoryId,
    #[error("账号标识重复")]
    DuplicateAccountId,
    #[error("分类名称不能为空")]
    BlankCategoryName,
    #[error("分类名称重复")]
    DuplicateCategoryName,
    #[error("账号引用的分类不存在")]
    MissingCategory,
    #[error("账号名不能为空")]
    BlankUsername,
    #[error("密码不能为空")]
    BlankPassword,
}

/// 分类写操作返回的稳定业务错误，不携带用户输入内容。
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CategoryError {
    #[error("分类不存在")]
    NotFound,
    #[error("分类中仍有账号")]
    NotEmpty,
    #[error("分类名称不能为空")]
    BlankName,
    #[error("分类名称已经存在")]
    DuplicateName,
}
