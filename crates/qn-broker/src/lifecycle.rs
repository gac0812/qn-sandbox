//! Session-owned grant transitions sharing the capture journal (#119).
use crate::{
    CaptureScope, GrantError, GrantRegistry,
    audit::{AuditError, BrokerEvent, BrokerJournal},
};
use qn_core::{GrantHandle, SessionId};
use std::{fmt, sync::Arc, time::Duration};
use tokio::{sync::Mutex, time::timeout};
use tokio_util::sync::CancellationToken;

const JOURNAL_LOCK_DEADLINE: Duration = Duration::from_secs(30);

/// Grant and durable evidence failures; neither contains routing or credential data.
#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    /// Authorization mutation failed closed.
    #[error(transparent)]
    Grant(#[from] GrantError),
    /// Evidence was not acknowledged; authorization must not continue.
    #[error(transparent)]
    Audit(#[from] AuditError),
}
struct Evidence {
    journal: BrokerJournal,
    revoked: bool,
}
struct Owner {
    registry: Arc<GrantRegistry>,
    handle: GrantHandle,
    session: SessionId,
    evidence: Mutex<Evidence>,
    failed: CancellationToken,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.registry.revoke(self.handle, self.session);
    }
}

/// A grant is exposed only after its creation evidence is durable.
/// Clones share one grant and journal. Explicitly await revoke for durable evidence;
/// last-owner Drop only invalidates best-effort. Never authorize using journal replay.
///
/// ```no_run
/// # async fn example(journal: qn_broker::audit::BrokerJournal) -> Result<(), Box<dyn std::error::Error>> {
/// use qn_broker::{AuditedGrant, CaptureScope, GrantRegistry};
/// let registry = std::sync::Arc::new(GrantRegistry::new(std::num::NonZeroUsize::MIN));
/// let scope = CaptureScope::new("service", "https://api.example:443", "GET", "/status")?;
/// let owner = AuditedGrant::issue(registry, journal, vec![scope.clone()], std::time::Duration::from_secs(60)).await?;
/// owner.authorize(&scope)?;
/// owner.revoke().await?;
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct AuditedGrant(Arc<Owner>);
impl fmt::Debug for AuditedGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuditedGrant(..)")
    }
}
impl AuditedGrant {
    /// Create a private grant and persist its creation before handing it to capture.
    /// Trusted composition supplies a fresh exclusive session journal and exact scopes.
    /// # Errors
    /// Failed/cancelled creation revokes the private grant; uncertain evidence may remain.
    pub async fn issue(
        registry: Arc<GrantRegistry>,
        journal: BrokerJournal,
        scopes: Vec<CaptureScope>,
        lifetime: Duration,
    ) -> Result<Self, LifecycleError> {
        if !journal.is_fresh() {
            return Err(AuditError::Configuration.into());
        }
        let session = journal.session();
        let handle = registry.grant(session, scopes, lifetime)?;
        let grant = Self(Arc::new(Owner {
            registry,
            handle,
            session,
            evidence: Mutex::new(Evidence {
                journal,
                revoked: false,
            }),
            failed: CancellationToken::new(),
        }));
        grant.record(BrokerEvent::GrantIssued).await?;
        Ok(grant)
    }
    /// Validate a scope before provider work; capture rechecks again before admission.
    /// # Errors
    /// Refuses expired, revoked or unavailable authorization.
    pub fn authorize(&self, scope: &CaptureScope) -> Result<(), GrantError> {
        self.0
            .registry
            .authorize(self.0.handle, self.0.session, scope)
    }
    pub(crate) fn invalidate(&self) -> Result<(), GrantError> {
        let result = self.0.registry.revoke(self.0.handle, self.0.session);
        if result.is_err() {
            self.0.failed.cancel();
        }
        result
    }
    /// Immediately invalidate when polled, then acknowledge durable revoke evidence.
    /// Repeated/concurrent calls record once. Cancellation never restores authority,
    /// but may leave missing or uncertain evidence; retain the journal on error.
    /// # Errors
    /// Registry or journal failures stop capture; no success is fabricated.
    pub async fn revoke(&self) -> Result<(), LifecycleError> {
        self.invalidate()?;
        self.record(BrokerEvent::GrantRevoked).await?;
        Ok(())
    }
    pub(crate) async fn record(&self, event: BrokerEvent) -> Result<(), AuditError> {
        // Journal serialization may await IO; the registry authorization mutex never does.
        let mut guard = PendingEvidence {
            grant: self,
            acknowledged: false,
        };
        let mut evidence = timeout(JOURNAL_LOCK_DEADLINE, self.0.evidence.lock())
            .await
            .map_err(|_| AuditError::Timeout)?;
        if event == BrokerEvent::GrantRevoked && evidence.revoked {
            guard.acknowledged = true;
            return Ok(());
        }
        evidence.journal.record(event).await?;
        if event == BrokerEvent::GrantRevoked {
            evidence.revoked = true;
        }
        guard.acknowledged = true;
        Ok(())
    }
}
struct PendingEvidence<'a> {
    grant: &'a AuditedGrant,
    acknowledged: bool,
}
impl Drop for PendingEvidence<'_> {
    fn drop(&mut self) {
        if !self.acknowledged {
            let _ = self.grant.invalidate();
            self.grant.0.failed.cancel();
        }
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
