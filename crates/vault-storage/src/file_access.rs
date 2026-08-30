use crate::VaultFileError;

/// 密码库用例所需的最小主文件接口。
///
/// 平台实现负责同目录发布和有界读取，存储用例不直接拼接系统路径。
pub trait VaultFiles {
    /// 只在主库尚不存在时发布第一份完整加密信封。
    fn publish_initial(&self, candidate: &[u8]) -> Result<(), VaultFileError>;

    /// 用完整候选替换已经存在的主库；返回成功时必须已提交调用方提供的完整原字节，
    /// 目标不存在时不得隐式创建。
    fn replace_main(&self, candidate: &[u8]) -> Result<(), VaultFileError>;

    /// 有界读取当前主库的完整加密信封。
    fn read_main(&self) -> Result<Vec<u8>, VaultFileError>;

    /// 读取上一份已同步完成的备份；平台不支持时返回 NotFound。
    fn read_backup(&self) -> Result<Vec<u8>, VaultFileError> {
        Err(VaultFileError::NotFound)
    }

    /// 用已经认证的完整备份字节恢复主库；平台实现必须保持固定备份只读。
    fn restore_main(&self, candidate: &[u8]) -> Result<(), VaultFileError> {
        self.replace_main(candidate)
    }
}
