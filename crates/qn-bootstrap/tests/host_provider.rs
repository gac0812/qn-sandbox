//! Portable real subprocess fixtures; all credential material is synthetic.
use qn_broker::provider::{CommandProvider, ProviderError};
use qn_infra::process::{CmdSpec, spawn_managed};
use std::{
    error::Error,
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use tracing::{
    Subscriber,
    span::{Attributes, Id, Record},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SECRET: &str = "synthetic-provider-private-value";
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mode = std::env::args().nth(1);
    match mode.as_deref() {
        Some("verify") => verify().await?,
        Some("success") => {
            let mut input = Vec::new();
            std::io::stdin().read_to_end(&mut input)?;
            assert!(input.is_empty());
            assert!(std::env::var_os("QN_PROVIDER_LEAK").is_none());
            assert!(std::env::var_os("HTTP_PROXY").is_none());
            assert_eq!(std::env::var("LANG")?, "C");
            assert_eq!(
                std::env::current_dir()?,
                std::path::PathBuf::from(std::env::var("HOME")?)
            );
            println!("  {SECRET} ");
        }
        Some("failure") => {
            eprintln!("{SECRET}");
            std::process::exit(7);
        }
        Some("large") => print!("{}", "x".repeat(16 * 1024 + 1)),
        Some("large_stderr") => {
            eprint!("{}", "x".repeat(16 * 1024 + 1));
            print!("{SECRET}");
        }
        Some("invalid") => std::io::stdout().write_all(&[255])?,
        Some("control") => std::io::stdout().write_all(b"a\nb")?,
        Some("empty") => println!("  "),
        Some("wait") => {
            if let Some(address) = std::env::args().nth(2) {
                let mut peer = TcpStream::connect(address).await?;
                peer.write_all(b"R").await?;
                while peer.read(&mut [0]).await? != 0 {}
            } else {
                std::future::pending::<()>().await;
            }
        }
        _ => {
            let mut spec = CmdSpec::new(std::env::current_exe()?.to_str().ok_or("UTF-8 required")?)
                .arg("verify")
                .env_clear()
                .env("QN_PROVIDER_LEAK", SECRET)
                .env("HTTP_PROXY", "http://invalid.invalid");
            if let Ok(value) = std::env::var("SystemRoot") {
                spec = spec.env("SystemRoot", value);
            }
            let output = spawn_managed(&spec, &qn_bootstrap::process_policy())?
                .wait(CancellationToken::new())
                .await?;
            assert!(!output.output().stdout.contains(SECRET));
            assert!(!output.output().stderr.contains(SECRET));
            assert!(
                output.output().success,
                "provider fixture failed: {}",
                output.output().stderr
            );
            println!(
                "host provider success, environment, rejection, timeout, cancellation and redaction passed"
            );
        }
    }
    Ok(())
}
fn provider(mode: &str, seconds: u64) -> Result<CommandProvider> {
    let mut provider = CommandProvider::new(
        std::env::current_exe()?.try_into()?,
        std::env::current_dir()?.try_into()?,
        Duration::from_secs(seconds),
    )?
    .arg(mode)
    .environment("LANG", "C")?;
    if let Ok(value) = std::env::var("SystemRoot") {
        provider = provider.environment("SystemRoot", &value)?;
    }
    Ok(provider)
}
async fn verify() -> Result<()> {
    assert!(std::env::var_os("QN_PROVIDER_LEAK").is_some());
    let logs = Arc::new(Mutex::new(String::new()));
    let _guard = tracing::subscriber::set_default(LogCapture(logs.clone()));
    let policy = qn_bootstrap::process_policy();
    let success = provider("success", 5)?;
    let secret = success.capture(&policy, CancellationToken::new()).await?;
    assert_eq!(secret.expose(), SECRET);
    assert!(!format!("{success:?} {secret:?}").contains(SECRET));
    verify_rejections().await?;
    verify_cancel().await?;
    let captured = logs.lock().map_err(|_| "log capture poisoned")?;
    assert!(captured.contains("managed process"));
    assert!(!captured.contains(SECRET));
    Ok(())
}
async fn verify_rejections() -> Result<()> {
    let policy = qn_bootstrap::process_policy();
    for (mode, failed) in [
        ("failure", true),
        ("large", false),
        ("large_stderr", false),
        ("invalid", false),
        ("control", false),
        ("empty", false),
    ] {
        let error = provider(mode, 5)?
            .capture(&policy, CancellationToken::new())
            .await
            .err()
            .ok_or("invalid output unexpectedly accepted")?;
        assert!(!format!("{error:?} {error}").contains(SECRET));
        assert!(
            matches!(&error, ProviderError::Failed) == failed,
            "unexpected rejection: {error}"
        );
        assert!(failed || matches!(error, ProviderError::Output));
    }
    assert!(matches!(
        provider("wait", 1)?
            .capture(&policy, CancellationToken::new())
            .await,
        Err(ProviderError::Timeout)
    ));
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let missing = CommandProvider::new(
        std::env::current_dir()?
            .join("missing-provider")
            .try_into()?,
        std::env::current_dir()?.try_into()?,
        Duration::from_secs(5),
    )?;
    assert!(matches!(
        missing.capture(&policy, cancelled).await,
        Err(ProviderError::Cancelled)
    ));
    assert!(matches!(
        missing.capture(&policy, CancellationToken::new()).await,
        Err(ProviderError::Process(_))
    ));
    assert!(provider("success", 0).is_err());
    assert!(provider("success", 301).is_err());
    assert!(
        provider("success", 5)?
            .environment("GH_TOKEN", SECRET)
            .is_err()
    );
    let home = std::env::current_dir()?.try_into()?;
    assert!(CommandProvider::new("relative".into(), home, Duration::from_secs(1)).is_err());
    Ok(())
}
async fn verify_cancel() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let provider = provider("wait", 10)?.arg(listener.local_addr()?.to_string());
    let cancel = CancellationToken::new();
    let cancel_after_ready = async {
        let (mut peer, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
        timeout(Duration::from_secs(5), peer.read_exact(&mut [0])).await??;
        cancel.cancel();
        let closed = timeout(Duration::from_secs(5), peer.read(&mut [0])).await?;
        assert!(
            matches!(closed, Ok(0))
                || matches!(&closed, Err(e) if matches!(e.kind(),
            std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted))
        );
        Ok::<_, Box<dyn Error>>(())
    };
    let policy = qn_bootstrap::process_policy();
    let (result, ready) = tokio::join!(
        provider.capture(&policy, cancel.clone()),
        cancel_after_ready
    );
    ready?;
    assert!(matches!(result, Err(ProviderError::Cancelled)));
    Ok(())
}
struct LogCapture(Arc<Mutex<String>>);
impl Subscriber for LogCapture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        struct Visitor<'a>(&'a mut String);
        impl tracing::field::Visit for Visitor<'_> {
            fn record_debug(&mut self, _: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                use std::fmt::Write;
                let _ = write!(self.0, "{value:?}");
            }
        }
        if let Ok(mut output) = self.0.lock() {
            event.record(&mut Visitor(&mut output));
        }
    }
}
