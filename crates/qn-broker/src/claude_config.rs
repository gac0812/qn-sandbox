//! Fixed Claude MCP template; nono supplies only loopback routing and a phantom (#125).
use crate::managed_config::{ManagedConfigError, Publication, decode, publish};
use camino::Utf8Path;
use serde::{Deserialize, Serialize};

const TEMPLATE: &[u8] = br#"{"mcpServers":{"qn_mcp":{"type":"http","url":"${QN_MCP_BASE_URL}/mcp","headers":{"Authorization":"Bearer ${QN_MCP_TOKEN}"}}}}"#;

#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(rename = "mcpServers")]
    servers: Servers,
}
#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Servers {
    qn_mcp: Server,
}
#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Server {
    #[serde(rename = "type")]
    transport: String,
    url: String,
    headers: Headers,
}
#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Headers {
    #[serde(rename = "Authorization")]
    authorization: String,
}

/// Return the fixed credential-free Claude template for a native `qn_mcp` route.
/// The host must configure nono to supply `QN_MCP_BASE_URL` and `QN_MCP_TOKEN`,
/// allow exactly /mcp, and keep the provider secret outside the sandbox.
#[must_use]
pub const fn claude_mcp_template() -> &'static [u8] {
    TEMPLATE
}

/// Validate and atomically create .claude.json in a fresh private host directory.
///
/// Only the exact fixed HTTP template is supported; no config merging or overwrite.
/// The directory and all ancestors must be host-owned and unavailable for sandbox
/// writes. Supply read-only access separately. This does not authorize a live
/// grant or verify a listener; nono/broker enforce every request. Blocking IO must
/// run in `spawn_blocking` for async callers. Directory power-loss durability is
/// not promised. Claude 2.1.304 expansion was measured under #125.
///
/// ```no_run
/// # fn example(root: &camino::Utf8Path) -> Result<(), qn_broker::ManagedConfigError> {
/// qn_broker::write_claude_mcp_config(root, qn_broker::claude_mcp_template())?;
/// # Ok(()) }
/// ```
/// # Errors
/// Rejects unsafe/unknown/duplicate input before IO, relative roots, existing
/// targets and filesystem failures without exposing input text.
pub fn write_claude_mcp_config(root: &Utf8Path, input: &[u8]) -> Result<(), ManagedConfigError> {
    let config: Config = decode(input)?;
    let expected: Config = decode(TEMPLATE)?;
    if config != expected {
        return Err(ManagedConfigError::BindingMismatch);
    }
    publish(
        root,
        ".claude.json",
        &serde_json::to_vec(&config)?,
        Publication::Create,
    )
}

#[cfg(test)]
#[path = "claude_config_tests.rs"]
mod tests;
