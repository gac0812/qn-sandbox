//! Runtime composition root for qn-sandbox.

use process_wrap::tokio::TokioCommandWrap;
use qn_infra::process::{ProcessPolicy, TerminationSafety};

struct HostProcessPolicy;
impl ProcessPolicy for HostProcessPolicy {
    fn configure(&self, command: &mut TokioCommandWrap) -> TerminationSafety {
        #[cfg(unix)]
        {
            command.wrap(process_wrap::tokio::ProcessGroup::leader());
            TerminationSafety::WhileParentOwned
        }
        #[cfg(windows)]
        {
            command.wrap(process_wrap::tokio::JobObject);
            TerminationSafety::RetainedGroupHandle
        }
    }
}

/// Assemble native group/job lifecycle control without exposing platform branches.
///
/// ```no_run
/// # async fn example() -> Result<(), qn_infra::Error> {
/// use qn_infra::process::{CmdSpec, spawn_managed};
/// let spec = CmdSpec::new("/absolute/path/to/program").env_clear();
/// let process = spawn_managed(&spec, &qn_bootstrap::process_policy())?;
/// let output = process.wait(Default::default()).await?;
/// assert!(output.output().success);
/// # Ok(()) }
/// ```
#[must_use]
pub fn process_policy() -> impl ProcessPolicy {
    HostProcessPolicy
}

/// Compose the versioned official-profile catalog and host registry.
///
/// The returned registry performs bounded local storage work only; it never
/// starts an engine or resolves a credential.
///
/// # Errors
///
/// Rejects a relative profile root and an invalid embedded catalog.
pub fn profile_registry(
    paths: &qn_config::paths::QnPaths,
) -> Result<impl qn_core::profile::ProfileRegistry, qn_core::profile::ProfileError> {
    qn_nono::registry::HostProfileRegistry::new(paths.profiles.clone())
}

/// Assemble the pinned native installer/inspector under the resolved cache root.
///
/// # Errors
/// Rejects invalid cache roots and hosts requiring managed WSL2 installation.
///
///     let paths = qn_config::paths::QnPaths::from_root("/tmp/qn");
///     let setup = qn_bootstrap::engine_setup(&paths);
///
pub fn engine_setup(
    paths: &qn_config::paths::QnPaths,
) -> Result<impl qn_core::setup::EngineSetup, qn_core::setup::SetupError> {
    qn_nono::setup::NonoSetup::new(&paths.engine_cache())
}
