use vault_storage::{VaultFiles, open_or_initialize, save_confirmed};

use crate::{atomic_file::WindowsVaultFiles, dpapi::DpapiKeyProtector};

#[test]
fn real_dpapi_and_file_boundary_reopen_same_encrypted_empty_vault() {
    let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
    let files = WindowsVaultFiles::new(directory.path());
    let protector = DpapiKeyProtector;

    let first = open_or_initialize(&files, &protector).expect("首次真实初始化应成功");
    let second = open_or_initialize(&files, &protector).expect("真实重开应成功");

    assert!(first.initialized);
    assert!(!second.initialized);
    assert_eq!(second.document, first.document);
    let stored = files.read_main().expect("真实主库应可读取");
    assert!(
        !stored
            .windows(b"schema_version".len())
            .any(|bytes| bytes == b"schema_version")
    );
}

#[test]
fn real_dpapi_and_file_boundary_save_and_reopen_category() {
    let directory = tempfile::tempdir().expect("应能创建隔离测试目录");
    let files = WindowsVaultFiles::new(directory.path());
    let protector = DpapiKeyProtector;
    let opened = open_or_initialize(&files, &protector).expect("首次真实初始化应成功");
    let mut candidate = opened.document;
    candidate.revision += 1;
    candidate
        .create_category("工作")
        .expect("测试分类应创建成功");

    let confirmed = save_confirmed(&files, &protector, &candidate).expect("真实保存应成功");
    let reopened = open_or_initialize(&files, &protector).expect("真实重开应成功");

    assert_eq!(confirmed, candidate);
    assert_eq!(reopened.document, candidate);
}
