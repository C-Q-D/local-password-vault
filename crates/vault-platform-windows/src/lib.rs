//! Windows 平台原语入口；后续按 DPAPI、文件、剪贴板和单实例锁分模块实现。

pub mod atomic_file;
pub mod clipboard;
pub mod dpapi;
pub mod process_lock;
