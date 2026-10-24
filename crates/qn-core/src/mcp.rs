//! Host-only MCP routing metadata; it never authorizes a request by itself.
use std::fmt;

/// Planned exact Streamable HTTP endpoint. Produced by a trusted routing port,
/// not deserialized from agent input; consumers still require a live grant.
pub struct McpHttpRoute {
    /// Approved route reference, never a credential value.
    pub name: String,
    /// Canonical host-owned origin without path, query or userinfo.
    pub origin: String,
    /// Exact endpoint path; all supported methods share it.
    pub path: String,
}
impl McpHttpRoute {
    /// Streamable HTTP needs POST messages, GET event streams and DELETE termination.
    /// This grants no method; the owner installs exact endpoint policy and grant scopes.
    #[must_use]
    pub const fn methods(&self) -> [&'static str; 3] {
        ["POST", "GET", "DELETE"]
    }
}
impl fmt::Debug for McpHttpRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("McpHttpRoute(..)")
    }
}

/// Closed routing failures never include an untrusted URL or credential string.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum McpRouteError {
    /// Route reference cannot be used as a managed native credential name.
    #[error(
        "invalid MCP route name; use 1-64 lowercase letters, digits, hyphens or underscores, starting with a letter"
    )]
    Name,
    /// Endpoint cannot be represented without ambiguous or credential-bearing components.
    #[error(
        "invalid MCP endpoint; use canonical HTTPS or numeric loopback HTTP without credentials, query or fragment"
    )]
    Endpoint,
}

/// Plan trusted host routing without opening a socket, granting access or storing secrets.
pub trait McpRouting: Send + Sync {
    /// Validate a route and split its canonical endpoint into exact origin and path.
    /// # Errors
    /// Rejects unsupported names and unsafe or ambiguous endpoint forms.
    fn plan(&self, name: &str, endpoint: &str) -> Result<McpHttpRoute, McpRouteError>;
}
