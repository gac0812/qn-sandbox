//! Strict projection of broker-owned evidence, never authentication (#110).
use crate::audit::BrokerEvent;
use camino::Utf8PathBuf;
use qn_core::{
    SessionId,
    audit::{
        AuditEvent, AuditEventKind, AuditIntegrity, AuditQuery, AuditQueryError, AuditQueryFuture,
        AuditSource, AuditTimeline, JournalState,
    },
};
use serde::Deserialize;
use std::{fmt, time::Duration};
use tokio::{fs::File, io::AsyncReadExt, time::timeout};

const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RECORDS: usize = 10_000;
const MAX_LINE: usize = 1024;

/// Reads only validated session filenames under a trusted host audit directory.
/// Host composition keeps the directory inaccessible to the sandbox and untrusted writers.
/// Files must be regular and immutable except for their exclusive journal append owner.
pub struct BrokerAuditQuery {
    root: Utf8PathBuf,
}
impl BrokerAuditQuery {
    /// Bind an absolute host root; no filesystem IO or directory creation is performed.
    /// # Errors
    /// Rejects relative roots or NUL bytes.
    pub fn new(root: Utf8PathBuf) -> Result<Self, AuditQueryError> {
        if !root.is_absolute() || root.as_str().contains('\0') {
            return Err(AuditQueryError::Configuration);
        }
        Ok(Self { root })
    }
    async fn snapshot(&self, session: SessionId) -> Result<AuditTimeline, AuditQueryError> {
        let path = self.root.join(format!("{session}.broker.jsonl"));
        let metadata = tokio::fs::symlink_metadata(&path).await.map_err(io_error)?;
        if !metadata.is_file() {
            return Err(AuditQueryError::Configuration);
        }
        if metadata.len() > MAX_BYTES {
            return Err(AuditQueryError::Limit);
        }
        let file = File::open(path).await.map_err(io_error)?;
        if !file.metadata().await?.is_file() {
            return Err(AuditQueryError::Configuration);
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes).await?;
        parse(&bytes, session)
    }
}
impl AuditQuery for BrokerAuditQuery {
    fn read(&self, session: SessionId) -> AuditQueryFuture<'_> {
        Box::pin(async move {
            timeout(Duration::from_secs(5), self.snapshot(session))
                .await
                .map_err(|_| AuditQueryError::Timeout)?
        })
    }
}
impl fmt::Debug for BrokerAuditQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrokerAuditQuery(..)")
    }
}
fn io_error(source: std::io::Error) -> AuditQueryError {
    if source.kind() == std::io::ErrorKind::NotFound {
        AuditQueryError::Missing
    } else {
        AuditQueryError::Io(source)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Source {
    Broker,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRecord {
    source: Source,
    session_id: String,
    sequence: u64,
    wall_millis: u128,
    elapsed_micros: u128,
    event: BrokerEvent,
}
struct DecodeError(serde_json::Error);
impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} at line {} column {}",
            self.0.classify(),
            self.0.line(),
            self.0.column()
        )
    }
}
impl fmt::Debug for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for DecodeError {}

fn parse(bytes: &[u8], session: SessionId) -> Result<AuditTimeline, AuditQueryError> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(AuditQueryError::Limit);
    }
    let mut result = AuditTimeline {
        session_id: session,
        source: AuditSource::Broker,
        integrity: AuditIntegrity::Unverified,
        state: JournalState::Empty,
        events: Vec::new(),
    };
    let mut previous_elapsed = 0;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        if line.len() > MAX_LINE || result.events.len() == MAX_RECORDS {
            return Err(AuditQueryError::Limit);
        }
        if result.state == JournalState::ObservedClosed {
            return Err(invalid("data after closure"));
        }
        if !line.ends_with(b"\n") {
            result.state = JournalState::Partial;
            break;
        }
        let row: WireRecord = serde_json::from_slice(line)
            .map_err(|error| AuditQueryError::Decode(Box::new(DecodeError(error))))?;
        let Source::Broker = row.source;
        if SessionId::try_from(row.session_id.as_str()).ok() != Some(session) {
            return Err(invalid("session mismatch"));
        }
        if row.sequence != result.events.len() as u64 + 1 || row.elapsed_micros < previous_elapsed {
            return Err(invalid("noncontiguous source order"));
        }
        previous_elapsed = row.elapsed_micros;
        result.state = if row.event == BrokerEvent::ServiceClosed {
            JournalState::ObservedClosed
        } else {
            JournalState::Open
        };
        result.events.push(AuditEvent {
            sequence: row.sequence,
            wall_millis: row.wall_millis,
            elapsed_micros: row.elapsed_micros,
            event: project(row.event),
        });
    }
    Ok(result)
}
fn invalid(reason: &'static str) -> AuditQueryError {
    AuditQueryError::Invalid { reason }
}
fn project(event: BrokerEvent) -> AuditEventKind {
    match event {
        BrokerEvent::GrantIssued => AuditEventKind::GrantIssued,
        BrokerEvent::GrantRevoked => AuditEventKind::GrantRevoked,
        BrokerEvent::ProviderRequested => AuditEventKind::ProviderRequested,
        BrokerEvent::DeliveryPrepared => AuditEventKind::DeliveryPrepared,
        BrokerEvent::ReplyWritten => AuditEventKind::ReplyWritten,
        BrokerEvent::Denied => AuditEventKind::Denied,
        BrokerEvent::DeniedAfterRevoke => AuditEventKind::DeniedAfterRevoke,
        BrokerEvent::Failed => AuditEventKind::Failed,
        BrokerEvent::ServiceClosed => AuditEventKind::ServiceClosed,
    }
}

#[cfg(test)]
#[path = "audit_query_tests.rs"]
mod tests;
