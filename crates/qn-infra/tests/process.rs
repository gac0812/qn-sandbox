//! qn-infra::process 集成测试。
//!
//! 测试用宿主 shell（Windows `cmd /c`、Unix `sh -c`）构造子进程——
//! 这是测试脚手架；生产代码经 CmdSpec 的 argv 数组约束无 shell 路径。
#![allow(clippy::unwrap_used, clippy::expect_used)] // spec 02 §7：测试允许 unwrap/expect

use std::time::Duration;

use qn_infra::process::{self, CmdSpec};
use qn_infra::Error;
use tokio_util::sync::CancellationToken;

/// 平台相关的“跑一条 shell 片段”脚手架（仅测试）。
fn shell(snippet: &str) -> CmdSpec {
    if cfg!(windows) {
        CmdSpec::new("cmd").args(["/C", snippet])
    } else {
        CmdSpec::new("sh").args(["-c", snippet])
    }
}

#[tokio::test]
async fn it_captures_stdout_and_success() {
    let spec = shell(if cfg!(windows) { "echo hello" } else { "echo hello" });
    let out = process::run(&spec).await.expect("echo must run");
    assert!(out.success, "echo should exit 0");
    assert_eq!(out.exit_code, Some(0));
    assert!(
        out.stdout.trim_end().contains("hello"),
        "stdout should contain hello, got: {:?}",
        out.stdout
    );
}

#[tokio::test]
async fn it_reports_nonzero_exit_code_in_outcome() {
    let spec = shell("exit 7");
    let out = process::run(&spec).await.expect("exit must run");
    assert!(!out.success, "exit 7 is a failure");
    assert_eq!(out.exit_code, Some(7));
}

#[tokio::test]
async fn it_maps_nonzero_to_failed_error_with_actionable_message() {
    let spec = shell("exit 7").label("probe-seven");
    let err = process::run_checked(&spec).await.expect_err("exit 7 must be Err");
    match err {
        Error::Failed {
            label,
            code,
            stderr_summary: _,
        } => {
            assert_eq!(label, "probe-seven", "error must carry the label");
            assert_eq!(code, 7);
        }
        other => panic!("expected Error::Failed, got: {other:?}"),
    }
}

#[tokio::test]
async fn it_times_out_and_kills_the_child() {
    // 挂起 ~30s 的子进程；超时 400ms 必须先到并 kill。
    let snippet = if cfg!(windows) {
        "ping -n 30 127.0.0.1 > nul"
    } else {
        "sleep 30"
    };
    let spec = shell(snippet).timeout(Duration::from_millis(400)).label("hang");
    let started = std::time::Instant::now();
    let err = process::run(&spec).await.expect_err("must time out");
    match err {
        Error::TimedOut {
            label,
            timeout_secs,
        } => {
            assert_eq!(label, "hang");
            assert_eq!(timeout_secs, 0, "sub-second timeout is allowed");
        }
        other => panic!("expected Error::TimedOut, got: {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "child must be killed promptly, not awaited to completion"
    );
}

#[tokio::test]
async fn it_cancels_immediately_via_token() {
    let snippet = if cfg!(windows) {
        "ping -n 30 127.0.0.1 > nul"
    } else {
        "sleep 30"
    };
    let spec = shell(snippet).timeout(Duration::from_secs(60)).label("cancellable");
    let cancel = CancellationToken::new();
    let spec_clone = spec.clone();
    let handle = tokio::spawn(async move { process::run_with_cancel(&spec_clone, cancel.clone()).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    cancel.cancel();
    let started = std::time::Instant::now();
    let result = handle.await.expect("join must succeed");
    let err = result.expect_err("must be cancelled");
    assert!(
        matches!(err, Error::Cancelled { label } if label == "cancellable"),
        "expected Error::Cancelled, got: {err:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "cancellation must take effect immediately"
    );
}

#[tokio::test]
async fn it_truncates_output_at_limit_and_flags_it() {
    // 输出远超上限（100 KiB vs 1 KiB 上限）。
    let snippet = if cfg!(windows) {
        // cmd 无 seq，用 ping -n 生成行数不可控，改用 certutil 不行——直接循环 echo。
        "@for /L %i in (1,1,200) do @echo 01234567890123456789012345678901234567890123456789012345678"
    } else {
        "for i in $(seq 1 200); do echo 01234567890123456789012345678901234567890123456789012345678; done"
    };
    let spec = shell(snippet).max_output_bytes(1024).label("flood");
    let out = process::run(&spec).await.expect("flood must run");
    assert!(out.success, "snippet itself must succeed");
    assert!(out.stdout_truncated, "stdout must be flagged truncated");
    assert!(
        out.stdout.len() <= 2048,
        "captured stdout must be near the limit, got {} bytes",
        out.stdout.len()
    );
}

#[tokio::test]
async fn it_passes_env_overrides_to_child() {
    let snippet = if cfg!(windows) {
        "echo %QN_TEST_VAR%"
    } else {
        "echo $QN_TEST_VAR"
    };
    let spec = shell(snippet).env("QN_TEST_VAR", "qn-value-42");
    let out = process::run(&spec).await.expect("echo must run");
    assert!(
        out.stdout.trim_end().contains("qn-value-42"),
        "env override must reach the child, got: {:?}",
        out.stdout
    );
}
