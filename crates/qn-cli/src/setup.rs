//! Present the core installation consumer without importing engine adapters.
use crate::{Error, escape_terminal_text};
use qn_core::setup::{EngineInstallation, SetupOperation};
use std::io::Write;

pub(crate) fn inspect(
    operation: SetupOperation,
    paths: &qn_config::paths::QnPaths,
) -> Result<EngineInstallation, Error> {
    let control = qn_daemon::ControlPlane::new(paths.clone()).map_err(daemon_error)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = match operation {
        SetupOperation::Inspect => runtime.block_on(control.inspect_engine()),
        SetupOperation::Install => runtime.block_on(control.install_engine()),
    };
    runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    Ok(result?)
}

fn daemon_error(error: qn_daemon::Error) -> crate::Error {
    match error {
        qn_daemon::Error::Setup(error) => crate::Error::Setup(error),
        qn_daemon::Error::Profile(error) => crate::Error::Profile(error),
        qn_daemon::Error::Audit(error) => crate::Error::Audit(error),
        qn_daemon::Error::NativeAudit(error) => crate::Error::NativeAudit(error),
        _ => crate::Error::Runtime(std::io::Error::other("control-plane composition failed")),
    }
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

pub(crate) fn render_json(
    installation: &EngineInstallation,
    output: &mut dyn Write,
) -> Result<(), Error> {
    let report = serde_json::json!({
        "schema_version": 1,
        "engine": EngineReport::from(installation),
        "installation": "verified",
        "runtime_checks": "not_checked",
    });
    writeln!(output, "{}", serde_json::to_string(&report)?)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::render_json;
    use qn_core::setup::EngineInstallation;
    use serde_json::Value;

    #[test]
    fn json_report_is_versioned_and_explicit_about_runtime_checks() {
        let installation = EngineInstallation {
            engine: "nono",
            version: "0.80.0",
            path: "/private/qn/cache/nono".into(),
        };
        let mut output = Vec::new();
        render_json(&installation, &mut output).unwrap();
        let report: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(report["schema_version"], 1);
        assert_eq!(report["installation"], "verified");
        assert_eq!(report["runtime_checks"], "not_checked");
        assert_eq!(report["engine"]["name"], "nono");
        assert_eq!(report["engine"]["version"], "0.80.0");
        assert_eq!(report["engine"]["binary"], "/private/qn/cache/nono");
    }
}
