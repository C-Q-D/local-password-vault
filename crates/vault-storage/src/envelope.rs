use aes_gcm::{
    Aes256Gcm, Nonce, Tag,
    aead::{AeadInOut, KeyInit},
};
use vault_core::VaultDocument;
use zeroize::Zeroizing;

use crate::{EnvelopeError, KeyProtector};

const MAGIC: [u8; 8] = *b"LPVAULT\0";
const FORMAT_VERSION: u16 = 1;
const FIXED_HEADER_LENGTH: usize = 8 + 2 + 4 + 4 + 8;
const NONCE_LENGTH: usize = 12;
const TAG_LENGTH: usize = 16;
const MAX_WRAPPED_KEY_LENGTH: usize = 64 * 1024;

/// 第一版允许读取或生成的最大信封大小，避免损坏长度触发无界分配。
pub const MAX_ENVELOPE_SIZE: usize = 64 * 1024 * 1024;

/// 把结构有效的密码库文档封装为版本化 AES-256-GCM 字节。
pub fn seal(
    document: &VaultDocument,
    key_protector: &impl KeyProtector,
) -> Result<Vec<u8>, EnvelopeError> {
    document
        .validate()
        .map_err(|_| EnvelopeError::InvalidDocument)?;

    // JSON 在完成加密后立即清零，磁盘侧只接收下方返回的密文信封。
    let mut ciphertext =
        Zeroizing::new(serde_json::to_vec(document).map_err(|_| EnvelopeError::EncodingFailed)?);
    let data_key = crate::DataKey::generate()?;
    let wrapped_key = key_protector
        .protect(&data_key)
        .map_err(|_| EnvelopeError::KeyProtectionFailed)?;

    if wrapped_key.is_empty() || wrapped_key.len() > MAX_WRAPPED_KEY_LENGTH {
        return Err(EnvelopeError::InvalidFormat);
    }

    let header = build_authenticated_header(&wrapped_key, ciphertext.len())?;
    let total_length = header
        .len()
        .checked_add(NONCE_LENGTH + TAG_LENGTH)
        .and_then(|length| length.checked_add(ciphertext.len()))
        .ok_or(EnvelopeError::TooLarge)?;
    if total_length > MAX_ENVELOPE_SIZE {
        return Err(EnvelopeError::TooLarge);
    }

    let mut nonce_bytes = [0_u8; NONCE_LENGTH];
    getrandom::fill(&mut nonce_bytes).map_err(|_| EnvelopeError::RandomFailed)?;
    let nonce =
        Nonce::try_from(nonce_bytes.as_slice()).map_err(|_| EnvelopeError::InvalidFormat)?;
    let cipher =
        Aes256Gcm::new_from_slice(data_key.as_bytes()).map_err(|_| EnvelopeError::InvalidFormat)?;
    let tag = cipher
        .encrypt_inout_detached(&nonce, &header, ciphertext.as_mut_slice().into())
        .map_err(|_| EnvelopeError::EncodingFailed)?;

    let mut envelope = Vec::with_capacity(total_length);
    envelope.extend_from_slice(&header);
    envelope.extend_from_slice(&nonce_bytes);
    envelope.extend_from_slice(tag.as_slice());
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

/// 认证并打开版本化信封；认证成功前不会解析其中的 JSON。
pub fn open(
    envelope: &[u8],
    key_protector: &impl KeyProtector,
) -> Result<VaultDocument, EnvelopeError> {
    let parsed = ParsedEnvelope::parse(envelope)?;
    let data_key = key_protector
        .unprotect(parsed.wrapped_key)
        .map_err(|_| EnvelopeError::KeyProtectionFailed)?;
    let cipher =
        Aes256Gcm::new_from_slice(data_key.as_bytes()).map_err(|_| EnvelopeError::InvalidFormat)?;
    let nonce = Nonce::try_from(parsed.nonce).map_err(|_| EnvelopeError::InvalidFormat)?;
    let tag = Tag::try_from(parsed.tag).map_err(|_| EnvelopeError::InvalidFormat)?;

    // 密文副本原地解密，作用域结束时连同临时明文一起清零。
    let mut plaintext = Zeroizing::new(parsed.ciphertext.to_vec());
    cipher
        .decrypt_inout_detached(
            &nonce,
            parsed.authenticated_header,
            plaintext.as_mut_slice().into(),
            &tag,
        )
        .map_err(|_| EnvelopeError::AuthenticationFailed)?;

    let document: VaultDocument =
        serde_json::from_slice(&plaintext).map_err(|_| EnvelopeError::InvalidPayload)?;
    document
        .validate()
        .map_err(|_| EnvelopeError::InvalidDocument)?;
    Ok(document)
}

fn build_authenticated_header(
    wrapped_key: &[u8],
    ciphertext_length: usize,
) -> Result<Vec<u8>, EnvelopeError> {
    let header_length = FIXED_HEADER_LENGTH
        .checked_add(wrapped_key.len())
        .ok_or(EnvelopeError::TooLarge)?;
    let header_length = u32::try_from(header_length).map_err(|_| EnvelopeError::TooLarge)?;
    let wrapped_key_length =
        u32::try_from(wrapped_key.len()).map_err(|_| EnvelopeError::TooLarge)?;
    let ciphertext_length =
        u64::try_from(ciphertext_length).map_err(|_| EnvelopeError::TooLarge)?;

    // 这些规范化头字节会原样作为 GCM AAD，解析语义和密文因而不可分离篡改。
    let mut header = Vec::with_capacity(header_length as usize);
    header.extend_from_slice(&MAGIC);
    header.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    header.extend_from_slice(&header_length.to_le_bytes());
    header.extend_from_slice(&wrapped_key_length.to_le_bytes());
    header.extend_from_slice(&ciphertext_length.to_le_bytes());
    header.extend_from_slice(wrapped_key);
    Ok(header)
}

struct ParsedEnvelope<'a> {
    authenticated_header: &'a [u8],
    wrapped_key: &'a [u8],
    nonce: &'a [u8],
    tag: &'a [u8],
    ciphertext: &'a [u8],
}

impl<'a> ParsedEnvelope<'a> {
    fn parse(envelope: &'a [u8]) -> Result<Self, EnvelopeError> {
        if envelope.len() > MAX_ENVELOPE_SIZE
            || envelope.len() < FIXED_HEADER_LENGTH + 1 + NONCE_LENGTH + TAG_LENGTH
        {
            return Err(EnvelopeError::InvalidFormat);
        }
        if envelope[..MAGIC.len()] != MAGIC {
            return Err(EnvelopeError::InvalidFormat);
        }

        let format_version = read_u16(envelope, 8)?;
        if format_version != FORMAT_VERSION {
            return Err(EnvelopeError::UnsupportedFormatVersion);
        }
        let header_length = read_u32(envelope, 10)? as usize;
        let wrapped_key_length = read_u32(envelope, 14)? as usize;
        let ciphertext_length =
            usize::try_from(read_u64(envelope, 18)?).map_err(|_| EnvelopeError::TooLarge)?;

        if wrapped_key_length == 0 || wrapped_key_length > MAX_WRAPPED_KEY_LENGTH {
            return Err(EnvelopeError::InvalidFormat);
        }
        let expected_header_length = FIXED_HEADER_LENGTH
            .checked_add(wrapped_key_length)
            .ok_or(EnvelopeError::InvalidFormat)?;
        if header_length != expected_header_length || header_length > envelope.len() {
            return Err(EnvelopeError::InvalidFormat);
        }
        let expected_total_length = header_length
            .checked_add(NONCE_LENGTH + TAG_LENGTH)
            .and_then(|length| length.checked_add(ciphertext_length))
            .ok_or(EnvelopeError::TooLarge)?;
        if expected_total_length != envelope.len() {
            return Err(EnvelopeError::InvalidFormat);
        }

        let wrapped_key = &envelope[FIXED_HEADER_LENGTH..header_length];
        let nonce_start = header_length;
        let tag_start = nonce_start + NONCE_LENGTH;
        let ciphertext_start = tag_start + TAG_LENGTH;
        Ok(Self {
            authenticated_header: &envelope[..header_length],
            wrapped_key,
            nonce: &envelope[nonce_start..tag_start],
            tag: &envelope[tag_start..ciphertext_start],
            ciphertext: &envelope[ciphertext_start..],
        })
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, EnvelopeError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(EnvelopeError::InvalidFormat)?;
    Ok(u16::from_le_bytes(
        value.try_into().map_err(|_| EnvelopeError::InvalidFormat)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, EnvelopeError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(EnvelopeError::InvalidFormat)?;
    Ok(u32::from_le_bytes(
        value.try_into().map_err(|_| EnvelopeError::InvalidFormat)?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, EnvelopeError> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or(EnvelopeError::InvalidFormat)?;
    Ok(u64::from_le_bytes(
        value.try_into().map_err(|_| EnvelopeError::InvalidFormat)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataKey, KeyProtectionError};

    struct TestKeyProtector;

    impl KeyProtector for TestKeyProtector {
        fn protect(&self, key: &DataKey) -> Result<Vec<u8>, KeyProtectionError> {
            // 仅供单元测试隔离平台依赖；固定异或不提供现实安全性。
            Ok(key.as_bytes().iter().map(|byte| byte ^ 0xA5).collect())
        }

        fn unprotect(&self, wrapped_key: &[u8]) -> Result<DataKey, KeyProtectionError> {
            let wrapped_key: [u8; 32] = wrapped_key
                .try_into()
                .map_err(|_| KeyProtectionError::new())?;
            Ok(DataKey::from_bytes(wrapped_key.map(|byte| byte ^ 0xA5)))
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn empty_vault_round_trip_succeeds_without_plaintext_fields() {
        let document = VaultDocument::empty();

        let envelope = seal(&document, &TestKeyProtector).expect("空库应能加密");
        let opened = open(&envelope, &TestKeyProtector).expect("空库应能认证解密");

        assert_eq!(opened, document);
        assert!(!contains(&envelope, b"schema_version"));
        assert!(!contains(&envelope, b"accounts"));
    }

    #[test]
    fn repeated_seal_uses_fresh_key_and_nonce() {
        let document = VaultDocument::empty();

        let first = seal(&document, &TestKeyProtector).expect("第一次加密应成功");
        let second = seal(&document, &TestKeyProtector).expect("第二次加密应成功");

        assert_ne!(first, second);
        assert_eq!(open(&first, &TestKeyProtector), Ok(document.clone()));
        assert_eq!(open(&second, &TestKeyProtector), Ok(document));
    }

    #[test]
    fn ciphertext_nonce_tag_and_wrapped_key_tampering_are_rejected() {
        let envelope = seal(&VaultDocument::empty(), &TestKeyProtector).expect("加密应成功");
        let parsed = ParsedEnvelope::parse(&envelope).expect("测试信封格式应有效");
        let wrapped_offset = parsed.wrapped_key.as_ptr() as usize - envelope.as_ptr() as usize;
        let nonce_offset = parsed.nonce.as_ptr() as usize - envelope.as_ptr() as usize;
        let tag_offset = parsed.tag.as_ptr() as usize - envelope.as_ptr() as usize;
        let ciphertext_offset = parsed.ciphertext.as_ptr() as usize - envelope.as_ptr() as usize;

        for offset in [wrapped_offset, nonce_offset, tag_offset, ciphertext_offset] {
            let mut tampered = envelope.clone();
            tampered[offset] ^= 0x01;
            assert!(open(&tampered, &TestKeyProtector).is_err());
        }
    }

    #[test]
    fn malformed_or_tampered_header_is_rejected() {
        let envelope = seal(&VaultDocument::empty(), &TestKeyProtector).expect("加密应成功");

        let mut bad_magic = envelope.clone();
        bad_magic[0] ^= 0x01;
        assert_eq!(
            open(&bad_magic, &TestKeyProtector),
            Err(EnvelopeError::InvalidFormat)
        );

        let mut bad_header_length = envelope.clone();
        bad_header_length[10] ^= 0x01;
        assert_eq!(
            open(&bad_header_length, &TestKeyProtector),
            Err(EnvelopeError::InvalidFormat)
        );

        let mut bad_ciphertext_length = envelope.clone();
        bad_ciphertext_length[18] ^= 0x01;
        assert_eq!(
            open(&bad_ciphertext_length, &TestKeyProtector),
            Err(EnvelopeError::InvalidFormat)
        );
    }

    #[test]
    fn oversized_and_truncated_envelopes_are_rejected_before_unwrap() {
        let oversized = vec![0_u8; MAX_ENVELOPE_SIZE + 1];
        assert_eq!(
            open(&oversized, &TestKeyProtector),
            Err(EnvelopeError::InvalidFormat)
        );

        let mut truncated = seal(&VaultDocument::empty(), &TestKeyProtector).expect("加密应成功");
        truncated.pop();
        assert_eq!(
            open(&truncated, &TestKeyProtector),
            Err(EnvelopeError::InvalidFormat)
        );
    }
}
