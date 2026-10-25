//! Secret-free authorization state for the host capture boundary (#79).
use qn_core::{GrantHandle, SessionId};
use std::{
    collections::HashMap,
    fmt,
    num::NonZeroUsize,
    sync::Mutex,
    time::{Duration, Instant},
};

/// One exact request scope, constructed from trusted, canonical routing metadata.
/// Origin includes scheme/port; path includes query. The capture adapter supplies
/// canonical values at creation and request time; mismatches fail closed.
#[derive(Clone, PartialEq, Eq)]
pub struct CaptureScope {
    reference: String,
    origin: String,
    method: String,
    path: String,
}
impl CaptureScope {
    /// Build an exact scope without accepting wildcards or control characters.
    /// # Errors
    /// Rejects empty/oversized values and invalid method or path structure.
    pub fn new(
        reference: &str,
        origin: &str,
        method: &str,
        path: &str,
    ) -> Result<Self, GrantError> {
        if [reference, origin, method, path].iter().any(|s| {
            s.is_empty() || s.len() > 4096 || s.chars().any(|c| c.is_control() || c == '*')
        }) || !method.bytes().all(|b| b.is_ascii_uppercase())
            || !path.starts_with('/')
        {
            return Err(GrantError::InvalidScope);
        }
        Ok(Self {
            reference: reference.into(),
            origin: origin.into(),
            method: method.into(),
            path: path.into(),
        })
    }
}
impl fmt::Debug for CaptureScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CaptureScope(..)")
    }
}

/// Typed failures; handles, routing values and provider data are never formatted.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GrantError {
    /// A requested scope is not an exact, bounded route.
    #[error("invalid capture scope; supply exact canonical routing metadata without wildcards")]
    InvalidScope,
    /// A grant needs bounded scopes and a positive, representable lifetime.
    #[error("invalid grant; supply 1-64 scopes and a nonzero representable lifetime")]
    InvalidGrant,
    /// Authorization failed for any unknown, expired, revoked or mismatched grant.
    #[error("capture denied; obtain a live grant for this session and exact request scope")]
    Denied,
    /// Capacity is exhausted by unexpired grants.
    #[error("grant capacity reached; revoke unused grants or wait for expiry")]
    Capacity,
    /// A panic poisoned authorization state, so further access fails closed.
    #[error("grant state unavailable; restart the host broker and obtain fresh grants")]
    Unavailable,
}
struct Grant {
    session: SessionId,
    scopes: Vec<CaptureScope>,
    expires: Instant,
}

/// Bounded host-only grants. A handle is an identifier, not caller authentication.
/// Restart drops all authorization; no secrets or durable authorization are stored.
/// ```
/// use qn_broker::{CaptureScope, GrantRegistry};
/// use qn_core::SessionId;
/// let registry = GrantRegistry::new(std::num::NonZeroUsize::MIN);
/// let session = SessionId::new();
/// let scope = CaptureScope::new("service", "https://api.example:443", "POST", "/v1/messages")?;
/// let handle = registry.grant(session, vec![scope.clone()], std::time::Duration::from_secs(30))?;
/// registry.authorize(handle, session, &scope)?;
/// registry.revoke(handle, session)?;
/// assert!(registry.authorize(handle, session, &scope).is_err());
/// # Ok::<(), qn_broker::GrantError>(())
/// ```
pub struct GrantRegistry {
    grants: Mutex<HashMap<GrantHandle, Grant>>,
    capacity: NonZeroUsize,
}
impl fmt::Debug for GrantRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GrantRegistry(..)")
    }
}
impl GrantRegistry {
    /// Construct an empty registry with a hard limit on live grants.
    #[must_use]
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
            capacity,
        }
    }
    /// Grant exact scopes to an authenticated session with monotonic expiry.
    /// # Errors
    /// Rejects invalid scope count/lifetime, capacity exhaustion or poisoned state.
    pub fn grant(
        &self,
        session: SessionId,
        scopes: Vec<CaptureScope>,
        lifetime: Duration,
    ) -> Result<GrantHandle, GrantError> {
        self.grant_at(session, scopes, lifetime, Instant::now())
    }
    fn grant_at(
        &self,
        session: SessionId,
        scopes: Vec<CaptureScope>,
        lifetime: Duration,
        now: Instant,
    ) -> Result<GrantHandle, GrantError> {
        if scopes.is_empty() || scopes.len() > 64 || lifetime.is_zero() {
            return Err(GrantError::InvalidGrant);
        }
        let expires = now.checked_add(lifetime).ok_or(GrantError::InvalidGrant)?;
        let mut grants = self.grants.lock().map_err(|_| GrantError::Unavailable)?;
        grants.retain(|_, grant| grant.expires > now);
        if grants.len() >= self.capacity.get() {
            return Err(GrantError::Capacity);
        }
        let handle = GrantHandle::new();
        grants.insert(
            handle,
            Grant {
                session,
                scopes,
                expires,
            },
        );
        Ok(handle)
    }
    /// Check before provider work; delivery must recheck.
    /// # Errors
    /// Returns Denied for unknown, expired, revoked or mismatched authorization.
    pub fn authorize(
        &self,
        handle: GrantHandle,
        session: SessionId,
        scope: &CaptureScope,
    ) -> Result<(), GrantError> {
        self.with_authorization(handle, session, scope, || ())
    }
    /// Recheck and admit a delivery atomically with respect to revoke.
    /// Run provider IO first, then use this closure only to enqueue its result.
    /// It must not block, await or call this registry. Revoke waits for admission;
    /// previously admitted/in-flight deliveries cannot be retroactively withdrawn.
    /// # Errors
    /// Refuses admission after expiry/revoke or if the registry is poisoned.
    pub fn with_authorization<T>(
        &self,
        handle: GrantHandle,
        session: SessionId,
        scope: &CaptureScope,
        deliver: impl FnOnce() -> T,
    ) -> Result<T, GrantError> {
        self.deliver_at(handle, session, scope, Instant::now, deliver)
    }
    fn deliver_at<T>(
        &self,
        handle: GrantHandle,
        session: SessionId,
        scope: &CaptureScope,
        clock: impl FnOnce() -> Instant,
        deliver: impl FnOnce() -> T,
    ) -> Result<T, GrantError> {
        let grants = self.grants.lock().map_err(|_| GrantError::Unavailable)?;
        let grant = grants.get(&handle).ok_or(GrantError::Denied)?;
        if grant.session != session || grant.expires <= clock() || !grant.scopes.contains(scope) {
            return Err(GrantError::Denied);
        }
        let result = deliver();
        drop(grants);
        Ok(result)
    }
    /// Revoke one session-owned grant; repeated revocation is a successful no-op.
    /// # Errors
    /// Refuses a handle belonging to another session or poisoned state.
    pub fn revoke(&self, handle: GrantHandle, session: SessionId) -> Result<(), GrantError> {
        let mut grants = self.grants.lock().map_err(|_| GrantError::Unavailable)?;
        if grants
            .get(&handle)
            .is_some_and(|grant| grant.session != session)
        {
            return Err(GrantError::Denied);
        }
        grants.remove(&handle);
        Ok(())
    }
    /// Revoke all current grants for an authenticated session and return their count.
    /// Future explicit grants require a separate authenticated live-session check.
    /// # Errors
    /// Returns Unavailable if authorization state is poisoned.
    pub fn revoke_session(&self, session: SessionId) -> Result<usize, GrantError> {
        let mut grants = self.grants.lock().map_err(|_| GrantError::Unavailable)?;
        let before = grants.len();
        grants.retain(|_, grant| grant.session != session);
        Ok(before - grants.len())
    }
}

#[cfg(test)]
#[path = "grant_tests.rs"]
mod tests;
