#![allow(clippy::unwrap_used)]
use super::*;
use qn_core::audit::{AuditEventKind, AuditIntegrity, AuditQueryFuture, AuditSource};
use std::sync::atomic::{AtomicUsize, Ordering};

fn snapshot(session_id: SessionId, state: JournalState, kinds: &[AuditEventKind]) -> AuditTimeline {
    AuditTimeline {
        session_id,
        source: AuditSource::Broker,
        integrity: AuditIntegrity::Unverified,
        state,
        events: kinds
            .iter()
            .enumerate()
            .map(|(i, event)| AuditEvent {
                sequence: i as u64 + 1,
                wall_millis: 100 - i as u128,
                elapsed_micros: i as u128,
                event: *event,
            })
            .collect(),
    }
}

#[test]
fn it_emits_only_new_matches_and_preserves_full_source_state() {
    use AuditEventKind::{Denied, GrantIssued, ServiceClosed};
    let session = SessionId::new();
    let filter = AuditFilter {
        since: None,
        event_type: Some(crate::AuditType::Denial),
    };
    let mut cursor = Cursor::default();
    let initial = snapshot(session, JournalState::Partial, &[GrantIssued]);
    let first = cursor.observe(initial.clone(), &filter).unwrap().unwrap();
    assert!(first.events.is_empty());
    assert_eq!(first.state, JournalState::Partial);
    assert!(cursor.observe(initial, &filter).unwrap().is_none());
    let completed = snapshot(session, JournalState::Open, &[GrantIssued, Denied]);
    let batch = cursor.observe(completed.clone(), &filter).unwrap().unwrap();
    assert_eq!(batch.events.len(), 1);
    assert_eq!(batch.events[0].sequence, 2);
    assert!(cursor.observe(completed, &filter).unwrap().is_none());
    let appended = snapshot(session, JournalState::Open, &[GrantIssued, Denied, Denied]);
    assert_eq!(
        cursor.observe(appended, &filter).unwrap().unwrap().events[0].sequence,
        3
    );
    let closed = snapshot(
        session,
        JournalState::ObservedClosed,
        &[GrantIssued, Denied, Denied, ServiceClosed],
    );
    let batch = cursor.observe(closed, &filter).unwrap().unwrap();
    assert!(batch.events.is_empty());
    assert_eq!(batch.state, JournalState::ObservedClosed);
    assert!(cursor.is_closed());
}

#[test]
fn it_rejects_truncation_and_changes_to_every_previously_observed_field() {
    let original = snapshot(
        SessionId::new(),
        JournalState::Open,
        &[AuditEventKind::GrantIssued],
    );
    let filter = AuditFilter {
        since: Some(1000),
        event_type: None,
    };
    for change in 0..6 {
        let mut cursor = Cursor::default();
        cursor.observe(original.clone(), &filter).unwrap();
        let mut changed = original.clone();
        match change {
            0 => changed.events.clear(),
            1 => changed.events[0].sequence += 1,
            2 => changed.events[0].wall_millis += 1,
            3 => changed.events[0].elapsed_micros += 1,
            4 => changed.events[0].event = AuditEventKind::Denied,
            _ => changed.session_id = SessionId::new(),
        }
        assert!(matches!(
            cursor.observe(changed, &filter),
            Err(Error::AuditChanged)
        ));
    }
}

struct Query {
    reads: AtomicUsize,
    pending: bool,
}
impl AuditQuery for Query {
    fn read(&self, session: SessionId) -> AuditQueryFuture<'_> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.pending {
                std::future::pending::<()>().await;
            }
            Ok(snapshot(session, JournalState::Open, &[]))
        })
    }
}

#[test]
fn it_cancels_pending_reads_and_timer_waits_without_spawning_tasks() {
    for pending in [true, false] {
        let query = Query {
            reads: AtomicUsize::new(0),
            pending,
        };
        let runtime = super::super::runtime().unwrap();
        runtime.block_on(async {
            let (send, receive) = tokio::sync::oneshot::channel();
            let filter = AuditFilter::default();
            let mut output = Vec::new();
            let follow = stream(
                &query,
                SessionId::new(),
                &filter,
                true,
                &mut output,
                async {
                    receive.await.unwrap();
                    Ok(())
                },
            );
            tokio::pin!(follow);
            std::future::poll_fn(|cx| {
                assert!(follow.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            assert_eq!(query.reads.load(Ordering::SeqCst), 1);
            send.send(()).unwrap();
            follow.await.unwrap();
            assert_eq!(query.reads.load(Ordering::SeqCst), 1);
        });
    }
}

#[test]
fn it_returns_signal_failure_before_reading_or_emitting_a_snapshot() {
    let query = Query {
        reads: AtomicUsize::new(0),
        pending: false,
    };
    let runtime = super::super::runtime().unwrap();
    let mut output = Vec::new();
    let result = runtime.block_on(stream(
        &query,
        SessionId::new(),
        &AuditFilter::default(),
        true,
        &mut output,
        async { Err(std::io::Error::other("signal setup failed")) },
    ));
    assert!(matches!(result, Err(Error::Signal(_))));
    assert!(output.is_empty());
    assert_eq!(query.reads.load(Ordering::SeqCst), 0);
}

struct BrokenOutput;
impl Write for BrokenOutput {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn it_stops_reading_when_the_output_consumer_closes() {
    let query = Query {
        reads: AtomicUsize::new(0),
        pending: false,
    };
    let runtime = super::super::runtime().unwrap();
    let result = runtime.block_on(stream(
        &query,
        SessionId::new(),
        &AuditFilter::default(),
        true,
        &mut BrokenOutput,
        std::future::pending(),
    ));
    assert!(
        matches!(result, Err(Error::Output(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
    );
    assert_eq!(query.reads.load(Ordering::SeqCst), 1);
}
