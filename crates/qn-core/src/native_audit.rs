//! Native network evidence keeps its own source ordering and clock semantics (#121).
use crate::{SessionId, audit::AuditIntegrity, setup::SetupError};
use serde::Serialize;
use std::{future::Future, pin::Pin};

/// Provenance label, not a cryptographic verification result.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NativeAuditSource {
    /// Events returned by the managed native engine's audit query.
    NonoAudit,
}

/// Explicit limit on which events this report contains.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NativeAuditCoverage {
    /// Network observations only; no filesystem denials or lifecycle completeness.
    NetworkOnly,
}

/// Recorded native decision, never inferred from child exit/output.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDecision {
    /// The native network rule allowed the operation.
    Allowed,
    /// The native network rule denied the operation.
    Denied,
    /// An approval backend was asked; no final authorization is implied.
    ApprovalRequested,
    /// The approval backend granted the request.
    ApprovalGranted,
    /// The approval backend or user denied the request.
    ApprovalDenied,
    /// The approval request timed out.
    ApprovalTimeout,
    /// The approval backend failed.
    ApprovalError,
}

/// Payload-free network event with source-local ordering.
#[derive(Clone, Debug, Serialize)]
pub struct NativeNetworkEvent {
    /// One-based position in the native network event array, not global sequence.
    pub index: u64,
    /// Optional native Unix wall-clock milliseconds; not synthesized when missing.
    pub wall_millis: Option<u64>,
    /// Validated native decision. Targets, paths and raw reasons are withheld.
    pub decision: NetworkDecision,
}

/// One exact native record projected without credential-bearing free text.
#[derive(Clone, Debug, Serialize)]
pub struct NativeAuditReport {
    /// Original qn association, validated before querying native state.
    #[serde(serialize_with = "serialize_session")]
    pub session_id: SessionId,
    /// Full native record pointer; adapters must validate its closed grammar.
    pub native_id: String,
    /// Native source kept separate from broker evidence.
    pub source: NativeAuditSource,
    /// Parsing or digest presence alone never upgrades provenance.
    pub integrity: AuditIntegrity,
    /// The report does not include filesystem-denial details.
    pub coverage: NativeAuditCoverage,
    /// Upstream total across its event types; may exceed network events shown here.
    pub reported_event_count: u64,
    /// Native recorded child result, not execution-tree cleanup proof.
    pub exit_code: Option<i32>,
    /// Native network array order; wall clocks may regress or be missing.
    pub events: Vec<NativeNetworkEvent>,
}
fn serialize_session<S: serde::Serializer>(
    id: &SessionId,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(id)
}

/// Failures preserve operational categories without exposing mapping or record bytes.
#[derive(Debug, thiserror::Error)]
pub enum NativeAuditError {
    /// No retained association exists for this exact qn session.
    #[error("native audit association missing; retain launch evidence and check the qn session")]
    MissingAssociation,
    /// Association is malformed, mismatched or outside supported host storage.
    #[error("native audit association invalid; retain the evidence and inspect managed storage")]
    InvalidAssociation,
    /// Host storage could not finish a bounded association read.
    #[error("native audit association read timed out; inspect host storage and retry")]
    Timeout,
    /// The managed engine could not pass its existing integrity check.
    #[error(transparent)]
    Setup(#[from] SetupError),
    /// Underlying native-query or association IO cause with redacted formatting.
    #[error("native audit query failed: {0}; retain the source for inspection")]
    Failed(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Runtime-neutral query future; this operation never restores process authority.
pub type NativeAuditFuture<'a> =
    Pin<Box<dyn Future<Output = Result<NativeAuditReport, NativeAuditError>> + Send + 'a>>;

/// Read retained native evidence through an exact launch-owned qn association.
pub trait NativeAuditQuery: Send + Sync {
    /// Check installation integrity and return the bounded, redacted native report.
    fn read(&self, session: SessionId) -> NativeAuditFuture<'_>;
}
