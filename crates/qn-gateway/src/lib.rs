//! Host-side MCP gateway adapter for qn-sandbox.
//! Native forwarding and credential injection remain in nono and qn-broker.
use qn_core::mcp::{McpHttpRoute, McpRouteError, McpRouting};
use url::{Host, Url};

/// Plan an exact approved Streamable HTTP route without activating it.
#[derive(Debug, Default)]
pub struct HttpMcpRouting;
impl McpRouting for HttpMcpRouting {
    fn plan(&self, name: &str, endpoint: &str) -> Result<McpHttpRoute, McpRouteError> {
        if name.is_empty()
            || name.len() > 64
            || !name.as_bytes()[0].is_ascii_lowercase()
            || !name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_'))
        {
            return Err(McpRouteError::Name);
        }
        let url = endpoint_url(endpoint)?;
        Ok(McpHttpRoute {
            name: name.into(),
            origin: url.origin().ascii_serialization(),
            path: url.path().into(),
        })
    }
}
fn endpoint_url(endpoint: &str) -> Result<Url, McpRouteError> {
    if endpoint.is_empty()
        || endpoint.len() > 4096
        || !endpoint.is_ascii()
        || endpoint
            .bytes()
            .any(|c| c.is_ascii_control() || c.is_ascii_whitespace() || matches!(c, b'\\' | b'*'))
    {
        return Err(McpRouteError::Endpoint);
    }
    let url = Url::parse(endpoint).map_err(|_| McpRouteError::Endpoint)?;
    let loopback = matches!(url.host(), Some(Host::Ipv4(ip)) if ip.is_loopback())
        || matches!(url.host(), Some(Host::Ipv6(ip)) if ip.is_loopback());
    if !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        || url.host().is_none()
        || url.cannot_be_a_base()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port() == Some(0)
        || url.as_str() != endpoint
        || url.path().contains('%')
        || url.path().starts_with("//")
    {
        return Err(McpRouteError::Endpoint);
    }
    Ok(url)
}

#[cfg(test)]
mod tests;
