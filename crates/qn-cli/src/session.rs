//! Foreground session presentation; no cleanup-success claim (#93).

use crate::Error;
use camino::Utf8PathBuf;
use qn_core::profile::ProfileRegistry as _;
use qn_core::runtime::{ObservedNativeState, RunEnd, RunPhase, StopMode};
use qn_core::{ProfileRef, SessionId};
use std::{io::Write, time::Duration};

pub(crate) fn run(
    profile: &str,
    command: Vec<String>,
    json: bool,
    output: &mut dyn Write,
) -> Result<(), Error> {
    let profile = ProfileRef::try_from(profile)?;
    let paths = qn_config::paths::QnPaths::resolve()?;
    let registry = qn_bootstrap::profile_registry(&paths)?;
    let resolved = registry.resolve(profile.clone())?;
    let workdir = std::env::current_dir().map_err(|_| Error::Workdir)?;
    let workdir = Utf8PathBuf::from_path_buf(workdir).map_err(|_| Error::Workdir)?;
    let spec = qn_core::RunSpec::new(SessionId::new(), profile, command, workdir)?;
    let runtime = runtime()?;
    let cancel = tokio_util::sync::CancellationToken::new();
    let signal = cancel.clone();
    let handle = runtime.spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        signal.cancel();
    });
    let result = runtime.block_on(qn_bootstrap::run_session(&paths, spec, &resolved, cancel));
    handle.abort();
    runtime.shutdown_timeout(Duration::from_millis(100));
    let report = result?;
    if json {
        let payload = serde_json::json!({
            "schema_version": 1,
            "session": report.session().to_string(),
            "profile": report.profile().to_string(),
            "native": report.native_id(),
            "end": end_label(report.end()),
            "exit_code": report.exit_code(),
            "cleanup": "unconfirmed",
        });
        writeln!(output, "{}", serde_json::to_string(&payload)?)?;
        return Ok(());
    }
    writeln!(output, "session: {}", report.session())?;
    writeln!(output, "profile: {}", report.profile())?;
    writeln!(output, "native: {}", report.native_id())?;
    writeln!(output, "end: {}", end_label(report.end()))?;
    writeln!(
        output,
        "exit_code: {}",
        report
            .exit_code()
            .map_or_else(|| "none".to_owned(), |code| code.to_string())
    )?;
    writeln!(
        output,
        "cleanup: unconfirmed (execution-tree cleanup is not proven; see #93)"
    )?;
    writeln!(
        output,
        "audit: qn audit native {} (network-only)",
        report.session()
    )?;
    output.flush()?;
    Ok(())
}

pub(crate) fn status(session: &str, json: bool, output: &mut dyn Write) -> Result<(), Error> {
    let id = SessionId::try_from(session)?;
    let paths = qn_config::paths::QnPaths::resolve()?;
    let runtime = runtime()?;
    let result = runtime.block_on(qn_bootstrap::status_session(&paths, id));
    runtime.shutdown_timeout(Duration::from_millis(100));
    let status = result?;
    if json {
        let payload = serde_json::json!({
            "schema_version": 1,
            "session": status.session().to_string(),
            "profile": status.profile().to_string(),
            "phase": phase_label(status.phase()),
            "end": status.end().map(end_label),
            "exit_code": status.exit_code(),
            "native": status.native().map(native_label),
            "cleanup": "unconfirmed",
        });
        writeln!(output, "{}", serde_json::to_string(&payload)?)?;
        return Ok(());
    }
    writeln!(output, "session: {}", status.session())?;
    writeln!(output, "profile: {}", status.profile())?;
    writeln!(output, "phase: {}", phase_label(status.phase()))?;
    if let Some(end) = status.end() {
        writeln!(output, "end: {}", end_label(end))?;
    }
    if let Some(code) = status.exit_code() {
        writeln!(output, "exit_code: {code}")?;
    }
    writeln!(
        output,
        "native: {}",
        status.native().map_or("unavailable", native_label)
    )?;
    writeln!(
        output,
        "cleanup: unconfirmed (not Stopped; execution-tree cleanup is not proven; see #93)"
    )?;
    output.flush()?;
    Ok(())
}

pub(crate) fn stop(
    session: &str,
    force: bool,
    timeout: u64,
    json: bool,
    output: &mut dyn Write,
) -> Result<(), Error> {
    if !force && !(1..=300).contains(&timeout) {
        return Err(Error::InvalidStopTimeout);
    }
    let id = SessionId::try_from(session)?;
    let mode = if force {
        StopMode::Force
    } else {
        StopMode::Graceful {
            timeout_secs: timeout,
        }
    };
    let paths = qn_config::paths::QnPaths::resolve()?;
    let runtime = runtime()?;
    let result = runtime.block_on(qn_bootstrap::stop_session(&paths, id, mode));
    runtime.shutdown_timeout(Duration::from_millis(100));
    let report = result?;
    if json {
        let payload = serde_json::json!({
            "schema_version": 1,
            "session": report.session().to_string(),
            "requested": report.requested(),
            "cleanup": "unconfirmed",
        });
        writeln!(output, "{}", serde_json::to_string(&payload)?)?;
        return Ok(());
    }
    writeln!(output, "session: {}", report.session())?;
    writeln!(output, "stop requested: {}", report.requested())?;
    writeln!(
        output,
        "cleanup: unconfirmed (native stop success is not a cleanup proof; see #93)"
    )?;
    output.flush()?;
    Ok(())
}

fn runtime() -> Result<tokio::runtime::Runtime, Error> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(Error::Runtime)
}

fn end_label(end: RunEnd) -> &'static str {
    match end {
        RunEnd::Exited => "exited",
        RunEnd::Cancelled => "cancelled",
        RunEnd::TimedOut => "timed_out",
    }
}

fn phase_label(phase: RunPhase) -> &'static str {
    match phase {
        RunPhase::Starting => "starting",
        RunPhase::Running => "running",
        RunPhase::ExitedObserved => "exited_observed",
        RunPhase::Failed => "failed",
    }
}

fn native_label(state: ObservedNativeState) -> &'static str {
    match state {
        ObservedNativeState::Running => "running",
        ObservedNativeState::Paused => "paused",
        ObservedNativeState::Exited => "exited (cleanup unconfirmed)",
    }
}
