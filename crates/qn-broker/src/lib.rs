//! Host-side credential broker adapter for qn-sandbox.
//! ```no_run
//! # fn example(root: &camino::Utf8Path) -> Result<(), Box<dyn std::error::Error>> {
//! use qn_broker::{ManagedConfigBinding, write_managed_config};
//! let session = qn_core::SessionId::new();
//! let grant = qn_core::GrantHandle::new(); // Authorize the grant before use.
//! let port = std::num::NonZeroU16::new(8080).ok_or("zero port")?;
//! let binding = ManagedConfigBinding::new(session, grant, port);
//! let json = serde_json::json!({"version":1,"session":session.to_string(),
//!     "grant":grant.to_string(),"proxy_port":port.get()});
//! write_managed_config(root, &binding, &serde_json::to_vec(&json)?)?;
//! # Ok(()) }
//! ```
mod managed_config;
pub use managed_config::{
    ManagedConfigBinding, ManagedConfigError, ManagedConfigJsonError, write_managed_config,
};
mod claude_config;
pub use claude_config::{claude_mcp_template, write_claude_mcp_config};
mod codex_config;
pub use codex_config::write_codex_mcp_config;

mod grants;
pub use grants::{CaptureScope, GrantError, GrantRegistry};

pub mod audit;
pub mod capture;
pub mod identity;
mod lifecycle;
pub mod provider;
pub use lifecycle::{AuditedGrant, LifecycleError};
