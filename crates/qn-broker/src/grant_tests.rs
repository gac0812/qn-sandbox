#![allow(clippy::unwrap_used)]
use super::*;
use std::sync::{Arc, Barrier};

fn scope() -> CaptureScope {
    CaptureScope::new("provider", "https://api.invalid:443", "POST", "/v1").unwrap()
}
fn fixture() -> (GrantRegistry, SessionId, GrantHandle) {
    let registry = GrantRegistry::new(NonZeroUsize::new(2).unwrap());
    let session = SessionId::new();
    let handle = registry
        .grant(session, vec![scope()], Duration::from_secs(60))
        .unwrap();
    (registry, session, handle)
}

#[test]
fn it_checks_every_scope_component_and_session_without_invoking_delivery() {
    let (registry, session, handle) = fixture();
    assert_eq!(registry.authorize(handle, session, &scope()), Ok(()));
    let mut variants = vec![scope(); 4];
    variants[0].reference = "other".into();
    variants[1].origin = "https://other.example:443".into();
    variants[2].method = "GET".into();
    variants[3].path = "/v1/messages?extra=true".into();
    for scope in variants {
        assert_eq!(
            registry
                .with_authorization(handle, session, &scope, || panic!("unauthorized delivery")),
            Err(GrantError::Denied)
        );
    }
    denied(registry.authorize(handle, SessionId::new(), &scope()));
    denied(registry.authorize(GrantHandle::new(), session, &scope()));
}

#[test]
fn it_expires_at_the_boundary_and_reclaims_capacity_without_sleeping() {
    let registry = GrantRegistry::new(NonZeroUsize::MIN);
    let session = SessionId::new();
    let now = Instant::now();
    let lifetime = Duration::from_secs(5);
    let handle = registry
        .grant_at(session, vec![scope()], lifetime, now)
        .unwrap();
    assert_eq!(
        registry.grant_at(session, vec![scope()], lifetime, now),
        Err(GrantError::Capacity)
    );
    assert_eq!(
        registry.deliver_at(handle, session, &scope(), || now, || 42),
        Ok(42)
    );
    assert_eq!(
        registry.deliver_at(
            handle,
            session,
            &scope(),
            || now + lifetime,
            || panic!("expired delivery")
        ),
        Err(GrantError::Denied)
    );
    assert!(
        registry
            .grant_at(session, vec![scope()], lifetime, now + lifetime)
            .is_ok()
    );
    denied(registry.authorize(handle, session, &scope()));
}

#[test]
fn it_rechecks_after_provider_work_and_revokes_only_owned_grants() {
    let (registry, session, handle) = fixture();
    denied(registry.revoke(handle, SessionId::new()));
    registry.authorize(handle, session, &scope()).unwrap();
    registry.revoke(handle, session).unwrap();
    registry.revoke(handle, session).unwrap();
    assert_eq!(
        registry.with_authorization(handle, session, &scope(), || panic!(
            "revoked provider result"
        )),
        Err(GrantError::Revoked)
    );
    let other = SessionId::new();
    let owned = registry
        .grant(session, vec![scope()], Duration::from_secs(60))
        .unwrap();
    let foreign = registry
        .grant(other, vec![scope()], Duration::from_secs(60))
        .unwrap();
    assert_eq!(registry.revoke_session(session), Ok(1));
    assert_eq!(
        registry.authorize(owned, session, &scope()),
        Err(GrantError::Revoked)
    );
    assert_eq!(registry.authorize(foreign, other, &scope()), Ok(()));
}

#[test]
fn it_serializes_delivery_admission_with_concurrent_revoke() {
    let (registry, session, handle) = fixture();
    let registry = Arc::new(registry);
    let barrier = Arc::new(Barrier::new(2));
    let worker = {
        let (registry, barrier) = (registry.clone(), barrier.clone());
        std::thread::spawn(move || {
            barrier.wait();
            registry.revoke(handle, session).unwrap();
        })
    };
    registry
        .with_authorization(handle, session, &scope(), || {
            barrier.wait();
            assert!(
                registry.grants.try_lock().is_err(),
                "admission holds the revoke lock"
            );
        })
        .unwrap();
    worker.join().unwrap();
    assert_eq!(
        registry.authorize(handle, session, &scope()),
        Err(GrantError::Revoked)
    );
}

#[test]
fn it_attributes_only_exact_unexpired_revocation_evidence() {
    let registry = GrantRegistry::new(NonZeroUsize::MIN);
    let session = SessionId::new();
    let now = Instant::now();
    let ttl = Duration::from_secs(60);
    let handle = registry.grant_at(session, vec![scope()], ttl, now).unwrap();
    registry.revoke(handle, session).unwrap();
    registry.revoke(handle, session).unwrap();
    denied(registry.revoke(handle, SessionId::new()));
    let other = CaptureScope::new("provider", "https://api.invalid:443", "GET", "/v1").unwrap();
    for (id, owner, route, time, expected) in [
        (handle, session, scope(), now, GrantError::Revoked),
        (handle, SessionId::new(), scope(), now, GrantError::Denied),
        (handle, session, other, now, GrantError::Denied),
        (
            GrantHandle::new(),
            session,
            scope(),
            now,
            GrantError::Denied,
        ),
        (handle, session, scope(), now + ttl, GrantError::Denied),
    ] {
        assert_eq!(
            registry.deliver_at(id, owner, &route, || time, || panic!("denied delivery")),
            Err(expected)
        );
    }
    let restarted = GrantRegistry::new(NonZeroUsize::MIN);
    denied(restarted.authorize(handle, session, &scope()));
}

#[test]
fn it_bounds_revocation_history_without_consuming_active_capacity() {
    let registry = GrantRegistry::new(NonZeroUsize::MIN);
    let session = SessionId::new();
    let first = registry
        .grant(session, vec![scope()], Duration::from_secs(60))
        .unwrap();
    registry.revoke(first, session).unwrap();
    let second = registry
        .grant(session, vec![scope()], Duration::from_secs(60))
        .unwrap();
    assert_eq!(
        registry.authorize(first, session, &scope()),
        Err(GrantError::Revoked)
    );
    assert_eq!(registry.revoke_session(session), Ok(1));
    assert_eq!(registry.revoke_session(session), Ok(0));
    denied(registry.authorize(first, session, &scope()));
    assert_eq!(
        registry.authorize(second, session, &scope()),
        Err(GrantError::Revoked)
    );
    let state = registry.grants.lock().unwrap();
    assert!(state.active.is_empty());
    assert_eq!(state.revoked.len(), 1);
}
fn denied<T>(result: Result<T, GrantError>) {
    assert_eq!(result.err(), Some(GrantError::Denied));
}

#[test]
fn it_refuses_invalid_grants_and_redacts_debug() {
    let (registry, session, _) = fixture();
    for (scopes, lifetime) in [
        (vec![], Duration::from_secs(1)),
        (vec![scope(); 65], Duration::from_secs(1)),
        (vec![scope()], Duration::ZERO),
    ] {
        assert_eq!(
            registry.grant(session, scopes, lifetime),
            Err(GrantError::InvalidGrant)
        );
    }
    for path in ["", "relative", "/wild*", "/newline\n"] {
        assert_eq!(
            CaptureScope::new("provider", "https://api.example:443", "POST", path),
            Err(GrantError::InvalidScope)
        );
    }
    assert!(!format!("{:?} {registry:?}", scope()).contains("api.invalid"));
}

#[test]
fn it_fails_closed_after_a_delivery_panic() {
    let (registry, session, handle) = fixture();
    let result = std::panic::catch_unwind(|| {
        registry.with_authorization(handle, session, &scope(), || panic!("synthetic fault"))
    });
    assert!(result.is_err());
    assert_eq!(
        registry.authorize(handle, session, &scope()),
        Err(GrantError::Unavailable)
    );
    assert_eq!(
        registry.revoke_session(session),
        Err(GrantError::Unavailable)
    );
}
