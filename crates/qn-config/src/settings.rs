//! 配置加载与覆盖合并：**env > toml > 默认**。
//!
//! 首批字段只有 `default_timeout_ms`（`qn-infra::process` 的用户侧默认）；
//! 新字段必须保持"默认 < 文件 < 环境"的三级语义与 serde 兼容（加字段只增不改）。

use std::time::Duration;

use camino::Utf8PathBuf;

use crate::Error;
use crate::paths::QnPaths;

/// 外部命令默认超时（毫秒）。与 qn-infra 的 `DEFAULT_TIMEOUT` 保持一致。
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// 用户配置（全部字段可选；缺失即用默认）。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Config {
    /// 外部命令默认超时（毫秒）。
    #[serde(default)]
    pub default_timeout_ms: Option<u64>,
}

impl Config {
    /// 内置默认值（无文件、无环境变量时的形态）。
    #[must_use]
    pub fn builtin() -> Self {
        Self::default()
    }

    /// 从 `<root>/config.toml` 加载；文件不存在 = 全默认（首启常态，非错误）。
    ///
    /// # Errors
    /// - [`Error::Read`]：文件存在但读失败；
    /// - [`Error::Parse`]：TOML 非法或字段类型不符。
    pub fn load(paths: &QnPaths) -> Result<Self, Error> {
        Self::load_from(&paths.config_file)
    }

    /// 从指定文件加载（测试与诊断用）。
    ///
    /// # Errors
    /// 见 [`Config::load`]。
    pub fn load_from(path: &Utf8PathBuf) -> Result<Self, Error> {
        let Some(text) = read_if_exists(path)? else {
            return Ok(Self::builtin());
        };
        let parsed: Self = toml::from_str(&text).map_err(|err| Error::Parse {
            path: path.clone(),
            message: err.to_string(),
        })?;
        Ok(parsed)
    }

    /// 应用 `QN_*` 环境变量覆盖（在 toml 之后调用，保证 env 优先）。
    ///
    /// # Errors
    /// - [`Error::Parse`]：环境变量值不是合法数字（路径字段写 `<env>`）。
    pub fn apply_env_overrides(self) -> Result<Self, Error> {
        self.apply_env_overrides_with(&|key| {
            std::env::var(key)
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
    }

    /// 注入 lookup 的覆盖逻辑（测试用；生产入口是 [`Config::apply_env_overrides`]）。
    ///
    /// # Errors
    /// 见 [`Config::apply_env_overrides`]。
    pub fn apply_env_overrides_with(
        self,
        lookup: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Self, Error> {
        let mut cfg = self;
        if let Some(raw) = lookup("QN_DEFAULT_TIMEOUT_MS") {
            cfg.default_timeout_ms = Some(raw.parse::<u64>().map_err(|_| Error::Parse {
                path: Utf8PathBuf::from("<env:QN_DEFAULT_TIMEOUT_MS>"),
                message: format!("not a valid u64: {raw}"),
            })?);
        }
        Ok(cfg)
    }

    /// `default_timeout_ms` 解析为 [`Duration`]（缺失用默认）。
    #[must_use]
    pub fn command_timeout(&self) -> Duration {
        Duration::from_millis(self.default_timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS))
    }
}

fn read_if_exists(path: &Utf8PathBuf) -> Result<Option<String>, Error> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(Error::Read {
            path: path.clone(),
            source: err,
        }),
    }
}
