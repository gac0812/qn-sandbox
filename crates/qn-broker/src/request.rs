//! Audited handling of one authenticated native capture request (#107).
use crate::{
    CaptureScope, GrantError,
    audit::{AuditError, BrokerEvent},
    capture::{CaptureBytes, CaptureError, CaptureRequest},
    identity::{CaptureIdentity, IdentityError},
    lifecycle::AuditedGrant,
    provider::{CommandProvider, ProviderError, ProviderSecret},
};
use qn_infra::process::ProcessPolicy;
use serde::Serialize;
use std::fmt;
use tokio_util::sync::CancellationToken;
/// Typed errors never expose request metadata or credential output.
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    /// Invalid predetermined native route.
    #[error("capture route rejected: {0}")]
    Identity(#[from] IdentityError),
    /// Host scope or grant state failure.
    #[error("capture authorization unavailable: {0}")]
    Grant(#[from] GrantError),
    /// Required supplemental evidence was not acknowledged.
    #[error("capture audit unavailable: {0}")]
    Audit(#[from] AuditError),
    /// Provider execution/cleanup failed; do not claim sandbox stopped.
    #[error("capture provider lifecycle failed: {0}")]
    Provider(#[from] ProviderError),
    /// Bounded encrypted response could not be constructed.
    #[error("capture response rejected: {0}")]
    Capture(#[from] CaptureError),
    /// Encoding closed host values failed without exposing them.
    #[error("capture response encoding failed; inspect broker installation")]
    Encoding(#[source] serde_json::Error),
}
/// Immutable trusted native identity, canonical origin and host credential command.
/// Neither the origin nor command is obtained from capture request metadata.
pub struct CaptureRoute {
    identity: CaptureIdentity,
    reference: String,
    origin: String,
    provider: CommandProvider,
}
impl CaptureRoute {
    /// Bind a managed native route before publishing the private relay binding.
    /// # Errors
    /// Rejects invalid native identity/reference or non-exact origin.
    pub fn new(
        native: &str,
        reference: &str,
        origin: &str,
        provider: CommandProvider,
    ) -> Result<Self, RequestError> {
        let identity = CaptureIdentity::new(native, reference)?;
        CaptureScope::new(reference, origin, "GET", "/")?;
        Ok(Self {
            identity,
            reference: reference.into(),
            origin: origin.into(),
            provider,
        })
    }
    /// Handle one authenticated request; the caller owns serving and shutdown.
    /// Provider work and audit writes occur outside the authorization lock.
    /// # Errors
    /// Fatal audit, registry, provider cleanup or encoding failures require shutdown.
    pub async fn process(
        &self,
        request: CaptureRequest,
        grant: &AuditedGrant,
        policy: &dyn ProcessPolicy,
        cancel: &CancellationToken,
    ) -> Result<(), RequestError> {
        RequestHandler { route: self, grant }
            .process(request, policy, cancel)
            .await
    }
}
impl fmt::Debug for CaptureRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CaptureRoute(..)")
    }
}

struct RequestHandler<'a> {
    route: &'a CaptureRoute,
    grant: &'a AuditedGrant,
}
impl RequestHandler<'_> {
    fn scope(&self, request: &CaptureRequest) -> Result<CaptureScope, RequestError> {
        let metadata = self.route.identity.validate(request.payload())?;
        Ok(CaptureScope::new(
            &self.route.reference,
            &self.route.origin,
            metadata.method(),
            metadata.path(),
        )?)
    }
    fn authorize(&self, scope: &CaptureScope) -> Result<(), GrantError> {
        self.grant.authorize(scope)
    }

    async fn process(
        &self,
        request: CaptureRequest,
        policy: &dyn ProcessPolicy,
        cancel: &CancellationToken,
    ) -> Result<(), RequestError> {
        let Ok(scope) = self.scope(&request) else {
            return record(self.grant, BrokerEvent::Denied, cancel).await;
        };
        if let Err(error) = self.authorize(&scope) {
            return self.denied(error, cancel).await;
        }
        record(self.grant, BrokerEvent::ProviderRequested, cancel).await?;
        if cancel.is_cancelled() || tokio::time::Instant::now() >= request.deadline() {
            return record(self.grant, BrokerEvent::Failed, cancel).await;
        }
        if let Err(error) = self.authorize(&scope) {
            return self.denied(error, cancel).await;
        }
        let Some(secret) = self.capture(policy, cancel, request.deadline()).await? else {
            return record(self.grant, BrokerEvent::Failed, cancel).await;
        };
        let response = response(&secret)?;
        record(self.grant, BrokerEvent::DeliveryPrepared, cancel).await?;
        if cancel.is_cancelled() {
            return record(self.grant, BrokerEvent::Failed, cancel).await;
        }
        let admitted = self.grant.admit(&scope, || (request, response));
        let (request, response) = match admitted {
            Ok(delivery) => delivery,
            Err(error) => return self.denied(error, cancel).await,
        };
        let written = tokio::select! {
            biased;
            () = cancel.cancelled() => false,
            result = request.reply(response) => result.is_ok(),
        };
        record(
            self.grant,
            if written {
                BrokerEvent::ReplyWritten
            } else {
                BrokerEvent::Failed
            },
            cancel,
        )
        .await
    }

    async fn denied(
        &self,
        error: GrantError,
        cancel: &CancellationToken,
    ) -> Result<(), RequestError> {
        record(self.grant, BrokerEvent::Denied, cancel).await?;
        if error == GrantError::Unavailable {
            return Err(error.into());
        }
        Ok(())
    }

    async fn capture(
        &self,
        policy: &dyn ProcessPolicy,
        cancel: &CancellationToken,
        deadline: tokio::time::Instant,
    ) -> Result<Option<ProviderSecret>, RequestError> {
        let child = cancel.child_token();
        let audit_failure = self.grant.failure();
        let capture = self.route.provider.capture(policy, child.clone());
        tokio::pin!(capture);
        let result = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                let revoked = self.grant.invalidate();
                child.cancel();
                let result = capture.await;
                revoked?;
                result
            }
            () = audit_failure.cancelled() => {
                child.cancel();
                let result = capture.await;
                result?;
                return Err(AuditError::Unavailable.into());
            }
            () = tokio::time::sleep_until(deadline) => { child.cancel(); capture.await }
            result = &mut capture => result,
        };
        match result {
            Ok(secret) if !cancel.is_cancelled() && !child.is_cancelled() => Ok(Some(secret)),
            Err(error @ ProviderError::Process(_)) => Err(error.into()),
            _ => Ok(None),
        }
    }
}
pub(crate) async fn record(
    grant: &AuditedGrant,
    event: BrokerEvent,
    cancel: &CancellationToken,
) -> Result<(), RequestError> {
    let append = grant.record(event);
    tokio::pin!(append);
    tokio::select! {
        biased;
        () = cancel.cancelled() => {
            let revoked = grant.invalidate();
            let recorded = append.await;
            revoked?;
            recorded?;
        }
        result = &mut append => result?,
    }
    Ok(())
}

fn response(secret: &ProviderSecret) -> Result<CaptureBytes, RequestError> {
    #[derive(Serialize)]
    struct Reply<'a> {
        material: Material<'a>,
    }
    #[derive(Serialize)]
    struct Material<'a> {
        r#type: &'static str,
        value: &'a str,
    }
    let reply = Reply {
        material: Material {
            r#type: "secret",
            value: secret.expose(),
        },
    };
    Ok(CaptureBytes::new(
        serde_json::to_vec(&reply).map_err(RequestError::Encoding)?,
    )?)
}
