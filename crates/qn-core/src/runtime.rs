//! Owned foreground session observations and cleanup semantics (#93).
//!
//! No type in this module can represent a confirmed cleanup. Until the
//! execution-tree proof exists, every run/stop report carries
//! `CleanupState::Unconfirmed` and never maps to a core `Stopped` state.

use crate::profile::ProfileError;
use crate::{ProfileRef, SessionId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Coarse phase recorded by the owning runner.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    /// Preparation or native startup has not completed.
    Starting,
    /// The managed parent is alive.
    Running,
    /// The managed parent exited; descendants may still be alive.
    ExitedObserved,
    /// The runner or native engine failed before a usable session existed.
    Failed,
}

/// Why the managed parent finished.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunEnd {
    /// The command exited without a termination request.
    Exited,
    /// The owner requested termination.
    Cancelled,
    /// The configured limit requested termination.
    TimedOut,
}

/// Deliberately has no `Confirmed` variant until #93 supplies a proof.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupState {
    /// Descendant termination is not proven; do not report `Stopped`.
    Unconfirmed,
}

/// Native observation, separate from the owning runner's phase.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedNativeState {
    /// Native record reports a running process.
    Running,
    /// Native record reports a paused process.
    Paused,
    /// Native record reports exit, without a descendant-cleanup assertion.
    Exited,
}

/// Stop request shape; native termination is not a cleanup proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopMode {
    /// Request graceful termination with a whole-second grace period.
    Graceful {
        /// Whole seconds in 1..=300.
        timeout_secs: u64,
    },
    /// Request the native immediate force-stop operation.
    Force,
}

/// Outcome of one owned foreground run.
#[derive(Clone)]
pub struct RunReport {
    session: SessionId,
    profile: ProfileRef,
    native_id: String,
    phase: RunPhase,
    end: RunEnd,
    exit_code: Option<i32>,
    cleanup: CleanupState,
}

impl RunReport {
    /// Build an exit observation; cleanup is always unconfirmed here.
    #[must_use]
    pub fn new(
        session: SessionId,
        profile: ProfileRef,
        native_id: String,
        phase: RunPhase,
        end: RunEnd,
        exit_code: Option<i32>,
    ) -> Self {
        Self {
            session,
            profile,
            native_id,
            phase,
            end,
            exit_code,
            cleanup: CleanupState::Unconfirmed,
        }
    }

    /// qn session identity.
    #[must_use]
    pub fn session(&self) -> SessionId {
        self.session
    }

    /// Requested managed profile.
    #[must_use]
    pub fn profile(&self) -> &ProfileRef {
        &self.profile
    }

    /// Predetermined native identity used by the launch binding.
    #[must_use]
    pub fn native_id(&self) -> &str {
        &self.native_id
    }

    /// Owner-observed phase.
    #[must_use]
    pub fn phase(&self) -> RunPhase {
        self.phase
    }

    /// Completion reason for the managed parent.
    #[must_use]
    pub fn end(&self) -> RunEnd {
        self.end
    }

    /// Managed-parent exit code; does not describe descendants.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Always `CleanupState::Unconfirmed` in this release.
    #[must_use]
    pub fn cleanup(&self) -> CleanupState {
        self.cleanup
    }
}

impl fmt::Debug for RunReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunReport")
            .field("session", &self.session)
            .field("phase", &self.phase)
            .field("end", &self.end)
            .field("exit_code", &self.exit_code)
            .field("cleanup", &self.cleanup)
            .finish_non_exhaustive()
    }
}

/// Persisted or live status for one qn session.
#[derive(Clone)]
pub struct SessionStatus {
    session: SessionId,
    profile: ProfileRef,
    phase: RunPhase,
    end: Option<RunEnd>,
    exit_code: Option<i32>,
    cleanup: CleanupState,
    native: Option<ObservedNativeState>,
}

impl SessionStatus {
    /// Build a status from the record plus an optional native observation.
    #[must_use]
    pub fn new(
        session: SessionId,
        profile: ProfileRef,
        phase: RunPhase,
        end: Option<RunEnd>,
        exit_code: Option<i32>,
        native: Option<ObservedNativeState>,
    ) -> Self {
        Self {
            session,
            profile,
            phase,
            end,
            exit_code,
            cleanup: CleanupState::Unconfirmed,
            native,
        }
    }

    /// qn session identity.
    #[must_use]
    pub fn session(&self) -> SessionId {
        self.session
    }

    /// Requested managed profile.
    #[must_use]
    pub fn profile(&self) -> &ProfileRef {
        &self.profile
    }

    /// Last owner-recorded phase.
    #[must_use]
    pub fn phase(&self) -> RunPhase {
        self.phase
    }

    /// Recorded completion reason, when the owner recorded an exit.
    #[must_use]
    pub fn end(&self) -> Option<RunEnd> {
        self.end
    }

    /// Recorded managed-parent exit code.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Always `CleanupState::Unconfirmed` in this release.
    #[must_use]
    pub fn cleanup(&self) -> CleanupState {
        self.cleanup
    }

    /// Live native observation, when the engine and record were reachable.
    #[must_use]
    pub fn native(&self) -> Option<ObservedNativeState> {
        self.native
    }
}

impl fmt::Debug for SessionStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionStatus")
            .field("session", &self.session)
            .field("phase", &self.phase)
            .field("end", &self.end)
            .field("exit_code", &self.exit_code)
            .field("cleanup", &self.cleanup)
            .field("native", &self.native)
            .finish_non_exhaustive()
    }
}

/// Result of asking the native engine to stop a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopReport {
    session: SessionId,
    requested: bool,
    cleanup: CleanupState,
}

impl StopReport {
    /// Record that a stop was requested; cleanup stays unconfirmed.
    #[must_use]
    pub fn new(session: SessionId, requested: bool) -> Self {
        Self {
            session,
            requested,
            cleanup: CleanupState::Unconfirmed,
        }
    }

    /// Session the request targeted.
    #[must_use]
    pub fn session(&self) -> SessionId {
        self.session
    }

    /// Whether the native stop request completed successfully.
    #[must_use]
    pub fn requested(&self) -> bool {
        self.requested
    }

    /// Always `CleanupState::Unconfirmed` in this release.
    #[must_use]
    pub fn cleanup(&self) -> CleanupState {
        self.cleanup
    }
}

/// Session runner failures; no variant carries a command payload or credential.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// Managed profile resolution or publication failed.
    #[error(transparent)]
    Profile(#[from] ProfileError),
    /// The managed engine is missing or failed its integrity/version inspection.
    #[error("managed engine is unavailable; run 'qn install' first")]
    Engine(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// No record exists for this qn session.
    #[error("session is unknown; check 'qn status <session>' and retain the session ID")]
    UnknownSession,
    /// The owner already observed an exit; cleanup remains unconfirmed.
    #[error("session is already observed exited; execution-tree cleanup remains unconfirmed")]
    AlreadyExited,
    /// The launch request is structurally inconsistent.
    #[error("invalid session request; check the profile and command")]
    InvalidRequest,
    /// This host has no native foreground runner route.
    #[error("foreground sessions are unsupported on this host; Windows requires the WSL2 route")]
    Unsupported,
    /// Session record storage failed.
    #[error("session storage failed; check qn home permissions and retry")]
    Storage(#[source] std::io::Error),
    /// Native process or adapter failure with a redacted public message.
    #[error("session process failed; check the managed state and retain the session ID")]
    Process(#[source] Box<dyn std::error::Error + Send + Sync>),
}
