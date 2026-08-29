use crate::VaultFileError;

/// 密码库用例所需的最小主文件接口。
///
/// 平台实现负责同目录发布和有界读取，存储用例不直接拼接系统路径。
pub trait VaultFiles {
    /// 只在主库尚不存在时发布第一份完整加密信封。
    fn publish_initial(&self, candidate: &[u8]) -> Result<(), VaultFileError>;

    /// 有界读取当前主库的完整加密信封。
    fn read_main(&self) -> Result<Vec<u8>, VaultFileError>;
}
