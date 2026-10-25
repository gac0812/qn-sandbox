//! Versioned MCP tool catalog contract for qn-sandbox.
//!
//! This crate contains metadata only. It never opens a transport, reads a
//! credential, invokes an adapter, or authorizes a request.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Current catalog wire version.
pub const CATALOG_SCHEMA_VERSION: u8 = 1;

/// Credential posture visible to an MCP consumer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialPosture {
    /// The host mediates credentials; the agent receives no credential value.
    HostMediated,
    /// The tool does not require credentials.
    None,
}

/// A fixed MCP tool name.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    /// List available official profiles.
    ProfileList,
    /// Show one resolved profile summary.
    ProfileShow,
    /// Start one foreground agent session.
    SessionRun,
    /// Read one session status.
    SessionStatus,
    /// Request one session to stop.
    SessionStop,
    /// Query bounded audit evidence.
    AuditQuery,
    /// Plan an approved MCP route without activating it.
    McpRoutePlan,
}

impl ToolName {
    /// Returns the stable wire name used by MCP and SDK consumers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProfileList => "profile_list",
            Self::ProfileShow => "profile_show",
            Self::SessionRun => "session_run",
            Self::SessionStatus => "session_status",
            Self::SessionStop => "session_stop",
            Self::AuditQuery => "audit_query",
            Self::McpRoutePlan => "mcp_route_plan",
        }
    }
}

/// A tool descriptor exposed to future MCP/SDK consumers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolDescriptor {
    /// Stable tool name.
    pub name: ToolName,
    /// Short user-facing description.
    pub description: String,
    /// JSON Schema object for tool input.
    pub input_schema: serde_json::Value,
    /// Whether credentials are mediated by the host.
    pub credential_posture: CredentialPosture,
}

/// Versioned, deterministic tool catalog.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolCatalog {
    /// Wire schema version.
    pub schema_version: u8,
    /// Tools sorted by their stable wire name.
    pub tools: Vec<ToolDescriptor>,
}

/// Errors while constructing or validating a catalog.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum CatalogError {
    /// A tool name is not a lowercase `snake_case` identifier.
    #[error("invalid MCP tool name: {0}")]
    InvalidName(String),
    /// A catalog contains duplicate wire names.
    #[error("duplicate MCP tool name: {0}")]
    DuplicateName(String),
    /// A schema contains a field that could carry a credential value.
    #[error("MCP input schema contains a credential-shaped field: {0}")]
    CredentialField(String),
}

impl ToolCatalog {
    /// Builds the standard qn-sandbox catalog.
    #[must_use]
    pub fn standard() -> Self {
        let mut catalog = Self {
            schema_version: CATALOG_SCHEMA_VERSION,
            tools: vec![
                descriptor(
                    ToolName::ProfileList,
                    "List official profiles.",
                    CredentialPosture::None,
                    json_schema(&[], &[]),
                ),
                descriptor(
                    ToolName::ProfileShow,
                    "Show one resolved profile.",
                    CredentialPosture::None,
                    json_schema(&["id"], &[]),
                ),
                descriptor(
                    ToolName::SessionRun,
                    "Start a restricted agent session.",
                    CredentialPosture::HostMediated,
                    json_schema(&["profile", "command"], &[]),
                ),
                descriptor(
                    ToolName::SessionStatus,
                    "Read one session status.",
                    CredentialPosture::None,
                    json_schema(&["session"], &[]),
                ),
                descriptor(
                    ToolName::SessionStop,
                    "Request one session to stop.",
                    CredentialPosture::None,
                    json_schema(&["session"], &[]),
                ),
                descriptor(
                    ToolName::AuditQuery,
                    "Query bounded audit evidence.",
                    CredentialPosture::None,
                    json_schema(&["session"], &[]),
                ),
                descriptor(
                    ToolName::McpRoutePlan,
                    "Plan an approved MCP route.",
                    CredentialPosture::HostMediated,
                    json_schema(&["name", "endpoint"], &[]),
                ),
            ],
        };
        catalog.tools.sort_by_key(|tool| tool.name.as_str());
        catalog
    }

    /// Validates wire names, ordering, uniqueness and schema credential posture.
    ///
    /// # Errors
    ///
    /// Returns a closed error when a name is invalid, duplicated, or a schema
    /// contains a credential-shaped field.
    pub fn validate(&self) -> Result<(), CatalogError> {
        let mut names = BTreeSet::new();
        for tool in &self.tools {
            let name = tool.name.as_str();
            if !is_snake_case(name) {
                return Err(CatalogError::InvalidName(name.to_owned()));
            }
            if !names.insert(name) {
                return Err(CatalogError::DuplicateName(name.to_owned()));
            }
            reject_credential_fields(&tool.input_schema, "")?;
        }
        Ok(())
    }
}

fn descriptor(
    name: ToolName,
    description: &str,
    posture: CredentialPosture,
    input_schema: serde_json::Value,
) -> ToolDescriptor {
    ToolDescriptor {
        name,
        description: description.to_owned(),
        input_schema,
        credential_posture: posture,
    }
}

fn json_schema(required: &[&str], optional: &[&str]) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    for field in required.iter().chain(optional) {
        properties.insert((*field).to_owned(), serde_json::json!({ "type": "string" }));
    }
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn is_snake_case(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !name.starts_with('_')
        && !name.ends_with('_')
        && !name.contains("__")
}

fn reject_credential_fields(value: &serde_json::Value, path: &str) -> Result<(), CatalogError> {
    if let Some(properties) = value
        .get("properties")
        .and_then(serde_json::Value::as_object)
    {
        for (name, child) in properties {
            let current = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            if ["api_key", "password", "secret", "token"].contains(&name.as_str()) {
                return Err(CatalogError::CredentialField(current));
            }
            reject_credential_fields(child, &current)?;
        }
    }
    if let Some(items) = value.get("items") {
        reject_credential_fields(items, &format!("{path}[]"))?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn standard_catalog_is_valid_and_deterministic() {
        let catalog = ToolCatalog::standard();
        catalog.validate().unwrap();
        let names: Vec<_> = catalog
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn catalog_round_trips_without_changing_wire_names() {
        let catalog = ToolCatalog::standard();
        let encoded = serde_json::to_vec(&catalog).unwrap();
        let decoded: ToolCatalog = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(catalog, decoded);
        assert_eq!(decoded.schema_version, 1);
    }

    #[test]
    fn invalid_names_and_credential_fields_are_rejected() {
        let mut catalog = ToolCatalog::standard();
        catalog.tools[0].input_schema = serde_json::json!({
            "type": "object",
            "properties": { "api_key": { "type": "string" } }
        });
        assert!(matches!(
            catalog.validate(),
            Err(CatalogError::CredentialField(_))
        ));
        assert!(is_snake_case("session_run"));
        assert!(!is_snake_case("SessionRun"));
        assert!(!is_snake_case("session__run"));
    }
}
