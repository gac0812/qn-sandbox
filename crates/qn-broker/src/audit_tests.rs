// AGENTS.md permits unwrap in tests only.
#![allow(clippy::unwrap_used)]
use super::*;
use std::{
    future::{Future, poll_fn},
    task::Poll,
};

async fn journal(file: std::fs::File, count: u32, session: SessionId) -> BrokerJournal {
    BrokerJournal::new(
        File::from_std(file),
        session,
        NonZeroU32::new(count).unwrap(),
        Duration::from_secs(1),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn it_persists_closed_records_in_order_and_refuses_exhaustion() {
    let private = tempfile::NamedTempFile::new().unwrap();
    let session = SessionId::new();
    let mut file = private.reopen().unwrap();
    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(32)).unwrap();
    let mut audit = journal(file, 6, session).await;
    assert_eq!(audit.session(), session);
    let events = [
        BrokerEvent::ProviderRequested,
        BrokerEvent::DeliveryPrepared,
        BrokerEvent::ReplyWritten,
        BrokerEvent::Denied,
        BrokerEvent::Failed,
        BrokerEvent::ServiceClosed,
    ];
    for event in events {
        audit.record(event).await.unwrap();
    }
    let text = tokio::fs::read_to_string(private.path()).await.unwrap();
    let rows: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 6);
    let mut elapsed = 0;
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.as_object().unwrap().len(), 6, "closed schema only");
        assert_eq!(row["source"], "broker");
        assert_eq!(row["session_id"], session.to_string());
        assert_eq!(row["sequence"], index + 1);
        assert_eq!(row["event"], serde_json::to_value(events[index]).unwrap());
        assert!(row["wall_millis"].as_u64().unwrap() > 0);
        assert!(row["elapsed_micros"].as_u64().unwrap() >= elapsed);
        elapsed = row["elapsed_micros"].as_u64().unwrap();
    }
    for _ in 0..2 {
        assert!(matches!(
            audit.record(BrokerEvent::ProviderRequested).await,
            Err(AuditError::Unavailable)
        ));
    }
    assert_eq!(
        tokio::fs::read_to_string(private.path()).await.unwrap(),
        text
    );
    assert!(!format!("{audit:?}").contains(private.path().to_str().unwrap()));
}

#[tokio::test]
async fn it_refuses_existing_evidence_and_catches_buffered_write_failure() {
    let private = tempfile::NamedTempFile::new().unwrap();
    tokio::fs::write(private.path(), b"existing evidence")
        .await
        .unwrap();
    assert!(matches!(
        BrokerJournal::new(
            File::from_std(private.reopen().unwrap()),
            SessionId::new(),
            NonZeroU32::MIN,
            Duration::from_secs(1)
        )
        .await,
        Err(AuditError::Configuration)
    ));
    let empty = tempfile::NamedTempFile::new().unwrap();
    assert!(matches!(
        BrokerJournal::new(
            File::from_std(empty.reopen().unwrap()),
            SessionId::new(),
            NonZeroU32::MIN,
            Duration::ZERO
        )
        .await,
        Err(AuditError::Configuration)
    ));
    let mut audit = journal(
        std::fs::File::open(empty.path()).unwrap(),
        2,
        SessionId::new(),
    )
    .await;
    assert!(matches!(
        audit.record(BrokerEvent::ProviderRequested).await,
        Err(AuditError::Io(_))
    ));
    assert!(matches!(
        audit.record(BrokerEvent::ProviderRequested).await,
        Err(AuditError::Unavailable)
    ));
    assert_eq!(tokio::fs::metadata(empty.path()).await.unwrap().len(), 0);
}

#[test]
fn it_permanently_refuses_after_cancelled_or_timed_out_io() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        for deadline in [false, true] {
            let private = tempfile::NamedTempFile::new().unwrap();
            let mut audit = journal(private.reopen().unwrap(), 3, SessionId::new()).await;
            let (release, wait) = std::sync::mpsc::channel();
            let (ready, started) = tokio::sync::oneshot::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                ready.send(()).unwrap();
                wait.recv().unwrap();
            });
            started.await.unwrap();
            tokio::time::pause();
            let mut append = Box::pin(audit.record(BrokerEvent::ProviderRequested));
            poll_fn(|cx| {
                assert!(append.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            if deadline {
                tokio::time::advance(Duration::from_secs(2)).await;
                assert!(matches!(append.await, Err(AuditError::Timeout)));
            } else {
                drop(append);
            }
            release.send(()).unwrap();
            blocker.await.unwrap();
            assert!(matches!(
                audit.record(BrokerEvent::ReplyWritten).await,
                Err(AuditError::Unavailable)
            ));
            tokio::time::resume();
        }
    });
}
