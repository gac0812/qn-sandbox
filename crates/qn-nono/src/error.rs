//! Typed failures of the nono installer and version probe.
//!
//! Every variant carries an actionable message; the source chain is kept
//! where an underlying cause exists (AGENTS §4.4).

use std::io;

/// Failures while installing, probing, or parsing the locked `nono` release.
#[derive(Debug, thiserror::Error)]
pub enum NonoError {
    /// A native session identifier is not a full pinned-format ID.
    #[error(
        "invalid nono session ID; use a full 16-character lowercase hex ID from local session records"
    )]
    InvalidNonoSessionId,

    /// A graceful stop cannot be represented as a bounded whole-second wait.
    #[error("invalid nono stop timeout; use 1 to 300 whole seconds")]
    InvalidStopTimeout,

    /// Bounded output could not satisfy the pinned protocol contract.
    #[error("invalid nono {operation} response: {reason}; check the pinned binary")]
    Protocol {
        /// Static operation name, never command arguments.
        operation: &'static str,
        /// Static rejection category, never raw output.
        reason: &'static str,
    },
    /// JSON source is retained behind a redacted error boundary.
    #[error("invalid nono {operation} JSON; check the pinned binary: {source}")]
    ProtocolJson {
        /// Static operation name.
        operation: &'static str,
        /// Parser cause with input values removed from all formatting.
        #[source]
        source: ProtocolJsonError,
    },
    /// An invocation argument is invalid; its value is never echoed.
    #[error(
        "invalid nono {field}; use absolute UTF-8 paths and a nonempty command without NUL bytes"
    )]
    InvalidArgument {
        /// Static field name identifying the invalid input.
        field: &'static str,
    },

    /// A relative executable path could resolve through the user's PATH.
    #[error("nono executable path must be absolute; use the managed installation path")]
    RelativeExecutable,

    /// The process did not exit successfully; untrusted output is not echoed.
    #[error("nono version probe failed with exit code {code:?}; check the managed installation")]
    VersionProbeFailed {
        /// Exit code, or None when terminated by a signal.
        code: Option<i32>,
    },

    /// The version response was incomplete or did not follow the CLI contract.
    #[error("invalid or incomplete nono version response; reinstall the pinned binary")]
    InvalidVersionOutput,

    /// The binary reports a different version from the workspace pin.
    #[error("nono version does not match {expected}; reinstall the pinned binary")]
    VersionMismatch {
        /// The version required by qn-config.
        expected: &'static str,
    },

    /// The shared runner failed to spawn, wait for, or finish the probe.
    #[error("nono version probe could not complete; check the managed installation: {source}")]
    Process {
        /// Original process error, including timeout and OS failures.
        #[from]
        source: qn_infra::Error,
    },

    /// The host platform (or explicit target) has no artifact in the locked
    /// manifest. Upstream ships no Windows build; WSL2 installs use the Linux
    /// artifact via `qn-wsl`.
    #[error(
        "no locked nono {version} artifact for target {target}; supported targets: {supported}"
    )]
    UnsupportedTarget {
        /// Target that could not be resolved.
        target: String,
        /// Locked nono version the manifest covers.
        version: &'static str,
        /// Comma-separated targets that do have artifacts.
        supported: String,
    },

    /// Release download failed: network, GitHub unavailability, or a missing
    /// upstream asset for the locked version.
    ///
    /// The upstream error is boxed: `ureq::Error` is far larger than the rest
    /// of this enum, and a compact `Result::Err` keeps every installer
    /// function return size reasonable.
    #[error(
        "failed to download {url}; check network access to github.com and that \
         release {version} still exists upstream"
    )]
    DownloadFailed {
        /// Full URL that could not be fetched.
        url: String,
        /// Locked nono version that was requested.
        version: &'static str,
        /// Underlying HTTP failure.
        #[source]
        source: Box<ureq::Error>,
    },

    /// The cached or downloaded archive does not match the manifest digest. The
    /// artifact is refused: never install an unverifiable binary.
    #[error(
        "nono archive checksum mismatch: expected {expected}, got {actual}; \
         refusing to install a tampered or corrupted artifact"
    )]
    ChecksumMismatch {
        /// Digest recorded in the locked manifest.
        expected: &'static str,
        /// Digest computed over the downloaded archive.
        actual: String,
    },

    /// The archive no longer contains the expected root `nono` binary —
    /// upstream archive layout drifted from what the installer locks.
    #[error(
        "release archive does not contain the expected `nono` binary; \
         upstream archive layout changed, regenerate the locked manifest (issue #29)"
    )]
    ArchiveMissingBinary,

    /// Filesystem failure during a named installer step.
    #[error("nono install failed while {step}: {source}")]
    Io {
        /// Short description of the failing step.
        step: &'static str,
        /// Underlying OS error.
        #[source]
        source: io::Error,
    },
}

/// Compatibility name for existing installer callers.
pub type InstallerError = NonoError;

/// Retains the parser cause without exposing payloads in Display, Debug or chains.
/// serde data errors may quote untrusted values, so the inner cause is private.
pub struct ProtocolJsonError(pub(crate) serde_json::Error);
impl std::fmt::Display for ProtocolJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} at line {} column {}",
            self.0.classify(),
            self.0.line(),
            self.0.column()
        )
    }
}
impl std::fmt::Debug for ProtocolJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for ProtocolJsonError {}
