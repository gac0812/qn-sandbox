//! Opt-in native process contract; synthetic disposable state only (#81).
#![cfg(unix)]

/// A positive diagnostic of a known limitation, not a cleanup-success assertion.
#[tokio::test]
#[ignore = "requires verified nono, private synthetic root and Ruby; demonstrates #92"]
async fn it_demonstrates_parent_exit_is_not_descendant_cleanup() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    qn_nono::verify_version(&fixture.binary).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let profile = fixture.root.join("profile.json");
    fs::write(&profile, json!({"meta":{"name":"qn-orphan-witness","version":"1"},
        "extends":"default", "filesystem":{"allow":[fixture.root.join("project").as_str()]},
        "network":{"block":true,"open_port":[port]},
        "security":{"capability_elevation":false,"signal_mode":"isolated","process_info_mode":"isolated"},
        "environment":{"allow_vars":["HOME","PATH","LANG"]}}).to_string())?;
    fixture
        .invoke(NonoArgs::profile_validate(&profile)?)
        .await?;
    let script = fixture.root.join("project/orphan.rb");
    fs::write(
        &script,
        r"require 'socket'
read_end, write_end = IO.pipe
fork do
  read_end.close
  STDIN.reopen('/dev/null', 'r')
  STDOUT.reopen('/dev/null', 'w')
  STDERR.reopen('/dev/null', 'w')
  socket = TCPSocket.new('127.0.0.1', Integer(ARGV.fetch(0)))
  socket.write('R')
  write_end.write('R')
  write_end.close
  if IO.select([socket], nil, nil, 12) && socket.read(1) == 'P'
    socket.write('A')
  end
  socket.close
  exit! 0
end
write_end.close
raise 'not ready' unless read_end.read(1) == 'R'
read_end.close
",
    )?;
    let args = NonoArgs::run(RunArgs {
        profile,
        name: qn_core::SessionId::new(),
        workdir: fixture.root.join("project"),
        mode: RunMode::Foreground,
        command: vec!["/usr/bin/ruby".into(), script.to_string(), port.to_string()],
    })?;
    let process = spawn_managed(&fixture.command(&args), &GroupPolicy)?;
    let (mut peer, _) = timeout(Duration::from_secs(8), listener.accept()).await??;
    let mut byte = [0];
    timeout(Duration::from_secs(2), peer.read_exact(&mut byte)).await??;
    assert_eq!(byte, *b"R", "child must be ready before waiting for parent");
    let output = timeout(
        Duration::from_secs(3),
        process.wait(CancellationToken::new()),
    )
    .await??;
    assert_eq!(
        output.end,
        ProcessEnd::Exited,
        "parent reports normal completion"
    );
    assert!(output.output().success, "parent reports exit 0");
    // The probe is sent only after wait returns. Its reply proves continuing execution.
    timeout(Duration::from_secs(2), peer.write_all(b"P")).await??;
    timeout(Duration::from_secs(2), peer.read_exact(&mut byte)).await??;
    assert_eq!(
        byte, *b"A",
        "descendant executes after successful parent completion"
    );
    assert_eq!(
        timeout(Duration::from_secs(2), peer.read(&mut byte)).await??,
        0,
        "witness closes and exits after replying; no numeric PID cleanup"
    );
    Ok(())
}

use camino::Utf8PathBuf;
use process_wrap::tokio::{ProcessGroup, TokioCommandWrap};
use qn_infra::process::{CmdSpec, ProcessEnd, ProcessPolicy, TerminationSafety, spawn_managed};
use qn_nono::{NonoArgs, RunArgs, RunMode};
use serde_json::json;
use std::{
    error::Error,
    fs,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

struct GroupPolicy;
impl ProcessPolicy for GroupPolicy {
    fn configure(&self, command: &mut TokioCommandWrap) -> TerminationSafety {
        command.wrap(ProcessGroup::leader());
        TerminationSafety::WhileParentOwned
    }
}
struct Fixture {
    _directory: tempfile::TempDir,
    root: Utf8PathBuf,
    binary: Utf8PathBuf,
}
impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let parent = std::env::var("QN_TEST_PROFILE_PARENT")?;
        let directory = tempfile::tempdir_in(parent)?;
        let root = Utf8PathBuf::from_path_buf(directory.path().canonicalize()?)
            .map_err(|_| "UTF-8 root required")?;
        for dir in ["home", "config", "state", "cache", "tmp", "project"] {
            fs::create_dir(root.join(dir))?;
        }
        let binary = Utf8PathBuf::from(std::env::var("QN_TEST_NONO")?);
        if !binary.is_absolute() {
            return Err("absolute verified nono required".into());
        }
        Ok(Self {
            _directory: directory,
            root,
            binary,
        })
    }
    fn command(&self, args: &NonoArgs) -> CmdSpec {
        let mut command = CmdSpec::new(self.binary.as_str())
            .args(args.as_slice().iter().cloned())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .cwd(self.root.join("project"))
            .timeout(Duration::from_secs(20));
        for (key, dir) in [
            ("HOME", "home"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_STATE_HOME", "state"),
            ("XDG_CACHE_HOME", "cache"),
            ("TMPDIR", "tmp"),
        ] {
            command = command.env(key, self.root.join(dir).as_str());
        }
        command
    }
    async fn invoke(&self, args: NonoArgs) -> Result<(), Box<dyn Error>> {
        let output = spawn_managed(&self.command(&args), &GroupPolicy)?
            .wait(CancellationToken::new())
            .await?;
        if !output.output().success || output.end != ProcessEnd::Exited {
            return Err("native prerequisite failed; output redacted".into());
        }
        Ok(())
    }
}

#[tokio::test]
#[ignore = "requires verified QN_TEST_NONO, private QN_TEST_PROFILE_PARENT and /usr/bin/ruby"]
async fn it_checks_real_foreground_termination_and_normal_exit() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    qn_nono::verify_version(&fixture.binary).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let profile = fixture.root.join("profile.json");
    fs::write(&profile, json!({"meta":{"name":"qn-managed-contract","version":"1"},
        "extends":"default", "filesystem":{"allow":[fixture.root.join("project").as_str()]},
        "network":{"block":true,"open_port":[port]},
        "security":{"capability_elevation":false,"signal_mode":"isolated","process_info_mode":"isolated"},
        "environment":{"allow_vars":["HOME","PATH","LANG"]}}).to_string())?;
    fixture
        .invoke(NonoArgs::profile_validate(&profile)?)
        .await?;
    let run = |command| {
        NonoArgs::run(RunArgs {
            profile: profile.clone(),
            name: qn_core::SessionId::new(),
            workdir: fixture.root.join("project"),
            mode: RunMode::Foreground,
            command,
        })
    };
    fixture
        .invoke(run(vec!["/bin/echo".into(), "normal-exit".into()])?)
        .await?;
    let script = fixture.root.join("project/child.rb");
    fs::write(
        &script,
        r"require 'socket'
pid = fork do
  socket = TCPSocket.new('127.0.0.1', Integer(ARGV.fetch(0)))
  socket.write('R')
  IO.select([socket], nil, nil, 12)
  socket.close
end
Process.wait(pid)
",
    )?;
    let args = run(vec![
        "/usr/bin/ruby".into(),
        script.to_string(),
        port.to_string(),
    ])?;
    for end in [ProcessEnd::Cancelled, ProcessEnd::TimedOut] {
        let command = fixture.command(&args).timeout(Duration::from_secs(5));
        verify_termination(&listener, command, end).await?;
    }
    Ok(())
}

async fn verify_termination(
    listener: &TcpListener,
    command: CmdSpec,
    end: ProcessEnd,
) -> Result<(), Box<dyn Error>> {
    let process = spawn_managed(&command, &GroupPolicy)?;
    let (mut peer, _) = match timeout(Duration::from_secs(8), listener.accept()).await {
        Ok(result) => result?,
        Err(error) => {
            let cancel = CancellationToken::new();
            cancel.cancel();
            let result = process.wait(cancel).await;
            return Err(format!("readiness failed: {error}; cleanup result {result:?}").into());
        }
    };
    let mut ready = [0];
    timeout(Duration::from_secs(2), peer.read_exact(&mut ready)).await??;
    assert_eq!(ready, *b"R", "grandchild readiness witness");
    let observed = Instant::now();
    let cancel = CancellationToken::new();
    if end == ProcessEnd::Cancelled {
        cancel.cancel();
    }
    let result = process.wait(cancel).await;
    let closed = timeout(Duration::from_secs(2), peer.read(&mut [0])).await;
    let stopped = observed.elapsed() < Duration::from_secs(10)
        && (matches!(closed, Ok(Ok(0)))
            || matches!(&closed, Ok(Err(e)) if matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted)));
    if !stopped {
        // Stop this exact connected witness; never signal an unowned numeric PID.
        peer.write_all(b"S").await?;
        let _ = timeout(Duration::from_secs(3), peer.read(&mut [0])).await;
    }
    assert!(
        stopped,
        "nono descendant outlived managed {end:?}; result: {result:?}"
    );
    assert_eq!(result?.end, end);
    Ok(())
}
