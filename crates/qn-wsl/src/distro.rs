//! 受管发行版的幂等供给（#85）。
//!
//! 策略：以 `Ubuntu-24.04` 为**种子**——受管发行版 `qn-sandbox` 通过
//! `wsl --export <种子>` → `wsl --import qn-sandbox <dir> <tar>` 复制而来；
//! 种子缺失时先 `wsl --install -d Ubuntu-24.04 --no-launch`。**只有本次供给
//! 自己安装的种子才会在导入后 unregister**——用户已有的发行版绝不动。
//!
//! 命令构造（[`plan_provision`]，纯函数）与执行（[`ensure_present`]）分离：
//! 计划可单测，执行全部经 `qn_infra::process`（R1/R3）。
//! 导出/导入/安装是分钟级操作（发行版 GB 级 tar），超时给 10 分钟量级。

use std::path::PathBuf;
use std::time::Duration;

use qn_infra::process::{self, CmdSpec};

use crate::MANAGED_DISTRO_NAME;
use crate::health::list_has_distro;

/// 供给用种子发行版（spike① 实测 `--no-launch` 静默安装可用）。
pub const SEED_DISTRO_NAME: &str = "Ubuntu-24.04";

/// 分钟级步骤（export/import/install）的单步超时。
const LONG_STEP_TIMEOUT: Duration = Duration::from_secs(600);
/// 列表/短命令超时。
const SHORT_TIMEOUT: Duration = Duration::from_secs(20);

/// 受管发行版当前状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistroState {
    /// 受管发行版已存在（无需供给）。
    Present,
    /// 不存在。
    Missing,
}

/// 供给结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionOutcome {
    /// 已存在，未做任何事（幂等 no-op）。
    AlreadyPresent,
    /// 本次完成供给。
    Provisioned {
        /// true = 导入后清理了本次自装的种子发行版。
        cleaned_up_seed: bool,
    },
}

/// 纯计划步骤（label + 完整命令；不执行）。比较按 label 语义即可（测试用）。
#[derive(Debug, Clone)]
pub struct PlannedStep {
    /// 人读标签（日志与错误用）。
    pub label: String,
    /// 将执行的命令。
    pub spec: CmdSpec,
}

/// 三态供给计划。
#[derive(Debug, Clone)]
pub struct ProvisionPlan {
    /// 依次执行的步骤（空 = 无事可做）。
    pub steps: Vec<PlannedStep>,
    /// 完成后是否清理自装种子。
    pub cleanup_seed: bool,
}

/// 供给输入（路径由调用方解析，默认布局见 [`default_layout`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionInput {
    /// 导出 tar 的临时路径。
    pub tar_path: PathBuf,
    /// 受管发行版安装目录（vhdx 落位处）。
    pub install_dir: PathBuf,
}

/// 生成供给计划（纯函数）。
///
/// 三态：受管已存在 → 空计划；种子存在 → export+import（不清理种子）；
/// 种子缺失 → install+export+import+unregister（种子是自装的，导入后回收）。
#[must_use]
pub fn plan_provision(
    managed: DistroState,
    seed: DistroState,
    input: &ProvisionInput,
) -> ProvisionPlan {
    if managed == DistroState::Present {
        return ProvisionPlan {
            steps: Vec::new(),
            cleanup_seed: false,
        };
    }
    let mut steps = Vec::new();
    if seed == DistroState::Missing {
        steps.push(PlannedStep {
            label: format!("install seed distro {SEED_DISTRO_NAME}"),
            spec: CmdSpec::new("wsl")
                .args(["--install", "-d", SEED_DISTRO_NAME, "--no-launch"])
                .timeout(LONG_STEP_TIMEOUT)
                .label("wsl --install seed"),
        });
    }
    let cleanup_seed = seed == DistroState::Missing;
    steps.push(PlannedStep {
        label: format!("export seed {SEED_DISTRO_NAME} to tar"),
        spec: CmdSpec::new("wsl")
            .args([
                "--export",
                SEED_DISTRO_NAME,
                &input.tar_path.to_string_lossy(),
            ])
            .timeout(LONG_STEP_TIMEOUT)
            .label("wsl --export seed"),
    });
    steps.push(PlannedStep {
        label: format!("import managed distro {MANAGED_DISTRO_NAME}"),
        spec: CmdSpec::new("wsl")
            .args([
                "--import",
                MANAGED_DISTRO_NAME,
                &input.install_dir.to_string_lossy(),
                &input.tar_path.to_string_lossy(),
            ])
            .timeout(LONG_STEP_TIMEOUT)
            .label("wsl --import managed"),
    });
    if cleanup_seed {
        steps.push(PlannedStep {
            label: format!("unregister self-installed seed {SEED_DISTRO_NAME}"),
            spec: CmdSpec::new("wsl")
                .args(["--unregister", SEED_DISTRO_NAME])
                .timeout(SHORT_TIMEOUT)
                .label("wsl --unregister seed"),
        });
    }
    ProvisionPlan {
        steps,
        cleanup_seed,
    }
}

/// 供给执行错误（探测步骤的"失败"不是错误——那是 `DistroState` 语义）。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProvisionError {
    /// 无法列举发行版（wsl 不可用/超时）。
    #[error("cannot list WSL distros to plan provisioning: {detail}")]
    ListUnavailable {
        /// 失败摘要。
        detail: String,
    },
    /// 计划中的某步执行失败（含步骤标签与摘要，行动指引见 label）。
    #[error("provisioning step `{label}` failed: {detail}")]
    StepFailed {
        /// 失败步骤的标签。
        label: String,
        /// 失败摘要（退出码/输出节选，无敏感内容）。
        detail: String,
    },
}

/// 探测指定发行版是否存在（`wsl -l -v`，UTF-16 解码 + 列匹配）。
async fn state_of(name: &str) -> Result<DistroState, ProvisionError> {
    let spec = CmdSpec::new("wsl")
        .args(["-l", "-v"])
        .timeout(SHORT_TIMEOUT)
        .label("wsl -l -v (provision)");
    let outcome = process::run(&spec)
        .await
        .map_err(|err| ProvisionError::ListUnavailable {
            detail: err.to_string(),
        })?;
    let text = crate::decode::decode_wsl_output(&outcome.stdout_raw);
    if list_has_distro(&text, name) {
        Ok(DistroState::Present)
    } else {
        Ok(DistroState::Missing)
    }
}

/// 幂等供给：已存在 no-op；缺失则按计划执行。
///
/// # Errors
/// - [`ProvisionError::ListUnavailable`]：无法探测现状；
/// - [`ProvisionError::StepFailed`]：计划步骤失败（tar 残留会尽力清理）。
pub async fn ensure_present(input: &ProvisionInput) -> Result<ProvisionOutcome, ProvisionError> {
    let managed = state_of(MANAGED_DISTRO_NAME).await?;
    let seed = state_of(SEED_DISTRO_NAME).await?;
    let plan = plan_provision(managed, seed, input);
    if plan.steps.is_empty() {
        return Ok(ProvisionOutcome::AlreadyPresent);
    }
    // wsl --import 要求安装目录已存在（"系统找不到指定的路径"实测）；tar 的
    // 父目录同理（wsl --export 不会创建路径）。失败即失败，不静默换路径。
    if let Err(err) = std::fs::create_dir_all(&input.install_dir) {
        return Err(ProvisionError::StepFailed {
            label: "create install dir".to_owned(),
            detail: err.to_string(),
        });
    }
    if let Some(parent) = input.tar_path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            return Err(ProvisionError::StepFailed {
                label: "create tar parent dir".to_owned(),
                detail: err.to_string(),
            });
        }
    }
    let result = execute(&plan).await;
    // 无论成败都清理导出 tar（安装产物是发行版本身，不是 tar）。
    let _ = std::fs::remove_file(&input.tar_path);
    result.map(|()| ProvisionOutcome::Provisioned {
        cleaned_up_seed: plan.cleanup_seed,
    })
}

async fn execute(plan: &ProvisionPlan) -> Result<(), ProvisionError> {
    for step in &plan.steps {
        // wsl.exe 的错误面走 stdout 且是 UTF-16——stderr 摘要抓不到真实原因，
        // 失败时把两条流都带进 detail（诊断优先；输出无敏感内容）。
        if let Err(err) = process::run_checked(&step.spec).await {
            let mut detail = err.to_string();
            if let qn_infra::Error::Failed { .. } = &err {
                if let Ok(outcome) = process::run(&step.spec).await {
                    let stdout = crate::decode::decode_wsl_output(&outcome.stdout_raw);
                    let first = stdout.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                    if !first.is_empty() {
                        detail.push_str("; stdout: ");
                        detail.push_str(first.trim());
                    }
                }
            }
            return Err(ProvisionError::StepFailed {
                label: step.label.clone(),
                detail,
            });
        }
    }
    Ok(())
}

/// 默认布局：受管发行版安装目录 `<qn-home>/wsl/<name>/`、tar 在系统临时目录。
///
/// # Errors
/// qn home 解析失败（见 qn-config）时返回 IO 错误摘要。
pub fn default_layout() -> Result<ProvisionInput, String> {
    let paths = qn_config::paths::QnPaths::resolve().map_err(|err| err.to_string())?;
    let install_dir: PathBuf = paths.root.join("wsl").join(MANAGED_DISTRO_NAME).into();
    let tar_path = std::env::temp_dir().join(format!("{MANAGED_DISTRO_NAME}-seed.tar"));
    Ok(ProvisionInput {
        tar_path,
        install_dir,
    })
}

#[cfg(test)]
#[path = "distro_tests.rs"]
mod tests;
