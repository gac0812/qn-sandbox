//! Local session argv, measured against nono 0.80.0 (#46 / #52).
use std::{fmt, time::Duration};

use super::NonoArgs;
use crate::NonoError;

/// A full native nono ID, distinct from the qn session name/UUID.
///
/// The pinned CLI generates 16 lowercase hex digits (`session.rs`, v0.80.0):
/// <https://github.com/nolabs-ai/nono/blob/v0.80.0/crates/nono-cli/src/session.rs>.
/// Prefixes and names are intentionally excluded; ownership must still be
/// checked by the future runner against its qn-to-nono session mapping.
///
/// ```
/// use qn_nono::args::NonoSessionId;
/// assert!(NonoSessionId::try_from("0123456789abcdef").is_ok());
/// assert!(NonoSessionId::try_from("0123").is_err());
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct NonoSessionId(String);

impl NonoSessionId {
    /// Borrow the full native identifier for trusted host routing, never authentication.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for NonoSessionId {
    type Error = NonoError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() != 16
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(NonoError::InvalidNonoSessionId);
        }
        Ok(Self(value.into()))
    }
}

impl fmt::Debug for NonoSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NonoSessionId").finish_non_exhaustive()
    }
}

/// CLI stop options, without promising that descendant processes are gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonoStopMode {
    /// Request SIGTERM, followed by nono's SIGKILL after the grace period.
    Graceful {
        /// Whole seconds in 1..=300; avoids rounding and unbounded CLI waits.
        timeout: Duration,
    },
    /// Request nono's immediate force-stop operation.
    Force,
}

impl NonoArgs {
    /// List local sessions as JSON, retaining exited records for status lookup.
    ///
    /// ```
    /// assert_eq!(qn_nono::NonoArgs::sessions().as_slice(), ["ps", "--all", "--json"]);
    /// ```
    #[must_use]
    pub fn sessions() -> Self {
        Self(vec!["ps".into(), "--all".into(), "--json".into()])
    }

    /// Inspect a validated native ID as JSON; does not fetch events/file changes.
    #[must_use]
    pub fn inspect(id: &NonoSessionId) -> Self {
        Self(vec![
            "inspect".into(),
            "--json".into(),
            "--".into(),
            id.0.clone(),
        ])
    }

    /// List local audits as JSON, without cleanup, sync or remote operations.
    #[must_use]
    pub fn audit_list() -> Self {
        Self(vec!["audit".into(), "list".into(), "--json".into()])
    }

    /// Show a current-format audit ID as JSON, without asserting provenance.
    /// Legacy date-based audit IDs are outside the pinned runner contract.
    #[must_use]
    pub fn audit_show(id: &NonoSessionId) -> Self {
        Self(vec![
            "audit".into(),
            "show".into(),
            "--json".into(),
            "--".into(),
            id.0.clone(),
        ])
    }

    /// Build a bounded graceful or immediate force-stop invocation.
    /// The future runner must separately enforce timeout and verify cleanup.
    ///
    /// # Errors
    /// Rejects graceful timeouts outside 1..=300 whole seconds.
    ///
    /// ```
    /// use qn_nono::{NonoArgs, args::{NonoSessionId, NonoStopMode}};
    /// let id = NonoSessionId::try_from("0123456789abcdef")?;
    /// let args = NonoArgs::stop(&id, NonoStopMode::Force)?;
    /// assert_eq!(args.as_slice(), ["stop", "--force", "--", "0123456789abcdef"]);
    /// # Ok::<(), qn_nono::NonoError>(())
    /// ```
    pub fn stop(id: &NonoSessionId, mode: NonoStopMode) -> Result<Self, NonoError> {
        let mut args = vec!["stop".into()];
        match mode {
            NonoStopMode::Force => args.push("--force".into()),
            NonoStopMode::Graceful { timeout } => {
                if !(1..=300).contains(&timeout.as_secs()) || timeout.subsec_nanos() != 0 {
                    return Err(NonoError::InvalidStopTimeout);
                }
                args.extend(["--timeout".into(), timeout.as_secs().to_string()]);
            }
        }
        args.extend(["--".into(), id.0.clone()]);
        Ok(Self(args))
    }
}
