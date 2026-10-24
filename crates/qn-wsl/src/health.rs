//! 平台健康探测：五项只读探测，产出结构化 `ProbeReport`。
//!
//! 设计约束（#74）：
//! - **探测失败是结果不是错误**：一切异常（wsl 缺失、发行版未建、nono 未装）
//!   都表达为 `Fail/Warn` 报告 + remediation 行动指引，绝不 Err 冒泡——
//!   doctor 类呈现（#44/#66）需要的是完整图景而非第一个异常。
//! - 外部命令一律经 `qn_infra::process`（R1/R3）；输出经 [`crate::decode::decode_wsl_output`]。
//! - 版本解析依赖**行序与非本地化特征**（首个三段版本号），不匹配中文标签文案。

use qn_infra::process::{self, CmdSpec};
use std::time::Duration;

use crate::MANAGED_DISTRO_NAME;
use crate::decode::decode_wsl_output;
use crate::kernel::KernelVersion;

/// 探测项标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeId {
    /// wsl.exe 可用性与版本。
    Wsl,
    /// 受管发行版存在与运行状态。
    Distro,
    /// 发行版内内核版本与 Landlock 下限判定。
    Kernel,
    /// 发行版内 nono 安装与锁定版本比对。
    Nono,
    /// Landlock ABI floor 推导（网络过滤能力前提）。
    LandlockAbi,
}

/// 探测结论等级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeStatus {
    /// 达标。
    Ok,
    /// 可运行但有限制（呈现层应提示）。
    Warn,
    /// 不达标（附 remediation）。
    Fail,
}

/// 单项探测结论。
#[derive(Debug, Clone)]
pub struct ProbeReport {
    /// 探测项。
    pub id: ProbeId,
    /// 结论等级。
    pub status: ProbeStatus,
    /// 事实描述（脱敏：只含版本号/名称，无用户数据）。
    pub detail: String,
    /// 行动指引（Fail 时必有）。
    pub remediation: Option<String>,
}

impl ProbeReport {
    fn ok(id: ProbeId, detail: String) -> Self {
        Self {
            id,
            status: ProbeStatus::Ok,
            detail,
            remediation: None,
        }
    }
    fn warn(id: ProbeId, detail: String, remediation: &str) -> Self {
        Self {
            id,
            status: ProbeStatus::Warn,
            detail,
            remediation: Some(remediation.to_owned()),
        }
    }
    fn fail(id: ProbeId, detail: String, remediation: &str) -> Self {
        Self {
            id,
            status: ProbeStatus::Fail,
            detail,
            remediation: Some(remediation.to_owned()),
        }
    }
}

/// 依次执行五项探测。任何单项的异常都被吸收为其自身的 Fail/Warn 报告；
/// 返回值 Vec 长度恒为 5 且顺序固定（呈现层据此对位）。
///
/// # Panics
///
/// 不 panic（探测哲学：失败是数据）。
pub async fn run_all() -> Vec<ProbeReport> {
    vec![
        probe_wsl().await,
        probe_distro().await,
        probe_kernel().await,
        probe_nono().await,
        probe_landlock_abi().await,
    ]
}

/// `wsl --version`：wsl.exe 存在性与版本（输出 UTF-16，经解码）。
async fn probe_wsl() -> ProbeReport {
    let spec = CmdSpec::new("wsl")
        .args(["--version"])
        .timeout(Duration::from_secs(15))
        .label("wsl --version");
    let Ok(outcome) = process::run(&spec).await else {
        return ProbeReport::fail(
            ProbeId::Wsl,
            "wsl.exe not found or failed to start".to_owned(),
            "install WSL: run `wsl --install` from an elevated terminal, then reboot",
        );
    };
    let text = decode_wsl_output(&outcome.stdout.into_bytes());
    match first_version(&text) {
        Some(v) if outcome.success => ProbeReport::ok(ProbeId::Wsl, format!("WSL version {v}")),
        _ => ProbeReport::warn(
            ProbeId::Wsl,
            format!("wsl.exe exited with code {:?}", outcome.exit_code),
            "check Windows optional feature 'VirtualMachinePlatform' and `wsl --update`",
        ),
    }
}

/// `wsl -l -v`：受管发行版存在性（输出 UTF-16）。后续探测全部依赖它。
async fn probe_distro() -> ProbeReport {
    let spec = CmdSpec::new("wsl")
        .args(["-l", "-v"])
        .timeout(Duration::from_secs(20))
        .label("wsl -l -v");
    let Ok(outcome) = process::run(&spec).await else {
        return ProbeReport::fail(
            ProbeId::Distro,
            "wsl.exe unavailable".to_owned(),
            "fix the Wsl probe first",
        );
    };
    let text = decode_wsl_output(&outcome.stdout.into_bytes());
    // 行序稳定：首行为表头，其后每行 `NAME STATE VERSION`（本地化仅影响表头与状态词，
    // 发行版名是 ASCII 标识符，直接做子串探测）。
    if line_contains_distro(&text, MANAGED_DISTRO_NAME, true) {
        let running = line_contains_distro(&text, MANAGED_DISTRO_NAME, false)
            && !text
                .lines()
                .filter(|l| l.contains(MANAGED_DISTRO_NAME))
                .any(|l| l.to_lowercase().contains("stopped"));
        let state = if running {
            "running"
        } else {
            "stopped (will start on first use)"
        };
        ProbeReport::ok(
            ProbeId::Distro,
            format!("distro `{MANAGED_DISTRO_NAME}` present, {state}"),
        )
    } else {
        ProbeReport::fail(
            ProbeId::Distro,
            format!("managed distro `{MANAGED_DISTRO_NAME}` not found"),
            "provisioning lands in a follow-up issue (#74 tracking); manually: `wsl --install -d Ubuntu-24.04 --no-launch`",
        )
    }
}

/// 发行版内 `uname -r`：内核版本与 ≥6.7 判定（distr-exec 输出为 UTF-8 透传）。
async fn probe_kernel() -> ProbeReport {
    let Some(release) = distro_exec("uname -r").await else {
        return dependent_fail(ProbeId::Kernel, "kernel check needs the managed distro");
    };
    match KernelVersion::parse(release.trim()) {
        Ok(v) if v.meets_landlock_net_floor() => ProbeReport::ok(
            ProbeId::Kernel,
            format!(
                "kernel {}.{:02}.{} (>= 6.7 floor)",
                v.major, v.minor, v.patch
            ),
        ),
        Ok(v) => ProbeReport::fail(
            ProbeId::Kernel,
            format!(
                "kernel {}.{:02}.{} below the 6.7 Landlock-network floor",
                v.major, v.minor, v.patch
            ),
            "upgrade WSL to >= 3.0.3 (ships kernel 6.18): `wsl --update` or the MSI from microsoft/WSL releases; degraded mode is offline-only (decision #17)",
        ),
        Err(err) => ProbeReport::warn(
            ProbeId::Kernel,
            format!("unparseable kernel release: {err}"),
            "report the exact `uname -r` output in an issue",
        ),
    }
}

/// 发行版内 `nono --version`：安装与锁定版本比对。
async fn probe_nono() -> ProbeReport {
    let Some(output) = distro_exec("nono --version").await else {
        return dependent_fail(ProbeId::Nono, "nono check needs the managed distro");
    };
    let pinned = qn_config::NONO_PINNED_VERSION;
    match first_version(output.trim()) {
        Some(v) if v == pinned => {
            ProbeReport::ok(ProbeId::Nono, format!("nono {v} (matches pinned {pinned})"))
        }
        Some(v) => ProbeReport::fail(
            ProbeId::Nono,
            format!("nono {v} found, pinned version is {pinned}"),
            &format!(
                "reinstall the pinned build inside the distro (v{pinned}, sha256-verified); version policy in spec 04 section 6"
            ),
        ),
        None => ProbeReport::fail(
            ProbeId::Nono,
            "nono not found inside the managed distro".to_owned(),
            "bootstrap module will install it (follow-up issue); manually: install the v0.80.0 deb inside the distro",
        ),
    }
}

/// ABI floor：由内核版本推导（无需 unsafe syscall；与 #16 附录 A 探针语义一致）。
async fn probe_landlock_abi() -> ProbeReport {
    let Some(release) = distro_exec("uname -r").await else {
        return dependent_fail(
            ProbeId::LandlockAbi,
            "ABI derivation needs the managed distro",
        );
    };
    match KernelVersion::parse(release.trim()) {
        Ok(v) => {
            let abi = v.landlock_abi_floor();
            if abi >= 4 {
                ProbeReport::ok(
                    ProbeId::LandlockAbi,
                    format!("Landlock ABI floor {abi} (network filtering available)"),
                )
            } else {
                ProbeReport::fail(
                    ProbeId::LandlockAbi,
                    format!("Landlock ABI floor {abi} (< 4: no network access rights)"),
                    "kernel >= 6.7 required; see the Kernel probe remediation (decision #17)",
                )
            }
        }
        Err(err) => ProbeReport::warn(
            ProbeId::LandlockAbi,
            format!("ABI derivation skipped: {err}"),
            "see the Kernel probe",
        ),
    }
}

/// 在受管发行版内执行命令并返回 stdout（UTF-8 透传）。发行版缺失返回 None。
async fn distro_exec(snippet: &str) -> Option<String> {
    let spec = CmdSpec::new("wsl")
        .args([
            "-d",
            MANAGED_DISTRO_NAME,
            "-u",
            "root",
            "--",
            "sh",
            "-c",
            snippet,
        ])
        .timeout(Duration::from_secs(30))
        .label("wsl distro exec");
    let outcome = process::run(&spec).await.ok()?;
    if outcome.success {
        Some(outcome.stdout)
    } else {
        None
    }
}

fn dependent_fail(id: ProbeId, why: &str) -> ProbeReport {
    ProbeReport::fail(
        id,
        why.to_owned(),
        "fix the Distro probe first (see its remediation)",
    )
}

fn first_version(text: &str) -> Option<String> {
    // 首个 x.y.z 形态；避免匹配 6.18.54.1 的第 4 段（无碍：取整段前缀亦可）。
    let mut cur = String::new();
    let mut dots = 0;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else if ch == '.' && !cur.is_empty() && dots < 2 {
            cur.push('.');
            dots += 1;
        } else {
            if cur.len() >= 5 && dots == 2 && cur.ends_with(|c: char| c.is_ascii_digit()) {
                return Some(cur);
            }
            cur.clear();
            dots = 0;
        }
    }
    (cur.len() >= 5 && dots == 2).then_some(cur)
}

/// 发行版行匹配：`name_column=true` 匹配行内含名字；false 仅检查行存在。
fn line_contains_distro(text: &str, name: &str, _name_column: bool) -> bool {
    text.lines().any(|l| l.contains(name))
}
