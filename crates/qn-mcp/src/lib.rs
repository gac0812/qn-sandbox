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

/// Versioned request sent from an MCP or SDK consumer to the control plane.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolRequest {
    /// Wire schema version.
    pub schema_version: u8,
    /// Caller-owned correlation identifier.
    pub request_id: String,
    /// Stable catalog tool name.
    pub tool: ToolName,
    /// JSON object arguments; credentials remain host-mediated.
    pub arguments: serde_json::Value,
}

/// Closed error categories for a failed tool invocation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultErrorCode {
    /// The requested tool is not in the catalog.
    UnknownTool,
    /// Arguments failed the tool schema.
    InvalidArguments,
    /// The operation was refused by policy.
    Denied,
    /// The operation exceeded its bounded deadline.
    Timeout,
    /// The operation failed without exposing source payloads.
    Failed,
}

/// Mutually exclusive tool success/error outcome.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ToolOutcome {
    /// Tool completed with a JSON payload.
    Ok {
        /// Tool output payload.
        value: serde_json::Value,
    },
    /// Tool failed with a closed, safe error.
    Error {
        /// Closed error category.
        code: ResultErrorCode,
        /// Safe human-readable message without source payloads.
        message: String,
    },
}

/// Versioned result returned to an MCP or SDK consumer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolResult {
    /// Wire schema version.
    pub schema_version: u8,
    /// Correlation identifier copied from the request.
    pub request_id: String,
    /// Success payload or closed error.
    pub outcome: ToolOutcome,
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
    /// Request metadata is invalid.
    #[error("MCP tool request metadata is invalid")]
    InvalidRequest,
    /// Tool arguments must be a JSON object.
    #[error("MCP tool arguments must be a JSON object")]
    ArgumentsObject,
}

impl ToolRequest {
    /// Validates version, correlation metadata and credential-free object arguments.
    ///
    /// # Errors
    ///
    /// Returns a closed error without echoing argument values.
    pub fn validate(&self) -> Result<(), CatalogError> {
        if self.schema_version != CATALOG_SCHEMA_VERSION || self.request_id.is_empty() {
            return Err(CatalogError::InvalidRequest);
        }
        if !self.arguments.is_object() {
            return Err(CatalogError::ArgumentsObject);
        }
        reject_credential_fields(&self.arguments, "")
    }
}

impl ToolResult {
    /// Validates result version and correlation metadata.
    ///
    /// # Errors
    ///
    /// Returns a closed error if the envelope metadata is invalid.
    pub fn validate(&self) -> Result<(), CatalogError> {
        if self.schema_version != CATALOG_SCHEMA_VERSION || self.request_id.is_empty() {
            return Err(CatalogError::InvalidRequest);
        }
        Ok(())
    }
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
            if is_credential_field(name) {
                return Err(CatalogError::CredentialField(current));
            }
            reject_credential_fields(child, &current)?;
        }
    }
    if let Some(object) = value.as_object() {
        for (name, child) in object {
            if [
                "properties",
                "items",
                "oneOf",
                "anyOf",
                "allOf",
                "type",
                "required",
                "additionalProperties",
            ]
            .contains(&name.as_str())
            {
                continue;
            }
            let current = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            if is_credential_field(name) {
                return Err(CatalogError::CredentialField(current));
            }
            reject_credential_fields(child, &current)?;
        }
    }
    if let Some(items) = value.get("items") {
        reject_credential_fields(items, &format!("{path}[]"))?;
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(branches) = value.get(key).and_then(serde_json::Value::as_array) {
            for (index, branch) in branches.iter().enumerate() {
                reject_credential_fields(branch, &format!("{path}.{key}[{index}]"))?;
            }
        }
    }
    Ok(())
}

fn is_credential_field(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "api_key" | "apikey" | "password" | "secret" | "token" | "access_token" | "refresh_token"
    )
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
        let safe = ToolCatalog::standard();
        assert!(safe.validate().is_ok());
    }
}

#[cfg(test)]
mod schema_contract_tests {
    use super::{CredentialPosture, ToolCatalog, ToolName};

    #[test]
    fn standard_catalog_has_the_c3_consumer_contract() {
        let catalog = ToolCatalog::standard();
        assert_eq!(catalog.schema_version, 1);
        let names: Vec<_> = catalog.tools.iter().map(|tool| tool.name).collect();
        assert_eq!(
            names,
            vec![
                ToolName::AuditQuery,
                ToolName::McpRoutePlan,
                ToolName::ProfileList,
                ToolName::ProfileShow,
                ToolName::SessionRun,
                ToolName::SessionStatus,
                ToolName::SessionStop,
            ]
        );
        let posture: Vec<_> = catalog
            .tools
            .iter()
            .map(|tool| (tool.name, tool.credential_posture))
            .collect();
        assert_eq!(posture[0], (ToolName::AuditQuery, CredentialPosture::None));
        assert_eq!(
            posture[1],
            (ToolName::McpRoutePlan, CredentialPosture::HostMediated)
        );
        assert_eq!(
            posture[4],
            (ToolName::SessionRun, CredentialPosture::HostMediated)
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod envelope_tests {
    use super::*;

    #[test]
    fn request_and_result_round_trip_with_correlated_schema() {
        let request = ToolRequest {
            schema_version: CATALOG_SCHEMA_VERSION,
            request_id: "req-1".into(),
            tool: ToolName::SessionStatus,
            arguments: serde_json::json!({"session": "sxn-test"}),
        };
        request.validate().unwrap();
        let request_bytes = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            serde_json::from_slice::<ToolRequest>(&request_bytes).unwrap(),
            request
        );

        let result = ToolResult {
            schema_version: CATALOG_SCHEMA_VERSION,
            request_id: request.request_id.clone(),
            outcome: ToolOutcome::Ok {
                value: serde_json::json!({"phase": "running"}),
            },
        };
        result.validate().unwrap();
        let result_bytes = serde_json::to_vec(&result).unwrap();
        assert_eq!(
            serde_json::from_slice::<ToolResult>(&result_bytes).unwrap(),
            result
        );
    }

    #[test]
    fn request_rejects_non_object_and_credential_arguments() {
        let mut request = ToolRequest {
            schema_version: CATALOG_SCHEMA_VERSION,
            request_id: "req-1".into(),
            tool: ToolName::SessionRun,
            arguments: serde_json::json!([]),
        };
        assert_eq!(request.validate(), Err(CatalogError::ArgumentsObject));
        request.arguments = serde_json::json!({"config": {"password": "secret"}});
        assert!(matches!(
            request.validate(),
            Err(CatalogError::CredentialField(_))
        ));
    }

    #[test]
    fn envelopes_reject_wrong_versions_or_empty_request_ids() {
        let request = ToolRequest {
            schema_version: 2,
            request_id: String::new(),
            tool: ToolName::ProfileList,
            arguments: serde_json::json!({}),
        };
        assert_eq!(request.validate(), Err(CatalogError::InvalidRequest));
        let result = ToolResult {
            schema_version: 2,
            request_id: String::new(),
            outcome: ToolOutcome::Error {
                code: ResultErrorCode::Failed,
                message: "safe failure".into(),
            },
        };
        assert_eq!(result.validate(), Err(CatalogError::InvalidRequest));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod sdk_contract_tests {
    use super::{CredentialPosture, ToolCatalog, ToolName};
    use serde_json::Value;

    #[test]
    fn sdk_fixture_names_and_postures_match_the_catalog() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../sdks/contracts/control-plane-v1.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema_version"], 1);
        let expected: Vec<_> = ToolCatalog::standard()
            .tools
            .into_iter()
            .map(|tool| {
                let posture = match tool.credential_posture {
                    CredentialPosture::None => "none",
                    CredentialPosture::HostMediated => "host_mediated",
                };
                (tool.name.as_str().to_owned(), posture)
            })
            .collect();
        let actual: Vec<_> = fixture["operations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|operation| {
                (
                    operation["name"].as_str().unwrap().to_owned(),
                    operation["credential_posture"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(ToolName::SessionRun.as_str(), "session_run");
    }
}
