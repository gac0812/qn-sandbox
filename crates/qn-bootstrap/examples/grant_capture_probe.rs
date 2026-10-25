//! Synthetic native consumer of the reusable audited capture service (#107).
use qn_broker::{
    AuditedGrant, CaptureScope, GrantRegistry,
    audit::BrokerJournal,
    capture::{CaptureError, CaptureListener},
    provider::CommandProvider,
    service::{CaptureRoute, CaptureService},
};
use qn_core::SessionId;
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::sync::CancellationToken;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if dispatch().await.is_err() {
        eprintln!("synthetic capture service failed");
        std::process::exit(1);
    }
}
async fn dispatch() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "provider") {
        let mut peer = TcpStream::connect(args.get(2).ok_or(CaptureError::Rejected)?).await?;
        peer.write_all(b"R").await?;
        peer.read_exact(&mut [0]).await?;
        print!(
            "{}",
            tokio::fs::read_to_string(args.get(3).ok_or(CaptureError::Rejected)?).await?
        );
        return Ok(());
    }
    tokio::time::timeout(Duration::from_secs(25), serve(&args)).await?
}
async fn serve(args: &[String]) -> Result<()> {
    let path = camino::Utf8Path::new(args.get(1).ok_or(CaptureError::Rejected)?);
    let origin = args.get(2).ok_or(CaptureError::Rejected)?;
    let mode = args.get(3).ok_or(CaptureError::Rejected)?;
    let native = args.get(4).ok_or(CaptureError::Rejected)?;
    let home = path.parent().ok_or(CaptureError::Rejected)?;
    let registry = Arc::new(GrantRegistry::new(NonZeroUsize::MIN));
    let session = SessionId::try_from(args.get(5).ok_or(CaptureError::Rejected)?.as_str())?;
    let scope = CaptureScope::new("qnprobe", origin, "GET", "/probe")?;
    let listener = CaptureListener::bind(Duration::from_secs(5)).await?;
    let control = TcpListener::bind("127.0.0.1:0").await?;
    let provider_ready = TcpListener::bind("127.0.0.1:0").await?;
    let provider = if mode == "race" {
        CommandProvider::new(
            std::env::current_exe()?.try_into()?,
            home.to_owned(),
            Duration::from_secs(5),
        )?
        .arg("provider")
        .arg(provider_ready.local_addr()?.to_string())
        .arg(home.join("provider-secret").as_str())
    } else {
        CommandProvider::new("/bin/cat".into(), home.to_owned(), Duration::from_secs(3))?
            .arg(home.join("provider-secret").as_str())
    };
    let audit_file = tempfile::NamedTempFile::new_in(home)?;
    let audit_path = home.join("broker-audit.jsonl");
    let audit_file = audit_file.persist_noclobber(&audit_path)?;
    let capacity = if mode == "audit_failure" {
        NonZeroU32::new(2).ok_or(CaptureError::Rejected)?
    } else {
        NonZeroU32::new(32).ok_or(CaptureError::Rejected)?
    };
    let journal = BrokerJournal::new(
        tokio::fs::File::from_std(audit_file),
        session,
        capacity,
        Duration::from_secs(2),
    )
    .await?;
    let grant =
        AuditedGrant::issue(registry, journal, vec![scope], Duration::from_secs(30)).await?;
    let route = CaptureRoute::new(native, "qnprobe", origin, provider)?;
    // These fixtures reside in a private 0700 host directory, not in sandbox grants.
    std::fs::write(path, serde_json::to_vec(listener.binding())?)?;
    std::fs::write(
        home.join("control-address"),
        control.local_addr()?.to_string(),
    )?;
    let service = CaptureService::new(listener, route, grant.clone());
    let cancel = CancellationToken::new();
    let control_flow = async {
        if mode == "race" {
            let (mut peer, _) = tokio::select! { result = provider_ready.accept() => result?, () = cancel.cancelled() => return Ok(()) };
            peer.read_exact(&mut [0]).await?;
            grant.revoke().await?;
            peer.write_all(b"S").await?;
        }
        loop {
            let (mut peer, _) = tokio::select! { result = control.accept() => result?, () = cancel.cancelled() => return Ok(()) };
            let mut command = [0];
            peer.read_exact(&mut command).await?;
            if command == [b'R'] {
                grant.revoke().await?;
            }
            if command == [b'Q'] {
                cancel.cancel();
            }
            peer.write_all(b"A").await?;
        }
    };
    println!("ready");
    let policy = qn_bootstrap::process_policy();
    let served = async {
        let result = service.run(&policy, cancel.clone()).await;
        cancel.cancel();
        result
    };
    let (outcome, controlled): (_, Result<()>) = tokio::join!(served, control_flow);
    controlled?;
    if mode == "audit_failure" {
        assert!(outcome.is_err());
    } else {
        outcome?;
    }
    println!("owned capture closed; audit recorded");
    Ok(())
}
