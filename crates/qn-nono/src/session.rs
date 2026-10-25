//! Native identity correlation for the owned runner; names are not authentication (#89).
use crate::{NonoArgs, NonoError, args::NonoSessionId, audit::AuditReader, wire};
use camino::Utf8PathBuf;
use qn_core::SessionId;
use qn_infra::process::ProcessPolicy;
use std::{fmt, sync::Arc};

/// Native observation only; Exited does not prove cleanup or a core Stopped state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSessionState {
    /// The native record reports a running process.
    Running,
    /// The native record reports a paused process.
    Paused,
    /// The native record reports exit, with no descendant-cleanup assertion.
    Exited,
}

/// Immutable qn-to-native correlation verified by list and inspect in a private root.
/// The runner must retain its process handle; this record grants no authority to stop it.
#[derive(Clone)]
pub struct NativeSession {
    session: SessionId,
    native_id: NonoSessionId,
    state: NativeSessionState,
    exit_code: Option<i32>,
}
impl NativeSession {
    /// Original qn identity, exactly matched against the native name.
    #[must_use]
    pub fn session(&self) -> SessionId {
        self.session
    }
    /// Full native ID for the same root's audit reader; never a prefix or caller name.
    #[must_use]
    pub fn native_id(&self) -> &NonoSessionId {
        &self.native_id
    }
    /// Native observation; the core lifecycle remains owned by the runner.
    #[must_use]
    pub fn state(&self) -> NativeSessionState {
        self.state
    }
    /// Native exit code; absent does not imply success or failure.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }
}
impl fmt::Debug for NativeSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeSession")
            .field("state", &self.state)
            .field("exit_code", &self.exit_code)
            .finish_non_exhaustive()
    }
}

/// Reads identities from a private host-owned runtime root using bounded queries.
/// Caller must verify the binary checksum and keep root inaccessible to the sandbox.
/// A fresh root and an owned launch are prerequisites for treating a match as owned.
/// No process is launched/stopped and no core lifecycle transition is inferred.
///
/// ```no_run
/// # async fn example(policy: std::sync::Arc<dyn qn_infra::process::ProcessPolicy>) -> Result<(), qn_nono::NonoError> {
/// use qn_nono::session::SessionReader;
/// let reader = SessionReader::new("/opt/qn/nono".into(), "/opt/qn/runtime".into(), policy)?;
/// if let Some(observed) = reader.find(qn_core::SessionId::new()).await? {
///     let refreshed = reader.inspect(&observed).await?;
///     assert_eq!(observed.session(), refreshed.session());
/// }
/// # Ok(()) }
/// ```
#[derive(Clone, Debug)]
pub struct SessionReader {
    query: AuditReader,
}
impl SessionReader {
    /// Bind an isolated root and process policy; performs no filesystem or process IO.
    /// # Errors
    /// Rejects nonabsolute paths and NUL bytes before any query is executed.
    pub fn new(
        binary: Utf8PathBuf,
        root: Utf8PathBuf,
        policy: Arc<dyn ProcessPolicy>,
    ) -> Result<Self, NonoError> {
        Ok(Self {
            query: AuditReader::new(binary, root, policy)?,
        })
    }
    /// Resolve a unique exact qn name and re-inspect its full native identity.
    /// No exact match returns None; callers must not infer process exit from absence.
    /// # Errors
    /// Refuses ambiguous list records, malformed IDs, failed/truncated queries and drift.
    pub async fn find(&self, session: SessionId) -> Result<Option<NativeSession>, NonoError> {
        let output = self.query.query(NonoArgs::sessions(), "ps").await?;
        let Some(found) = select(&wire::sessions(output.output())?, session)? else {
            return Ok(None);
        };
        self.inspect(&found).await.map(Some)
    }
    /// Refresh an immutable observation within the same trusted runtime root.
    /// # Errors
    /// Rejects mismatched qn/native IDs and any failed or invalid native response.
    pub async fn inspect(&self, expected: &NativeSession) -> Result<NativeSession, NonoError> {
        let output = self
            .query
            .query(NonoArgs::inspect(&expected.native_id), "inspect")
            .await?;
        verify(&wire::inspect(output.output())?, expected)
    }
}
fn mismatch(reason: &'static str) -> NonoError {
    NonoError::Protocol {
        operation: "session identity",
        reason,
    }
}
fn select(
    records: &[wire::SessionRecord],
    session: SessionId,
) -> Result<Option<NativeSession>, NonoError> {
    let name = session.to_string();
    let mut matches = records
        .iter()
        .filter(|record| record.name.as_deref() == Some(&name));
    let Some(record) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some()
        || records
            .iter()
            .filter(|r| r.session_id == record.session_id)
            .count()
            != 1
    {
        return Err(mismatch("ambiguous native record"));
    }
    observation(record, session).map(Some)
}
fn observation(
    record: &wire::SessionRecord,
    session: SessionId,
) -> Result<NativeSession, NonoError> {
    let native_id = NonoSessionId::try_from(record.session_id.as_str())?;
    let state = match record.status {
        wire::Status::Running => NativeSessionState::Running,
        wire::Status::Paused => NativeSessionState::Paused,
        wire::Status::Exited => NativeSessionState::Exited,
    };
    if state != NativeSessionState::Exited && record.exit_code.is_some() {
        return Err(mismatch("active record has an exit code"));
    }
    Ok(NativeSession {
        session,
        native_id,
        state,
        exit_code: record.exit_code,
    })
}
fn verify(
    record: &wire::SessionRecord,
    expected: &NativeSession,
) -> Result<NativeSession, NonoError> {
    if record.name.as_deref() != Some(&expected.session.to_string()) {
        return Err(mismatch("qn identity changed"));
    }
    let actual = observation(record, expected.session)?;
    if actual.native_id != expected.native_id {
        return Err(mismatch("native identity changed"));
    }
    Ok(actual)
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
