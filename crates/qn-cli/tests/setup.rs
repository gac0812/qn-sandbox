//! Executable setup consumer; real pinned archive acceptance is opt-in and offline.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, command: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qn"))
        .arg(command)
        .env("QN_HOME", root)
        .env_remove("QN_DEFAULT_TIMEOUT_MS")
        .output()
        .unwrap()
}
fn host_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        _ => None,
    }
}
#[test]
fn it_refuses_missing_engine_without_downloading_or_creating_cache() {
    let root = tempfile::tempdir().unwrap();
    let output = run(root.path(), "doctor");
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains(if host_target().is_some() {
        "qn install"
    } else {
        "WSL2"
    }));
    assert!(!root.path().join("cache").exists());
}

#[test]
#[ignore = "downloads the pinned native release from GitHub into a disposable home"]
fn it_downloads_and_inspects_the_pinned_engine_in_a_fresh_home() {
    host_target().expect("native host supported");
    let root = tempfile::tempdir().unwrap();
    assert!(!root.path().join("cache").exists());
    for operation in ["install", "doctor"] {
        let output = run(root.path(), operation);
        assert!(output.status.success(), "{operation}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains("archive integrity and version verified")
        );
    }
}
#[test]
fn it_rejects_corrupt_cache_and_preserves_existing_binary() {
    let Some(target) = host_target() else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let version = qn_config::NONO_PINNED_VERSION;
    let managed = root.path().join("cache/nono").join(target).join(version);
    fs::create_dir_all(&managed).unwrap();
    fs::write(
        managed.join(format!("nono-v{version}-{target}.tar.gz")),
        b"sensitive-cache-canary",
    )
    .unwrap();
    fs::write(managed.join("nono"), b"unverified-binary-canary").unwrap();
    for operation in ["doctor", "install"] {
        let output = run(root.path(), operation);
        assert!(!output.status.success());
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!text.contains("canary") && !text.contains("integrity and version verified"));
    }
    assert_eq!(
        fs::read(managed.join("nono")).unwrap(),
        b"unverified-binary-canary"
    );
}
#[test]
#[ignore = "requires QN_TEST_NONO_ARCHIVE for the native pinned host release"]
fn it_installs_inspects_refuses_tampering_and_repairs_the_real_engine() {
    let target = host_target().expect("native host supported");
    let root = tempfile::tempdir().unwrap();
    let version = qn_config::NONO_PINNED_VERSION;
    let managed = root.path().join("cache/nono").join(target).join(version);
    fs::create_dir_all(&managed).unwrap();
    fs::copy(
        std::env::var("QN_TEST_NONO_ARCHIVE").unwrap(),
        managed.join(format!("nono-v{version}-{target}.tar.gz")),
    )
    .unwrap();
    let binary = managed.join("nono");
    for operation in ["install", "doctor"] {
        let output = run(root.path(), operation);
        assert!(output.status.success(), "{operation}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains("archive integrity and version verified")
        );
    }
    let json = Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(["doctor", "--json"])
        .env("QN_HOME", root.path())
        .env_remove("QN_DEFAULT_TIMEOUT_MS")
        .output()
        .unwrap();
    assert!(json.status.success(), "{json:?}");
    assert!(json.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["sandbox_engine"], "binary_verified");
    assert_eq!(report["engine"]["version"], version);
    assert_eq!(report["engine"]["runtime_checks"], "not_checked");
    assert_eq!(report["engine"]["binary"], binary.to_str().unwrap());
    let before = fs::metadata(&binary).unwrap().modified().unwrap();
    assert!(run(root.path(), "doctor").status.success());
    assert_eq!(fs::metadata(&binary).unwrap().modified().unwrap(), before);
    let marker = binary.with_extension("invoked");
    fs::write(
        &binary,
        format!("#!/bin/sh\n: > \"$0.invoked\"\nprintf 'nono {version}\\n'\n"),
    )
    .unwrap();
    let refused = run(root.path(), "doctor");
    assert!(!refused.status.success() && !marker.exists());
    // Prove the replacement was executable and could forge the correct version.
    let forged = Command::new(&binary)
        .env_clear()
        .arg("--version")
        .output()
        .unwrap();
    assert!(forged.status.success() && marker.exists());
    assert_eq!(
        String::from_utf8(forged.stdout).unwrap().trim(),
        format!("nono {version}")
    );
    fs::remove_file(&marker).unwrap();
    assert!(run(root.path(), "install").status.success());
    assert!(run(root.path(), "doctor").status.success());
    assert!(!marker.exists());
}

#[test]
fn it_emits_no_partial_json_when_engine_inspection_fails() {
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(["doctor", "--json"])
        .env("QN_HOME", root.path())
        .env_remove("QN_DEFAULT_TIMEOUT_MS")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!root.path().join("cache").exists());
}
