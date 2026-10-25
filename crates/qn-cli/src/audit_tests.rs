#![allow(clippy::unwrap_used)]
use super::*;
use qn_core::audit::{AuditQueryError, AuditQueryFuture};
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

struct StalledRead(Arc<Mutex<mpsc::Receiver<()>>>);
impl AuditQuery for StalledRead {
    fn read(&self, _: SessionId) -> AuditQueryFuture<'_> {
        let release = self.0.clone();
        Box::pin(async move {
            let (ready, started) = tokio::sync::oneshot::channel();
            tokio::task::spawn_blocking(move || {
                ready.send(()).unwrap();
                release.lock().unwrap().recv().unwrap();
            });
            started.await.unwrap();
            Err(AuditQueryError::Timeout)
        })
    }
}

#[test]
fn it_returns_timeout_without_waiting_for_unfinished_host_io() {
    let (release, blocked) = mpsc::channel();
    let query = StalledRead(Arc::new(Mutex::new(blocked)));
    let (send, receive) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        send.send(read(&query, SessionId::new())).unwrap();
    });
    let returned = receive.recv_timeout(Duration::from_secs(2));
    // Always release and join, even against the buggy implementation.
    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(matches!(
        returned,
        Ok(Err(Error::Audit(AuditQueryError::Timeout)))
    ));
}
