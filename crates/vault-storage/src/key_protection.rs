use std::fmt;

use zeroize::Zeroize;

use crate::{EnvelopeError, KeyProtectionError};

const DATA_KEY_LENGTH: usize = 32;

/// 只在内存中短暂存在的 AES-256 数据密钥。
///
/// 该类型不实现 `Clone`，并在离开作用域时主动清零自身缓冲区。
pub struct DataKey([u8; DATA_KEY_LENGTH]);

impl DataKey {
    /// 从平台解包得到的固定长度字节构造数据密钥。
    #[must_use]
    pub const fn from_bytes(bytes: [u8; DATA_KEY_LENGTH]) -> Self {
        Self(bytes)
    }

    /// 从借用缓冲直接复制到会在销毁时清零的密钥字段。
    #[must_use]
    pub fn copy_from_bytes(bytes: &[u8; DATA_KEY_LENGTH]) -> Self {
        let mut key = Self([0_u8; DATA_KEY_LENGTH]);
        key.0.copy_from_slice(bytes);
        key
    }

    pub(crate) fn generate() -> Result<Self, EnvelopeError> {
        let mut bytes = [0_u8; DATA_KEY_LENGTH];
        getrandom::fill(&mut bytes).map_err(|_| EnvelopeError::RandomFailed)?;
        Ok(Self(bytes))
    }

    /// 只向密码算法和平台包装边界借用密钥，不转移所有权。
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; DATA_KEY_LENGTH] {
        &self.0
    }
}

impl Drop for DataKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for DataKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DataKey([已隐藏])")
    }
}

/// 把数据密钥绑定到当前操作系统用户的最小平台接口。
pub trait KeyProtector {
    /// 包装固定长度数据密钥，返回可以写入加密信封的字节。
    fn protect(&self, key: &DataKey) -> Result<Vec<u8>, KeyProtectionError>;

    /// 解包信封中的平台数据，并且只返回固定长度数据密钥。
    fn unprotect(&self, wrapped_key: &[u8]) -> Result<DataKey, KeyProtectionError>;
}
