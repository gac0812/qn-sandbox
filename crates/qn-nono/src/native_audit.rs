//! Retained launch correlation and safe native audit presentation (#121).
use crate::{NonoError, audit::AuditReader, setup::NonoSetup, wire};
use camino::{Utf8Path, Utf8PathBuf};
use qn_core::{
    SessionId,
    audit::AuditIntegrity,
    native_audit::{
        NativeAuditCoverage, NativeAuditError, NativeAuditFuture, NativeAuditQuery,
        NativeAuditReport, NativeAuditSource, NativeNetworkEvent, NetworkDecision,
    },
    setup::{EngineSetup, SetupOperation},
};
use qn_infra::process::ProcessPolicy;
use std::{fmt, sync::Arc, time::Duration};
pub(crate) mod association;

/// Native queries resolve only retained qn identities and verify the installed engine.
pub struct ManagedNativeAudit {
    audit_root: Utf8PathBuf,
    setup: NonoSetup,
    policy: Arc<dyn ProcessPolicy>,
}
impl ManagedNativeAudit {
    /// Bind trusted host paths; neither paths nor process policy may come from the sandbox.
    /// # Errors
    /// Rejects invalid roots and hosts needing a separate WSL installation route.
    pub fn new(
        audit_root: Utf8PathBuf,
        cache: &Utf8Path,
        policy: Arc<dyn ProcessPolicy>,
    ) -> Result<Self, NativeAuditError> {
        if !audit_root.is_absolute() || audit_root.as_str().contains(char::from(0)) {
            return Err(NativeAuditError::InvalidAssociation);
        }
        Ok(Self {
            audit_root,
            setup: NonoSetup::new(cache)?,
            policy,
        })
    }
}
impl fmt::Debug for ManagedNativeAudit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedNativeAudit").finish_non_exhaustive()
    }
}
impl NativeAuditQuery for ManagedNativeAudit {
    fn read(&self, session: SessionId) -> NativeAuditFuture<'_> {
        Box::pin(async move {
            let root = self.audit_root.clone();
            let worker = tokio::task::spawn_blocking(move || association::load(&root, session));
            let retained = tokio::time::timeout(Duration::from_secs(5), worker)
                .await
                .map_err(|_| NativeAuditError::Timeout)?
                .map_err(|source| NativeAuditError::Failed(Box::new(QueryWorkerCause(source))))??;
            let installation = self.setup.run(SetupOperation::Inspect).await?;
            let reader = AuditReader::new(installation.path, retained.root, self.policy.clone())
                .map_err(failure)?;
            let record = reader.show(&retained.native).await.map_err(failure)?;
            project(session, record)
        })
    }
}
fn project(
    session: SessionId,
    record: wire::AuditRecord,
) -> Result<NativeAuditReport, NativeAuditError> {
    crate::args::NonoSessionId::try_from(record.session_id.as_str()).map_err(failure)?;
    let events = record
        .network_events
        .into_iter()
        .enumerate()
        .map(|(index, event)| {
            // Pinned vocabulary: nono v0.80.0, crates/nono/src/undo/types.rs,
            // NetworkAuditDecision. Approval observations are not allow/deny results.
            let decision = match event.decision.as_str() {
                "allow" => NetworkDecision::Allowed,
                "deny" => NetworkDecision::Denied,
                "approve_requested" => NetworkDecision::ApprovalRequested,
                "approve_granted" => NetworkDecision::ApprovalGranted,
                "approve_denied" => NetworkDecision::ApprovalDenied,
                "approve_timeout" => NetworkDecision::ApprovalTimeout,
                "approve_error" => NetworkDecision::ApprovalError,
                _ => {
                    return Err(failure(NonoError::Protocol {
                        operation: "native network audit",
                        reason: "unknown decision",
                    }));
                }
            };
            Ok(NativeNetworkEvent {
                index: index as u64 + 1,
                wall_millis: event.timestamp_unix_ms,
                decision,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NativeAuditReport {
        session_id: session,
        native_id: record.session_id,
        source: NativeAuditSource::NonoAudit,
        integrity: AuditIntegrity::Unverified,
        coverage: NativeAuditCoverage::NetworkOnly,
        reported_event_count: record.audit_event_count,
        exit_code: record.exit_code,
        events,
    })
}
struct QueryWorkerCause(tokio::task::JoinError);
impl fmt::Display for QueryWorkerCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_cancelled() {
            "audit association reader cancelled"
        } else {
            "audit association reader did not complete"
        })
    }
}
impl fmt::Debug for QueryWorkerCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for QueryWorkerCause {}

struct QueryCause(NonoError);
impl fmt::Display for QueryCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match &self.0 {
            NonoError::Io { .. } => "managed audit storage unavailable",
            NonoError::Protocol { .. }
            | NonoError::ProtocolJson { .. }
            | NonoError::InvalidNonoSessionId => "native audit identity or schema rejected",
            NonoError::CommandTimedOut { .. } => "native audit command timed out",
            _ => "managed native audit command failed",
        })
    }
}
impl fmt::Debug for QueryCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for QueryCause {}
fn failure(source: NonoError) -> NativeAuditError {
    NativeAuditError::Failed(Box::new(QueryCause(source)))
}

#[cfg(test)]
mod tests;
