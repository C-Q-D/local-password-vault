//! 基于 Windows 文件原语的同目录发布、替换与备份边界。

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use vault_storage::{MAX_ENVELOPE_SIZE, VaultFileError, VaultFiles};
use windows::{
    Win32::Foundation::ERROR_UNABLE_TO_MOVE_REPLACEMENT_2,
    Win32::Storage::FileSystem::{
        FileEncryptionStatusW, MoveFileW, REPLACE_FILE_FLAGS, ReplaceFileW,
    },
    core::{HRESULT, PCWSTR},
};

const MAIN_FILE_NAME: &str = "vault.dat";
const BACKUP_FILE_NAME: &str = "vault.backup.dat";
const TEMP_CREATE_ATTEMPTS: usize = 32;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 指向一个调用方明确提供的数据根目录的 Windows 主库文件实现。
#[derive(Debug, Clone)]
pub struct WindowsVaultFiles {
    root: PathBuf,
}

impl WindowsVaultFiles {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn main_path(&self) -> PathBuf {
        self.root.join(MAIN_FILE_NAME)
    }

    fn backup_path(&self) -> PathBuf {
        self.root.join(BACKUP_FILE_NAME)
    }

    fn create_owned_temp(&self) -> Result<OwnedTemp, VaultFileError> {
        for _ in 0..TEMP_CREATE_ATTEMPTS {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let name = format!(".vault-{}-{sequence}.tmp", std::process::id());
            let path = self.root.join(name);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok(OwnedTemp::new(path, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(VaultFileError::Io),
            }
        }
        Err(VaultFileError::Io)
    }
}

impl VaultFiles for WindowsVaultFiles {
    fn publish_initial(&self, candidate: &[u8]) -> Result<(), VaultFileError> {
        if candidate.is_empty() {
            return Err(VaultFileError::EmptyCandidate);
        }
        if candidate.len() > MAX_ENVELOPE_SIZE {
            return Err(VaultFileError::TooLarge);
        }

        fs::create_dir_all(&self.root).map_err(|_| VaultFileError::Io)?;
        let main_path = self.main_path();
        if main_path.try_exists().map_err(|_| VaultFileError::Io)? {
            return Err(VaultFileError::AlreadyExists);
        }

        let mut temporary = self.create_owned_temp()?;
        temporary
            .file_mut()
            .write_all(candidate)
            .map_err(|_| VaultFileError::Io)?;
        temporary
            .file_mut()
            .sync_all()
            .map_err(|_| VaultFileError::Io)?;
        // MoveFileExW 接管前先关闭写句柄，确保后续移动不受本进程文件共享模式影响。
        temporary.close();

        if main_path.try_exists().map_err(|_| VaultFileError::Io)? {
            return Err(VaultFileError::AlreadyExists);
        }

        let temp_wide = wide_path(temporary.path());
        let main_wide = wide_path(&main_path);
        // MoveFileW 在目标存在时拒绝覆盖；真实 EFS 用户目录不兼容 MoveFileExW，不能在这里替换成后者。
        let move_result = unsafe {
            MoveFileW(
                PCWSTR::from_raw(temp_wide.as_ptr()),
                PCWSTR::from_raw(main_wide.as_ptr()),
            )
        };
        if move_result.is_err() {
            return if main_path.try_exists().unwrap_or(false) {
                Err(VaultFileError::AlreadyExists)
            } else {
                Err(VaultFileError::Io)
            };
        }

        temporary.mark_published();
        Ok(())
    }

    fn read_main(&self) -> Result<Vec<u8>, VaultFileError> {
        read_bounded(&self.main_path(), "read_main")
    }

    fn replace_main(&self, candidate: &[u8]) -> Result<(), VaultFileError> {
        if candidate.is_empty() {
            return Err(VaultFileError::EmptyCandidate);
        }
        if candidate.len() > MAX_ENVELOPE_SIZE {
            return Err(VaultFileError::TooLarge);
        }

        let main_path = self.main_path();
        if !main_path.try_exists().map_err(|_| VaultFileError::Io)? {
            return Err(VaultFileError::NotFound);
        }

        if encryption_status(&main_path)? == 1 {
            let previous = self.read_main()?;
            write_synced(&self.backup_path(), &previous, true, "write_backup")?;
            return write_synced(&main_path, candidate, false, "write_efs_main");
        }

        let mut temporary = self.create_owned_temp()?;
        temporary
            .file_mut()
            .write_all(candidate)
            .map_err(|_| VaultFileError::Io)?;
        temporary
            .file_mut()
            .sync_all()
            .map_err(|_| VaultFileError::Io)?;
        temporary.close();

        let main_wide = wide_path(&main_path);
        let temp_wide = wide_path(temporary.path());
        let backup_path = self.backup_path();
        let backup_wide = wide_path(&backup_path);
        let replace_result = unsafe {
            ReplaceFileW(
                PCWSTR::from_raw(main_wide.as_ptr()),
                PCWSTR::from_raw(temp_wide.as_ptr()),
                PCWSTR::from_raw(backup_wide.as_ptr()),
                REPLACE_FILE_FLAGS(0),
                None,
                None,
            )
        };
        if let Err(error) = replace_result {
            let code = if error.code() == HRESULT::from_win32(ERROR_UNABLE_TO_MOVE_REPLACEMENT_2.0)
            {
                ERROR_UNABLE_TO_MOVE_REPLACEMENT_2.0
            } else {
                error.code().0 as u32
            };
            log_windows_error("replace_main", code);
            if let Err(recovery_error) = recover_replaced_main(code, &main_path, &backup_path) {
                log_io_error("recover_replaced_main", &recovery_error);
            }
            return Err(VaultFileError::Io);
        }

        temporary.mark_published();
        Ok(())
    }

    fn read_backup(&self) -> Result<Vec<u8>, VaultFileError> {
        read_bounded(&self.backup_path(), "read_backup")
    }

    fn restore_main(&self, candidate: &[u8]) -> Result<(), VaultFileError> {
        validate_candidate(candidate)?;
        write_synced(&self.main_path(), candidate, true, "restore_main")
    }
}

/// 1177 表示旧主库已被改名到 backup，而候选仍停留在临时名；只恢复缺失的 main。
fn recover_replaced_main(error_code: u32, main: &Path, backup: &Path) -> io::Result<bool> {
    if error_code != ERROR_UNABLE_TO_MOVE_REPLACEMENT_2.0 {
        return Ok(false);
    }
    if main.try_exists()? || !backup.try_exists()? {
        return Err(io::Error::other("1177 文件状态与恢复前置条件不一致"));
    }

    let backup_wide = wide_path(backup);
    let main_wide = wide_path(main);
    unsafe {
        MoveFileW(
            PCWSTR::from_raw(backup_wide.as_ptr()),
            PCWSTR::from_raw(main_wide.as_ptr()),
        )
    }
    .map_err(|error| io::Error::from_raw_os_error(error.code().0))?;
    Ok(true)
}

fn encryption_status(path: &Path) -> Result<u32, VaultFileError> {
    let path_wide = wide_path(path);
    let mut encryption_status = 0_u32;
    if let Err(error) = unsafe {
        FileEncryptionStatusW(PCWSTR::from_raw(path_wide.as_ptr()), &mut encryption_status)
    } {
        log_windows_error("query_encryption_status", error.code().0 as u32);
        return Err(VaultFileError::Io);
    }
    Ok(encryption_status)
}

fn validate_candidate(candidate: &[u8]) -> Result<(), VaultFileError> {
    if candidate.is_empty() {
        return Err(VaultFileError::EmptyCandidate);
    }
    if candidate.len() > MAX_ENVELOPE_SIZE {
        return Err(VaultFileError::TooLarge);
    }
    Ok(())
}

fn write_synced(
    path: &Path,
    bytes: &[u8],
    create: bool,
    operation: &str,
) -> Result<(), VaultFileError> {
    let mut options = OpenOptions::new();
    options.write(true).truncate(true).create(create);
    let mut file = options.open(path).map_err(|error| {
        log_io_error(operation, &error);
        VaultFileError::Io
    })?;
    file.write_all(bytes).map_err(|error| {
        log_io_error(operation, &error);
        VaultFileError::Io
    })?;
    file.sync_all().map_err(|error| {
        log_io_error(operation, &error);
        VaultFileError::Io
    })
}

fn read_bounded(path: &Path, operation: &str) -> Result<Vec<u8>, VaultFileError> {
    let file = File::open(path).map_err(|error| map_read_error(operation, error))?;
    let mut bytes = Vec::new();
    file.take((MAX_ENVELOPE_SIZE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            log_io_error(operation, &error);
            VaultFileError::Io
        })?;
    if bytes.len() > MAX_ENVELOPE_SIZE {
        return Err(VaultFileError::TooLarge);
    }
    Ok(bytes)
}

fn log_windows_error(operation: &str, code: u32) {
    // 只记录操作类别和系统码，便于开发定位；不输出路径或密码库内容。
    eprintln!("[vault-platform-windows] operation={operation} win32_code=0x{code:08X}");
}

fn log_io_error(operation: &str, error: &io::Error) {
    let code = error.raw_os_error().unwrap_or_default();
    eprintln!("[vault-platform-windows] operation={operation} os_code={code}");
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn map_read_error(operation: &str, error: io::Error) -> VaultFileError {
    if error.kind() == io::ErrorKind::NotFound {
        VaultFileError::NotFound
    } else {
        log_io_error(operation, &error);
        VaultFileError::Io
    }
}

/// 只清理本次通过 create_new 成功取得所有权的临时路径。
struct OwnedTemp {
    path: PathBuf,
    file: Option<File>,
    published: bool,
}

impl OwnedTemp {
    fn new(path: PathBuf, file: File) -> Self {
        Self {
            path,
            file: Some(file),
            published: false,
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("临时文件仅在发布前写入")
    }

    fn close(&mut self) {
        self.file.take();
    }

    fn mark_published(&mut self) {
        self.published = true;
    }
}

impl Drop for OwnedTemp {
    fn drop(&mut self) {
        self.file.take();
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_publish_can_be_read_back() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let files = WindowsVaultFiles::new(directory.path());
        let candidate = b"encrypted-candidate";

        files.publish_initial(candidate).expect("第一次发布应成功");

        assert_eq!(files.read_main(), Ok(candidate.to_vec()));
    }

    #[test]
    fn existing_main_is_not_overwritten() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let files = WindowsVaultFiles::new(directory.path());
        files.publish_initial(b"old").expect("第一次发布应成功");

        assert_eq!(
            files.publish_initial(b"new"),
            Err(VaultFileError::AlreadyExists)
        );
        assert_eq!(files.read_main(), Ok(b"old".to_vec()));
    }

    #[test]
    fn existing_main_can_be_atomically_replaced() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let files = WindowsVaultFiles::new(directory.path());
        files.publish_initial(b"old").expect("第一次发布应成功");

        files.replace_main(b"new").expect("已有主库应能替换");

        assert_eq!(files.read_main(), Ok(b"new".to_vec()));
        assert_eq!(files.read_backup(), Ok(b"old".to_vec()));
    }

    #[test]
    fn repeated_replacement_keeps_the_immediately_previous_main_as_backup() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let files = WindowsVaultFiles::new(directory.path());
        files.publish_initial(b"first").expect("第一次发布应成功");
        files.replace_main(b"second").expect("第一次替换应成功");

        files
            .replace_main(b"third")
            .expect("已有备份时仍应替换成功");

        assert_eq!(files.read_main(), Ok(b"third".to_vec()));
        assert_eq!(files.read_backup(), Ok(b"second".to_vec()));
    }

    #[test]
    fn error_1177_layout_restores_old_backup_without_publishing_candidate() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let main = directory.path().join(MAIN_FILE_NAME);
        let backup = directory.path().join(BACKUP_FILE_NAME);
        let candidate = directory.path().join("owned-candidate.tmp");
        fs::write(&backup, b"old-main").expect("应能模拟已改名的旧主库");
        fs::write(&candidate, b"new-candidate").expect("应能模拟仍在原名的候选");

        assert!(
            recover_replaced_main(ERROR_UNABLE_TO_MOVE_REPLACEMENT_2.0, &main, &backup)
                .expect("1177 布局应恢复成功")
        );
        assert_eq!(fs::read(&main).expect("应能读取恢复主库"), b"old-main");
        assert!(!backup.exists());
        assert_eq!(
            fs::read(&candidate).expect("候选应仍停留在临时名"),
            b"new-candidate"
        );
    }

    #[test]
    fn non_1177_error_does_not_move_backup_or_overwrite_main() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let main = directory.path().join(MAIN_FILE_NAME);
        let backup = directory.path().join(BACKUP_FILE_NAME);
        fs::write(&main, b"current-main").expect("应能创建当前主库");
        fs::write(&backup, b"previous-main").expect("应能创建已有备份");

        assert!(!recover_replaced_main(5, &main, &backup).expect("非 1177 不应恢复"));
        assert_eq!(fs::read(&main).expect("当前主库应保持"), b"current-main");
        assert_eq!(fs::read(&backup).expect("已有备份应保持"), b"previous-main");
    }

    #[test]
    fn error_1177_never_overwrites_an_existing_main() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let main = directory.path().join(MAIN_FILE_NAME);
        let backup = directory.path().join(BACKUP_FILE_NAME);
        fs::write(&main, b"existing-main").expect("应能创建现有主库");
        fs::write(&backup, b"old-main").expect("应能创建旧备份");

        assert!(
            recover_replaced_main(ERROR_UNABLE_TO_MOVE_REPLACEMENT_2.0, &main, &backup).is_err()
        );
        assert_eq!(
            fs::read(&main).expect("现有主库不得被覆盖"),
            b"existing-main"
        );
        assert_eq!(fs::read(&backup).expect("备份不得被移动"), b"old-main");
    }

    #[test]
    fn replacement_requires_existing_main_and_valid_candidate() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let files = WindowsVaultFiles::new(directory.path());
        assert_eq!(files.replace_main(b"new"), Err(VaultFileError::NotFound));

        files.publish_initial(b"old").expect("第一次发布应成功");
        assert_eq!(files.replace_main(&[]), Err(VaultFileError::EmptyCandidate));
        assert_eq!(files.read_main(), Ok(b"old".to_vec()));
    }

    #[test]
    fn empty_or_oversized_candidate_does_not_create_main() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let files = WindowsVaultFiles::new(directory.path());

        assert_eq!(
            files.publish_initial(&[]),
            Err(VaultFileError::EmptyCandidate)
        );
        assert_eq!(files.read_main(), Err(VaultFileError::NotFound));

        let oversized = vec![0_u8; MAX_ENVELOPE_SIZE + 1];
        assert_eq!(
            files.publish_initial(&oversized),
            Err(VaultFileError::TooLarge)
        );
        assert_eq!(files.read_main(), Err(VaultFileError::NotFound));
    }

    #[test]
    fn failed_publish_cleans_owned_temp_without_creating_main() {
        let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
        let root_as_file = directory.path().join("not-a-directory");
        fs::write(&root_as_file, b"blocking file").expect("应能创建阻断文件");
        let files = WindowsVaultFiles::new(&root_as_file);

        assert_eq!(files.publish_initial(b"candidate"), Err(VaultFileError::Io));
        assert!(!root_as_file.join(MAIN_FILE_NAME).exists());
    }
}
