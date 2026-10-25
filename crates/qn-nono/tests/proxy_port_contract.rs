//! Opt-in macOS arm64 fixed-port regression against the pinned binary (#137).
#![allow(clippy::unwrap_used, clippy::expect_used)]
use camino::{Utf8Path, Utf8PathBuf};
use qn_core::SessionId;
use qn_infra::process::{self, CmdSpec};
use qn_nono::{NonoArgs, RunArgs, RunMode, launch::LaunchBinding};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, io::ErrorKind, net::TcpListener, num::NonZeroU16, time::Duration};

fn environment(command: CmdSpec, root: &Utf8Path) -> CmdSpec {
    let mut command = command
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .cwd(root.join("project"))
        .timeout(Duration::from_secs(12))
        .max_output_bytes(1 << 20)
        .label("synthetic native proxy port contract");
    for (key, directory) in [
        ("HOME", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
        ("TMPDIR", "tmp"),
    ] {
        command = command.env(key, root.join(directory).as_str());
    }
    command
}

#[tokio::test]
#[ignore = "requires verified arm64 QN_TEST_NONO and private QN_TEST_PROFILE_PARENT on macOS"]
async fn it_binds_the_exact_proxy_port_or_refuses_before_child_start() {
    assert_eq!(std::env::consts::OS, "macos");
    assert_eq!(std::env::consts::ARCH, "aarch64");
    let binary = Utf8PathBuf::from(std::env::var("QN_TEST_NONO").unwrap());
    assert!(binary.is_absolute());
    assert_eq!(
        hex::encode(Sha256::digest(fs::read(&binary).unwrap())),
        "e95479a2ef64b19e3245a90a60024bb248e96e8ec234ffeb25e062ddd45b1417"
    );
    qn_nono::verify_version(&binary).await.unwrap();
    for occupied in [false, true] {
        case(&binary, occupied).await;
    }
}

async fn case(binary: &Utf8Path, occupied: bool) {
    let parent = std::env::var("QN_TEST_PROFILE_PARENT").unwrap();
    let directory = tempfile::tempdir_in(parent).unwrap();
    let root = Utf8Path::from_path(directory.path()).unwrap();
    for name in ["home", "config", "state", "cache", "tmp", "project"] {
        fs::create_dir(root.join(name)).unwrap();
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = NonZeroU16::new(listener.local_addr().unwrap().port()).unwrap();
    listener.set_nonblocking(true).unwrap();
    // Releasing an available-port probe is not ownership. Native must bind or fail.
    let listener = occupied.then_some(listener);
    let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    upstream.set_nonblocking(true).unwrap();
    let profile = root.join("profile.json");
    fs::write(
        &profile,
        json!({
            "meta": {"name": "qn-port-contract", "version": "1"}, "extends": "default",
            "filesystem": {"allow": [root.join("project").as_str()]},
            "security": {"capability_elevation": false, "signal_mode": "isolated", "process_info_mode": "isolated"},
            "environment": {"allow_vars": ["PATH", "HOME", "LANG"]},
            "network": {"credentials": ["qnprobe"], "custom_credentials": {"qnprobe": {
                "upstream": format!("http://{}", upstream.local_addr().unwrap()),
                "credential_key": "cmd://qnprobe", "env_var": "QNPROBE_TOKEN",
                "inject_header": "Authorization", "credential_format": "Bearer {}",
                "endpoint_rules": [{"method": "GET", "path": "/probe"}]
            }}},
            "credential_capture": {"qnprobe": {"provider": {"command": ["/usr/bin/false"], "config": {}},
                "stdin": "request_json", "cache_ttl_secs": 0, "timeout_secs": 3}}
        }).to_string(),
    ).unwrap();
    let script = root.join("project/client.rb");
    fs::write(
        &script,
        "File.write('child-started', 'yes'); puts ENV.fetch('QNPROBE_BASE_URL')",
    )
    .unwrap();
    let validation = NonoArgs::profile_validate(&profile).unwrap();
    let output = process::run(&environment(
        CmdSpec::new(binary.as_str())
            .args(validation.as_slice().iter().cloned())
            .env_clear(),
        root,
    ))
    .await
    .unwrap();
    assert!(
        output.success,
        "native profile validation failed; output redacted"
    );
    let binding = LaunchBinding::reserve(root, SessionId::new()).unwrap();
    let command = binding
        .command_with_proxy_port(
            binary,
            RunArgs {
                profile,
                name: binding.session(),
                workdir: root.join("project"),
                mode: RunMode::Foreground,
                command: vec!["/usr/bin/ruby".into(), script.into_string()],
            },
            port,
        )
        .unwrap();
    let output = process::run(&environment(command, root)).await.unwrap();
    assert!(!output.stdout_truncated && !output.stderr_truncated);
    if let Some(listener) = listener {
        assert!(
            !output.success && output.exit_code.is_some(),
            "occupied port must refuse startup"
        );
        assert!(
            !root.join("project/child-started").exists(),
            "child ran after bind failure"
        );
        assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
    } else {
        assert!(
            output.success,
            "available port must start; diagnostics redacted"
        );
        assert_eq!(
            fs::read(root.join("project/child-started")).unwrap(),
            b"yes"
        );
        assert_eq!(
            output.stdout.trim(),
            format!("http://127.0.0.1:{port}/qnprobe")
        );
    }
    assert_eq!(upstream.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
}
