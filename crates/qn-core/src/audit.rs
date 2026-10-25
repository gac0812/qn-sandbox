//! Minimal audit query contract consumed by CLI and implemented by the broker (#110).
use crate::SessionId;
use serde::Serialize;
use std::{future::Future, pin::Pin};

/// Supported evidence sources; never infer native events from broker records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSource {
    /// Supplemental host-side broker evidence, separate from native nono audit.
    Broker,
}

/// Observed journal boundary, not a claim of authenticated completeness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalState {
    /// Existing file contains no bytes; distinct from a missing journal.
    Empty,
    /// Whole records exist without an observed closing event.
    Open,
    /// Final bytes are an unfinished line and have not been interpreted as an event.
    Partial,
    /// Last whole record says serving closed; sandbox cleanup remains unproven.
    ObservedClosed,
}

/// Cryptographic provenance has not been verified for this presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditIntegrity {
    /// Neither parse validity nor a closing record proves authenticity or absence of tampering.
    Unverified,
}

/// Closed, payload-free broker observations; intent and delivery remain distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    /// Host provider intent was recorded before execution.
    ProviderRequested,
    /// Reply was prepared, before final authorization.
    DeliveryPrepared,
    /// Reply write completed; receipt and upstream use are not proven.
    ReplyWritten,
    /// Identity or authorization was denied; no more specific cause was recorded.
    Denied,
    /// Request/provider/transport failed; precise cause was not recorded.
    Failed,
    /// Serving ended, not necessarily the sandbox execution tree.
    ServiceClosed,
}

/// One event ordered by the source sequence; wall clocks may move backwards.
#[derive(Clone, Debug, Serialize)]
pub struct AuditEvent {
    /// One-based sequence inside this single source journal.
    pub sequence: u64,
    /// Wall-clock milliseconds since Unix epoch, for display only.
    pub wall_millis: u128,
    /// Monotonic microseconds since this journal writer started, not a global clock.
    pub elapsed_micros: u128,
    /// Closed event meaning; contains no untrusted free text.
    pub event: AuditEventKind,
}

/// Bounded, read-only presentation for one exact session and source.
#[derive(Clone, Debug, Serialize)]
pub struct AuditTimeline {
    /// Original validated session, not a caller-controlled path.
    #[serde(serialize_with = "serialize_session")]
    pub session_id: SessionId,
    /// Evidence source kept visible to users.
    pub source: AuditSource,
    /// Provenance confidence; parsing never upgrades this value.
    pub integrity: AuditIntegrity,
    /// Observed boundary and incomplete-tail status.
    pub state: JournalState,
    /// Verified record prefix in source sequence order.
    pub events: Vec<AuditEvent>,
}
fn serialize_session<S: serde::Serializer>(
    id: &SessionId,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(id)
}

/// Query errors contain no record bytes or arbitrary target names.
#[derive(Debug, thiserror::Error)]
pub enum AuditQueryError {
    /// Session has no readable journal at the configured source.
    #[error("audit journal not found; verify the session and configured audit root")]
    Missing,
    /// Root/target is not a supported host journal location.
    #[error("invalid audit source; use a trusted absolute root and a regular journal file")]
    Configuration,
    /// An entire query exceeded its time budget.
    #[error("audit query timed out; inspect host storage and retry")]
    Timeout,
    /// Input exceeds the bounded byte or record budget; never return a misleading prefix.
    #[error("audit journal exceeds query limits; retain the source and inspect retention policy")]
    Limit,
    /// Record has a known structural inconsistency, described without payload values.
    #[error("audit evidence invalid: {reason}; retain the source for inspection")]
    Invalid {
        /// Static validation category, never source contents.
        reason: &'static str,
    },
    /// Parsing failed with a redacted source supplied by the adapter.
    #[error("audit record malformed; retain the source for inspection: {0}")]
    Decode(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// Handle operation failed; adapters must not include user data in IO errors.
    #[error("audit IO failed; inspect host storage: {0}")]
    Io(#[from] std::io::Error),
}

/// Owned result future permits runtime-neutral dynamic composition.
pub type AuditQueryFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AuditTimeline, AuditQueryError>> + Send + 'a>>;

/// Read one session without changing authorization, source files or process state.
pub trait AuditQuery: Send + Sync {
    /// Query a bounded source snapshot. Missing, invalid and incomplete are distinct.
    fn read(&self, session: SessionId) -> AuditQueryFuture<'_>;
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
