//! Owned capture listener and shutdown; nono owns credential proxy injection (#107).
pub use crate::request::CaptureRoute;
use crate::{
    audit::{AuditError, BrokerEvent},
    capture::CaptureListener,
    lifecycle::{AuditedGrant, LifecycleError},
    request::{RequestError, record},
};
use qn_infra::process::ProcessPolicy;
use tokio_util::sync::CancellationToken;

/// Redacted serving failures retain all evidence and shutdown errors.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// Request handling failed; serving must stop.
    #[error(transparent)]
    Request(#[from] RequestError),
    /// Shared evidence is unavailable; authorization must stop.
    #[error(transparent)]
    Audit(#[from] AuditError),
    /// An operation failed and its failure evidence could not be persisted.
    #[error("capture operation and failure audit both failed; inspect retained evidence")]
    Evidence {
        /// Original redacted failure.
        #[source]
        operation: Box<ServiceError>,
        /// Failure-evidence IO error.
        audit: AuditError,
    },
    /// Preserve serving and shutdown failures together; all cleanup was attempted.
    #[error(
        "capture service terminated with incomplete work; inspect redacted causes before restarting"
    )]
    Termination {
        /// Original serving failure, if any.
        #[source]
        serving: Option<Box<ServiceError>>,
        /// Grant revocation failure, if any.
        revoke: Option<LifecycleError>,
        /// Closing evidence failure, if any.
        audit: Option<AuditError>,
    },
}

/// One listener and one request/provider at a time; no detached tasks or secret cache.
/// The audited owner binds one grant to one session. Trusted composition verifies the
/// running native process association before enabling this service (#93/#103).
/// Cancel and await run for shutdown; dropping it only revokes/closes best-effort.
pub struct CaptureService {
    listener: Option<CaptureListener>,
    route: CaptureRoute,
    grant: AuditedGrant,
}
impl CaptureService {
    /// Own all capture resources. Provision listener.binding only to the host relay.
    #[must_use]
    pub fn new(listener: CaptureListener, route: CaptureRoute, grant: AuditedGrant) -> Self {
        Self {
            listener: Some(listener),
            route,
            grant,
        }
    }

    /// Serve until cancellation or fatal audit/provider/registry failure; always close
    /// the listener, revoke the grant and attempt closure evidence before returning.
    /// # Errors
    /// Returns all serving/revoke/audit failures. Success refers only to these owned
    /// resources; it never proves the sandbox's whole execution tree has stopped.
    pub async fn run(
        mut self,
        policy: &dyn ProcessPolicy,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let audit_failure = self.grant.failure();
        let mut serving = self
            .serve(policy, &cancel, &audit_failure)
            .await
            .err()
            .map(Box::new);
        drop(self.listener.take());
        let revoke = self.grant.revoke().await.err();
        if let Some(operation) = serving.take() {
            serving = Some(match self.grant.record(BrokerEvent::Failed).await {
                Ok(()) => operation,
                Err(audit) => Box::new(ServiceError::Evidence { operation, audit }),
            });
        }
        let audit = self.grant.record(BrokerEvent::ServiceClosed).await.err();
        if serving.is_some() || revoke.is_some() || audit.is_some() {
            return Err(ServiceError::Termination {
                serving,
                revoke,
                audit,
            });
        }
        Ok(())
    }

    async fn serve(
        &mut self,
        policy: &dyn ProcessPolicy,
        cancel: &CancellationToken,
        audit_failure: &CancellationToken,
    ) -> Result<(), ServiceError> {
        loop {
            let Some(listener) = self.listener.as_ref() else {
                return Ok(());
            };
            let accepted = tokio::select! {
                biased;
                () = audit_failure.cancelled() => return Err(AuditError::Unavailable.into()),
                () = cancel.cancelled() => return Ok(()),
                accepted = listener.accept() => accepted,
            };
            match accepted {
                Ok(request) => {
                    self.route
                        .process(request, &self.grant, policy, cancel)
                        .await?;
                }
                Err(_) => record(&self.grant, BrokerEvent::Failed, cancel).await?,
            }
        }
    }
}

impl Drop for CaptureService {
    fn drop(&mut self) {
        let _ = self.grant.invalidate();
    }
}
