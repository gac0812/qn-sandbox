//! `qn-wsl::distro` 单元测试（计划构造纯函数；实跑集成见 `tests/wsl_provision.rs`）。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use camino::Utf8PathBuf;
use qn_infra::process::CmdSpec;
use std::time::Duration;

fn input() -> ProvisionInput {
    ProvisionInput {
        tar_path: Utf8PathBuf::from(r"C:\tmp\seed.tar"),
        install_dir: Utf8PathBuf::from(r"C:\qn\wsl\qn-sandbox"),
    }
}

impl PlannedStep {
    fn args_contain(&self, needle: &str) -> bool {
        self.spec.arg_values().iter().any(|a| a.contains(needle))
    }
}

#[test]
fn it_plans_nothing_when_managed_distro_exists() {
    let plan = plan_provision(DistroState::Present, DistroState::Present, &input());
    assert!(
        plan.steps.is_empty(),
        "present managed distro must be a no-op"
    );
    assert!(!plan.cleanup_seed);
}

#[test]
fn it_plans_export_import_only_when_user_seed_exists() {
    // 用户的种子发行版已存在：绝不能出现 install，更不能出现 unregister。
    let plan = plan_provision(DistroState::Missing, DistroState::Present, &input());
    let labels: Vec<&str> = plan.steps.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels.len(), 2, "export + import only: {labels:?}");
    assert!(labels[0].contains("export"));
    assert!(labels[1].contains("import"));
    assert!(!plan.cleanup_seed, "user's seed must never be unregistered");
    assert!(
        plan.steps.iter().all(|s| !s.label.contains("unregister")),
        "no unregister step for user seed"
    );
}

#[test]
fn it_plans_full_chain_and_cleans_up_self_installed_seed() {
    let plan = plan_provision(DistroState::Missing, DistroState::Missing, &input());
    let labels: Vec<&str> = plan.steps.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(
        labels.len(),
        4,
        "install+export+import+unregister: {labels:?}"
    );
    assert!(labels[0].contains("install"));
    assert!(labels[3].contains("unregister"));
    assert!(plan.cleanup_seed);
}

#[test]
fn it_targets_only_the_seed_on_unregister() {
    let plan = plan_provision(DistroState::Missing, DistroState::Missing, &input());
    let unregister = plan
        .steps
        .iter()
        .find(|s| s.label.contains("unregister"))
        .unwrap();
    assert!(
        unregister.args_contain(SEED_DISTRO_NAME),
        "unregister must reference the seed"
    );
    assert!(
        !unregister.args_contain(MANAGED_DISTRO_NAME),
        "unregister must never reference the managed name"
    );
}

#[test]
fn it_binds_paths_into_commands() {
    let plan = plan_provision(DistroState::Missing, DistroState::Present, &input());
    let export = &plan.steps[0];
    assert!(export.args_contain(r"C:\tmp\seed.tar"));
    let import = &plan.steps[1];
    assert!(import.args_contain(r"C:\qn\wsl\qn-sandbox"));
    assert!(import.args_contain(r"C:\tmp\seed.tar"));
}

#[tokio::test]
async fn execute_reports_step_failures_with_label_and_decoded_detail() {
    // 无副作用的失败命令：未知旗标 → wsl.exe 报错退出（state_of 前置通过）。
    let plan = ProvisionPlan {
        steps: vec![PlannedStep {
            label: "synthetic failing step".to_owned(),
            spec: CmdSpec::new("wsl")
                .args(["--qn-unknown-flag-for-test"])
                .timeout(Duration::from_secs(20))
                .label("synthetic"),
        }],
        cleanup_seed: false,
    };
    let err = execute(&plan).await.expect_err("failing step must surface");
    match err {
        ProvisionError::StepFailed { label, detail } => {
            assert_eq!(label, "synthetic failing step");
            assert!(!detail.is_empty(), "detail must carry the failure facts");
        }
        other => panic!("expected StepFailed, got: {other:?}"),
    }
}

#[tokio::test]
async fn execute_completes_on_a_success_single_step_plan() {
    // 无副作用的成功命令：wsl --version（state_of 同款只读探测）。
    let plan = ProvisionPlan {
        steps: vec![PlannedStep {
            label: "wsl --version".to_owned(),
            spec: CmdSpec::new("wsl")
                .args(["--version"])
                .timeout(Duration::from_secs(20))
                .label("version"),
        }],
        cleanup_seed: false,
    };
    execute(&plan).await.expect("version probe must succeed");
}
