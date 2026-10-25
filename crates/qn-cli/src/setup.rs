//! Present the core installation consumer without importing engine adapters.
use crate::{Error, escape_terminal_text};
use qn_core::setup::{EngineInstallation, EngineSetup, SetupOperation};
use std::io::Write;

pub(crate) fn inspect(
    operation: SetupOperation,
    paths: &qn_config::paths::QnPaths,
) -> Result<EngineInstallation, Error> {
    let setup = qn_bootstrap::engine_setup(paths)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    // Keep the runtime alive until the owned blocking worker completes.
    Ok(runtime.block_on(setup.run(operation))?)
}

pub(crate) fn render(
    installation: &EngineInstallation,
    output: &mut dyn Write,
) -> Result<(), Error> {
    writeln!(
        output,
        "engine: {} {} (archive integrity and version verified)",
        installation.engine, installation.version
    )?;
    writeln!(
        output,
        "binary: {}",
        escape_terminal_text(installation.path.as_str())
    )?;
    writeln!(
        output,
        "Sandbox isolation, credentials and full process cleanup: not verified by this check."
    )?;
    Ok(())
}

#[derive(serde::Serialize)]
pub(crate) struct EngineReport {
    name: &'static str,
    version: &'static str,
    binary: String,
    runtime_checks: &'static str,
}
impl From<&EngineInstallation> for EngineReport {
    fn from(installation: &EngineInstallation) -> Self {
        Self {
            name: installation.engine,
            version: installation.version,
            binary: escape_terminal_text(installation.path.as_str()),
            runtime_checks: "not_checked",
        }
    }
}
