//! qn-config：配置文件、`QN_*` 环境变量覆盖、平台路径与锁定常量的唯一权威
//! （spec 01 §2；R6 允许 `#[cfg(target_os)]` 的位置之一）。
//!
//! **禁止**：运行时业务逻辑、任何凭证字段（08 §4——配置文件里出现
//! 密钥即违规）、路径创建（只解析，创建属使用方）。

pub mod consts;
pub mod paths;

mod error;
mod settings;

pub use consts::{NONO_PINNED_VERSION, WSL_MIN_KERNEL_SEMVER};
pub use error::Error;
pub use settings::{Config, DEFAULT_TIMEOUT_MS};

/// 环境变量前缀（覆盖键形如 `QN_DEFAULT_TIMEOUT_MS`）。
pub const ENV_PREFIX: &str = "QN_";
