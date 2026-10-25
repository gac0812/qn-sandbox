#![allow(clippy::unwrap_used)]
use super::*;
use crate::audit::BrokerJournal;
use std::{error::Error, num::NonZeroU32};

fn row(session: SessionId, sequence: u64, event: &str) -> Vec<u8> {
    let mut text = serde_json::json!({"source":"broker","session_id":session.to_string(),
        "sequence":sequence,"wall_millis":10,"elapsed_micros":sequence,"event":event})
    .to_string();
    text.push('\n');
    text.into_bytes()
}

#[tokio::test]
async fn it_reads_the_real_writer_with_explicit_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_owned()).unwrap();
    let session = SessionId::new();
    let query = BrokerAuditQuery::new(root.clone()).unwrap();
    assert!(matches!(
        query.read(session).await,
        Err(AuditQueryError::Missing)
    ));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(format!("{session}.broker.jsonl")))
        .unwrap();
    let mut journal = BrokerJournal::new(
        File::from_std(file),
        session,
        NonZeroU32::new(7).unwrap(),
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    assert_eq!(
        query.read(session).await.unwrap().state,
        JournalState::Empty
    );
    for event in [
        BrokerEvent::GrantIssued,
        BrokerEvent::ProviderRequested,
        BrokerEvent::DeliveryPrepared,
        BrokerEvent::ReplyWritten,
        BrokerEvent::GrantRevoked,
        BrokerEvent::Denied,
    ] {
        journal.record(event).await.unwrap();
    }
    let active = query.read(session).await.unwrap();
    assert_eq!(active.state, JournalState::Open);
    assert_eq!(active.events.len(), 6);
    journal.record(BrokerEvent::ServiceClosed).await.unwrap();
    let closed = query.read(session).await.unwrap();
    assert_eq!(closed.state, JournalState::ObservedClosed);
    assert_eq!(closed.integrity, AuditIntegrity::Unverified);
    assert_eq!(closed.source, AuditSource::Broker);
    assert_eq!(closed.session_id, session);
    assert_eq!(closed.events[0].event, AuditEventKind::GrantIssued);
    assert_eq!(closed.events[4].event, AuditEventKind::GrantRevoked);
    assert_eq!(closed.events[5].event, AuditEventKind::Denied);
    let json = serde_json::to_value(closed).unwrap();
    assert_eq!(json["session_id"], session.to_string());
    assert_eq!(json["integrity"], "unverified");
    assert!(!format!("{query:?}").contains(root.as_str()));
}

#[test]
fn it_reports_partial_tail_without_promoting_or_exposing_it() {
    let session = SessionId::new();
    for prefix in [Vec::new(), row(session, 1, "denied")] {
        let mut bytes = prefix.clone();
        bytes.extend_from_slice(b"{\"secret\":\"synthetic-sensitive-canary");
        let result = parse(&bytes, session).unwrap();
        assert_eq!(result.state, JournalState::Partial);
        assert_eq!(result.events.len(), usize::from(!prefix.is_empty()));
        assert!(!serde_json::to_string(&result).unwrap().contains("canary"));
    }
}

#[test]
fn it_refuses_foreign_unknown_duplicate_and_leaking_payloads() {
    let session = SessionId::new();
    let good = String::from_utf8(row(session, 1, "denied")).unwrap();
    let cases = [
        good.replace(&session.to_string(), &SessionId::new().to_string()),
        good.replace("broker", "nono"),
        good.replace("denied", "synthetic-sensitive-canary"),
        good.replacen('{', "{\"secret\":\"synthetic-sensitive-canary\",", 1),
        good.replacen('{', "{\"source\":\"broker\",", 1),
        good.replace("\"sequence\":1", "\"sequence\":0"),
        good.replace("\"sequence\":1", "\"sequence\":2"),
        good.replace("\"wall_millis\":10", "\"wall_millis\":-1"),
        "synthetic-sensitive-canary\n".to_owned(),
        "\n".to_owned(),
    ];
    for bytes in cases {
        let error = parse(bytes.as_bytes(), session).unwrap_err();
        let mut source: Option<&dyn Error> = Some(&error);
        while let Some(value) = source {
            assert!(!format!("{value} {value:?}").contains("canary"));
            source = value.source();
        }
    }
}

#[test]
fn it_orders_by_sequence_and_rejects_regression_or_postclosure_data() {
    let session = SessionId::new();
    let first = row(session, 1, "denied");
    let second = String::from_utf8(row(session, 2, "failed")).unwrap();
    let backwards_wall = second.replace("\"wall_millis\":10", "\"wall_millis\":1");
    let bytes = [first.clone(), backwards_wall.into_bytes()].concat();
    assert_eq!(parse(&bytes, session).unwrap().events[1].wall_millis, 1);
    for next in [
        second
            .replace("\"elapsed_micros\":2", "\"elapsed_micros\":0")
            .into_bytes(),
        row(session, 1, "denied"),
        row(session, 3, "denied"),
    ] {
        assert!(parse(&[first.clone(), next].concat(), session).is_err());
    }
    let closed = row(session, 1, "service_closed");
    for tail in [row(session, 2, "denied"), b"partial".to_vec()] {
        assert!(parse(&[closed.clone(), tail].concat(), session).is_err());
    }
}

#[tokio::test]
async fn it_bounds_files_lines_records_and_refuses_nonregular_targets() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_owned()).unwrap();
    assert!(BrokerAuditQuery::new("relative".into()).is_err());
    let session = SessionId::new();
    let query = BrokerAuditQuery::new(root.clone()).unwrap();
    let path = root.join(format!("{session}.broker.jsonl"));
    std::fs::create_dir(&path).unwrap();
    assert!(matches!(
        query.read(session).await,
        Err(AuditQueryError::Configuration)
    ));
    std::fs::remove_dir(&path).unwrap();
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_BYTES + 1).unwrap();
    assert!(matches!(
        query.read(session).await,
        Err(AuditQueryError::Limit)
    ));
    assert!(matches!(
        parse(&vec![b'x'; MAX_LINE + 1], session),
        Err(AuditQueryError::Limit)
    ));
    let bytes: Vec<_> = (1..=10_001)
        .flat_map(|seq| row(session, seq, "denied"))
        .collect();
    assert!(matches!(
        parse(&bytes, session),
        Err(AuditQueryError::Limit)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn it_rejects_a_symlink_without_reading_its_target() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_owned()).unwrap();
    let session = SessionId::new();
    let secret = root.join("synthetic-host-secret");
    std::fs::write(&secret, b"sensitive-canary").unwrap();
    std::os::unix::fs::symlink(&secret, root.join(format!("{session}.broker.jsonl"))).unwrap();
    let error = BrokerAuditQuery::new(root)
        .unwrap()
        .read(session)
        .await
        .unwrap_err();
    assert!(matches!(error, AuditQueryError::Configuration));
    assert!(!format!("{error:?} {error}").contains("canary"));
}

#[test]
fn it_times_out_when_host_file_work_cannot_start() {
    use std::{future::poll_fn, task::Poll};
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_owned()).unwrap();
        let query = BrokerAuditQuery::new(root).unwrap();
        let (release, wait) = std::sync::mpsc::channel();
        let (ready, started) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            ready.send(()).unwrap();
            wait.recv().unwrap();
        });
        started.await.unwrap();
        tokio::time::pause();
        let mut read = query.read(SessionId::new());
        poll_fn(|cx| {
            assert!(read.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        tokio::time::advance(Duration::from_secs(6)).await;
        let result = read.await;
        release.send(()).unwrap();
        blocker.await.unwrap();
        assert!(matches!(result, Err(AuditQueryError::Timeout)));
        tokio::time::resume();
    });
}
