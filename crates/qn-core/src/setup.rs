//! Engine installation and inspection consumed by CLI/bootstrap (#114/#117).
use std::{future::Future, pin::Pin};

/// Explicit mutation versus read-only managed installation inspection.
#[derive(Clone, Copy, Debug)]
pub enum SetupOperation {
    /// Download or repair a pinned release, then inspect its version.
    Install,
    /// Check existing integrity and version; never download or repair.
    Inspect,
}

/// Evidence about the managed binary, not about sandbox or credential readiness.
#[derive(Clone, Debug)]
pub struct EngineInstallation {
    /// Product-visible engine name.
    pub engine: &'static str,
    /// Pinned release version confirmed by execution.
    pub version: &'static str,
    /// Managed executable location; escape controls before terminal rendering.
    pub path: camino::Utf8PathBuf,
}

/// Redacted setup failure; adapters never expose credential-bearing environment or output.
#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    /// Managed cache root is not a usable absolute path.
    #[error("invalid engine cache; set QN_HOME to an absolute UTF-8 directory")]
    Configuration,
    /// No native installation route exists on this host.
    #[error("native engine installation unsupported; Windows requires the managed WSL2 setup path")]
    Unsupported,
    /// Adapter retains its original cause behind redacted formatting.
    #[error("engine setup failed: {0}; run 'qn install' to restore the pinned release")]
    Failed(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Runtime-neutral future for one setup operation.
/// Callers must await completion; dropping this future does not guarantee that
/// a started blocking filesystem operation is cancelled or rolled back.
pub type SetupFuture<'a> =
    Pin<Box<dyn Future<Output = Result<EngineInstallation, SetupError>> + Send + 'a>>;

/// Provision or inspect the engine selected by the host composition root.
/// Implementations must check integrity before executing an existing binary and
/// redact error formatting and exposed sources. A successful result describes
/// only the managed executable, not sandbox isolation or credential readiness.
///
/// ```
/// use qn_core::setup::{EngineSetup, SetupFuture, SetupOperation};
///
/// fn inspect(setup: &dyn EngineSetup) -> SetupFuture<'_> {
///     setup.run(SetupOperation::Inspect)
/// }
/// ```
pub trait EngineSetup: Send + Sync {
    /// Complete one operation; success does not prove sandbox policy or lifecycle readiness.
    fn run(&self, operation: SetupOperation) -> SetupFuture<'_>;
}
