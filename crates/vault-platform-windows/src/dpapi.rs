//! DPAPI CurrentUser 数据密钥包装边界。

use std::slice;

use vault_storage::{DataKey, KeyProtectionError, KeyProtector};
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    },
    core::PCWSTR,
};
use zeroize::Zeroize;

const DATA_KEY_LENGTH: usize = 32;

/// 使用当前 Windows 用户凭据包装数据密钥的 DPAPI 实现。
#[derive(Debug, Default, Clone, Copy)]
pub struct DpapiKeyProtector;

impl KeyProtector for DpapiKeyProtector {
    fn protect(&self, key: &DataKey) -> Result<Vec<u8>, KeyProtectionError> {
        let input = input_blob(key.as_bytes())?;
        let mut output = LocalBlob::new(false);

        // 不传 optional entropy，也不启用 LocalMachine；唯一 flag 禁止系统弹出交互 UI。
        let result = unsafe {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                output.as_mut_blob(),
            )
        };
        result.map_err(platform_error)?;
        Ok(output.as_slice()?.to_vec())
    }

    fn unprotect(&self, wrapped_key: &[u8]) -> Result<DataKey, KeyProtectionError> {
        let input = input_blob(wrapped_key)?;
        // guard 在 API 调用前存在；即使错误或长度校验提前返回，也会清零并释放系统缓冲。
        let mut output = LocalBlob::new(true);

        let result = unsafe {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                output.as_mut_blob(),
            )
        };
        result.map_err(platform_error)?;

        let plaintext = output.as_slice()?;
        let bytes: &[u8; DATA_KEY_LENGTH] = plaintext
            .try_into()
            .map_err(|_| KeyProtectionError::new())?;
        Ok(DataKey::copy_from_bytes(bytes))
    }
}

fn input_blob(bytes: &[u8]) -> Result<CRYPT_INTEGER_BLOB, KeyProtectionError> {
    if bytes.is_empty() {
        return Err(KeyProtectionError::new());
    }
    let length = u32::try_from(bytes.len()).map_err(|_| KeyProtectionError::new())?;
    Ok(CRYPT_INTEGER_BLOB {
        cbData: length,
        // Win32 的结构字段历史上是可变指针，但两个 DPAPI 输入参数都只读该缓冲。
        pbData: bytes.as_ptr().cast_mut(),
    })
}

fn platform_error(error: windows::core::Error) -> KeyProtectionError {
    // HRESULT 只作为非敏感内部诊断码保存，不拼接系统消息或输入内容。
    KeyProtectionError::with_platform_code(error.code().0 as u32)
}

/// 接管 DPAPI 通过 LocalAlloc 返回的缓冲区。
struct LocalBlob {
    blob: CRYPT_INTEGER_BLOB,
    sensitive: bool,
}

impl LocalBlob {
    const fn new(sensitive: bool) -> Self {
        Self {
            blob: CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            },
            sensitive,
        }
    }

    fn as_mut_blob(&mut self) -> *mut CRYPT_INTEGER_BLOB {
        &mut self.blob
    }

    fn as_slice(&self) -> Result<&[u8], KeyProtectionError> {
        if self.blob.pbData.is_null() || self.blob.cbData == 0 {
            return Err(KeyProtectionError::new());
        }
        let length = usize::try_from(self.blob.cbData).map_err(|_| KeyProtectionError::new())?;

        // 指针和长度来自成功的 DPAPI 调用，并且借用不超过 guard 的生命周期。
        Ok(unsafe { slice::from_raw_parts(self.blob.pbData, length) })
    }
}

impl Drop for LocalBlob {
    fn drop(&mut self) {
        if self.blob.pbData.is_null() {
            return;
        }

        if self.sensitive && self.blob.cbData > 0 {
            let length = self.blob.cbData as usize;
            // zeroize 使用防优化写入；必须覆盖系统返回的完整长度，而不只是期望的 32 字节。
            unsafe { slice::from_raw_parts_mut(self.blob.pbData, length) }.zeroize();
        }

        // DPAPI 文档规定输出必须由 LocalFree 释放；返回空句柄表示释放成功。
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.blob.pbData.cast())));
        }
        self.blob.pbData = std::ptr::null_mut();
        self.blob.cbData = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_key() -> DataKey {
        let mut bytes = [0_u8; DATA_KEY_LENGTH];
        getrandom::fill(&mut bytes).expect("测试环境应提供系统随机数");
        DataKey::from_bytes(bytes)
    }

    #[test]
    fn current_user_round_trip_restores_same_key() {
        let protector = DpapiKeyProtector;
        let key = synthetic_key();

        let wrapped = protector.protect(&key).expect("DPAPI 包装应成功");
        let restored = protector.unprotect(&wrapped).expect("同用户解包应成功");

        assert_eq!(restored.as_bytes(), key.as_bytes());
    }

    #[test]
    fn independently_wrapped_keys_can_both_be_restored() {
        let protector = DpapiKeyProtector;
        let key = synthetic_key();

        let first = protector.protect(&key).expect("第一次包装应成功");
        let second = protector.protect(&key).expect("第二次包装应成功");

        assert_eq!(
            protector
                .unprotect(&first)
                .expect("第一次结果应能解包")
                .as_bytes(),
            key.as_bytes()
        );
        assert_eq!(
            protector
                .unprotect(&second)
                .expect("第二次结果应能解包")
                .as_bytes(),
            key.as_bytes()
        );
    }

    #[test]
    fn tampered_blob_never_restores_original_key() {
        let protector = DpapiKeyProtector;
        let key = synthetic_key();
        let mut wrapped = protector.protect(&key).expect("DPAPI 包装应成功");
        let middle = wrapped.len() / 2;
        wrapped[middle] ^= 0x01;

        if let Ok(restored) = protector.unprotect(&wrapped) {
            // DPAPI 文档允许损坏输入偶尔返回损坏输出；AES-GCM 会在组合原子中最终认证拒绝。
            assert_ne!(restored.as_bytes(), key.as_bytes());
        }
    }

    #[test]
    fn invalid_input_error_does_not_contain_key_material() {
        let key = synthetic_key();
        let error = DpapiKeyProtector
            .unprotect(&[])
            .expect_err("空 blob 应被拒绝");
        let rendered = format!("{error:?} {error}");

        assert!(!rendered.contains(&format!("{:?}", key.as_bytes())));
        assert!(!rendered.contains("pbData"));
    }
}
