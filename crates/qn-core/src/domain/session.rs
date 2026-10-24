//! Session identity and lifecycle.

use core::fmt;

use uuid::{Uuid, Variant};

use crate::error::{Error, Result};

/// Stable identifier for one restricted process session.
///
/// Values use the `sxn-` prefix followed by a canonical lowercase UUID v7
/// with an RFC 4122-compatible variant:
///
/// ```
/// let id = qn_core::SessionId::new();
/// assert!(id.to_string().starts_with("sxn-"));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Creates a session identifier with a time-ordered UUID v7.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<&str> for SessionId {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        let raw = value.strip_prefix("sxn-").ok_or(Error::InvalidSessionId)?;
        let uuid = Uuid::parse_str(raw).map_err(|_| Error::InvalidSessionId)?;

        if uuid.get_version_num() != 7
            || uuid.get_variant() != Variant::RFC4122
            || uuid.to_string() != raw
        {
            return Err(Error::InvalidSessionId);
        }

        Ok(Self(uuid))
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "sxn-{}", self.0)
    }
}

/// Lifecycle state for one restricted process session.
///
/// Valid transitions are exhaustive in the `transition` table. Repeated
/// transitions to the current state are successful no-ops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// The session is being prepared and its restrictions are not active yet.
    Starting,
    /// The session is running under the selected sandbox profile.
    Running,
    /// The session ended normally and cannot restart.
    Stopped,
    /// The session failed and cannot restart.
    Failed,
}

impl SessionState {
    /// Applies the authoritative session state transition table.
    ///
    /// Repeating the current state is an idempotent no-op. Terminal states
    /// (`Stopped` and `Failed`) cannot transition to any other state.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidSessionStateTransition`] when the requested
    /// transition is not allowed.
    pub fn transition(&self, next: Self) -> Result<Self> {
        let allowed = matches!(
            (*self, next),
            (
                Self::Starting,
                Self::Starting | Self::Running | Self::Stopped | Self::Failed
            ) | (Self::Running, Self::Running | Self::Stopped | Self::Failed)
                | (Self::Stopped, Self::Stopped)
                | (Self::Failed, Self::Failed)
        );

        if allowed {
            Ok(next)
        } else {
            Err(Error::InvalidSessionStateTransition {
                from: *self,
                to: next,
            })
        }
    }
}

impl fmt::Display for SessionState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        };
        formatter.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, SessionId, SessionState};
    use uuid::{Uuid, Variant, Version};

    const STATES: [SessionState; 4] = [
        SessionState::Starting,
        SessionState::Running,
        SessionState::Stopped,
        SessionState::Failed,
    ];

    #[test]
    fn it_generates_a_prefixed_uuid_v7() {
        let id = SessionId::new();
        let text = id.to_string();
        let Some(raw) = text.strip_prefix("sxn-") else {
            panic!("generated session id {id} should use the sxn- prefix");
        };
        let Ok(uuid) = Uuid::parse_str(raw) else {
            panic!("generated session id {id} should contain a UUID");
        };

        assert_eq!(
            uuid.get_version(),
            Some(Version::SortRand),
            "generated session id {id} should use UUID v7"
        );
        assert_eq!(
            uuid.get_variant(),
            Variant::RFC4122,
            "generated session id {id} should use the RFC 4122-compatible variant"
        );
        assert_eq!(
            uuid.to_string(),
            raw,
            "generated session id {id} should use canonical lowercase UUID text"
        );
    }

    #[test]
    fn it_round_trips_its_text_representation() {
        let id = SessionId::new();
        let text = id.to_string();

        let Ok(parsed) = SessionId::try_from(text.as_str()) else {
            panic!("generated session id {id} should parse again");
        };

        assert_eq!(parsed, id, "session id {id} should round trip through text");
    }

    #[test]
    fn it_rejects_invalid_session_ids() {
        let cases = [
            ("missing prefix", "0190f2f8-2b0a-7f5e-9a3c-4b5d6e7f8091"),
            ("invalid uuid", "sxn-not-a-uuid"),
            ("uuid v4", "sxn-0190f2f8-2b0a-4f5e-9a3c-4b5d6e7f8091"),
            (
                "non-RFC variant",
                "sxn-0190f2f8-2b0a-7f5e-4a3c-4b5d6e7f8091",
            ),
            ("unhyphenated uuid", "sxn-0190f2f82b0a7f5e9a3c4b5d6e7f8091"),
            ("braced uuid", "sxn-{0190f2f8-2b0a-7f5e-9a3c-4b5d6e7f8091}"),
            (
                "urn uuid",
                "sxn-urn:uuid:0190f2f8-2b0a-7f5e-9a3c-4b5d6e7f8091",
            ),
            ("uppercase uuid", "sxn-0190F2F8-2B0A-7F5E-9A3C-4B5D6E7F8091"),
        ];

        for (case, raw) in cases {
            assert_eq!(
                SessionId::try_from(raw),
                Err(Error::InvalidSessionId),
                "session id parser should reject {case} with InvalidSessionId: {raw}"
            );
        }
    }

    #[test]
    fn it_allows_only_the_authoritative_state_transitions() {
        let allowed = [
            [true, true, true, true],
            [false, true, true, true],
            [false, false, true, false],
            [false, false, false, true],
        ];

        for (from_index, from) in STATES.iter().copied().enumerate() {
            for (to_index, to) in STATES.iter().copied().enumerate() {
                let result = from.transition(to);

                assert_eq!(
                    result.is_ok(),
                    allowed[from_index][to_index],
                    "unexpected transition result from {from:?} to {to:?}: {result:?}"
                );
                if allowed[from_index][to_index] {
                    assert_eq!(
                        result,
                        Ok(to),
                        "allowed transition from {from:?} to {to:?} should return the requested state"
                    );
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(Error::InvalidSessionStateTransition {
                                from: actual_from,
                                to: actual_to,
                            }) if actual_from == from && actual_to == to
                        ),
                        "rejected transition from {from:?} to {to:?} should report both states"
                    );
                }
            }
        }
    }
}
