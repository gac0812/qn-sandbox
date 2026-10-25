//! Shared local control-plane composition for qn-sandbox.
//!
//! This crate owns no adapter behavior or transport. It assembles the trusted
//! bootstrap services once so CLI, UI and later MCP surfaces can share them.

use qn_core::audit::AuditQuery;
use qn_core::native_audit::NativeAuditQuery;
use qn_core::profile::ProfileRegistry;
use qn_core::runtime::{RunReport, SessionError, SessionStatus, StopMode, StopReport};
use qn_core::setup::EngineSetup;
use qn_core::{RunSpec, SessionId};
use qn_mcp::{ToolCatalog, ToolOutcome, ToolRequest, ToolResult};
use tokio_util::sync::CancellationToken;

/// Errors raised while composing the local control plane.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A trusted profile registry could not be assembled.
    #[error(transparent)]
    Profile(#[from] qn_core::profile::ProfileError),
    /// A trusted engine setup service could not be assembled.
    #[error(transparent)]
    Setup(#[from] qn_core::setup::SetupError),
    /// A trusted broker audit reader could not be assembled.
    #[error(transparent)]
    Audit(#[from] qn_core::audit::AuditQueryError),
    /// A trusted native audit reader could not be assembled.
    #[error(transparent)]
    NativeAudit(#[from] qn_core::native_audit::NativeAuditError),
}

/// Shared local control-plane composition root.
///
/// The paths are resolved once at construction and then reused by every
/// surface. Adapter-specific types remain hidden behind core traits.
pub struct ControlPlane {
    paths: qn_config::paths::QnPaths,
    profiles: Box<dyn ProfileRegistry>,
    engine: Box<dyn EngineSetup>,
    audit: Box<dyn AuditQuery>,
    native_audit: Box<dyn NativeAuditQuery>,
}

impl ControlPlane {
    /// Validates an MCP request against the catalog without executing a tool.
    ///
    /// This boundary is transport-neutral; later dispatchers can map a
    /// validated request to shared services without exposing adapter types.
    #[must_use]
    pub fn validate_mcp_request(&self, request: &ToolRequest) -> ToolResult {
        let request_id = request.request_id.clone();
        if let Err(error) = request.validate() {
            return ToolResult {
                schema_version: qn_mcp::CATALOG_SCHEMA_VERSION,
                request_id,
                outcome: ToolOutcome::Error {
                    code: qn_mcp::ResultErrorCode::InvalidArguments,
                    message: error.to_string(),
                },
            };
        }
        let catalog = ToolCatalog::standard();
        if !catalog.tools.iter().any(|tool| tool.name == request.tool) {
            return ToolResult {
                schema_version: qn_mcp::CATALOG_SCHEMA_VERSION,
                request_id,
                outcome: ToolOutcome::Error {
                    code: qn_mcp::ResultErrorCode::UnknownTool,
                    message: "requested MCP tool is not in the catalog".into(),
                },
            };
        }
        ToolResult {
            schema_version: qn_mcp::CATALOG_SCHEMA_VERSION,
            request_id,
            outcome: ToolOutcome::Error {
                code: qn_mcp::ResultErrorCode::Failed,
                message: "MCP dispatch is not activated; use a configured surface".into(),
            },
        }
    }

    /// Dispatches read-only MCP tools through the shared control plane.
    ///
    /// Mutating or credential-sensitive tools return a closed denial until
    /// their authorization and mediation boundaries are connected.
    pub async fn dispatch_mcp_request(&self, request: ToolRequest) -> ToolResult {
        let request_id = request.request_id.clone();
        if let Err(error) = request.validate() {
            return invalid_result(request_id, error.to_string());
        }
        match request.tool {
            qn_mcp::ToolName::ProfileList => self.dispatch_profile_list(request_id),
            qn_mcp::ToolName::ProfileShow => self.dispatch_profile_show(request_id, &request),
            qn_mcp::ToolName::SessionStatus => {
                self.dispatch_session_status(request_id, &request).await
            }
            qn_mcp::ToolName::AuditQuery => self.dispatch_audit_query(request_id, &request).await,
            qn_mcp::ToolName::SessionRun
            | qn_mcp::ToolName::SessionStop
            | qn_mcp::ToolName::McpRoutePlan => denied_result(request_id),
        }
    }

    fn dispatch_profile_list(&self, request_id: String) -> ToolResult {
        match self.list_profiles() {
            Ok(profiles) => ok_result(
                request_id,
                serde_json::json!({
                    "profiles": profiles.iter().map(|profile| serde_json::json!({
                        "id": profile.id().as_str(),
                        "version": profile.version().as_str(),
                    })).collect::<Vec<_>>()
                }),
            ),
            Err(error) => failed_result(request_id, error.to_string()),
        }
    }

    fn dispatch_profile_show(&self, request_id: String, request: &ToolRequest) -> ToolResult {
        let Some(value) = request
            .arguments
            .get("id")
            .and_then(serde_json::Value::as_str)
        else {
            return invalid_result(request_id, "profile id must be a string".into());
        };
        let Ok(id) = qn_core::ProfileRef::try_from(value) else {
            return invalid_result(request_id, "profile id is invalid".into());
        };
        match self.resolve_profile(id) {
            Ok(profile) => ok_result(
                request_id,
                serde_json::json!({
                    "id": profile.summary().id().as_str(),
                    "version": profile.summary().version().as_str(),
                    "state_dir": profile.state_dir(),
                    "network_hosts": profile.network_hosts(),
                }),
            ),
            Err(error) => failed_result(request_id, error.to_string()),
        }
    }

    async fn dispatch_session_status(
        &self,
        request_id: String,
        request: &ToolRequest,
    ) -> ToolResult {
        let Some(value) = request
            .arguments
            .get("session")
            .and_then(serde_json::Value::as_str)
        else {
            return invalid_result(request_id, "session must be a string".into());
        };
        let Ok(id) = SessionId::try_from(value) else {
            return invalid_result(request_id, "session id is invalid".into());
        };
        match self.status_session(id).await {
            Ok(status) => ok_result(
                request_id,
                serde_json::json!({
                    "session": status.session().to_string(),
                    "profile": status.profile().to_string(),
                    "exit_code": status.exit_code(),
                    "cleanup": "unconfirmed",
                }),
            ),
            Err(error) => failed_result(request_id, error.to_string()),
        }
    }

    async fn dispatch_audit_query(&self, request_id: String, request: &ToolRequest) -> ToolResult {
        let Some(value) = request
            .arguments
            .get("session")
            .and_then(serde_json::Value::as_str)
        else {
            return invalid_result(request_id, "session must be a string".into());
        };
        let Ok(id) = SessionId::try_from(value) else {
            return invalid_result(request_id, "session id is invalid".into());
        };
        match self.read_audit(id).await {
            Ok(timeline) => ok_result(
                request_id,
                serde_json::to_value(timeline)
                    .unwrap_or_else(|_| serde_json::json!({"error": "audit serialization failed"})),
            ),
            Err(error) => failed_result(request_id, error.to_string()),
        }
    }

    /// Builds the local composition from an absolute qn home.
    ///
    /// # Errors
    ///
    /// Returns an error if any trusted adapter cannot be assembled.
    pub fn new(paths: qn_config::paths::QnPaths) -> Result<Self, Error> {
        if !paths.root.is_absolute() {
            return Err(Error::Profile(qn_core::profile::ProfileError::InvalidRoot));
        }
        let profiles = qn_bootstrap::profile_registry(&paths)?;
        let engine = qn_bootstrap::engine_setup(&paths)?;
        let audit = qn_bootstrap::audit_query(paths.audit.clone())?;
        let native_audit = qn_bootstrap::native_audit_query(&paths)?;
        Ok(Self {
            paths,
            profiles: Box::new(profiles),
            engine: Box::new(engine),
            audit: Box::new(audit),
            native_audit: Box::new(native_audit),
        })
    }

    /// Returns the resolved qn paths used by this composition.
    #[must_use]
    pub fn paths(&self) -> &qn_config::paths::QnPaths {
        &self.paths
    }

    /// Returns the shared official profile registry for read-only consumers.
    #[must_use]
    pub fn profiles(&self) -> &dyn ProfileRegistry {
        self.profiles.as_ref()
    }

    /// Returns the shared engine setup service for read-only consumers.
    #[must_use]
    pub fn engine(&self) -> &dyn EngineSetup {
        self.engine.as_ref()
    }

    /// Returns the shared broker audit query service for read-only consumers.
    #[must_use]
    pub fn audit(&self) -> &dyn AuditQuery {
        self.audit.as_ref()
    }

    /// Returns the shared native audit query service for read-only consumers.
    #[must_use]
    pub fn native_audit(&self) -> &dyn NativeAuditQuery {
        self.native_audit.as_ref()
    }

    /// Lists compiled and installed profile summaries.
    ///
    /// # Errors
    ///
    /// Preserves profile registry validation and storage errors.
    pub fn list_profiles(
        &self,
    ) -> Result<Vec<qn_core::profile::ProfileSummary>, qn_core::profile::ProfileError> {
        self.profiles.list()
    }

    /// Resolves one profile identity through the shared registry.
    ///
    /// # Errors
    ///
    /// Preserves unknown, missing and integrity failures.
    pub fn resolve_profile(
        &self,
        id: qn_core::ProfileRef,
    ) -> Result<qn_core::profile::ResolvedProfile, qn_core::profile::ProfileError> {
        self.profiles.resolve(id)
    }

    /// Inspects the managed engine without downloading or repairing it.
    ///
    /// # Errors
    ///
    /// Preserves setup integrity, version and host support failures.
    pub async fn inspect_engine(
        &self,
    ) -> Result<qn_core::setup::EngineInstallation, qn_core::setup::SetupError> {
        self.engine
            .run(qn_core::setup::SetupOperation::Inspect)
            .await
    }

    /// Installs or repairs the pinned managed engine.
    ///
    /// # Errors
    ///
    /// Preserves setup download, integrity, version and host support failures.
    pub async fn install_engine(
        &self,
    ) -> Result<qn_core::setup::EngineInstallation, qn_core::setup::SetupError> {
        self.engine
            .run(qn_core::setup::SetupOperation::Install)
            .await
    }

    /// Reads one bounded broker timeline.
    ///
    /// # Errors
    ///
    /// Preserves missing, invalid, incomplete and bounded-query errors.
    pub async fn read_audit(
        &self,
        id: SessionId,
    ) -> Result<qn_core::audit::AuditTimeline, qn_core::audit::AuditQueryError> {
        self.audit.read(id).await
    }

    /// Reads one bounded retained native report.
    ///
    /// # Errors
    ///
    /// Preserves association, setup and native query failures.
    pub async fn read_native_audit(
        &self,
        id: SessionId,
    ) -> Result<qn_core::native_audit::NativeAuditReport, qn_core::native_audit::NativeAuditError>
    {
        self.native_audit.read(id).await
    }

    /// Runs one foreground session through the bootstrap composition.
    ///
    /// # Errors
    ///
    /// Preserves the bootstrap session error categories.
    pub async fn run_session(
        &self,
        spec: RunSpec,
        resolved: &qn_core::profile::ResolvedProfile,
        cancel: CancellationToken,
    ) -> Result<RunReport, SessionError> {
        qn_bootstrap::run_session(&self.paths, spec, resolved, cancel).await
    }

    /// Reads one owner session status through the bootstrap composition.
    ///
    /// # Errors
    ///
    /// Preserves bootstrap storage and native observation errors.
    pub async fn status_session(&self, id: SessionId) -> Result<SessionStatus, SessionError> {
        qn_bootstrap::status_session(&self.paths, id).await
    }

    /// Requests one owner session stop through the bootstrap composition.
    ///
    /// # Errors
    ///
    /// Preserves bootstrap session and native stop errors.
    pub async fn stop_session(
        &self,
        id: SessionId,
        mode: StopMode,
    ) -> Result<StopReport, SessionError> {
        qn_bootstrap::stop_session(&self.paths, id, mode).await
    }
}

fn ok_result(request_id: String, value: serde_json::Value) -> ToolResult {
    ToolResult {
        schema_version: qn_mcp::CATALOG_SCHEMA_VERSION,
        request_id,
        outcome: ToolOutcome::Ok { value },
    }
}
fn invalid_result(request_id: String, message: String) -> ToolResult {
    ToolResult {
        schema_version: qn_mcp::CATALOG_SCHEMA_VERSION,
        request_id,
        outcome: ToolOutcome::Error {
            code: qn_mcp::ResultErrorCode::InvalidArguments,
            message,
        },
    }
}
fn failed_result(request_id: String, message: String) -> ToolResult {
    ToolResult {
        schema_version: qn_mcp::CATALOG_SCHEMA_VERSION,
        request_id,
        outcome: ToolOutcome::Error {
            code: qn_mcp::ResultErrorCode::Failed,
            message,
        },
    }
}
fn denied_result(request_id: String) -> ToolResult {
    ToolResult {
        schema_version: qn_mcp::CATALOG_SCHEMA_VERSION,
        request_id,
        outcome: ToolOutcome::Error {
            code: qn_mcp::ResultErrorCode::Denied,
            message: "MCP operation is not enabled on this control-plane boundary".into(),
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::ControlPlane;

    #[test]
    fn it_rejects_relative_composition_roots_without_adapter_work() {
        let paths = qn_config::paths::QnPaths::from_root("relative/qn");
        assert!(ControlPlane::new(paths).is_err());
    }

    #[test]
    fn it_composes_absolute_profile_and_query_surfaces() {
        let paths = qn_config::paths::QnPaths::from_root("/tmp/qn-daemon-test");
        let plane =
            ControlPlane::new(paths).unwrap_or_else(|error| panic!("composition failed: {error}"));
        assert!(plane.paths().root.is_absolute());
        assert!(!plane.list_profiles().unwrap_or_default().is_empty());
        assert!(
            plane
                .resolve_profile(
                    qn_core::ProfileRef::try_from("qn/claude-code")
                        .unwrap_or_else(|error| panic!("valid fixture: {error}"))
                )
                .is_ok()
        );
    }

    #[test]
    fn it_reports_unknown_profiles_without_touching_runtime_state() {
        let paths = qn_config::paths::QnPaths::from_root("/tmp/qn-daemon-test-unknown");
        let plane =
            ControlPlane::new(paths).unwrap_or_else(|error| panic!("composition failed: {error}"));
        let result = plane.resolve_profile(
            qn_core::ProfileRef::try_from("qn/unknown")
                .unwrap_or_else(|error| panic!("valid fixture: {error}")),
        );
        assert!(matches!(
            result,
            Err(qn_core::profile::ProfileError::UnknownProfile)
        ));
    }

    #[test]
    fn it_lists_profiles_without_creating_runtime_or_audit_state() {
        let root = tempfile::tempdir().unwrap();
        let paths = qn_config::paths::QnPaths::from_root(
            camino::Utf8PathBuf::from_path_buf(root.path().to_owned()).unwrap(),
        );
        let plane = ControlPlane::new(paths).unwrap();
        let summaries = plane.list_profiles().unwrap();
        assert_eq!(summaries.len(), 2);
        assert!(!root.path().join("runtime").exists());
        assert!(!root.path().join("audit").exists());
    }

    #[cfg(test)]
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    mod mcp_dispatch_tests {
        use super::ControlPlane;
        use qn_mcp::ToolOutcome;

        #[test]
        fn it_returns_closed_results_without_executing_a_tool() {
            let plane =
                ControlPlane::new(qn_config::paths::QnPaths::from_root("/tmp/qn-daemon-mcp"))
                    .unwrap();
            let request = qn_mcp::ToolRequest {
                schema_version: 1,
                request_id: "req-1".into(),
                tool: qn_mcp::ToolName::SessionStatus,
                arguments: serde_json::json!({"session": "sxn-test"}),
            };
            let result = plane.validate_mcp_request(&request);
            assert_eq!(result.request_id, "req-1");
            assert!(matches!(
                result.outcome,
                ToolOutcome::Error {
                    code: qn_mcp::ResultErrorCode::Failed,
                    ..
                }
            ));
            let bad = qn_mcp::ToolRequest {
                arguments: serde_json::json!(["secret"]),
                ..request
            };
            let result = plane.validate_mcp_request(&bad);
            assert!(matches!(
                result.outcome,
                ToolOutcome::Error {
                    code: qn_mcp::ResultErrorCode::InvalidArguments,
                    ..
                }
            ));
        }
    }
}
