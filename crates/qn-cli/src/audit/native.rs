//! Native evidence is displayed separately from the broker monotonic timeline.
use crate::Error;
use qn_core::{SessionId, native_audit::NativeAuditQuery};
use std::{io::Write, time::Duration};

pub(super) fn run(id: &str, json: bool, output: &mut dyn Write) -> Result<(), Error> {
    let session = SessionId::try_from(id)?;
    let paths = qn_config::paths::QnPaths::resolve()?;
    let query = qn_bootstrap::native_audit_query(&paths)?;
    let runtime = super::runtime()?;
    let result = runtime.block_on(query.read(session));
    runtime.shutdown_timeout(Duration::from_millis(100));
    let report = result?;
    if json {
        let encoded = serde_json::to_string(&serde_json::json!({
            "schema_version": 1,
            "report": report,
        }))?;
        writeln!(output, "{encoded}")?;
    } else {
        writeln!(
            output,
            "session: {}; native record: {}",
            report.session_id, report.native_id
        )?;
        writeln!(
            output,
            "source: nono_audit; integrity: unverified; coverage: network_only"
        )?;
        writeln!(
            output,
            "exit_code: {:?}; reported native event count: {}",
            report.exit_code, report.reported_event_count
        )?;
        writeln!(
            output,
            "Filesystem denials and process-tree cleanup are not proven; free-text targets are withheld."
        )?;
        for event in report.events {
            writeln!(
                output,
                "{} wall_ms={:?} {:?}",
                event.index, event.wall_millis, event.decision
            )?;
        }
    }
    output.flush()?;
    Ok(())
}
