//! Codex MCP routing contains only an approved local port and phantom variable (#136).
use crate::managed_config::{ManagedConfigError, Publication, publish};
use camino::Utf8Path;
use serde::Serialize;
use std::num::NonZeroU16;

#[derive(Serialize)]
struct Config {
    mcp_servers: Servers,
}
#[derive(Serialize)]
struct Servers {
    qn_mcp: Server,
}
#[derive(Serialize)]
struct Server {
    url: String,
    bearer_token_env_var: &'static str,
}

/// Create a fixed Codex config.toml in a new private host-owned directory.
///
/// The approved port must match the native proxy listener. The application receives
/// only a loopback URL and the name of a nono-generated phantom environment variable.
/// Neither a credential nor an arbitrary URL/config can be supplied. Keep the file
/// and parent unavailable for sandbox writes; grant application state separately.
/// A port is not listener ownership or authorization. Real requests must still pass
/// native authentication and broker grant checks. Blocking IO requires `spawn_blocking`
/// in async callers; directory power-loss durability is not promised.
///
/// ```no_run
/// # fn example(root: &camino::Utf8Path, port: std::num::NonZeroU16) -> Result<(), qn_broker::ManagedConfigError> {
/// qn_broker::write_codex_mcp_config(root, port)?;
/// # Ok(()) }
/// ```
/// # Errors
/// Rejects relative roots, existing config.toml and serialization/IO failures.
pub fn write_codex_mcp_config(root: &Utf8Path, port: NonZeroU16) -> Result<(), ManagedConfigError> {
    let config = Config {
        mcp_servers: Servers {
            qn_mcp: Server {
                url: format!("http://127.0.0.1:{port}/qn_mcp/mcp"),
                bearer_token_env_var: "QN_MCP_TOKEN",
            },
        },
    };
    publish(
        root,
        "config.toml",
        toml::to_string(&config)?.as_bytes(),
        Publication::Create,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::fs;

    #[test]
    fn it_serializes_exact_loopback_routing_and_never_replaces_existing_config() {
        let root = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(root.path()).unwrap();
        let port = NonZeroU16::new(1234).unwrap();
        write_codex_mcp_config(root, port).unwrap();
        let input = fs::read_to_string(root.join("config.toml")).unwrap();
        let config: toml::Value = toml::from_str(&input).unwrap();
        assert_eq!(
            config["mcp_servers"]["qn_mcp"]["url"].as_str(),
            Some("http://127.0.0.1:1234/qn_mcp/mcp")
        );
        assert_eq!(
            config["mcp_servers"]["qn_mcp"]["bearer_token_env_var"].as_str(),
            Some("QN_MCP_TOKEN")
        );
        assert_eq!(config.as_table().unwrap().len(), 1);
        assert_eq!(config["mcp_servers"].as_table().unwrap().len(), 1);
        assert_eq!(config["mcp_servers"]["qn_mcp"].as_table().unwrap().len(), 2);
        assert!(write_codex_mcp_config(root, port).is_err());
        assert_eq!(fs::read_to_string(root.join("config.toml")).unwrap(), input);
        assert_eq!(fs::read_dir(root).unwrap().count(), 1);
        assert!(matches!(
            write_codex_mcp_config(Utf8Path::new("relative"), port),
            Err(ManagedConfigError::InvalidRoot)
        ));
    }
}
