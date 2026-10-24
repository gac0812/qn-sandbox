//! Core error types.

use thiserror::Error;

use crate::domain::session::SessionState;

/// Errors produced by core domain rules.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    /// A session identifier is not a canonical UUID v7 with the `sxn-` prefix.
    #[error(
        "invalid session id; expected `sxn-` followed by a lowercase UUID v7 with an RFC 4122-compatible variant"
    )]
    InvalidSessionId,

    /// A requested session state change is not part of the lifecycle contract.
    #[error(
        "invalid session state transition from {from} to {to}; check the current session status before retrying"
    )]
    InvalidSessionStateTransition {
        /// State before the rejected transition.
        from: SessionState,
        /// Requested state.
        to: SessionState,
    },
}

/// Result type used by core domain rules.
pub type Result<T> = core::result::Result<T, Error>;
