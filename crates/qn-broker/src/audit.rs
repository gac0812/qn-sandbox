//! Supplemental broker evidence, never a substitute for native nono audit (#105).
use qn_core::SessionId;
use serde::Serialize;
use std::{
    fmt,
    num::NonZeroU32,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
    time::timeout,
};

/// Closed events exclude paths, handles, references and all request/provider payloads.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrokerEvent {
    /// Persisted before the trusted credential command starts.
    ProviderRequested,
    /// A response is ready; final live-grant admission is still required.
    DeliveryPrepared,
    /// Encrypted reply write completed; peer receipt is not asserted.
    ReplyWritten,
    /// Identity or current authorization rejected this request.
    Denied,
    /// Transport, provider, cancellation or delivery failed.
    Failed,
    /// Owned capture serving ended; this does not prove sandbox process cleanup.
    ServiceClosed,
}

/// Journal errors never contain a pathname or peer/provider text.
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// File or deadline configuration cannot support a new journal.
    #[error(
        "invalid broker audit setup; supply an empty private regular file and a 1–30 second deadline"
    )]
    Configuration,
    /// Capacity or an earlier failed/cancelled append makes further use unsafe.
    #[error("broker audit unavailable; stop capture and provision a new private journal")]
    Unavailable,
    /// Write completion was not acknowledged within the deadline.
    #[error("broker audit timed out; stop capture, retain evidence and inspect host storage")]
    Timeout,
    /// Operating-system failure from handle-based operations (no caller text).
    #[error("broker audit IO failed; stop capture and inspect host storage: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization of closed, host-owned values failed.
    #[error("broker audit encoding failed; stop capture and inspect the broker installation")]
    Encoding(#[source] serde_json::Error),
    /// Host clock cannot represent a timestamp since the Unix epoch.
    #[error("broker audit clock invalid; correct the host clock before starting capture")]
    Clock(#[from] std::time::SystemTimeError),
}

/// An exclusively owned, bounded per-session JSON-lines journal.
///
/// Host composition must create a private file in a sandbox-inaccessible directory,
/// persist its directory entry, and retain the path for query/retention. Do not clone
/// the file handle or allow other writers. No files are opened from request metadata.
/// The journal has no rotation/recovery or cryptographic integrity claim. An uncertain
/// append permanently disables this instance, including when its future is dropped.
/// An unacknowledged record may exist on disk; it never permits privileged delivery.
///
/// # Example
///
/// The host provisions the private file before constructing this journal.
///
///     # async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     use qn_broker::audit::{BrokerEvent, BrokerJournal};
///     let private = tempfile::NamedTempFile::new()?;
///     let mut journal = BrokerJournal::new(
///         tokio::fs::File::from_std(private.reopen()?), qn_core::SessionId::new(),
///         std::num::NonZeroU32::MIN, std::time::Duration::from_secs(1),
///     ).await?;
///     journal.record(BrokerEvent::ProviderRequested).await?;
///     # Ok(()) }
pub struct BrokerJournal {
    file: File,
    session: SessionId,
    sequence: u64,
    remaining: u32,
    started: Instant,
    deadline: Duration,
    failed: bool,
}
impl fmt::Debug for BrokerJournal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerJournal")
            .field("sequence", &self.sequence)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

impl BrokerJournal {
    /// Validate a newly provisioned regular file without truncating existing evidence.
    /// # Errors
    /// Rejects nonempty/nonregular files, invalid timeout, metadata IO or timeout.
    pub async fn new(
        mut file: File,
        session: SessionId,
        capacity: NonZeroU32,
        deadline: Duration,
    ) -> Result<Self, AuditError> {
        if !(Duration::from_secs(1)..=Duration::from_secs(30)).contains(&deadline) {
            return Err(AuditError::Configuration);
        }
        let metadata = timeout(deadline, file.metadata())
            .await
            .map_err(|_| AuditError::Timeout)??;
        if !metadata.is_file() || metadata.len() != 0 {
            return Err(AuditError::Configuration);
        }
        timeout(deadline, file.seek(std::io::SeekFrom::Start(0)))
            .await
            .map_err(|_| AuditError::Timeout)??;
        Ok(Self {
            file,
            session,
            sequence: 0,
            remaining: capacity.get(),
            started: Instant::now(),
            deadline,
            failed: false,
        })
    }

    /// Session owning every record; serving composition must match this identity.
    #[must_use]
    pub fn session(&self) -> SessionId {
        self.session
    }

    /// Persist one event before allowing its corresponding privileged operation.
    /// Serial ownership replaces a lock; never call this while holding a grant lock.
    /// # Errors
    /// All failures permanently disable this journal. Dropping a pending append also
    /// disables it; callers must cancel credential work and close the serving lifecycle.
    pub async fn record(&mut self, event: BrokerEvent) -> Result<(), AuditError> {
        if self.failed || self.remaining == 0 {
            self.failed = true;
            return Err(AuditError::Unavailable);
        }
        self.failed = true;
        let record = Record {
            source: "broker",
            session_id: self.session.to_string(),
            sequence: self.sequence + 1,
            wall_millis: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
            elapsed_micros: self.started.elapsed().as_micros(),
            event,
        };
        let mut bytes = serde_json::to_vec(&record).map_err(AuditError::Encoding)?;
        bytes.push(b'\n');
        timeout(self.deadline, async {
            self.file.write_all(&bytes).await?;
            // Tokio File buffers writes in a blocking task. Flush surfaces its error
            // before sync_data; sync alone must not hide a failed buffered write.
            self.file.flush().await?;
            self.file.sync_data().await
        })
        .await
        .map_err(|_| AuditError::Timeout)??;
        self.sequence += 1;
        self.remaining -= 1;
        self.failed = false;
        Ok(())
    }
}

#[derive(Serialize)]
struct Record {
    source: &'static str,
    session_id: String,
    sequence: u64,
    wall_millis: u128,
    elapsed_micros: u128,
    event: BrokerEvent,
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
