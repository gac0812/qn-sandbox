//! Real portable process fixture: a live grandchild proves group/job termination.
use qn_infra::process::{self, CmdSpec, ProcessEnd, spawn_managed};
use std::{
    error::Error,
    io::{Read, Write},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_util::sync::CancellationToken;

fn command(mode: &str) -> Result<CmdSpec, Box<dyn Error>> {
    let exe = std::env::current_exe()?
        .into_os_string()
        .into_string()
        .map_err(|_| "UTF-8 executable required")?;
    Ok(CmdSpec::new(exe)
        .arg(mode)
        .env_clear()
        .timeout(Duration::from_secs(5))
        .max_output_bytes(64))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("echo") => {
            let mut input = Vec::new();
            std::io::stdin().read_to_end(&mut input)?;
            std::io::stdout().write_all(&input)?;
            std::io::stderr().write_all(&input)?;
        }
        Some("leaf") => {
            let mut stream =
                TcpStream::connect(std::env::args().nth(2).ok_or("address required")?).await?;
            stream.write_all(b"R").await?;
            while stream.read(&mut [0]).await? != 0 {}
        }
        Some("worker") => {
            let addr = std::env::args().nth(2).ok_or("address required")?;
            process::run(&command("leaf")?.arg(addr)).await?;
        }
        _ => verify().await?,
    }
    Ok(())
}

async fn verify() -> Result<(), Box<dyn Error>> {
    let policy = qn_bootstrap::process_policy();
    let mut child = spawn_managed(&command("echo")?, &policy)?;
    let mut input = child.take_stdin().ok_or("stdin missing")?;
    let marker = "synthetic-private-output";
    input.write_all(marker.repeat(64).as_bytes()).await?;
    drop(input);
    let output = child.wait(CancellationToken::new()).await?;
    assert_eq!(output.end, ProcessEnd::Exited);
    assert!(output.output().success);
    assert!(output.output().stdout_truncated && output.output().stderr_truncated);
    assert_eq!(output.output().stdout.len(), 64);
    assert!(!format!("{output:?}").contains(marker));
    assert!(
        spawn_managed(&command("echo")?, &policy)?
            .wait(CancellationToken::new())
            .await?
            .output()
            .success
    );
    let missing = spawn_managed(&CmdSpec::new("").arg(marker), &policy);
    assert!(matches!(
        missing,
        Err(qn_infra::Error::ManagedIo {
            operation: "spawn",
            ..
        })
    ));
    verify_cleanup().await
}

async fn verify_cleanup() -> Result<(), Box<dyn Error>> {
    let policy = qn_bootstrap::process_policy();
    for mode in ["cancel", "timeout", "drop"] {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let child = spawn_managed(
            &command("worker")?.arg(listener.local_addr()?.to_string()),
            &policy,
        )?;
        let (mut peer, _) = timeout(Duration::from_secs(10), listener.accept()).await??;
        let mut ready = [0];
        timeout(Duration::from_secs(3), peer.read_exact(&mut ready)).await??;
        assert_eq!(ready, *b"R");
        match mode {
            "cancel" => {
                let cancel = CancellationToken::new();
                cancel.cancel();
                assert_eq!(child.wait(cancel).await?.end, ProcessEnd::Cancelled);
            }
            "timeout" => assert_eq!(
                child.wait(CancellationToken::new()).await?.end,
                ProcessEnd::TimedOut
            ),
            _ => drop(child),
        }
        let closed = timeout(Duration::from_secs(3), peer.read(&mut [0])).await?;
        assert!(
            matches!(closed, Ok(0))
                || matches!(&closed, Err(e) if matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted)),
            "grandchild must close its socket after {mode}"
        );
    }
    println!("managed process stdin, capture, cancellation, timeout and group cleanup verified");
    Ok(())
}
