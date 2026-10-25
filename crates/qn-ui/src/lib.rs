//! Pure control-plane view contracts for the future qn Tauri shell.
//!
//! This crate deliberately performs no filesystem, network, credential, or
//! adapter work. A later daemon client will populate these safe presentation
//! values; the Tauri layer will render them.

use serde::{Deserialize, Serialize};

/// Stable top-level view identifiers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewRoute {
    /// Official profile catalog and selection state.
    Profiles,
    /// Credential/permission posture visible to the user.
    Permissions,
    /// Foreground session status and lifecycle actions.
    Sessions,
    /// Retained audit evidence and integrity posture.
    Audit,
}

impl ViewRoute {
    /// Returns the stable route label used by navigation and telemetry.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Profiles => "profiles",
            Self::Permissions => "permissions",
            Self::Sessions => "sessions",
            Self::Audit => "audit",
        }
    }
}

/// Loading state for one view's daemon-backed data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewState<T> {
    /// The view has not received its first snapshot.
    Loading,
    /// The view has a safe, renderable snapshot.
    Ready(T),
    /// The capability is not available in the current product phase.
    Unavailable {
        /// Safe reason the view cannot provide data.
        reason: UnavailableReason,
    },
    /// The daemon returned a classified failure without raw source text.
    Failed {
        /// Classified failure code without raw source text.
        code: FailureCode,
    },
}

/// Safe explanation for a capability that is not wired yet.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The daemon/API contract is not connected to this view yet.
    ControlPlanePending,
    /// The host platform cannot provide the capability.
    PlatformUnsupported,
    /// The current session has no evidence for this view.
    NoEvidence,
}

/// Classified failure codes safe for user-facing rendering.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    /// The daemon could not be reached.
    ControlPlaneUnavailable,
    /// The daemon returned data that did not match the contract.
    InvalidResponse,
    /// The request was refused by policy.
    PolicyDenied,
}

/// Safe profile summary for the Profiles view.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProfileCard {
    /// Canonical profile identity.
    pub id: String,
    /// Verified profile version.
    pub version: String,
    /// Whether this is a compiled or host-installed profile.
    pub source: String,
}

/// Safe permission posture for the Permissions view.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PermissionPosture {
    /// The host broker mediates credentials and the agent receives no values.
    HostMediated,
    /// No credential grant is currently available.
    Unavailable,
}

/// Safe session card for the Sessions view.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionCard {
    /// Canonical qn session identifier.
    pub session: String,
    /// Profile identity used by the session.
    pub profile: String,
    /// Owner-observed lifecycle phase.
    pub phase: String,
    /// Cleanup remains explicitly unconfirmed until the execution-tree proof lands.
    pub cleanup: String,
}

/// Safe audit summary for the Audit view.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AuditCard {
    /// Session whose evidence is displayed.
    pub session: String,
    /// Evidence source label.
    pub source: String,
    /// Integrity/provenance posture.
    pub integrity: String,
    /// Observed journal boundary.
    pub state: String,
    /// Number of validated events in the snapshot.
    pub event_count: usize,
}

/// Stable snapshot shape that a future daemon client can populate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ControlPlaneSnapshot {
    /// Profile view state.
    pub profiles: ViewState<Vec<ProfileCard>>,
    /// Permission view state.
    pub permissions: ViewState<PermissionPosture>,
    /// Session view state.
    pub sessions: ViewState<Vec<SessionCard>>,
    /// Audit view state.
    pub audit: ViewState<Vec<AuditCard>>,
}

impl Default for ControlPlaneSnapshot {
    fn default() -> Self {
        Self {
            profiles: ViewState::Unavailable {
                reason: UnavailableReason::ControlPlanePending,
            },
            permissions: ViewState::Unavailable {
                reason: UnavailableReason::ControlPlanePending,
            },
            sessions: ViewState::Unavailable {
                reason: UnavailableReason::ControlPlanePending,
            },
            audit: ViewState::Unavailable {
                reason: UnavailableReason::ControlPlanePending,
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn routes_have_stable_labels() {
        assert_eq!(ViewRoute::Profiles.label(), "profiles");
        assert_eq!(ViewRoute::Permissions.label(), "permissions");
        assert_eq!(ViewRoute::Sessions.label(), "sessions");
        assert_eq!(ViewRoute::Audit.label(), "audit");
    }

    #[test]
    fn default_snapshot_is_explicitly_unavailable() {
        let snapshot = ControlPlaneSnapshot::default();
        assert!(matches!(
            snapshot.permissions,
            ViewState::Unavailable {
                reason: UnavailableReason::ControlPlanePending
            }
        ));
        assert!(matches!(
            snapshot.audit,
            ViewState::Unavailable {
                reason: UnavailableReason::ControlPlanePending
            }
        ));
    }

    #[test]
    fn snapshot_round_trips_without_raw_error_text() {
        let snapshot = ControlPlaneSnapshot {
            profiles: ViewState::Ready(vec![ProfileCard {
                id: "qn/claude-code".into(),
                version: "1.0.0".into(),
                source: "builtin".into(),
            }]),
            permissions: ViewState::Unavailable {
                reason: UnavailableReason::NoEvidence,
            },
            sessions: ViewState::Ready(vec![SessionCard {
                session: "sxn-test".into(),
                profile: "qn/claude-code".into(),
                phase: "running".into(),
                cleanup: "unconfirmed".into(),
            }]),
            audit: ViewState::Failed {
                code: FailureCode::ControlPlaneUnavailable,
            },
        };
        let encoded = serde_json::to_string(&snapshot).unwrap();
        let decoded: ControlPlaneSnapshot = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, snapshot);
        assert!(!encoded.contains("secret"));
        assert!(!encoded.contains("/private/"));
    }
}
