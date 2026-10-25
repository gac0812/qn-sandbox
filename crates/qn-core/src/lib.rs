//! Domain layer for qn-sandbox.

/// Read-only audit presentation contracts; sources retain their own evidence semantics.
pub mod audit;
/// Domain models and rules.
pub mod domain;
/// Core error types and result alias.
pub mod error;
/// Host-only MCP route planning contract.
pub mod mcp;
/// Versioned official-profile catalog and installed-registry contracts.
pub mod profile;
/// Managed engine installation and inspection contracts.
pub mod setup;

pub use domain::execution::{GrantHandle, ProfileRef, RunSpec};
pub use domain::session::{SessionId, SessionState};
pub use error::{Error, Result};
