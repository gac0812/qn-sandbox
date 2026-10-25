//! Real provider/socket fixtures; every credential is synthetic (#107).
use qn_broker::{
    AuditedGrant, CaptureScope, GrantRegistry,
    audit::BrokerJournal,
    capture::{CaptureBinding, CaptureBytes, CaptureListener},
    provider::CommandProvider,
    service::{CaptureRoute, CaptureService},
};
use qn_core::SessionId;
use std::{
    error::Error,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_util::sync::CancellationToken;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SECRET: &str = "synthetic-service-only-secret";
const NATIVE: &str = "0123456789abcdef";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    if let Some(address) = std::env::args().nth(1) {
        let mut peer = TcpStream::connect(address).await?;
        peer.write_all(b"R").await?;
        let mut command = [0];
        if peer.read_exact(&mut command).await.is_ok() && command == [b'S'] {
            print!("{SECRET}");
        }
        return Ok(());
    }
    for mode in [
        "normal",
        "race",
        "recovery",
        "audit_before",
        "audit_after",
        "revoke_audit",
        "cancel",
        "deadline",
        "idle",
        "handshake",
    ] {
        timeout(Duration::from_secs(15), verify(mode)).await??;
        println!("capture service {mode}: PASS");
    }
    Ok(())
}

fn request(native: &str, path: &str) -> Result<CaptureBytes> {
    Ok(CaptureBytes::new(serde_json::to_vec(
        &serde_json::json!({
            "protocol":"nono.credential-provider.v1", "session_id":native,
            "credential_name":"probe", "route_id":"probe", "request_host":"probe",
            "cache_scope":"probe", "request_method":"GET", "request_path":path,
            "config":{}, "interaction":{}
        }),
    )?)?)
}
async fn exchange(binding: &CaptureBinding, native: &str, path: &str) -> Result<CaptureBytes> {
    Ok(binding
        .exchange(request(native, path)?, Duration::from_secs(8))
        .await?)
}
fn events(path: &std::path::Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    assert!(!text.contains(SECRET));
    text.lines()
        .map(|line| {
            let row: serde_json::Value = serde_json::from_str(line)?;
            Ok(row["event"].as_str().ok_or("missing event")?.to_owned())
        })
        .collect()
}

async fn verify(mode: &str) -> Result<()> {
    let provider_ready = TcpListener::bind("127.0.0.1:0").await?;
    let mut provider = CommandProvider::new(
        std::env::current_exe()?.try_into()?,
        std::env::current_dir()?.try_into()?,
        Duration::from_secs(10),
    )?
    .arg(provider_ready.local_addr()?.to_string());
    if let Ok(value) = std::env::var("SystemRoot") {
        provider = provider.environment("SystemRoot", &value)?;
    }
    let session = SessionId::new();
    let scope = CaptureScope::new("probe", "https://example.test:443", "GET", "/probe")?;
    let registry = Arc::new(GrantRegistry::new(NonZeroUsize::MIN));
    let lifetime = if mode == "deadline" {
        Duration::from_millis(200)
    } else {
        Duration::from_secs(8)
    };
    let listener = CaptureListener::bind(lifetime).await?;
    let wire = serde_json::to_vec(listener.binding())?;
    let binding: CaptureBinding = serde_json::from_slice(&wire)?;
    let binding_value: serde_json::Value = serde_json::from_slice(&wire)?;
    let address = binding_value["address"]
        .as_str()
        .ok_or("missing address")?
        .to_owned();
    let journal_file = tempfile::NamedTempFile::new()?;
    let file = journal_file.reopen()?;
    let capacity = if mode == "audit_before" {
        NonZeroU32::MIN
    } else if matches!(mode, "audit_after" | "revoke_audit") {
        NonZeroU32::new(2).ok_or("capacity")?
    } else {
        NonZeroU32::new(32).ok_or("capacity")?
    };
    let journal = BrokerJournal::new(
        tokio::fs::File::from_std(file),
        session,
        capacity,
        Duration::from_secs(2),
    )
    .await?;
    let grant = AuditedGrant::issue(
        registry,
        journal,
        vec![scope.clone()],
        Duration::from_secs(30),
    )
    .await?;
    let route = CaptureRoute::new(NATIVE, "probe", "https://example.test:443", provider)?;
    let service = CaptureService::new(listener, route, grant.clone());
    let cancel = CancellationToken::new();
    let fixture = Fixture {
        provider_ready,
        grant,
        binding,
        address,
        cancel,
    };
    let driver = fixture.drive(mode);
    let policy = qn_bootstrap::process_policy();
    let (served, checked) = tokio::join!(service.run(&policy, fixture.cancel.clone()), driver);
    checked?;
    if mode.starts_with("audit_") || mode == "revoke_audit" {
        assert!(served.is_err());
    } else {
        served?;
    }
    assert!(fixture.grant.authorize(&scope).is_err(), "shutdown revokes");
    assert!(
        TcpStream::connect(&fixture.address).await.is_err(),
        "listener closed"
    );
    verify_events(mode, journal_file.path())?;
    // A queued connection proves unexpected provider IO; poll once, no sleeps.
    let mut pending = Box::pin(fixture.provider_ready.accept());
    std::future::poll_fn(|cx| {
        use std::future::Future;
        assert!(
            pending.as_mut().poll(cx).is_pending(),
            "unexpected provider invocation"
        );
        std::task::Poll::Ready(())
    })
    .await;
    Ok(())
}

struct Fixture {
    provider_ready: TcpListener,
    grant: AuditedGrant,
    binding: CaptureBinding,
    address: String,
    cancel: CancellationToken,
}
impl Fixture {
    async fn drive(&self, mode: &str) -> Result<()> {
        if matches!(mode, "idle" | "handshake") {
            let peer = if mode == "handshake" {
                Some(TcpStream::connect(&self.address).await?)
            } else {
                None
            };
            self.cancel.cancel();
            drop(peer);
            return Ok::<_, Box<dyn Error>>(());
        }
        if mode == "normal" {
            // A raw malformed peer and two authenticated denials must not stop serving.
            let mut peer = TcpStream::connect(&self.address).await?;
            peer.write_all(&[0, 0, 0, 0]).await?;
            drop(peer);
            assert!(
                exchange(&self.binding, "fedcba9876543210", "/probe")
                    .await
                    .is_err()
            );
            assert!(exchange(&self.binding, NATIVE, "/wrong").await.is_err());
        }
        if mode == "recovery" {
            let fail_provider = async {
                let (mut peer, _) = self.provider_ready.accept().await?;
                peer.read_exact(&mut [0]).await?;
                peer.write_all(b"F").await?;
                Ok::<_, Box<dyn Error>>(())
            };
            let (reply, failed) =
                tokio::join!(exchange(&self.binding, NATIVE, "/probe"), fail_provider);
            failed?;
            assert!(reply.is_err());
        }
        let interaction = async {
            if mode == "audit_before" {
                return Ok(());
            }
            let (mut peer, _) = self.provider_ready.accept().await?;
            let mut ready = [0];
            peer.read_exact(&mut ready).await?;
            assert_eq!(ready, [b'R']);
            if mode == "race" {
                self.grant.revoke().await?;
            }
            if matches!(mode, "cancel" | "deadline" | "revoke_audit") {
                if mode == "revoke_audit" {
                    assert!(self.grant.revoke().await.is_err());
                }
                if mode == "cancel" {
                    self.cancel.cancel();
                }
                let closed = peer.read(&mut [0]).await;
                assert!(
                    matches!(closed, Ok(0))
                        || matches!(closed, Err(ref error) if matches!(error.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted))
                );
            } else {
                peer.write_all(b"S").await?;
            }
            Ok::<_, Box<dyn Error>>(())
        };
        let (reply, interacted) =
            tokio::join!(exchange(&self.binding, NATIVE, "/probe"), interaction);
        interacted?;
        if matches!(mode, "normal" | "recovery") {
            let reply = reply?;
            let decoded: serde_json::Value = serde_json::from_slice(reply.as_bytes())?;
            assert_eq!(decoded["material"]["value"], SECRET);
            self.grant.revoke().await?;
            assert!(exchange(&self.binding, NATIVE, "/probe").await.is_err());
        } else {
            assert!(reply.is_err(), "{mode} must not deliver");
        }
        self.cancel.cancel();
        Ok(())
    }
}

fn verify_events(mode: &str, path: &std::path::Path) -> Result<()> {
    let recorded = events(path)?;
    let expected: &[&str] = match mode {
        "normal" => &[
            "grant_issued",
            "failed",
            "denied",
            "denied",
            "provider_requested",
            "delivery_prepared",
            "reply_written",
            "grant_revoked",
            "denied_after_revoke",
            "service_closed",
        ],
        "race" => &[
            "grant_issued",
            "provider_requested",
            "grant_revoked",
            "delivery_prepared",
            "denied_after_revoke",
            "service_closed",
        ],
        "recovery" => &[
            "grant_issued",
            "provider_requested",
            "failed",
            "provider_requested",
            "delivery_prepared",
            "reply_written",
            "grant_revoked",
            "denied_after_revoke",
            "service_closed",
        ],
        "audit_before" => &["grant_issued"],
        "audit_after" | "revoke_audit" => &["grant_issued", "provider_requested"],
        "cancel" | "deadline" => &[
            "grant_issued",
            "provider_requested",
            "failed",
            "grant_revoked",
            "service_closed",
        ],
        _ => &["grant_issued", "grant_revoked", "service_closed"],
    };
    if matches!(mode, "normal" | "recovery") {
        let mut admitted_before_revoke = expected.to_vec();
        let written = expected
            .iter()
            .position(|event| *event == "reply_written")
            .ok_or("missing reply event")?;
        admitted_before_revoke.swap(written, written + 1);
        assert!(
            recorded == expected || recorded == admitted_before_revoke,
            "{mode} audit sequence: {recorded:?}"
        );
    } else {
        assert_eq!(recorded, expected, "{mode} audit sequence");
    }
    Ok(())
}
