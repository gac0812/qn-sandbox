//! Domain layer for qn-sandbox.

/// Read-only audit presentation contracts; sources retain their own evidence semantics.
pub mod audit;
/// Domain models and rules.
pub mod domain;
/// Core error types and result alias.
pub mod error;
/// Managed engine installation and inspection contracts.
pub mod setup;

pub use domain::execution::{GrantHandle, ProfileRef, RunSpec};
pub use domain::session::{SessionId, SessionState};
pub use error::{Error, Result};
