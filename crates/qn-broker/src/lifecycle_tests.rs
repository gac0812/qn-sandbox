#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;
use std::{
    future::{Future, poll_fn},
    num::{NonZeroU32, NonZeroUsize},
    task::Poll,
};

fn scope() -> CaptureScope {
    CaptureScope::new(
        "private-ref",
        "https://private-host",
        "GET",
        "/private-path",
    )
    .unwrap()
}
fn assert_capacity_available(registry: &GrantRegistry) {
    let session = SessionId::new();
    let handle = registry
        .grant(session, vec![scope()], Duration::from_secs(30))
        .unwrap();
    registry.revoke(handle, session).unwrap();
}
async fn issue(registry: Arc<GrantRegistry>, journal: BrokerJournal) -> AuditedGrant {
    AuditedGrant::issue(registry, journal, vec![scope()], Duration::from_secs(60))
        .await
        .unwrap()
}
async fn pending(mut future: std::pin::Pin<&mut impl Future>) {
    poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
}
async fn journal(file: std::fs::File, capacity: u32) -> BrokerJournal {
    BrokerJournal::new(
        tokio::fs::File::from_std(file),
        SessionId::new(),
        NonZeroU32::new(capacity).unwrap(),
        Duration::from_secs(1),
    )
    .await
    .unwrap()
}
fn events(file: &tempfile::NamedTempFile) -> Vec<String> {
    let text = std::fs::read_to_string(file.path()).unwrap();
    assert!(!text.contains("private-") && !text.contains("grt-"));
    text.lines()
        .enumerate()
        .map(|(index, line)| {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(value["sequence"], index + 1);
            value["event"].as_str().unwrap().to_owned()
        })
        .collect()
}

#[tokio::test]
async fn it_invalidates_before_waiting_for_evidence_and_acknowledges_revoke_once() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let registry = Arc::new(GrantRegistry::new(NonZeroUsize::MIN));
    let grant = issue(registry, journal(file.reopen().unwrap(), 8).await).await;
    assert_eq!(events(&file), ["grant_issued"]);
    grant.authorize(&scope()).unwrap();
    let lock = grant.0.evidence.lock().await;
    let mut revoke = Box::pin(grant.revoke());
    pending(revoke.as_mut()).await;
    assert_eq!(grant.authorize(&scope()), Err(GrantError::Revoked));
    assert_eq!(events(&file), ["grant_issued"]);
    drop(lock);
    let (first, second) = tokio::join!(revoke, grant.revoke());
    first.unwrap();
    second.unwrap();
    grant.revoke().await.unwrap();
    assert_eq!(events(&file), ["grant_issued", "grant_revoked"]);
}

#[tokio::test]
async fn it_keeps_revocation_effective_on_audit_failure_or_cancelled_wait() {
    for exhausted in [true, false] {
        let file = tempfile::NamedTempFile::new().unwrap();
        let registry = Arc::new(GrantRegistry::new(NonZeroUsize::MIN));
        let grant = issue(
            registry,
            journal(file.reopen().unwrap(), if exhausted { 1 } else { 8 }).await,
        )
        .await;
        if exhausted {
            assert!(matches!(
                grant.revoke().await,
                Err(LifecycleError::Audit(AuditError::Unavailable))
            ));
        } else {
            let lock = grant.0.evidence.lock().await;
            let mut revoke = Box::pin(grant.revoke());
            pending(revoke.as_mut()).await;
            drop(revoke);
            drop(lock);
            grant.revoke().await.unwrap();
        }
        assert_eq!(grant.authorize(&scope()), Err(GrantError::Revoked));
        assert!(grant.0.failed.is_cancelled());
        assert_eq!(events(&file).len(), if exhausted { 1 } else { 2 });
    }
}

#[tokio::test]
async fn it_refuses_issue_when_evidence_cannot_be_written_and_releases_capacity() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let registry = Arc::new(GrantRegistry::new(NonZeroUsize::MIN));
    let result = AuditedGrant::issue(
        registry.clone(),
        journal(std::fs::File::open(file.path()).unwrap(), 4).await,
        vec![scope()],
        Duration::from_secs(30),
    )
    .await;
    assert!(matches!(
        result,
        Err(LifecycleError::Audit(AuditError::Io(_)))
    ));
    assert!(events(&file).is_empty());
    assert_capacity_available(&registry);
}

#[test]
fn it_reclaims_private_grants_when_pending_issue_is_dropped() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let file = tempfile::NamedTempFile::new().unwrap();
        let journal = journal(file.reopen().unwrap(), 4).await;
        let registry = Arc::new(GrantRegistry::new(NonZeroUsize::MIN));
        let (release, wait) = std::sync::mpsc::channel();
        let (ready, started) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            ready.send(()).unwrap();
            wait.recv().unwrap();
        });
        started.await.unwrap();
        let mut issue = Box::pin(AuditedGrant::issue(
            registry.clone(),
            journal,
            vec![scope()],
            Duration::from_secs(30),
        ));
        pending(issue.as_mut()).await;
        assert_eq!(
            registry.grant(SessionId::new(), vec![scope()], Duration::from_secs(30)),
            Err(GrantError::Capacity)
        );
        drop(issue);
        assert_capacity_available(&registry);
        release.send(()).unwrap();
        blocker.await.unwrap();
    });
}

#[tokio::test]
async fn it_invalidates_when_the_last_owner_drops_without_fabricating_a_revoke_event() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let registry = Arc::new(GrantRegistry::new(NonZeroUsize::MIN));
    let grant = issue(registry.clone(), journal(file.reopen().unwrap(), 4).await).await;
    let (handle, session) = (grant.0.handle, grant.0.session);
    let copy = grant.clone();
    drop(grant);
    copy.authorize(&scope()).unwrap();
    drop(copy);
    assert_eq!(
        registry.authorize(handle, session, &scope()),
        Err(GrantError::Revoked)
    );
    assert_eq!(events(&file), ["grant_issued"]);
}

#[tokio::test]
async fn it_bounds_journal_lock_wait_without_delaying_revocation() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let registry = Arc::new(GrantRegistry::new(NonZeroUsize::MIN));
    let grant = issue(registry, journal(file.reopen().unwrap(), 4).await).await;
    let lock = grant.0.evidence.lock().await;
    tokio::time::pause();
    let mut revoke = Box::pin(grant.revoke());
    pending(revoke.as_mut()).await;
    assert_eq!(grant.authorize(&scope()), Err(GrantError::Revoked));
    tokio::time::advance(JOURNAL_LOCK_DEADLINE + Duration::from_secs(1)).await;
    assert!(matches!(
        revoke.await,
        Err(LifecycleError::Audit(AuditError::Timeout))
    ));
    assert!(grant.0.failed.is_cancelled());
    tokio::time::resume();
    drop(lock);
    assert_eq!(events(&file), ["grant_issued"]);
    grant.revoke().await.unwrap();
    assert_eq!(events(&file), ["grant_issued", "grant_revoked"]);
}

#[tokio::test]
async fn it_refuses_reusing_a_journal_that_already_contains_events() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let registry = Arc::new(GrantRegistry::new(NonZeroUsize::MIN));
    let mut journal = journal(file.reopen().unwrap(), 4).await;
    journal.record(BrokerEvent::ServiceClosed).await.unwrap();
    assert!(matches!(
        AuditedGrant::issue(
            registry.clone(),
            journal,
            vec![scope()],
            Duration::from_secs(30)
        )
        .await,
        Err(LifecycleError::Audit(AuditError::Configuration))
    ));
    assert_eq!(events(&file), ["service_closed"]);
    assert_capacity_available(&registry);
}
