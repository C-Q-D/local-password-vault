use vault_core::VaultDocument;

use crate::{
    KeyProtector, VaultFileError, VaultFiles, VaultOpenError, open as open_envelope, seal,
};

/// 经过磁盘读取和认证后确认可供应用使用的密码库状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedVault {
    pub document: VaultDocument,
    pub initialized: bool,
}

/// 加载已有主库，或者只在主库确实不存在时创建并重新确认空库。
pub fn open_or_initialize(
    files: &impl VaultFiles,
    key_protector: &impl KeyProtector,
) -> Result<OpenedVault, VaultOpenError> {
    match files.read_main() {
        Ok(envelope) => match open_confirmed(&envelope, key_protector, false) {
            Ok(opened) => Ok(opened),
            Err(main_error) => recover_backup(files, key_protector)?.ok_or(main_error),
        },
        Err(VaultFileError::NotFound) => recover_backup(files, key_protector)?
            .map_or_else(|| initialize(files, key_protector), Ok),
        Err(error) => recover_backup(files, key_protector)?.ok_or(VaultOpenError::File(error)),
    }
}

/// 备份必须先通过完整认证和领域校验，之后才允许恢复为主库。
fn recover_backup(
    files: &impl VaultFiles,
    key_protector: &impl KeyProtector,
) -> Result<Option<OpenedVault>, VaultOpenError> {
    let backup = match files.read_backup() {
        Ok(backup) => backup,
        Err(VaultFileError::NotFound) => return Ok(None),
        Err(error) => return Err(VaultOpenError::File(error)),
    };
    open_confirmed(&backup, key_protector, false)?;
    files.restore_main(&backup).map_err(VaultOpenError::File)?;
    let restored = files.read_main().map_err(VaultOpenError::File)?;
    Ok(Some(open_confirmed(&restored, key_protector, false)?))
}

/// 保存完整候选后重新读取和认证磁盘主库，只返回已经确认的文档。
pub fn save_confirmed(
    files: &impl VaultFiles,
    key_protector: &impl KeyProtector,
    candidate: &VaultDocument,
) -> Result<VaultDocument, VaultOpenError> {
    let envelope = seal(candidate, key_protector).map_err(VaultOpenError::Envelope)?;
    files
        .replace_main(&envelope)
        .map_err(VaultOpenError::File)?;
    let confirmed_envelope = files.read_main().map_err(VaultOpenError::File)?;
    open_envelope(&confirmed_envelope, key_protector).map_err(VaultOpenError::Envelope)
}

fn initialize(
    files: &impl VaultFiles,
    key_protector: &impl KeyProtector,
) -> Result<OpenedVault, VaultOpenError> {
    let candidate =
        seal(&VaultDocument::empty(), key_protector).map_err(VaultOpenError::Envelope)?;

    let initialized = match files.publish_initial(&candidate) {
        Ok(()) => true,
        // 另一个协作实例抢先完成初始化时，只能重新读取它发布的主库。
        Err(VaultFileError::AlreadyExists) => false,
        Err(error) => return Err(VaultOpenError::File(error)),
    };

    // 即使本次发布成功，也必须重新读取并认证磁盘字节，不能直接返回内存候选。
    let envelope = files.read_main().map_err(VaultOpenError::File)?;
    open_confirmed(&envelope, key_protector, initialized)
}

fn open_confirmed(
    envelope: &[u8],
    key_protector: &impl KeyProtector,
    initialized: bool,
) -> Result<OpenedVault, VaultOpenError> {
    let document = open_envelope(envelope, key_protector).map_err(VaultOpenError::Envelope)?;
    Ok(OpenedVault {
        document,
        initialized,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::{DataKey, KeyProtectionError};
    use zeroize::Zeroizing;

    use super::*;

    #[derive(Default)]
    struct MemoryFiles {
        main: Mutex<Option<Vec<u8>>>,
        backup: Mutex<Option<Vec<u8>>>,
        fail_replace: bool,
        corrupt_replacement: bool,
    }

    impl VaultFiles for MemoryFiles {
        fn publish_initial(&self, candidate: &[u8]) -> Result<(), VaultFileError> {
            let mut main = self.main.lock().expect("测试互斥锁不应中毒");
            if main.is_some() {
                return Err(VaultFileError::AlreadyExists);
            }
            *main = Some(candidate.to_vec());
            Ok(())
        }

        fn replace_main(&self, candidate: &[u8]) -> Result<(), VaultFileError> {
            if self.fail_replace {
                return Err(VaultFileError::Io);
            }
            let mut main = self.main.lock().expect("测试互斥锁不应中毒");
            if main.is_none() {
                return Err(VaultFileError::NotFound);
            }
            *main = Some(if self.corrupt_replacement {
                b"corrupted-replacement".to_vec()
            } else {
                candidate.to_vec()
            });
            Ok(())
        }

        fn read_main(&self) -> Result<Vec<u8>, VaultFileError> {
            self.main
                .lock()
                .expect("测试互斥锁不应中毒")
                .clone()
                .ok_or(VaultFileError::NotFound)
        }

        fn read_backup(&self) -> Result<Vec<u8>, VaultFileError> {
            self.backup
                .lock()
                .expect("测试互斥锁不应中毒")
                .clone()
                .ok_or(VaultFileError::NotFound)
        }

        fn restore_main(&self, candidate: &[u8]) -> Result<(), VaultFileError> {
            *self.main.lock().expect("测试互斥锁不应中毒") = Some(candidate.to_vec());
            Ok(())
        }
    }

    struct TestKeyProtector;

    impl KeyProtector for TestKeyProtector {
        fn protect(&self, key: &DataKey) -> Result<Vec<u8>, KeyProtectionError> {
            Ok(key.as_bytes().iter().map(|byte| byte ^ 0x5A).collect())
        }

        fn unprotect(&self, wrapped_key: &[u8]) -> Result<DataKey, KeyProtectionError> {
            let wrapped: &[u8; 32] = wrapped_key
                .try_into()
                .map_err(|_| KeyProtectionError::new())?;
            let mut plaintext = Zeroizing::new([0_u8; 32]);
            for (output, input) in plaintext.iter_mut().zip(wrapped) {
                *output = input ^ 0x5A;
            }
            Ok(DataKey::copy_from_bytes(&plaintext))
        }
    }

    #[test]
    fn first_call_initializes_and_second_call_loads_same_empty_vault() {
        let files = MemoryFiles::default();

        let first = open_or_initialize(&files, &TestKeyProtector).expect("首次初始化应成功");
        let second = open_or_initialize(&files, &TestKeyProtector).expect("再次加载应成功");

        assert!(first.initialized);
        assert!(!second.initialized);
        assert_eq!(first.document, VaultDocument::empty());
        assert_eq!(second.document, first.document);
        let stored = files.read_main().expect("主库应已写入");
        assert!(
            !stored
                .windows(b"schema_version".len())
                .any(|bytes| bytes == b"schema_version")
        );
    }

    #[test]
    fn corrupted_existing_main_is_not_replaced_with_empty_vault() {
        let files = MemoryFiles {
            main: Mutex::new(Some(b"corrupted-main".to_vec())),
            ..MemoryFiles::default()
        };

        assert!(matches!(
            open_or_initialize(&files, &TestKeyProtector),
            Err(VaultOpenError::Envelope(_))
        ));
        assert_eq!(
            files.read_main().expect("损坏主库仍应原样存在"),
            b"corrupted-main"
        );
    }

    #[test]
    fn existing_valid_main_is_loaded_without_overwrite() {
        let files = MemoryFiles::default();
        let envelope = seal(&VaultDocument::empty(), &TestKeyProtector).expect("测试信封应生成");
        *files.main.lock().expect("测试互斥锁不应中毒") = Some(envelope.clone());

        let opened = open_or_initialize(&files, &TestKeyProtector).expect("已有主库应加载");

        assert!(!opened.initialized);
        assert_eq!(files.read_main(), Ok(envelope));
    }

    #[test]
    fn valid_candidate_is_saved_and_reopened_from_confirmed_bytes() {
        let files = MemoryFiles::default();
        open_or_initialize(&files, &TestKeyProtector).expect("应先创建主库");
        let mut candidate = VaultDocument::empty();
        candidate.revision = 1;
        candidate
            .create_category("工作")
            .expect("测试分类应创建成功");

        let confirmed =
            save_confirmed(&files, &TestKeyProtector, &candidate).expect("候选应保存成功");

        assert_eq!(confirmed, candidate);
        let reopened = open_or_initialize(&files, &TestKeyProtector).expect("保存后应能重开");
        assert_eq!(reopened.document, candidate);
    }

    #[test]
    fn replace_failure_keeps_previous_main_and_does_not_return_candidate() {
        let files = MemoryFiles::default();
        open_or_initialize(&files, &TestKeyProtector).expect("应先创建主库");
        let previous = files.read_main().expect("主库应存在");
        let failing_files = MemoryFiles {
            main: Mutex::new(Some(previous.clone())),
            backup: Mutex::new(None),
            fail_replace: true,
            corrupt_replacement: false,
        };
        let mut candidate = VaultDocument::empty();
        candidate.revision = 1;

        assert_eq!(
            save_confirmed(&failing_files, &TestKeyProtector, &candidate),
            Err(VaultOpenError::File(VaultFileError::Io))
        );
        assert_eq!(failing_files.read_main(), Ok(previous));
    }

    #[test]
    fn corrupted_replacement_is_not_returned_as_confirmed_document() {
        let files = MemoryFiles::default();
        open_or_initialize(&files, &TestKeyProtector).expect("应先创建主库");
        let corrupting_files = MemoryFiles {
            main: Mutex::new(files.main.lock().expect("测试互斥锁不应中毒").clone()),
            backup: Mutex::new(None),
            fail_replace: false,
            corrupt_replacement: true,
        };

        assert!(matches!(
            save_confirmed(
                &corrupting_files,
                &TestKeyProtector,
                &VaultDocument::empty()
            ),
            Err(VaultOpenError::Envelope(_))
        ));
    }

    #[test]
    fn authenticated_backup_restores_a_corrupted_main() {
        let mut recovered_document = VaultDocument::empty();
        recovered_document.revision = 3;
        recovered_document
            .create_category("工作")
            .expect("测试分类应创建成功");
        let backup = seal(&recovered_document, &TestKeyProtector).expect("测试备份应生成");
        let files = MemoryFiles {
            main: Mutex::new(Some(b"corrupted-main".to_vec())),
            backup: Mutex::new(Some(backup.clone())),
            ..MemoryFiles::default()
        };

        let opened = open_or_initialize(&files, &TestKeyProtector).expect("有效备份应恢复主库");

        assert_eq!(opened.document, recovered_document);
        assert_eq!(files.read_main(), Ok(backup));
    }
}
