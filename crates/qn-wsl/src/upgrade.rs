//! 内核升级闭环（#17 裁决：kernel ≥6.7 = Windows P0 硬前置，降级档 = offline-only）。
//!
//! 计划/执行分离（与 [`crate::distro`] 同款）：
//! - [`plan_upgrade`] 纯函数：`wsl --update` → `wsl --shutdown`（**破坏性步骤**）→ 复验
//! - [`run`] 编排执行；失败（实测 CDN 403，spike① §4）→ `Err(Error::UpgradeIncomplete)` + 手动渠道指引
//!
//! **调用方契约**：`wsl --shutdown` 会终止**全部** WSL 发行版（含用户的 Docker），
//! CLI/UI 必须在调用 [`run`] 前取得用户明确确认——本模块绝不静默打断用户环境。
//! 升级失败不阻塞其余功能（降级档 = offline-only，裁决 #17 第 4 条）。

use std::time::Duration;

use qn_infra::process::{self, CmdSpec};

use crate::Error;

use crate::kernel::KernelVersion;

/// 升级步骤单步超时（`wsl --update` 下载分钟级）。
const UPDATE_TIMEOUT: Duration = Duration::from_secs(600);

/// 升级闭环结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeOutcome {
    /// 内核已满足下限，无需升级。
    AlreadySatisfied {
        /// 升级前实测的内核版本。
        version: KernelVersion,
    },
    /// 升级完成且复验通过。
    Updated {
        /// 升级后复验的内核版本。
        version: KernelVersion,
    },
}

/// 计划步骤。
#[derive(Debug, Clone)]
pub struct PlannedStep {
    /// 人读标签。
    pub label: String,
    /// 将执行的命令。
    pub spec: CmdSpec,
    /// true = 破坏性步骤（会终止用户全部发行版），执行前必须用户确认。
    pub disruptive: bool,
}

/// 升级计划。
#[derive(Debug, Clone)]
pub struct UpgradePlan {
    /// 依次执行的步骤（空 = 已满足）。
    pub steps: Vec<PlannedStep>,
}

/// 生成升级计划（纯函数）。`needs_upgrade=false` → 空计划。
#[must_use]
pub fn plan_upgrade(needs_upgrade: bool) -> UpgradePlan {
    if !needs_upgrade {
        return UpgradePlan { steps: Vec::new() };
    }
    UpgradePlan {
        steps: vec![
            PlannedStep {
                label: "wsl --update (downloads the latest WSL release)".to_owned(),
                spec: CmdSpec::new("wsl")
                    .args(["--update"])
                    .timeout(UPDATE_TIMEOUT)
                    .label("wsl --update"),
                disruptive: false,
            },
            PlannedStep {
                // 新内核在虚拟机重启后才生效（spike① §5 风险 2）。
                label: "wsl --shutdown (DISRUPTIVE: stops ALL distros incl. user Docker)"
                    .to_owned(),
                spec: CmdSpec::new("wsl")
                    .args(["--shutdown"])
                    .timeout(Duration::from_secs(60))
                    .label("wsl --shutdown"),
                disruptive: true,
            },
            PlannedStep {
                label: "verify kernel version after restart".to_owned(),
                spec: verify_spec(),
                disruptive: false,
            },
        ],
    }
}

/// 复验命令的唯一构造点（计划第三步与 run 的快路径共用，防索引漂移）。
fn verify_spec() -> CmdSpec {
    CmdSpec::new("wsl")
        .args([
            "-d",
            crate::MANAGED_DISTRO_NAME,
            "-u",
            "root",
            "--",
            "uname",
            "-r",
        ])
        .timeout(Duration::from_secs(60))
        .label("verify kernel after upgrade")
}

/// 执行升级计划并复验。
///
/// # Errors
/// 计划步骤失败或复验仍不达标时返回 [`crate::Error::UpgradeIncomplete`]——
/// 调用方应呈现降级档（offline-only，#17 裁决）与 remediation。
/// 升级失败的容错语义（实测驱动）：`wsl --update` 在"已最新"或网络受限时
/// 会失败（本机实测 exit -1 / CDN 403）——但内核可能本已达标。因此 update 失败
/// **不中断闭环**，继续 shutdown/verify；verify 才是裁判：verify 通过且 update
/// 曾失败 → [`UpgradeOutcome::AlreadySatisfied`]（失败步骤未阻止内核达标）。
pub async fn run(plan: &UpgradePlan) -> Result<UpgradeOutcome, crate::Error> {
    let mut update_failures: Vec<String> = Vec::new();
    for step in &plan.steps {
        // 一次执行、同次取流（绝不重跑命令取诊断——distro.rs 纪律）。
        let outcome = match process::run(&step.spec).await {
            Ok(outcome) => outcome,
            Err(err) => {
                if step_is_verify(step) {
                    return Err(Error::UpgradeIncomplete {
                        detail: format!("verification failed: {err}"),
                        remediation: manual_channels(),
                    });
                }
                update_failures.push(format!("step `{}` failed: {err}", step.label));
                continue;
            }
        };
        if outcome.success && !step_is_verify(step) {
            continue;
        }
        let stdout = crate::decode::decode_wsl_output(&outcome.stdout_raw);
        let first = stdout
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim();
        if step_is_verify(step) {
            // verify 是唯一裁判：计划内执行这一次，就地判定，不再重跑。
            return classify_release(
                first,
                plan_had_steps(plan),
                !update_failures.is_empty(),
                &update_failures,
            );
        }
        update_failures.push(format!(
            "step `{}` failed: exit {:?}; stdout: {first}",
            step.label, outcome.exit_code
        ));
    }
    // 空计划（needs_upgrade=false）快路径：verify 只在这里执行。
    let outcome = process::run(&verify_spec())
        .await
        .map_err(|err| Error::UpgradeIncomplete {
            detail: format!("verification failed: {err}"),
            remediation: manual_channels(),
        })?;
    if !outcome.success {
        return Err(Error::UpgradeIncomplete {
            detail: format!("verification exited with {:?}", outcome.exit_code),
            remediation: manual_channels(),
        });
    }
    classify_release(outcome.stdout.trim(), false, false, &[])
}

/// 纯函数标注（分支全覆盖，单测钉死）：空计划 → `AlreadySatisfied`；
/// 有步骤失败但 verify 通过 → `AlreadySatisfied`（失败步骤未阻止达标）；
/// 全部成功 → `Updated`。
fn label_outcome(
    plan_had_steps: bool,
    had_failures: bool,
    version: KernelVersion,
) -> UpgradeOutcome {
    if !plan_had_steps || had_failures {
        UpgradeOutcome::AlreadySatisfied { version }
    } else {
        UpgradeOutcome::Updated { version }
    }
}

fn classify_release(
    release: &str,
    plan_had_steps: bool,
    had_failures: bool,
    update_failures: &[String],
) -> Result<UpgradeOutcome, Error> {
    // 返回完整 Error（调用方无需再包装）。
    let remediation = manual_channels();
    match KernelVersion::parse(release) {
        Ok(v) if v.meets_landlock_net_floor() => Ok(label_outcome(plan_had_steps, had_failures, v)),
        Ok(v) => {
            for failure in update_failures {
                tracing::warn!(
                    detail = %failure,
                    "upgrade step failure preceding below-floor verdict"
                );
            }
            Err(Error::UpgradeIncomplete {
                detail: format!(
                    "kernel {}.{:02}.{} still below the 6.7 floor after upgrade",
                    v.major, v.minor, v.patch
                ),
                remediation,
            })
        }
        Err(err) => {
            for failure in update_failures {
                tracing::warn!(
                    detail = %failure,
                    "upgrade step failure preceding unparseable verdict"
                );
            }
            Err(Error::UpgradeIncomplete {
                detail: format!("unparseable kernel release after upgrade: {err}"),
                remediation,
            })
        }
    }
}

fn plan_had_steps(plan: &UpgradePlan) -> bool {
    !plan.steps.is_empty()
}

fn step_is_verify(step: &PlannedStep) -> bool {
    step.label.contains("verify")
}

fn manual_channels() -> String {
    "manual channels: MSI or msixbundle from microsoft/WSL releases (3.0.3 ships kernel 6.18.54.1), or self-built 6.18.y via .wslconfig; see spike #16 section 4 channel matrix".to_owned()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn it_plans_empty_when_already_satisfied() {
        let plan = plan_upgrade(false);
        assert!(
            plan.steps.is_empty(),
            "satisfied kernel must not plan any step"
        );
    }

    #[test]
    fn it_plans_update_shutdown_verify_in_order() {
        let plan = plan_upgrade(true);
        assert_eq!(plan.steps.len(), 3);
        assert!(!plan.steps[0].disruptive, "wsl --update is not disruptive");
        assert!(
            plan.steps[1].disruptive,
            "wsl --shutdown must be flagged disruptive (stops user Docker)"
        );
        assert!(plan.steps[1].label.contains("DISRUPTIVE"));
        assert!(plan.steps[2].label.contains("verify"));
        // shutdown 必须排在 verify 之前（新内核重启后才生效）。
        assert!(plan.steps[1].label.contains("shutdown") && plan.steps[2].label.contains("verify"));
    }

    #[test]
    fn it_carries_manual_channels_in_errors() {
        let err = Error::UpgradeIncomplete {
            detail: "synthetic".to_owned(),
            remediation: manual_channels(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("offline-only") || msg.contains("MSI"),
            "remediation must name a channel: {msg}"
        );
        assert!(
            msg.contains("6.18.54.1"),
            "remediation must cite the pinned kernel source"
        );
    }
}

#[cfg(test)]
mod outcome_label_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn v(major: u32, minor: u32, patch: u32) -> KernelVersion {
        KernelVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn label_outcome_empty_plan_is_already_satisfied() {
        assert!(matches!(
            label_outcome(false, false, v(6, 18, 54)),
            UpgradeOutcome::AlreadySatisfied { .. }
        ));
    }

    #[test]
    fn label_outcome_all_steps_ok_is_updated() {
        assert!(matches!(
            label_outcome(true, false, v(6, 18, 54)),
            UpgradeOutcome::Updated { .. }
        ));
    }

    #[test]
    fn label_outcome_with_failures_but_floor_met_is_already_satisfied() {
        assert!(matches!(
            label_outcome(true, true, v(6, 18, 54)),
            UpgradeOutcome::AlreadySatisfied { .. }
        ));
    }

    #[test]
    fn classify_current_release_with_clean_plan_is_updated() {
        assert!(matches!(
            classify_release("6.18.54.1-microsoft-standard-WSL2", true, false, &[]),
            Ok(UpgradeOutcome::Updated { .. })
        ));
    }

    #[test]
    fn classify_current_release_with_failed_steps_is_already_satisfied() {
        let failures = ["step `wsl --update` failed: exit -1".to_owned()];
        assert!(matches!(
            classify_release("6.18.54.1-microsoft-standard-WSL2", true, true, &failures),
            Ok(UpgradeOutcome::AlreadySatisfied { .. })
        ));
    }

    #[test]
    fn classify_below_floor_release_is_err() {
        let Err(err) = classify_release("6.6.87.2-microsoft-standard-WSL2", true, false, &[])
        else {
            panic!("below-floor must be Err");
        };
        assert!(err.to_string().contains("below the 6.7 floor"), "{err}");
    }

    #[test]
    fn classify_unparseable_release_is_err() {
        assert!(classify_release("garbage", true, false, &[]).is_err());
    }
}
