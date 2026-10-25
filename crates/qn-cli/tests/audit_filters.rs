//! CLI audit filter contracts over validated synthetic timelines.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use qn_cli::{AuditFilter, AuditType};
use qn_core::audit::{AuditEvent, AuditEventKind};

fn event(sequence: u64, wall_millis: u128, kind: AuditEventKind) -> AuditEvent {
    AuditEvent {
        sequence,
        wall_millis,
        elapsed_micros: u128::from(sequence),
        event: kind,
    }
}

#[test]
fn filters_preserve_sequence_and_select_event_groups() {
    let events = [
        event(1, 10, AuditEventKind::ProviderRequested),
        event(2, 20, AuditEventKind::DeniedAfterRevoke),
        event(3, 30, AuditEventKind::GrantRevoked),
        event(4, 40, AuditEventKind::ServiceClosed),
    ];
    let denial = AuditFilter {
        since: Some(15),
        event_type: Some(AuditType::Denial),
    };
    let selected: Vec<_> = events
        .iter()
        .filter(|event| denial.matches(event))
        .collect();
    assert_eq!(
        selected
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [2]
    );

    let grant = AuditFilter {
        since: None,
        event_type: Some(AuditType::Grant),
    };
    let selected: Vec<_> = events.iter().filter(|event| grant.matches(event)).collect();
    assert_eq!(
        selected
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [3]
    );
}

#[test]
fn filters_keep_all_events_when_unset() {
    let events = [
        event(1, 10, AuditEventKind::Failed),
        event(2, 20, AuditEventKind::ServiceClosed),
    ];
    let filter = AuditFilter::default();
    assert_eq!(
        events.iter().filter(|event| filter.matches(event)).count(),
        2
    );
}
