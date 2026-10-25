//! qn 目录布局唯一权威（架构拆分 §7 数据视图）。
//!
//! 只解析不创建——目录/文件的实际创建属使用方（qn-daemon 等）。
//! 布局变更必须同步修订架构拆分 §7 与本表。

use camino::Utf8PathBuf;

/// 覆盖根目录的环境变量（测试与便携安装场景）。
pub const QN_HOME_ENV: &str = "QN_HOME";

/// qn 的全部标准路径。
///
/// 根目录解析优先级：`QN_HOME` 环境变量 > 平台默认
/// （Windows `%LOCALAPPDATA%\qn`；Unix `~/.qn`）。
///
/// # Errors
///
/// 解析失败（既无 `QN_HOME` 且平台默认目录不可得，见 [`QnPaths::resolve_with`]）
/// 返回 [`crate::Error::PlatformRootUnavailable`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QnPaths {
    /// 配置根目录（即"qn home"）。
    pub root: Utf8PathBuf,
    /// 用户配置文件 `<root>/config.toml`。
    pub config_file: Utf8PathBuf,
    /// qnd 本地访问凭证 `<root>/token`（0600，见 08 §5）。
    pub token: Utf8PathBuf,
    /// 会话注册表 `<root>/sessions.json`（fail-closed，架构拆分 §7）。
    pub sessions: Utf8PathBuf,
    /// profile 缓存目录 `<root>/profiles/`。
    pub profiles: Utf8PathBuf,
    /// 审计呈现副本目录 `<root>/audit/`（防篡改源在 nono 侧）。
    pub audit: Utf8PathBuf,
    /// 会话运行根目录 `<root>/runtime/`（私有会话记录、控制关联与隔离 HOME）。
    pub runtime: Utf8PathBuf,
    /// 日志目录 `<root>/logs/`。
    pub logs: Utf8PathBuf,
    /// 可随时清空的缓存目录 `<root>/cache/`。
    pub cache: Utf8PathBuf,
}

/// 解析平台默认根目录（不含 `QN_HOME` 覆盖逻辑，覆盖见 [`QnPaths::resolve`]）。
///
/// 这是 qn-config 内部唯一使用 `#[cfg(target_os)]` 的位置（R6 白名单）。
/// 返回 `None` 的两种情形：平台目录不可得、或路径非 UTF-8（病态环境；
/// 此时建议用户显式设置 `QN_HOME`，错误信息亦如此指引）。
#[cfg(target_os = "windows")]
#[must_use]
pub fn platform_default_root() -> Option<Utf8PathBuf> {
    dirs::data_local_dir().and_then(|p| Utf8PathBuf::from_path_buf(p.join("qn")).ok())
}

/// 解析平台默认根目录（Unix 变体）。
#[cfg(not(target_os = "windows"))]
#[must_use]
pub fn platform_default_root() -> Option<Utf8PathBuf> {
    dirs::home_dir().and_then(|p| Utf8PathBuf::from_path_buf(p.join(".qn")).ok())
}

/// Resolve the host user's home directory for profile path guards.
///
/// Returns `None` when the platform home cannot be resolved or is not UTF-8;
/// callers must fail closed rather than skip protected-path checks.
#[must_use]
pub fn platform_host_home() -> Option<Utf8PathBuf> {
    dirs::home_dir().and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
}

impl QnPaths {
    /// Managed engine cache namespace; the adapter adds target and pinned version.
    ///
    ///     let paths = qn_config::paths::QnPaths::from_root("/tmp/qn");
    ///     assert_eq!(paths.engine_cache(), paths.cache.join("nono"));
    #[must_use]
    pub fn engine_cache(&self) -> Utf8PathBuf {
        self.cache.join("nono")
    }

    /// 以给定根目录构造全部标准路径。
    #[must_use]
    pub fn from_root(root: impl Into<Utf8PathBuf>) -> Self {
        let root = root.into();
        Self {
            config_file: root.join("config.toml"),
            token: root.join("token"),
            sessions: root.join("sessions.json"),
            profiles: root.join("profiles"),
            audit: root.join("audit"),
            runtime: root.join("runtime"),
            logs: root.join("logs"),
            cache: root.join("cache"),
            root,
        }
    }

    /// 纯函数形态的解析逻辑（可测试：home 由参数注入）。
    ///
    /// `qn_home` 非空（非空白）则用之；否则回退 [`platform_default_root`]。
    ///
    /// # Errors
    ///
    /// 既无有效 `qn_home` 且平台默认目录不可得时，返回
    /// [`crate::Error::PlatformRootUnavailable`]（行动指引：显式设置 `QN_HOME`）。
    pub fn resolve_with(qn_home: Option<String>) -> Result<Self, crate::Error> {
        if let Some(home) = qn_home.filter(|s| !s.trim().is_empty()) {
            return Ok(Self::from_root(home));
        }
        let root = platform_default_root().ok_or(crate::Error::PlatformRootUnavailable)?;
        Ok(Self::from_root(root))
    }

    /// 解析当前环境的路径：`QN_HOME` 优先，其次平台默认。
    ///
    /// # Errors
    ///
    /// 见 [`QnPaths::resolve_with`]；若 `QN_HOME` 存在但不是合法 UTF-8，
    /// 返回 [`crate::Error::EnvironmentNotUnicode`]。
    pub fn resolve() -> Result<Self, crate::Error> {
        match std::env::var(QN_HOME_ENV) {
            Ok(home) => Self::resolve_with(Some(home)),
            Err(std::env::VarError::NotPresent) => Self::resolve_with(None),
            Err(std::env::VarError::NotUnicode(_)) => {
                Err(crate::Error::EnvironmentNotUnicode { name: QN_HOME_ENV })
            }
        }
    }
}
