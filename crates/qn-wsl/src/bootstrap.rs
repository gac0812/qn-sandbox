//! 发行版内 nono 引导（#86 计划，继 #74 health 与 #85 distro 之后）。
//!
//! 两段式：宿主侧经 `qn_nono::installer`（锁版 + SHA256 校验 + `with_target`
//! staging Linux 工件）落盘；再经 `wsl -d <name> -- install` 递入发行版安装 deb。
//! 环境事实（spike① #16）：受管发行版由种子导出、种子已含 nono 时本模块退化为
//! 版本校验（快路径）；NAT 模式下发行版内直连 GitHub 不可达——**下载必须在宿主侧**。

use std::time::Duration;

use qn_infra::process::{self, CmdSpec};

use crate::MANAGED_DISTRO_NAME;

/// 引导结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapOutcome {
    /// 已是锁定版本（快路径：种子自带或先前已装）。
    AlreadyPinned,
    /// 本次完成安装（宿主 staging → 发行版内 dpkg）。
    Installed,
}

/// 单步超时（dpkg 安装秒级；staging 含下载，给足余量）。
const STEP_TIMEOUT: Duration = Duration::from_secs(300);

/// 引导执行错误。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// 宿主侧 staging 失败（下载/校验/解包，来自 qn-nono installer）。
    #[error("host-side staging of nono failed: {source}")]
    Stage {
        /// installer 的原始错误。
        #[source]
        source: qn_nono::InstallerError,
    },
    /// 发行版内安装步骤失败。
    /// 行动指引：detail 含 dpkg 输出首行；常见原因是磁盘满或架构不符。
    #[error("in-distro install failed: {detail}")]
    Install {
        /// 失败摘要（退出码 + dpkg 输出首行）。
        detail: String,
    },
}

/// 引导发行版内 nono 到锁定版本，返回结果。
///
/// 快路径：发行版内 `nono --version` 已是锁定版本 → 直接返回（幂等）。
/// 慢路径：宿主 staging Linux 工件 → `wsl -d <name> -- dpkg -i` 安装 → 复验版本。
///
/// # Errors
/// - [`Error::Stage`]：宿主侧下载/校验/解包失败；
/// - [`Error::Install`]：发行版内 dpkg 失败或安装后版本不符。
pub async fn ensure_pinned() -> Result<BootstrapOutcome, Error> {
    if version_matches().await? {
        return Ok(BootstrapOutcome::AlreadyPinned);
    }
    // 宿主侧 staging：锁版 + SHA256 + target 选择全在 qn-nono installer（A 的
    // 模块，复用而非重写——R1）。Linux 工件落到 Windows 可见目录，供 /mnt 递入。
    let staged = qn_nono::installer::install(
        &qn_nono::installer::InstallOptions::new(stage_root())
            .with_target(qn_nono::target::NonoTarget::X8664UnknownLinuxGnu),
    )
    .map_err(|source| Error::Stage { source })?;

    // 递入安装：宿主 staged 二进制经 /mnt/<盘> 拷入发行版 /usr/local/bin。
    let host_binary = to_distro_path(&staged.path);
    install_binary_in_distro(&host_binary).await?;

    // 安装后复验：版本不符视为失败（fail-closed）。
    if !version_matches().await? {
        return Err(Error::Install {
            detail: "post-install version check failed; pinned version mismatch".to_owned(),
        });
    }
    Ok(BootstrapOutcome::Installed)
}

/// 发行版内当前 nono 版本是否等于锁定版本。
async fn version_matches() -> Result<bool, Error> {
    version_matches_inner().await
}

async fn version_matches_inner() -> Result<bool, Error> {
    let spec = CmdSpec::new("wsl")
        .args([
            "-d",
            MANAGED_DISTRO_NAME,
            "-u",
            "root",
            "--",
            "nono",
            "--version",
        ])
        .timeout(Duration::from_secs(30))
        .label("wsl nono --version");
    let outcome = process::run(&spec).await.map_err(|err| Error::Install {
        detail: err.to_string(),
    })?;
    if !outcome.success {
        return Ok(false); // 未安装 = 版本不符（走慢路径）
    }
    Ok(outcome
        .stdout
        .trim()
        .ends_with(qn_config::NONO_PINNED_VERSION))
}

async fn install_binary_in_distro(host_path_in_distro: &str) -> Result<(), Error> {
    // 拷入 + 可执行位 + --version 自检，一步完成；版本复验在调用方（fail-closed）。
    let script = format!("install -m 0755 '{host_path_in_distro}' /usr/local/bin/nono; echo RC=$?");
    let spec = CmdSpec::new("wsl")
        .args([
            "-d",
            MANAGED_DISTRO_NAME,
            "-u",
            "root",
            "--",
            "sh",
            "-c",
            &script,
        ])
        .timeout(STEP_TIMEOUT)
        .label("wsl install nono binary");
    let outcome = process::run(&spec).await.map_err(|err| Error::Install {
        detail: err.to_string(),
    })?;
    let rc = outcome
        .stdout
        .lines()
        .find_map(|l| l.strip_prefix("RC=").map(str::to_owned))
        .unwrap_or_else(|| "none".to_owned());
    if outcome.success && rc == "0" {
        Ok(())
    } else {
        Err(Error::Install {
            detail: format!("install rc={rc}"),
        })
    }
}

/// 宿主 staging 目录（Windows 路径）——qn home 下的 bootstrap 缓存。
fn stage_root() -> std::path::PathBuf {
    qn_config::paths::QnPaths::resolve().map_or_else(
        |_| std::env::temp_dir().join("qn-nono-stage"),
        |paths| paths.root.join("cache").join("nono").into_std_path_buf(),
    )
}

/// Windows 宿主路径 → 发行版内 /mnt/<盘>/... 路径（`C:\x\y` → `/mnt/c/x/y`）。
fn to_distro_path(windows_path: &std::path::Path) -> String {
    let text = windows_path.to_string_lossy().replace('\\', "/");
    let (drive, rest) = text.split_once(":/").unwrap_or(("c", &text));
    format!("/mnt/{}/{}", drive.to_ascii_lowercase(), rest)
}

#[cfg(test)]
mod tests {
    use super::to_distro_path;

    #[test]
    fn it_converts_windows_paths_to_distro_mounts() {
        assert_eq!(
            to_distro_path(std::path::Path::new(
                r"C:\Users\x\qn\cache\nono\0.80.0\nono"
            )),
            "/mnt/c/Users/x/qn/cache/nono/0.80.0/nono"
        );
        assert_eq!(
            to_distro_path(std::path::Path::new(r"D:\data\nono")),
            "/mnt/d/data/nono"
        );
    }

    #[test]
    fn it_handles_unc_and_relative_gracefully() {
        // UNC 与相对路径没有 "X:/" 形态——走兜底分支，不 panic 即可。
        let _ = to_distro_path(std::path::Path::new(r"\server\share\nono"));
        let _ = to_distro_path(std::path::Path::new("relative/nono"));
    }
}
