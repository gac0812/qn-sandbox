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

/// Stable examples for UI smoke tests and desktop preview fixtures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreviewPreset {
    /// All views contain safe representative data.
    Ready,
    /// Permission and audit views are unavailable while profiles/sessions are ready.
    Degraded,
    /// All views expose classified control-plane failure.
    Failed,
}

impl PreviewPreset {
    /// Builds a deterministic snapshot without I/O or credentials.
    #[must_use]
    pub fn snapshot(self) -> ControlPlaneSnapshot {
        let profiles = vec![ProfileCard {
            id: "qn/claude-code".into(),
            version: "1.0.0".into(),
            source: "builtin".into(),
        }];
        let sessions = vec![SessionCard {
            session: "sxn-preview".into(),
            profile: "qn/claude-code".into(),
            phase: "running".into(),
            cleanup: "unconfirmed".into(),
        }];
        let audit = vec![AuditCard {
            session: "sxn-preview".into(),
            source: "broker".into(),
            integrity: "unverified".into(),
            state: "partial".into(),
            event_count: 1,
        }];
        match self {
            Self::Ready => ControlPlaneSnapshot {
                profiles: ViewState::Ready(profiles),
                permissions: ViewState::Ready(PermissionPosture::HostMediated),
                sessions: ViewState::Ready(sessions),
                audit: ViewState::Ready(audit),
            },
            Self::Degraded => ControlPlaneSnapshot {
                profiles: ViewState::Ready(profiles),
                permissions: ViewState::Unavailable {
                    reason: UnavailableReason::NoEvidence,
                },
                sessions: ViewState::Ready(sessions),
                audit: ViewState::Unavailable {
                    reason: UnavailableReason::NoEvidence,
                },
            },
            Self::Failed => ControlPlaneSnapshot {
                profiles: ViewState::Failed {
                    code: FailureCode::ControlPlaneUnavailable,
                },
                permissions: ViewState::Failed {
                    code: FailureCode::ControlPlaneUnavailable,
                },
                sessions: ViewState::Failed {
                    code: FailureCode::ControlPlaneUnavailable,
                },
                audit: ViewState::Failed {
                    code: FailureCode::ControlPlaneUnavailable,
                },
            },
        }
    }
}

/// Errors raised while decoding a UI snapshot envelope.
#[derive(Debug, thiserror::Error)]
pub enum UiEnvelopeError {
    /// The JSON document is malformed or violates the DTO shape.
    #[error("invalid UI snapshot JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The envelope schema is newer than this UI understands.
    #[error("unsupported UI snapshot schema version: {0}")]
    UnsupportedSchema(u8),
}

/// Versioned envelope exchanged between a future daemon client and the UI shell.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UiSnapshotEnvelope {
    /// Wire format version for forward-compatible desktop clients.
    pub schema_version: u8,
    /// Safe four-view snapshot.
    pub snapshot: ControlPlaneSnapshot,
}

impl UiSnapshotEnvelope {
    /// Current envelope schema version.
    pub const SCHEMA_VERSION: u8 = 1;

    /// Wraps a snapshot in the current schema version.
    #[must_use]
    pub fn new(snapshot: ControlPlaneSnapshot) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            snapshot,
        }
    }

    /// Serializes this envelope for a future daemon/UI transport.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the contract changes in an incompatible way.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parses and checks a versioned UI envelope.
    ///
    /// # Errors
    ///
    /// Returns a serialization error for malformed JSON or an unsupported schema version.
    pub fn from_json(text: &str) -> Result<Self, UiEnvelopeError> {
        let envelope: Self = serde_json::from_str(text)?;
        if envelope.schema_version != Self::SCHEMA_VERSION {
            return Err(UiEnvelopeError::UnsupportedSchema(envelope.schema_version));
        }
        Ok(envelope)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod envelope_tests {
    use super::*;

    #[test]
    fn envelope_round_trips_preview_states_with_schema_version() {
        for preset in [
            PreviewPreset::Ready,
            PreviewPreset::Degraded,
            PreviewPreset::Failed,
        ] {
            let original = UiSnapshotEnvelope::new(preset.snapshot());
            let encoded = original.to_json().unwrap();
            let decoded = UiSnapshotEnvelope::from_json(&encoded).unwrap();
            assert_eq!(decoded, original);
            assert_eq!(decoded.schema_version, 1);
        }
    }

    #[test]
    fn unsupported_schema_is_rejected_before_ui_consumption() {
        let mut encoded = UiSnapshotEnvelope::new(PreviewPreset::Ready.snapshot())
            .to_json()
            .unwrap();
        encoded = encoded.replacen("\"schema_version\":1", "\"schema_version\":99", 1);
        let error = UiSnapshotEnvelope::from_json(&encoded)
            .expect_err("future schema must not be silently consumed");
        assert!(
            error
                .to_string()
                .contains("unsupported UI snapshot schema version")
        );
    }
}

/// Navigation and refresh state for the desktop shell.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UiNavigation {
    /// Currently selected view route.
    pub current: ViewRoute,
    /// Whether the selected view is waiting for a refresh response.
    pub refreshing: bool,
}

impl Default for UiNavigation {
    fn default() -> Self {
        Self {
            current: ViewRoute::Profiles,
            refreshing: false,
        }
    }
}

impl UiNavigation {
    /// Selects a view without starting I/O.
    #[must_use]
    pub fn select(&self, route: ViewRoute) -> Self {
        Self {
            current: route,
            refreshing: false,
        }
    }

    /// Marks the selected view as waiting for a daemon snapshot.
    #[must_use]
    pub fn begin_refresh(&self) -> Self {
        Self {
            current: self.current,
            refreshing: true,
        }
    }

    /// Marks a refresh response as received.
    #[must_use]
    pub fn finish_refresh(&self, route: ViewRoute) -> Self {
        Self {
            current: route,
            refreshing: false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod navigation_tests {
    use super::*;

    #[test]
    fn navigation_is_deterministic_and_serializable() {
        let navigation = UiNavigation::default()
            .select(ViewRoute::Sessions)
            .begin_refresh()
            .finish_refresh(ViewRoute::Sessions);
        assert_eq!(navigation.current, ViewRoute::Sessions);
        assert!(!navigation.refreshing);
        let encoded = serde_json::to_string(&navigation).unwrap();
        let decoded: UiNavigation = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, navigation);
    }
}

/// Client boundary for the future daemon-backed desktop shell.
pub trait ControlPlaneClient {
    /// Fetches one route snapshot.
    ///
    /// # Errors
    ///
    /// Returns a classified UI error without raw transport details.
    fn fetch(&self, route: ViewRoute) -> Result<ViewResult, UiError>;
    /// Executes a stop effect.
    ///
    /// # Errors
    ///
    /// Returns a classified UI error without raw transport details.
    fn stop_session(&self, session: &str) -> Result<(), UiError>;
    /// Executes a revoke effect.
    ///
    /// # Errors
    ///
    /// Returns a classified UI error without raw transport details.
    fn revoke_session(&self, session: &str) -> Result<(), UiError>;
}

/// Applies one client effect to the pure UI model.
///
/// # Errors
///
/// Returns the client or model error without exposing transport details.
pub fn apply_effect_result<C: ControlPlaneClient>(
    model: &mut UiModel,
    client: &C,
    effect: ViewEffect,
) -> Result<(), UiError> {
    match effect {
        ViewEffect::Fetch { route } => model.complete(route, client.fetch(route)?),
        ViewEffect::StopSession { session } => {
            client.stop_session(&session)?;
            model.complete_action()
        }
        ViewEffect::RevokeSession { session } => {
            client.revoke_session(&session)?;
            model.complete_action()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod client_tests {
    use super::*;
    struct FakeClient;
    impl ControlPlaneClient for FakeClient {
        fn fetch(&self, route: ViewRoute) -> Result<ViewResult, UiError> {
            Ok(match route {
                ViewRoute::Profiles => ViewResult::Ready(ViewPayload::Profiles(Vec::new())),
                ViewRoute::Permissions => ViewResult::Unavailable {
                    reason: UnavailableReason::NoEvidence,
                },
                ViewRoute::Sessions => ViewResult::Ready(ViewPayload::Sessions(Vec::new())),
                ViewRoute::Audit => ViewResult::Failed {
                    code: FailureCode::ControlPlaneUnavailable,
                },
            })
        }
        fn stop_session(&self, _: &str) -> Result<(), UiError> {
            Ok(())
        }
        fn revoke_session(&self, _: &str) -> Result<(), UiError> {
            Ok(())
        }
    }
    #[test]
    fn client_effects_complete_fetch_and_actions_without_transport_knowledge() {
        let client = FakeClient;
        let mut model = UiModel::default();
        let fetch = model
            .dispatch(UiIntent::Refresh {
                route: ViewRoute::Profiles,
            })
            .unwrap();
        apply_effect_result(&mut model, &client, fetch).unwrap();
        assert!(matches!(model.snapshot.profiles, ViewState::Ready(_)));
        let revoke = model
            .dispatch(UiIntent::RevokeSession {
                session: "sxn-test".into(),
            })
            .unwrap();
        apply_effect_result(&mut model, &client, revoke).unwrap();
        assert!(model.pending.is_none());
    }
}
