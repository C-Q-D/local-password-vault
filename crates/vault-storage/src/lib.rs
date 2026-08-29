//! 加密信封、原子保存与恢复协议。平台原语由消费方定义的最小接口注入。

mod envelope;
mod error;
mod key_protection;

pub use envelope::{MAX_ENVELOPE_SIZE, open, seal};
pub use error::{EnvelopeError, KeyProtectionError};
pub use key_protection::{DataKey, KeyProtector};
