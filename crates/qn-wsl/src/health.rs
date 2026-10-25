//! 平台健康探测：五项探测，产出结构化 `ProbeReport`。
//!
//! 设计约束（#74）：
//! - **探测失败是结果不是错误**：一切异常（wsl 缺失、发行版未建、nono 未装）
//!   都表达为 `Fail/Warn` 报告 + remediation 行动指引，绝不 Err 冒泡——
//!   doctor 类呈现（#44/#66）需要的是完整图景而非第一个异常。
//! - 外部命令一律经 `qn_infra::process`（R1/R3）；wsl.exe 自身的 UI 输出
//!   （UTF-16）从 [`qn_infra::process::CmdOutcome::stdout_raw`] 取**原始字节**再经
//!   [`crate::decode::decode_wsl_output`] 解码——不能走 lossy 字符串往返
//!   （本地化字符的 UTF-16 字节会被 U+FFFD 替换破坏，实测见 #76 评审）。
//!   发行版内命令（`wsl -d <name> -- …`）是 UTF-8 透传，直接用 `stdout`。
//! - 版本与名称解析只依赖**行序、列序与 ASCII 标识符**，不匹配本地化文案
//!   （`-l -v` 的状态词是本地化的，因此本模块只报 present、不判定运行态）。
//! - 探测不改文件系统状态，但 `wsl -d <name>` 会按需 boot 停止状态的发行版
//!   （良性、可观测的状态变化；30s 超时对冷启足够）。
//! - 各探测超时上限合计约 2 分钟（15+20+30×3），健康环境秒级完成。

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
    /// 受管发行版存在性（运行态是本地化文案，不判定）。
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
pub async fn run_all() -> Vec<ProbeReport> {
    vec![
        probe_wsl().await,
        probe_distro().await,
        probe_kernel().await,
        probe_nono().await,
        probe_landlock_abi().await,
    ]
}

/// `wsl --version`：wsl.exe 存在性与版本（UTF-16 UI 输出，走原始字节解码）。
async fn probe_wsl() -> ProbeReport {
    let spec = CmdSpec::new("wsl")
        .args(["--version"])
        .timeout(Duration::from_secs(15))
        .label("wsl --version");
    let outcome = match process::run(&spec).await {
        Ok(outcome) => outcome,
        Err(qn_infra::Error::Spawn { .. }) => {
            return ProbeReport::fail(
                ProbeId::Wsl,
                "wsl.exe not found".to_owned(),
                "install WSL: run `wsl --install` from an elevated terminal, then reboot",
            );
        }
        Err(qn_infra::Error::TimedOut { .. }) => {
            return ProbeReport::warn(
                ProbeId::Wsl,
                "wsl --version timed out".to_owned(),
                "check Windows optional feature 'VirtualMachinePlatform' and retry",
            );
        }
        Err(err) => {
            return ProbeReport::warn(
                ProbeId::Wsl,
                format!("wsl --version failed: {err}"),
                "check WSL installation (`wsl --update`)",
            );
        }
    };
    let text = decode_wsl_output(&outcome.stdout_raw);
    match first_version(&text) {
        Some(version) if outcome.success => {
            ProbeReport::ok(ProbeId::Wsl, format!("WSL version {version}"))
        }
        Some(version) => ProbeReport::warn(
            ProbeId::Wsl,
            format!(
                "parsed version {version}, but wsl.exe exited with code {:?}",
                outcome.exit_code
            ),
            "run `wsl --update` and retry",
        ),
        None => ProbeReport::warn(
            ProbeId::Wsl,
            "could not parse a version from `wsl --version` output".to_owned(),
            "report the exact output in an issue; parsing relies on the first x.y.z token",
        ),
    }
}

/// `wsl -l -v`：受管发行版存在性（UTF-16 UI 输出；名称列是首列，按列匹配防
/// `qn-sandbox-dev` 一类前缀误报；状态词本地化故不判定运行态）。
async fn probe_distro() -> ProbeReport {
    let spec = CmdSpec::new("wsl")
        .args(["-l", "-v"])
        .timeout(Duration::from_secs(20))
        .label("wsl -l -v");
    let Ok(outcome) = process::run(&spec).await else {
        return ProbeReport::fail(
            ProbeId::Distro,
            "wsl.exe unavailable".to_owned(),
            "fix the Wsl probe first (see its remediation)",
        );
    };
    let text = decode_wsl_output(&outcome.stdout_raw);
    if list_has_distro(&text, MANAGED_DISTRO_NAME) {
        ProbeReport::ok(
            ProbeId::Distro,
            format!("distro `{MANAGED_DISTRO_NAME}` present"),
        )
    } else {
        ProbeReport::fail(
            ProbeId::Distro,
            format!("managed distro `{MANAGED_DISTRO_NAME}` not found"),
            "provisioning lands in a follow-up issue (#74 tracking); manually: `wsl --install -d Ubuntu-24.04 --no-launch`",
        )
    }
}

/// 发行版内 `uname -r`：内核版本与 ≥6.7 判定（distro-exec 是 UTF-8 透传）。
async fn probe_kernel() -> ProbeReport {
    match distro_exec("uname -r").await {
        Ok(release) => match KernelVersion::parse(release.trim()) {
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
        },
        Err(why) => dependent_fail(ProbeId::Kernel, &why),
    }
}

/// 发行版内 `nono --version`：安装与锁定版本比对。
async fn probe_nono() -> ProbeReport {
    match distro_exec("nono --version").await {
        Ok(output) => {
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
                    "nono command produced no parseable version".to_owned(),
                    "bootstrap module will install it (follow-up issue); manually: install the pinned deb inside the distro",
                ),
            }
        }
        Err(why) => dependent_fail(ProbeId::Nono, &why),
    }
}

/// ABI floor：由内核版本推导（无需 unsafe syscall；与 #16 附录 A 探针语义一致）。
async fn probe_landlock_abi() -> ProbeReport {
    match distro_exec("uname -r").await {
        Ok(release) => match KernelVersion::parse(release.trim()) {
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
        },
        Err(why) => dependent_fail(ProbeId::LandlockAbi, &why),
    }
}

/// 在受管发行版内执行命令（UTF-8 透传）。失败时 Err 携带退出码与输出摘要——
/// 发行版缺失、sh 缺失、boot 失败等原因可分辨（wsl 的错误面走 stdout 且是 UTF-16）。
async fn distro_exec(snippet: &str) -> Result<String, String> {
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
    let outcome = process::run(&spec)
        .await
        .map_err(|err| format!("wsl invocation failed: {err}"))?;
    if outcome.success {
        return Ok(outcome.stdout);
    }
    let hint = decode_wsl_output(&outcome.stdout_raw);
    let hint: String = hint
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_owned();
    Err(format!(
        "distro exec exited with code {:?}{}",
        outcome.exit_code,
        if hint.is_empty() {
            String::new()
        } else {
            format!(": {hint}")
        }
    ))
}

fn dependent_fail(id: ProbeId, why: &str) -> ProbeReport {
    ProbeReport::fail(
        id,
        why.to_owned(),
        "fix the Distro probe first (see its remediation)",
    )
}

/// 提取首个 `x.y.z` 形态的版本号（不吃第 4 段；结尾必须是数字，拒绝 `6.18.`）。
fn first_version(text: &str) -> Option<String> {
    let mut current = String::new();
    let mut dots = 0usize;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if ch == '.' && !current.is_empty() && !current.ends_with('.') && dots < 2 {
            current.push('.');
            dots += 1;
        } else if is_complete(&current, dots) {
            return Some(current);
        } else {
            current.clear();
            dots = 0;
        }
    }
    is_complete(&current, dots).then_some(current)
}

fn is_complete(candidate: &str, dots: usize) -> bool {
    dots == 2 && candidate.len() >= 5 && candidate.ends_with(|c: char| c.is_ascii_digit())
}

/// `wsl -l -v` 输出中按**名称列**（每行首列，兼容默认发行版的 `*` 前缀标记）
/// 匹配发行版名——防 `qn-sandbox-dev` 类前缀误报；表头/状态词本地化不影响
/// 首列 ASCII 标识符。
pub(crate) fn list_has_distro(text: &str, name: &str) -> bool {
    text.lines().any(|line| {
        let mut tokens = line.split_whitespace();
        match tokens.next() {
            Some("*") => tokens.next() == Some(name),
            Some(first) => first == name,
            None => false,
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn it_extracts_versions_from_real_samples() {
        // 实测样本（spike① 与 #76 本机）：版本号前有本地化标签、后缀有第 4 段。
        assert_eq!(
            first_version("WSL 版本: 3.0.3.0\n内核版本: 6.18.54.1-1").as_deref(),
            Some("3.0.3")
        );
        assert_eq!(first_version("nono 0.80.0").as_deref(), Some("0.80.0"));
        assert_eq!(
            first_version("6.18.54.1-microsoft-standard-WSL2").as_deref(),
            Some("6.18.54")
        );
    }

    #[test]
    fn it_rejects_malformed_versions() {
        assert_eq!(
            first_version("6.18."),
            None,
            "trailing dot is not a version"
        );
        assert_eq!(first_version("no digits here"), None);
        assert_eq!(first_version("6.18"), None, "two segments are not enough");
    }

    #[test]
    fn it_matches_distro_by_first_column_only() {
        let listing = "  NAME            STATE      VERSION\n* Ubuntu-24.04    Stopped    2\n  qn-sandbox-dev Running    2\n";
        assert!(list_has_distro(listing, "Ubuntu-24.04"));
        assert!(
            !list_has_distro(listing, "qn-sandbox"),
            "prefix names must not match"
        );
        assert!(list_has_distro(listing, "qn-sandbox-dev"));
        assert!(!list_has_distro(listing, "qn-sandbox2"));
    }

    #[test]
    fn it_matches_distro_when_star_marker_precedes_name() {
        // 默认发行版行以 `*` 前缀标记，split_whitespace 首词是 `*`——需要兼容。
        let listing = "  NAME STATE VERSION\n* qn-sandbox Running 2\n";
        assert!(list_has_distro(listing, "qn-sandbox"));
    }
}
