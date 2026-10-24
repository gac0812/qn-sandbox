//! Domain layer for qn-sandbox.

/// Domain models and rules.
pub mod domain;
/// Core error types and result alias.
pub mod error;

pub use domain::session::SessionId;
pub use error::{Error, Result};
