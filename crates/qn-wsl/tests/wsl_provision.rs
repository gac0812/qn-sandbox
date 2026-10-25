//! `qn-wsl::distro` 集成测试：**真实供给**，仅本机 opt-in（`QN_WSL_PROVISION_IT=1`）。
//!
//! CI runner 不适合安装发行版（网络/时长/隔离），默认跳过；本机运行会：
//! ① 从种子供给受管发行版 `qn-sandbox`（分钟级）；② 立即二次调用验证幂等 no-op；
//! ③ 把 health 的 Distro 探测拉起来（同机 E2E 联动证据）。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(all(target_os = "windows", feature = "provision-it"))]

use qn_wsl::distro::{self, ProvisionOutcome};
use qn_wsl::health::{self, ProbeStatus};

fn enabled() -> bool {
    std::env::var("QN_WSL_PROVISION_IT")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[tokio::test]
async fn it_provisions_idempotently_and_flips_the_health_probe() {
    if !enabled() {
        eprintln!("skipping: set QN_WSL_PROVISION_IT=1 to run the real provisioning");
        return;
    }
    let input = distro::default_layout().expect("default layout");
    // 预清理：保证从"缺失"开始（测试可重复）。
    let _ = qn_infra::process::run_checked(
        &qn_infra::process::CmdSpec::new("wsl")
            .args(["--unregister", qn_wsl::MANAGED_DISTRO_NAME])
            .timeout(std::time::Duration::from_secs(30))
            .label("cleanup managed distro"),
    )
    .await;

    let first = distro::ensure_present(&input)
        .await
        .expect("first provisioning must succeed");
    assert!(
        matches!(first, ProvisionOutcome::Provisioned { .. }),
        "from-missing run must provision, got {first:?}"
    );

    let second = distro::ensure_present(&input)
        .await
        .expect("second call must succeed");
    assert_eq!(
        second,
        ProvisionOutcome::AlreadyPresent,
        "must be an idempotent no-op"
    );

    // 同机联动：health 的 Distro 探测应转 Ok。
    let reports = health::run_all().await;
    let distro_report = reports
        .iter()
        .find(|r| r.id == health::ProbeId::Distro)
        .expect("distro probe present");
    assert_eq!(
        distro_report.status,
        ProbeStatus::Ok,
        "health Distro probe must flip to Ok after provisioning: {}",
        distro_report.detail
    );
}
