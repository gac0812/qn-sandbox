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

    /// Returns the shared official profile registry.
    #[must_use]
    pub fn profiles(&self) -> &dyn ProfileRegistry {
        self.profiles.as_ref()
    }

    /// Returns the shared managed engine setup service.
    #[must_use]
    pub fn engine(&self) -> &dyn EngineSetup {
        self.engine.as_ref()
    }

    /// Returns the shared broker audit query service.
    #[must_use]
    pub fn audit(&self) -> &dyn AuditQuery {
        self.audit.as_ref()
    }

    /// Returns the shared retained native audit query service.
    #[must_use]
    pub fn native_audit(&self) -> &dyn NativeAuditQuery {
        self.native_audit.as_ref()
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

#[cfg(test)]
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
        assert!(!plane.profiles().list().unwrap_or_default().is_empty());
    }
}
