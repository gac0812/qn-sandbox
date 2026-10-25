//! Owned process lifecycle. Group/job mechanisms come from process-wrap (#65).
use super::{
    CmdOutcome, CmdSpec, DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_TIMEOUT, ReaderResult, read_bounded,
};
use crate::Error;
use process_wrap::tokio::{KillOnDrop, TokioChildWrapper, TokioCommandWrap};
use std::{
    fmt,
    process::{ExitStatus, Stdio},
    time::{Duration, Instant},
};
use tokio::{process::ChildStdin, task::JoinHandle};
use tokio_util::sync::CancellationToken;

const CLEANUP_TIMEOUT: Duration = Duration::from_secs(3);

/// Whether signalling remains safe after the parent has been reaped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminationSafety {
    /// A numeric group ID is only used while the parent identity remains owned.
    WhileParentOwned,
    /// The policy retains a kernel object handle independent of PID reuse.
    RetainedGroupHandle,
}

/// Platform assembly hook; implementations install the group/job wrapper.
/// Production callers use the policy supplied by qn-bootstrap.
pub trait ProcessPolicy: Send + Sync {
    /// Configure owned-group semantics and declare the resulting identity safety.
    fn configure(&self, command: &mut TokioCommandWrap) -> TerminationSafety;
}

/// Why an owned process finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessEnd {
    /// The command exited without a termination request.
    Exited,
    /// The cancellation token requested termination.
    Cancelled,
    /// The command's time limit requested termination.
    TimedOut,
}

/// Captured result whose Debug omits payloads; explicit access is for parsers.
pub struct ManagedOutput {
    /// Completion reason, separate from the command's exit code.
    pub end: ProcessEnd,
    outcome: CmdOutcome,
}
impl ManagedOutput {
    /// Access bounded output explicitly; do not log credential-bearing payloads.
    #[must_use]
    pub fn output(&self) -> &CmdOutcome {
        &self.outcome
    }
}
impl fmt::Debug for ManagedOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedOutput")
            .field("end", &self.end)
            .field("exit_code", &self.outcome.exit_code)
            .finish_non_exhaustive()
    }
}

/// Owns the child and capture tasks. Explicit wait/termination reaps the child.
/// Drop attempts safe termination and aborts readers; it cannot await cleanup.
pub struct ManagedProcess {
    child: Box<dyn TokioChildWrapper>,
    stdout: JoinHandle<ReaderResult>,
    stderr: JoinHandle<ReaderResult>,
    started: Instant,
    timeout: Duration,
    safety: TerminationSafety,
}
impl fmt::Debug for ManagedProcess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedProcess")
            .field("id", &self.child.id())
            .finish_non_exhaustive()
    }
}

/// Spawn within a Tokio runtime using an explicitly supplied group/job policy.
/// Arguments, environment and captured output are never logged here.
/// # Errors
/// Returns a typed spawn failure. See qn-bootstrap for a runnable composition example.
pub fn spawn_managed(spec: &CmdSpec, policy: &dyn ProcessPolicy) -> Result<ManagedProcess, Error> {
    let mut command = spec.command();
    match spec.stdio_mode() {
        super::StdioMode::Captured => {
            command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
        }
        super::StdioMode::Inherited => {
            command
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }
    }
    let mut wrapped = TokioCommandWrap::from(command);
    wrapped.wrap(KillOnDrop);
    let safety = policy.configure(&mut wrapped);
    let mut child = wrapped
        .spawn()
        .map_err(|source| io_error("spawn", source))?;
    let max = spec.max_output_bytes.unwrap_or(DEFAULT_MAX_OUTPUT_BYTES);
    let stdout = tokio::spawn(read_bounded(child.stdout().take(), max));
    let stderr = tokio::spawn(read_bounded(child.stderr().take(), max));
    tracing::info!(pid = child.id(), "managed process spawned");
    Ok(ManagedProcess {
        child,
        stdout,
        stderr,
        started: Instant::now(),
        timeout: spec.timeout.unwrap_or(DEFAULT_TIMEOUT),
        safety,
    })
}

impl ManagedProcess {
    /// Managed parent PID while it remains owned; `None` after reaping.
    ///
    /// A numeric PID is an observation for session tracking, never a signal
    /// identity on its own. Callers pair it with the process start time.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Take the stdin pipe. The caller must close it when input is complete.
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin().take()
    }

    /// Wait with cancellation, closing retained stdin; cancel the token to terminate.
    /// Dropping the waiter still owns cleanup via Drop.
    /// # Errors
    /// Cleanup uncertainty, I/O failure and drain deadlines are typed errors.
    pub async fn wait(mut self, cancel: CancellationToken) -> Result<ManagedOutput, Error> {
        drop(self.child.stdin().take());
        let remaining = self.timeout.saturating_sub(self.started.elapsed());
        let (status, end) = tokio::select! {
            biased;
            () = cancel.cancelled() => (self.kill_and_wait().await?, ProcessEnd::Cancelled),
            () = tokio::time::sleep(remaining) => (self.kill_and_wait().await?, ProcessEnd::TimedOut),
            status = Box::into_pin(self.child.wait()) => (status.map_err(|e| io_error("wait", e))?, ProcessEnd::Exited),
        };
        self.finish(status, end, cancel).await
    }

    async fn kill_and_wait(&mut self) -> Result<ExitStatus, Error> {
        if self.child.id().is_none() && self.safety == TerminationSafety::WhileParentOwned {
            return Err(Error::CleanupUnconfirmed);
        }
        self.child
            .start_kill()
            .map_err(|e| io_error("terminate", e))?;
        tokio::time::timeout(CLEANUP_TIMEOUT, Box::into_pin(self.child.wait()))
            .await
            .map_err(|_| Error::ManagedDeadline { operation: "reap" })?
            .map_err(|e| io_error("reap", e))
    }

    async fn finish(
        &mut self,
        status: ExitStatus,
        end: ProcessEnd,
        cancel: CancellationToken,
    ) -> Result<ManagedOutput, Error> {
        let limit = if end == ProcessEnd::Exited {
            self.timeout.saturating_sub(self.started.elapsed())
        } else {
            CLEANUP_TIMEOUT
        };
        let readers = async { tokio::join!(&mut self.stdout, &mut self.stderr) };
        let (stdout, stderr) = tokio::select! {
            result = tokio::time::timeout(limit, readers) => result.map_err(|_| Error::ManagedDeadline { operation: "output drain" })?,
            () = cancel.cancelled(), if end == ProcessEnd::Exited => return Err(Error::CleanupUnconfirmed),
        };
        let (stdout, stdout_truncated) = stdout.unwrap_or((Vec::new(), true));
        let (stderr, stderr_truncated) = stderr.unwrap_or((Vec::new(), true));
        tracing::info!(?end, exit_code = status.code(), "managed process finished");
        Ok(ManagedOutput {
            end,
            outcome: CmdOutcome {
                exit_code: status.code(),
                success: status.success(),
                stdout: String::from_utf8_lossy(&stdout).into_owned(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
                stdout_raw: stdout,
                stderr_raw: stderr,
                stdout_truncated,
                stderr_truncated,
                duration: self.started.elapsed(),
            },
        })
    }
}
impl Drop for ManagedProcess {
    fn drop(&mut self) {
        if self.child.id().is_some() || self.safety == TerminationSafety::RetainedGroupHandle {
            let _ = self.child.start_kill();
        }
        self.stdout.abort();
        self.stderr.abort();
    }
}
fn io_error(operation: &'static str, source: std::io::Error) -> Error {
    Error::ManagedIo { operation, source }
}
