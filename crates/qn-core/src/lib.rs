//! Domain layer for qn-sandbox.

/// Domain models and rules.
pub mod domain;
/// Core error types and result alias.
pub mod error;
/// Host-only MCP route planning contract.
pub mod mcp;

pub use domain::execution::{GrantHandle, ProfileRef, RunSpec};
pub use domain::session::{SessionId, SessionState};
pub use error::{Error, Result};
