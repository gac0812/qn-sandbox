//! Synthetic integration consumer for #96; not a production credential backend.
use qn_broker::{
    CaptureScope, GrantRegistry,
    capture::{CaptureBytes, CaptureError, CaptureListener},
};
use qn_core::SessionId;
use std::{num::NonZeroUsize, time::Duration};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if serve().await.is_err() {
        eprintln!("synthetic host broker verification failed");
        std::process::exit(1);
    }
}
async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).ok_or(CaptureError::Rejected)?;
    let origin = args.get(2).ok_or(CaptureError::Rejected)?;
    let mode = args.get(3).ok_or(CaptureError::Rejected)?;
    let native = args.get(4).ok_or(CaptureError::Rejected)?;
    let identity = qn_broker::identity::CaptureIdentity::new(native, "qnprobe")?;
    let registry = GrantRegistry::new(NonZeroUsize::MIN);
    let session = SessionId::new();
    let scope = CaptureScope::new("qnprobe", origin, "GET", "/probe")?;
    let handle = registry.grant(session, vec![scope.clone()], Duration::from_secs(30))?;
    let listener = CaptureListener::bind(Duration::from_secs(5)).await?;
    // Parent is a test-owned 0700 directory, never in the sandbox whitelist.
    std::fs::write(path, serde_json::to_vec(listener.binding())?)?;
    println!("ready");
    let mut admitted = 0;
    let mut denied = 0;
    let mut provider_calls = 0;
    for _ in 0..if mode == "normal" { 2 } else { 1 } {
        let request = tokio::time::timeout(Duration::from_secs(12), listener.accept()).await??;
        let Ok(metadata) = identity.validate(request.payload()) else {
            denied += 1;
            drop(request);
            continue;
        };
        let scope = CaptureScope::new("qnprobe", origin, metadata.method(), metadata.path())?;
        if registry.authorize(handle, session, &scope).is_err() {
            denied += 1;
            drop(request);
            continue;
        }
        provider_calls += 1;
        let home = camino::Utf8Path::new(path)
            .parent()
            .ok_or(CaptureError::Rejected)?;
        let provider = qn_broker::provider::CommandProvider::new(
            "/bin/cat".into(),
            home.to_path_buf(),
            Duration::from_secs(3),
        )?
        .arg(home.join("provider-secret").as_str());
        let secret = provider
            .capture(
                &qn_bootstrap::process_policy(),
                tokio_util::sync::CancellationToken::new(),
            )
            .await?;
        let material = CaptureBytes::new(serde_json::to_vec(&serde_json::json!({
            "material": {"type": "secret", "value": secret.expose()}
        }))?)?;
        if mode == "race" {
            registry.revoke(handle, session)?;
        }
        match registry.with_authorization(handle, session, &scope, || (request, material)) {
            Ok((request, material)) => {
                admitted += 1;
                registry.revoke(handle, session)?;
                request.reply(material).await?;
            }
            Err(_) => denied += 1,
        }
    }
    let expected = match mode.as_str() {
        "race" => (0, 1, 1),
        "mismatch" => (0, 1, 0),
        _ => (1, 1, 1),
    };
    if (admitted, denied, provider_calls) != expected {
        return Err(CaptureError::Rejected.into());
    }
    println!("admitted={admitted} denied={denied} provider_calls={provider_calls}");
    Ok(())
}
