//! Core error types.

use thiserror::Error;

/// Errors produced by core domain rules.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    /// A session identifier does not match `sxn-` followed by a UUID v7.
    #[error("invalid session id; expected `sxn-` followed by a UUID v7")]
    InvalidSessionId,
}

/// Result type used by core domain rules.
pub type Result<T> = core::result::Result<T, Error>;
