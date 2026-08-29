use thiserror::Error;

/// 平台密钥保护失败时使用的安全错误。
///
/// 可选平台码只用于定位 API 类别，不包含密钥或受保护数据。
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("系统密钥保护操作失败")]
pub struct KeyProtectionError {
    platform_code: Option<u32>,
}

impl KeyProtectionError {
    /// 构造不带平台错误码的失败结果。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            platform_code: None,
        }
    }

    /// 构造带内部平台错误码的失败结果。
    #[must_use]
    pub const fn with_platform_code(platform_code: u32) -> Self {
        Self {
            platform_code: Some(platform_code),
        }
    }

    /// 返回可用于内部诊断的非敏感平台错误码。
    #[must_use]
    pub const fn platform_code(self) -> Option<u32> {
        self.platform_code
    }
}

impl Default for KeyProtectionError {
    fn default() -> Self {
        Self::new()
    }
}

/// 加密信封对外暴露的稳定错误类别。
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeError {
    #[error("密码库文档不满足数据规则")]
    InvalidDocument,
    #[error("密码库文档无法编码")]
    EncodingFailed,
    #[error("无法取得安全随机数")]
    RandomFailed,
    #[error("系统密钥保护操作失败")]
    KeyProtectionFailed,
    #[error("密码库文件超过大小限制")]
    TooLarge,
    #[error("密码库文件格式无效")]
    InvalidFormat,
    #[error("密码库文件版本不受支持")]
    UnsupportedFormatVersion,
    #[error("密码库文件认证失败")]
    AuthenticationFailed,
    #[error("密码库内容无法解析")]
    InvalidPayload,
}
