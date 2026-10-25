//! `qn-wsl::health` 集成测试（cfg windows 门：Linux CI 只编译不执行）。
#![allow(clippy::unwrap_used, clippy::expect_used)] // spec 02 §7
#![cfg(target_os = "windows")]

use qn_wsl::health::{self, ProbeId, ProbeStatus};

#[tokio::test]
async fn it_returns_five_reports_in_fixed_order() {
    // 结构断言在任何环境都成立（CI windows runner 可能没有受管发行版，
    // 探测结果允许 Fail，但数量/顺序/枚举必须稳定——呈现层据此对位）。
    let reports = health::run_all().await;
    assert_eq!(reports.len(), 5, "exactly five probes");
    let expected = [
        ProbeId::Wsl,
        ProbeId::Distro,
        ProbeId::Kernel,
        ProbeId::Nono,
        ProbeId::LandlockAbi,
    ];
    for (report, want) in reports.iter().zip(expected) {
        assert_eq!(report.id, want);
        assert!(
            !report.detail.is_empty(),
            "detail must always carry facts: {:?}",
            report.id
        );
        if report.status == ProbeStatus::Fail {
            assert!(
                report.remediation.as_deref().is_some_and(|r| !r.is_empty()),
                "every Fail must ship a remediation: {:?}",
                report.id
            );
        }
    }
}

#[tokio::test]
async fn it_uses_the_shared_wsl_runtime_without_hanging() {
    // 两次全量探测均应在远小于总超时的时间内完成（每项 ≤30s，5 项上限 2 分钟；
    // 实际 wsl 调用秒级）。此用例钉住"探测不会挂死 doctor"。
    let started = std::time::Instant::now();
    let _ = health::run_all().await;
    assert!(
        started.elapsed() < std::time::Duration::from_secs(90),
        "probes must stay interactive, took {:?}",
        started.elapsed()
    );
}
