//! Audit presentation consumes only the core contract; bootstrap selects its adapter.
use crate::Error;
use clap::{Args, Subcommand, ValueEnum};
use qn_core::{
    SessionId,
    audit::{AuditEvent, AuditEventKind, AuditQuery, AuditTimeline},
};
use std::{io::Write, time::Duration};
mod follow;
mod native;

/// Closed broker event groups used only to select displayed records.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AuditType {
    /// Denied requests, including those denied after confirmed revocation.
    Denial,
    /// Observed grant creation and revocation.
    Grant,
    /// Provider requests, prepared replies and completed reply writes.
    Delivery,
    /// Recorded request, provider or transport failures.
    Failure,
    /// Observed capture service closure, not sandbox cleanup.
    Service,
}

/// Presentation selectors applied after full, unfiltered source validation.
#[derive(Clone, Debug, Default, Args)]
pub struct AuditFilter {
    /// Include wall timestamps at or after this Unix-epoch millisecond value.
    #[arg(long, value_name = "UNIX_MILLIS")]
    pub since: Option<u128>,
    /// Select one closed broker event group; preserve original sequence numbers.
    #[arg(long = "type", value_enum)]
    pub event_type: Option<AuditType>,
}
impl AuditFilter {
    fn matches(&self, event: &AuditEvent) -> bool {
        if self.since.is_some_and(|since| event.wall_millis < since) {
            return false;
        }
        let Some(kind) = self.event_type else {
            return true;
        };
        match kind {
            AuditType::Denial => matches!(event.event, AuditEventKind::Denied),
            AuditType::Grant => matches!(
                event.event,
                AuditEventKind::GrantIssued | AuditEventKind::GrantRevoked
            ),
            AuditType::Delivery => matches!(
                event.event,
                AuditEventKind::ProviderRequested
                    | AuditEventKind::DeliveryPrepared
                    | AuditEventKind::ReplyWritten
            ),
            AuditType::Failure => event.event == AuditEventKind::Failed,
            AuditType::Service => event.event == AuditEventKind::ServiceClosed,
        }
    }
}

/// Separate read-only broker/native queries; unified and cross-session views remain in #66.
#[derive(Debug, Subcommand)]
pub enum AuditCommand {
    /// Query retained native network evidence separately from broker events.
    Native {
        /// Canonical qn session identifier, never an arbitrary native ID or path.
        session: String,
        /// Emit a versioned native report with explicit network-only coverage.
        #[arg(long)]
        json: bool,
    },
    /// Show a session timeline with explicit source and integrity status.
    Show {
        /// Canonical qn session identifier.
        session: String,
        /// Emit the closed machine-readable timeline schema.
        #[arg(long)]
        json: bool,
        /// Select records without changing source validation or closure state.
        #[command(flatten)]
        filter: AuditFilter,
    },
    /// List broker records for one required session.
    List {
        /// Canonical qn session identifier; cross-session order is not inferred.
        #[arg(long)]
        session: String,
        /// Emit the closed machine-readable timeline schema.
        #[arg(long)]
        json: bool,
        /// Select records without changing source validation or closure state.
        #[command(flatten)]
        filter: AuditFilter,
    },
    /// Poll one broker journal for appended records until closure or Ctrl-C.
    Follow {
        /// Canonical qn session identifier.
        #[arg(long)]
        session: String,
        /// Emit newline-delimited timeline batches containing only new matches.
        #[arg(long)]
        json: bool,
        /// Filter displayed records; continuity still checks the entire prefix.
        #[command(flatten)]
        filter: AuditFilter,
    },
}

pub(crate) fn run(command: &AuditCommand, output: &mut dyn Write) -> Result<(), Error> {
    let (id, json, filter) = match command {
        AuditCommand::Native { session, json } => return native::run(session, *json, output),
        AuditCommand::Show {
            session,
            json,
            filter,
        }
        | AuditCommand::List {
            session,
            json,
            filter,
        }
        | AuditCommand::Follow {
            session,
            json,
            filter,
        } => (session, json, filter),
    };
    let session = SessionId::try_from(id.as_str())?;
    let paths = qn_config::paths::QnPaths::resolve()?;
    let query = qn_bootstrap::audit_query(paths.audit)?;
    if matches!(command, AuditCommand::Follow { .. }) {
        let runtime = runtime()?;
        let result = runtime.block_on(follow::stream(
            &query,
            session,
            filter,
            *json,
            output,
            tokio::signal::ctrl_c(),
        ));
        runtime.shutdown_timeout(Duration::from_millis(100));
        result
    } else {
        let mut timeline = read(&query, session)?;
        timeline.events.retain(|event| filter.matches(event));
        render(&timeline, *json, output)
    }
}

fn render(timeline: &AuditTimeline, json: bool, output: &mut dyn Write) -> Result<(), Error> {
    if json {
        crate::render_audit_json(timeline, output)?;
    } else {
        crate::render_audit_text(timeline, output)?;
    }
    output.flush()?;
    Ok(())
}

fn read(query: &dyn AuditQuery, session: SessionId) -> Result<AuditTimeline, Error> {
    let runtime = runtime()?;
    let result = runtime.block_on(query.read(session));
    // Tokio 1.54.1 Runtime shutdown cannot cancel blocking filesystem work.
    // Bound this read-only CLI wait too; the OS read may outlive its timeout.
    runtime.shutdown_timeout(Duration::from_millis(100));
    Ok(result?)
}

fn runtime() -> Result<tokio::runtime::Runtime, Error> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
