#![allow(clippy::unwrap_used)]
use super::*;

#[test]
fn it_serializes_the_session_and_evidence_qualifiers_without_source_payloads() {
    let session = SessionId::new();
    let timeline = AuditTimeline {
        session_id: session,
        source: AuditSource::Broker,
        integrity: AuditIntegrity::Unverified,
        state: JournalState::Partial,
        events: vec![AuditEvent {
            sequence: 1,
            wall_millis: 2,
            elapsed_micros: 3,
            event: AuditEventKind::DeliveryPrepared,
        }],
    };
    let value = serde_json::to_value(timeline).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "session_id":session.to_string(),"source":"broker","integrity":"unverified",
            "state":"partial","events":[{"sequence":1,"wall_millis":2,"elapsed_micros":3,
                "event":"delivery_prepared"}]
        })
    );
    assert_eq!(
        SessionId::try_from(value["session_id"].as_str().unwrap()).unwrap(),
        session
    );
}

#[test]
fn it_keeps_missing_empty_open_partial_and_observed_closed_distinct() {
    let labels: Vec<_> = [
        JournalState::Empty,
        JournalState::Open,
        JournalState::Partial,
        JournalState::ObservedClosed,
    ]
    .into_iter()
    .map(|state| serde_json::to_value(state).unwrap())
    .collect();
    assert_eq!(labels, vec!["empty", "open", "partial", "observed_closed"]);
    assert!(AuditQueryError::Missing.to_string().contains("not found"));
    assert_ne!(
        AuditEventKind::DeliveryPrepared,
        AuditEventKind::ReplyWritten
    );
    assert_ne!(AuditEventKind::ServiceClosed, AuditEventKind::ReplyWritten);
}
