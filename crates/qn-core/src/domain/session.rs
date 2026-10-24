//! Session identity and lifecycle.

use core::fmt;

use uuid::Uuid;

use crate::error::{Error, Result};

/// Stable identifier for one restricted process session.
///
/// Values use the `sxn-` prefix followed by a UUID v7:
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

        if uuid.get_version_num() != 7 {
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

#[cfg(test)]
mod tests {
    use super::SessionId;
    use uuid::{Uuid, Version};

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
        ];

        for (case, raw) in cases {
            assert!(
                SessionId::try_from(raw).is_err(),
                "session id parser should reject {case}: {raw}"
            );
        }
    }
}
