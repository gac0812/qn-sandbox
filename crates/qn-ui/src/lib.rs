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

/// User intent emitted by a view. A daemon client may translate it into transport.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UiIntent {
    /// Load or reload one view.
    Refresh {
        /// View to refresh.
        route: ViewRoute,
    },
    /// Retry a failed or unavailable view.
    Retry {
        /// View to retry.
        route: ViewRoute,
    },
    /// Request a session stop without claiming cleanup.
    StopSession {
        /// Canonical session reference.
        session: String,
    },
    /// Request credential/session revocation through the host control plane.
    RevokeSession {
        /// Canonical session reference.
        session: String,
    },
}

/// Effect returned to the future daemon client after a UI intent is accepted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ViewEffect {
    /// Fetch a snapshot for one route.
    Fetch {
        /// View to fetch.
        route: ViewRoute,
    },
    /// Ask the control plane to stop a session.
    StopSession {
        /// Canonical session reference.
        session: String,
    },
    /// Ask the control plane to revoke a session.
    RevokeSession {
        /// Canonical session reference.
        session: String,
    },
}

/// Typed view payload used to complete one pending fetch.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ViewPayload {
    /// Profile cards.
    Profiles(Vec<ProfileCard>),
    /// Permission posture.
    Permissions(PermissionPosture),
    /// Session cards.
    Sessions(Vec<SessionCard>),
    /// Audit cards.
    Audit(Vec<AuditCard>),
}

/// Safe result sent back from a future daemon client.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ViewResult {
    /// Renderable payload.
    Ready(ViewPayload),
    /// Capability unavailable.
    Unavailable {
        /// Safe reason.
        reason: UnavailableReason,
    },
    /// Classified failure.
    Failed {
        /// Safe failure code.
        code: FailureCode,
    },
}

/// Errors raised by the pure UI state machine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UiError {
    /// An operation is already pending.
    Busy,
    /// A result arrived for another view.
    RoutePayloadMismatch,
    /// The session reference is unsafe or empty.
    InvalidSessionReference,
}

/// Mutable UI state independent of any desktop toolkit or transport.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct UiModel {
    /// Current snapshot for all views.
    pub snapshot: ControlPlaneSnapshot,
    /// Pending daemon effect, if any.
    pub pending: Option<ViewEffect>,
}

impl UiModel {
    /// Dispatches one UI intent and returns the effect for a future daemon client.
    ///
    /// # Errors
    ///
    /// Returns [`UiError::Busy`] when another effect is pending or
    /// [`UiError::InvalidSessionReference`] for unsafe action references.
    pub fn dispatch(&mut self, intent: UiIntent) -> Result<ViewEffect, UiError> {
        if self.pending.is_some() {
            return Err(UiError::Busy);
        }
        let effect = match intent {
            UiIntent::Refresh { route } | UiIntent::Retry { route } => {
                self.snapshot.set_loading(route);
                ViewEffect::Fetch { route }
            }
            UiIntent::StopSession { session } => ViewEffect::StopSession {
                session: safe_session(session)?,
            },
            UiIntent::RevokeSession { session } => ViewEffect::RevokeSession {
                session: safe_session(session)?,
            },
        };
        self.pending = Some(effect.clone());
        Ok(effect)
    }
    /// Applies a daemon response to the route requested by a pending fetch.
    ///
    /// # Errors
    ///
    /// Returns [`UiError::Busy`] without a pending fetch or
    /// [`UiError::RoutePayloadMismatch`] for a route/payload mismatch.
    pub fn complete(&mut self, route: ViewRoute, result: ViewResult) -> Result<(), UiError> {
        let Some(ViewEffect::Fetch { route: pending }) = self.pending.take() else {
            return Err(UiError::Busy);
        };
        if pending != route {
            self.pending = Some(ViewEffect::Fetch { route: pending });
            return Err(UiError::RoutePayloadMismatch);
        }
        self.snapshot.apply_result(route, result)
    }
    /// Marks a stop/revoke effect complete while preserving the snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`UiError::Busy`] when the pending effect is not an action.
    pub fn complete_action(&mut self) -> Result<(), UiError> {
        match self.pending.take() {
            Some(ViewEffect::StopSession { .. } | ViewEffect::RevokeSession { .. }) => Ok(()),
            Some(effect) => {
                self.pending = Some(effect);
                Err(UiError::Busy)
            }
            None => Err(UiError::Busy),
        }
    }
}
fn safe_session(session: String) -> Result<String, UiError> {
    if session.trim().is_empty() || session.chars().any(char::is_control) {
        return Err(UiError::InvalidSessionReference);
    }
    Ok(session)
}
impl ControlPlaneSnapshot {
    fn set_loading(&mut self, route: ViewRoute) {
        match route {
            ViewRoute::Profiles => self.profiles = ViewState::Loading,
            ViewRoute::Permissions => self.permissions = ViewState::Loading,
            ViewRoute::Sessions => self.sessions = ViewState::Loading,
            ViewRoute::Audit => self.audit = ViewState::Loading,
        }
    }
    fn apply_result(&mut self, route: ViewRoute, result: ViewResult) -> Result<(), UiError> {
        match (route, result) {
            (ViewRoute::Profiles, ViewResult::Ready(ViewPayload::Profiles(value))) => {
                self.profiles = ViewState::Ready(value);
            }
            (ViewRoute::Permissions, ViewResult::Ready(ViewPayload::Permissions(value))) => {
                self.permissions = ViewState::Ready(value);
            }
            (ViewRoute::Sessions, ViewResult::Ready(ViewPayload::Sessions(value))) => {
                self.sessions = ViewState::Ready(value);
            }
            (ViewRoute::Audit, ViewResult::Ready(ViewPayload::Audit(value))) => {
                self.audit = ViewState::Ready(value);
            }
            (route, ViewResult::Unavailable { reason }) => match route {
                ViewRoute::Profiles => self.profiles = ViewState::Unavailable { reason },
                ViewRoute::Permissions => self.permissions = ViewState::Unavailable { reason },
                ViewRoute::Sessions => self.sessions = ViewState::Unavailable { reason },
                ViewRoute::Audit => self.audit = ViewState::Unavailable { reason },
            },
            (route, ViewResult::Failed { code }) => match route {
                ViewRoute::Profiles => self.profiles = ViewState::Failed { code },
                ViewRoute::Permissions => self.permissions = ViewState::Failed { code },
                ViewRoute::Sessions => self.sessions = ViewState::Failed { code },
                ViewRoute::Audit => self.audit = ViewState::Failed { code },
            },
            _ => return Err(UiError::RoutePayloadMismatch),
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod state_machine_tests {
    use super::*;
    #[test]
    fn refresh_sets_loading_and_accepts_matching_payload() {
        let mut model = UiModel::default();
        assert_eq!(
            model
                .dispatch(UiIntent::Refresh {
                    route: ViewRoute::Profiles
                })
                .unwrap(),
            ViewEffect::Fetch {
                route: ViewRoute::Profiles
            }
        );
        assert!(matches!(model.snapshot.profiles, ViewState::Loading));
        model
            .complete(
                ViewRoute::Profiles,
                ViewResult::Ready(ViewPayload::Profiles(vec![])),
            )
            .unwrap();
        assert!(matches!(model.snapshot.profiles, ViewState::Ready(_)));
    }
    #[test]
    fn mismatched_payload_keeps_pending_and_unavailable_is_explicit() {
        let mut model = UiModel::default();
        model
            .dispatch(UiIntent::Refresh {
                route: ViewRoute::Audit,
            })
            .unwrap();
        assert_eq!(
            model.complete(
                ViewRoute::Sessions,
                ViewResult::Unavailable {
                    reason: UnavailableReason::NoEvidence
                }
            ),
            Err(UiError::RoutePayloadMismatch)
        );
        assert!(model.pending.is_some());
        model
            .complete(
                ViewRoute::Audit,
                ViewResult::Unavailable {
                    reason: UnavailableReason::NoEvidence,
                },
            )
            .unwrap();
        assert!(matches!(
            model.snapshot.audit,
            ViewState::Unavailable {
                reason: UnavailableReason::NoEvidence
            }
        ));
    }
    #[test]
    fn actions_validate_session_and_are_busy_until_completed() {
        let mut model = UiModel::default();
        assert_eq!(
            model.dispatch(UiIntent::StopSession {
                session: "bad\n".into()
            }),
            Err(UiError::InvalidSessionReference)
        );
        assert_eq!(
            model
                .dispatch(UiIntent::RevokeSession {
                    session: "sxn-test".into()
                })
                .unwrap(),
            ViewEffect::RevokeSession {
                session: "sxn-test".into()
            }
        );
        assert_eq!(
            model.dispatch(UiIntent::Refresh {
                route: ViewRoute::Audit
            }),
            Err(UiError::Busy)
        );
        model.complete_action().unwrap();
    }
}
