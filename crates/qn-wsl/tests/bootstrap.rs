//! `qn-wsl::bootstrap` 集成测试：真实发行版快路径，仅本机 opt-in
//! （`QN_WSL_BOOTSTRAP_IT=1`，前置=受管发行版已供给且 nono 已锁定）。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(all(target_os = "windows", feature = "bootstrap-it"))]

use qn_wsl::bootstrap;

fn enabled() -> bool {
    std::env::var("QN_WSL_BOOTSTRAP_IT")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[tokio::test]
async fn it_returns_already_pinned_when_distro_has_locked_nono() {
    if !enabled() {
        eprintln!("skipping: set QN_WSL_BOOTSTRAP_IT=1 (needs a provisioned distro with nono)");
        return;
    }
    let outcome = bootstrap::ensure_pinned().await.expect("fast path");
    assert_eq!(
        outcome,
        bootstrap::BootstrapOutcome::AlreadyPinned,
        "seed-carried nono should hit the fast path"
    );
}
