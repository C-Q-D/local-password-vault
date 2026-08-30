//! Windows 剪贴板写入凭证与条件清理边界。

use std::{
    ffi::c_void,
    ptr::copy_nonoverlapping,
    slice,
    sync::atomic::{AtomicU64, Ordering},
};

use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
                OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        },
    },
    core::PCWSTR,
};
use zeroize::Zeroizing;

const CF_UNICODETEXT_FORMAT: u32 = 13;
const TOKEN_FORMAT_NAME: &str = "LocalPasswordVault.ClipboardToken.v1";
const EXCLUDE_MONITOR_FORMAT_NAME: &str = "ExcludeClipboardContentFromMonitorProcessing";

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(0);

/// 剪贴板操作只暴露稳定类别，错误中不附带密码、token 或系统错误文本。
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardError {
    #[error("剪贴板内容包含不支持的空字符")]
    ContentContainsNul,
    #[error("剪贴板窗口不可用")]
    InvalidOwner,
    #[error("系统剪贴板当前不可用")]
    Unavailable,
    #[error("系统剪贴板格式注册失败")]
    FormatRegistrationFailed,
    #[error("系统剪贴板内存分配失败")]
    AllocationFailed,
    #[error("系统剪贴板写入失败")]
    WriteFailed,
}

/// 条件清理只区分“已清理”和“剪贴板已不属于本次写入”。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearOutcome {
    Cleared,
    NotOwned,
}

/// Windows 剪贴板使用真实主窗口作为 owner，避免 EmptyClipboard 后失去写入资格。
#[derive(Debug, Clone, Copy)]
pub struct WindowsClipboard {
    owner_hwnd: isize,
}

impl WindowsClipboard {
    pub fn new(owner_hwnd: isize) -> Result<Self, ClipboardError> {
        if owner_hwnd == 0 {
            return Err(ClipboardError::InvalidOwner);
        }
        Ok(Self { owner_hwnd })
    }

    /// 账号名属于普通文本，不写密码专用 token，也不安排自动清理。
    pub fn copy_text(&self, text: &str) -> Result<(), ClipboardError> {
        let encoded = encode_unicode_text(text)?;
        let mut text_memory = GlobalMemory::from_bytes(wide_as_bytes(&encoded))?;
        let _clipboard = ClipboardSession::open(self.owner_hwnd)?;

        // EmptyClipboard 与 SetClipboardData 必须位于同一次打开会话内。
        unsafe { EmptyClipboard() }.map_err(|_| ClipboardError::WriteFailed)?;
        set_clipboard_memory(CF_UNICODETEXT_FORMAT, &mut text_memory)?;
        Ok(())
    }

    /// 密码最后写入；前两种格式成功前不会把明文放入系统剪贴板。
    pub fn copy_password(
        &self,
        password: &str,
    ) -> Result<PasswordClipboardReceipt, ClipboardError> {
        let expected_utf16 = encode_unicode_text(password)?;
        let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let token_format = register_format(TOKEN_FORMAT_NAME)?;
        let exclude_format = register_format(EXCLUDE_MONITOR_FORMAT_NAME)?;

        // 打开剪贴板前完成全部分配，避免持锁期间分配失败留下部分内容。
        let mut exclude_memory = GlobalMemory::from_bytes(&[1])?;
        let mut token_memory = GlobalMemory::from_bytes(&token.to_le_bytes())?;
        let mut text_memory = GlobalMemory::from_bytes(wide_as_bytes(&expected_utf16))?;
        let clipboard = ClipboardSession::open(self.owner_hwnd)?;

        unsafe { EmptyClipboard() }.map_err(|_| ClipboardError::WriteFailed)?;
        set_clipboard_memory(exclude_format, &mut exclude_memory)?;
        set_clipboard_memory(token_format, &mut token_memory)?;
        set_clipboard_memory(CF_UNICODETEXT_FORMAT, &mut text_memory)?;

        let sequence = unsafe { GetClipboardSequenceNumber() };
        if sequence == 0 {
            // 无法取得所有权凭证时不留下无法安全识别的密码。
            let _ = unsafe { EmptyClipboard() };
            return Err(ClipboardError::WriteFailed);
        }
        drop(clipboard);

        Ok(PasswordClipboardReceipt {
            owner_hwnd: self.owner_hwnd,
            token_format,
            token,
            sequence,
            expected_utf16,
        })
    }
}

/// 该凭证只在 Rust 后台清理任务中存活，不实现 Debug 或序列化。
pub struct PasswordClipboardReceipt {
    owner_hwnd: isize,
    token_format: u32,
    token: u64,
    sequence: u32,
    expected_utf16: Zeroizing<Vec<u16>>,
}

impl PasswordClipboardReceipt {
    /// 比较和清空共享同一个 ClipboardSession，避免检查后出现 TOCTOU 窗口。
    pub fn clear_if_owned(self) -> Result<ClearOutcome, ClipboardError> {
        let _clipboard = ClipboardSession::open(self.owner_hwnd)?;
        let current_sequence = unsafe { GetClipboardSequenceNumber() };

        if !receipt_matches_observed(
            self.sequence,
            self.token,
            &self.expected_utf16,
            current_sequence,
            clipboard_data_matches(self.token_format, &self.token.to_le_bytes()),
            clipboard_data_matches(CF_UNICODETEXT_FORMAT, wide_as_bytes(&self.expected_utf16)),
        ) {
            return Ok(ClearOutcome::NotOwned);
        }

        unsafe { EmptyClipboard() }.map_err(|_| ClipboardError::WriteFailed)?;
        Ok(ClearOutcome::Cleared)
    }
}

struct ClipboardSession;

impl ClipboardSession {
    fn open(owner_hwnd: isize) -> Result<Self, ClipboardError> {
        let hwnd = HWND(owner_hwnd as *mut c_void);
        unsafe { OpenClipboard(Some(hwnd)) }.map_err(|_| ClipboardError::Unavailable)?;
        Ok(Self)
    }
}

impl Drop for ClipboardSession {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

/// 尚未交给 SetClipboardData 的 HGLOBAL 由此 guard 回收；移交后立即 disarm。
struct GlobalMemory {
    handle: Option<HGLOBAL>,
}

impl GlobalMemory {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ClipboardError> {
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }
            .map_err(|_| ClipboardError::AllocationFailed)?;
        let target = unsafe { GlobalLock(handle) };
        if target.is_null() {
            let _ = unsafe { GlobalFree(Some(handle)) };
            return Err(ClipboardError::AllocationFailed);
        }

        unsafe { copy_nonoverlapping(bytes.as_ptr(), target.cast::<u8>(), bytes.len()) };
        // GlobalUnlock 返回零也可能表示锁计数成功降为零，不把该值当作写入失败。
        let _ = unsafe { GlobalUnlock(handle) };
        Ok(Self {
            handle: Some(handle),
        })
    }

    fn as_handle(&self) -> HANDLE {
        HANDLE(self.handle.expect("未移交的全局内存必须存在").0)
    }

    fn disarm(&mut self) {
        self.handle = None;
    }
}

impl Drop for GlobalMemory {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = unsafe { GlobalFree(Some(handle)) };
        }
    }
}

fn set_clipboard_memory(format: u32, memory: &mut GlobalMemory) -> Result<(), ClipboardError> {
    unsafe { SetClipboardData(format, Some(memory.as_handle())) }
        .map_err(|_| ClipboardError::WriteFailed)?;
    memory.disarm();
    Ok(())
}

fn encode_unicode_text(text: &str) -> Result<Zeroizing<Vec<u16>>, ClipboardError> {
    if text.encode_utf16().any(|unit| unit == 0) {
        return Err(ClipboardError::ContentContainsNul);
    }

    let mut encoded = Zeroizing::new(text.encode_utf16().collect::<Vec<_>>());
    encoded.push(0);
    Ok(encoded)
}

fn wide_as_bytes(value: &[u16]) -> &[u8] {
    unsafe { slice::from_raw_parts(value.as_ptr().cast::<u8>(), size_of_val(value)) }
}

fn register_format(name: &str) -> Result<u32, ClipboardError> {
    let mut wide_name = name.encode_utf16().collect::<Vec<_>>();
    wide_name.push(0);
    let format = unsafe { RegisterClipboardFormatW(PCWSTR(wide_name.as_ptr())) };
    if format == 0 {
        Err(ClipboardError::FormatRegistrationFailed)
    } else {
        Ok(format)
    }
}

fn clipboard_data_matches(format: u32, expected: &[u8]) -> bool {
    let Ok(handle) = (unsafe { GetClipboardData(format) }) else {
        return false;
    };
    let global = HGLOBAL(handle.0);
    if unsafe { GlobalSize(global) } != expected.len() {
        return false;
    }

    let data = unsafe { GlobalLock(global) };
    if data.is_null() {
        return false;
    }
    let matches = unsafe { slice::from_raw_parts(data.cast::<u8>(), expected.len()) } == expected;
    let _ = unsafe { GlobalUnlock(global) };
    matches
}

fn receipt_matches_observed(
    expected_sequence: u32,
    expected_token: u64,
    expected_text: &[u16],
    observed_sequence: u32,
    token_matches: bool,
    text_matches: bool,
) -> bool {
    observed_sequence != 0
        && observed_sequence == expected_sequence
        && expected_token != 0
        && !expected_text.is_empty()
        && token_matches
        && text_matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_encoding_supports_unicode_and_rejects_embedded_nul() {
        let encoded = encode_unicode_text("密码-🔐").expect("Unicode 密码应可编码");

        assert_eq!(encoded.last(), Some(&0));
        assert_eq!(
            String::from_utf16(&encoded[..encoded.len() - 1]).unwrap(),
            "密码-🔐"
        );
        assert_eq!(
            encode_unicode_text("invalid\0password"),
            Err(ClipboardError::ContentContainsNul)
        );
    }

    #[test]
    fn conditional_clear_requires_sequence_token_and_text_to_match() {
        let text = [0x5bc6, 0x7801, 0];

        assert!(receipt_matches_observed(42, 7, &text, 42, true, true));
        assert!(!receipt_matches_observed(42, 7, &text, 43, true, true));
        assert!(!receipt_matches_observed(42, 7, &text, 42, false, true));
        assert!(!receipt_matches_observed(42, 7, &text, 42, true, false));
        assert!(!receipt_matches_observed(42, 7, &text, 0, true, true));
    }

    #[test]
    fn windows_clipboard_requires_real_owner_window() {
        assert_eq!(
            WindowsClipboard::new(0).unwrap_err(),
            ClipboardError::InvalidOwner
        );
    }
}
