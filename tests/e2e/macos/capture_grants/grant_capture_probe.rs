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
    let registry = GrantRegistry::new(NonZeroUsize::MIN);
    let session = SessionId::new();
    let scope = CaptureScope::new("qnprobe", origin, "GET", "/probe")?;
    let handle = registry.grant(session, vec![scope.clone()], Duration::from_secs(30))?;
    let listener = CaptureListener::bind(Duration::from_secs(5)).await?;
    // Parent is a test-owned 0700 directory, never in the sandbox whitelist.
    std::fs::write(path, serde_json::to_vec(listener.binding())?)?;
    println!("ready");
    let mut native = None;
    let mut admitted = 0;
    let mut denied = 0;
    let mut provider_calls = 0;
    for _ in 0..if mode == "race" { 1 } else { 2 } {
        let request = tokio::time::timeout(Duration::from_secs(12), listener.accept()).await??;
        let value: serde_json::Value = serde_json::from_slice(request.payload())?;
        let valid = value["protocol"] == "nono.credential-provider.v1"
            && value["credential_name"] == "qnprobe" && value["route_id"] == "qnprobe"
            && value["request_method"] == "GET" && value["request_path"] == "/probe"
            // nono 0.80.0 reverse capture reports the configured route, not upstream authority.
            && value["request_host"] == "qnprobe" && value["config"] == serde_json::json!({})
            && value["interaction"] == serde_json::json!({});
        let id = value["session_id"].as_str().ok_or(CaptureError::Rejected)?;
        if !valid || id.len() != 16 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CaptureError::Rejected.into());
        }
        if let Some(expected) = &native {
            if expected != id {
                return Err(CaptureError::Rejected.into());
            }
        } else {
            // Synthetic per-session listener is provisioned to exactly one nono instance.
            native = Some(id.to_string());
        }
        if registry.authorize(handle, session, &scope).is_err() {
            denied += 1;
            drop(request);
            continue;
        }
        provider_calls += 1;
        let material = CaptureBytes::new(
            br#"{"material":{"type":"secret","value":"qn-synthetic-credential"}}"#.to_vec(),
        )?;
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
    let expected = if mode == "race" { (0, 1, 1) } else { (1, 1, 1) };
    if (admitted, denied, provider_calls) != expected {
        return Err(CaptureError::Rejected.into());
    }
    println!("admitted={admitted} denied={denied} provider_calls={provider_calls}");
    Ok(())
}
