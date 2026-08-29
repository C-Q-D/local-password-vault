//! Windows 单实例与数据根独占锁边界。

use std::{fs, os::windows::ffi::OsStrExt, path::Path};

use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_SHARING_VIOLATION, GENERIC_READ, GENERIC_WRITE, HANDLE},
        Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_NONE, OPEN_ALWAYS},
    },
    core::PCWSTR,
};

const LOCK_FILE_NAME: &str = "vault.lock";

/// 获取数据根独占锁时返回的稳定错误类别。
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ProcessLockError {
    #[error("密码库已被另一个实例占用")]
    Busy,
    #[error("无法获取密码库实例锁")]
    Io,
}

/// 在其生命周期内持有 Windows 独占文件句柄的单实例 guard。
#[derive(Debug)]
#[must_use = "guard 必须由应用状态持有，否则锁会立即释放"]
pub struct ProcessLockGuard {
    handle: HANDLE,
}

impl ProcessLockGuard {
    /// 在指定数据目录打开或创建 `vault.lock`，并拒绝任何共享打开。
    pub fn acquire(data_root: &Path) -> Result<Self, ProcessLockError> {
        fs::create_dir_all(data_root).map_err(|_| ProcessLockError::Io)?;
        let lock_path = data_root.join(LOCK_FILE_NAME);
        let wide_path: Vec<u16> = lock_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let access = GENERIC_READ.0 | GENERIC_WRITE.0;
        let result = unsafe {
            CreateFileW(
                PCWSTR::from_raw(wide_path.as_ptr()),
                access,
                FILE_SHARE_NONE,
                None,
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };
        match result {
            Ok(handle) => Ok(Self { handle }),
            Err(error) if error.code() == ERROR_SHARING_VIOLATION.to_hresult() => {
                Err(ProcessLockError::Busy)
            }
            Err(_) => Err(ProcessLockError::Io),
        }
    }
}

impl Drop for ProcessLockGuard {
    fn drop(&mut self) {
        // 进程异常退出时 Windows 也会回收句柄；正常退出在这里立即释放共享限制。
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_guard_is_busy_until_first_guard_drops() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let first = ProcessLockGuard::acquire(directory.path()).expect("第一次获取锁应成功");

        assert!(matches!(
            ProcessLockGuard::acquire(directory.path()),
            Err(ProcessLockError::Busy)
        ));

        drop(first);
        let reacquired = ProcessLockGuard::acquire(directory.path()).expect("释放后应能重新获取");
        drop(reacquired);
    }

    #[test]
    fn invalid_data_root_returns_io_instead_of_busy() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let root_as_file = directory.path().join("not-a-directory");
        fs::write(&root_as_file, b"blocking file").expect("应能创建阻断文件");

        assert!(matches!(
            ProcessLockGuard::acquire(&root_as_file),
            Err(ProcessLockError::Io)
        ));
    }
}
