//! qn-config crate 统一错误（spec 02 §4：类型化、保留来源、可行动）。

/// 配置加载/解析错误。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// 平台默认根目录不可得（如 Windows 缺 `%LOCALAPPDATA%`，或平台路径非 UTF-8）。
    /// 行动指引：显式设置 `QN_HOME` 环境变量指定 qn home。
    #[error("platform default root unavailable; set QN_HOME to configure qn's home explicitly")]
    PlatformRootUnavailable,

    /// 配置文件存在但无法读取。
    /// 行动指引：检查文件权限（`qn doctor` 会给出诊断）。
    #[error("failed to read config file `{path}`: {source}")]
    Read {
        /// 配置文件路径。
        path: camino::Utf8PathBuf,
        /// 底层 IO 错误。
        #[source]
        source: std::io::Error,
    },

    /// 配置内容不合法：TOML 语法/字段类型错误、未知键，或环境变量值不是
    /// 合法数字（此时 `path` 形如 `<env:QN_…>`）。
    /// 错误摘要不含配置源码、字段名或字段值。
    /// 行动指引：按文件或环境变量标识检查对应配置的类型与受支持字段。
    #[error("failed to parse config `{path}`: {message}")]
    Parse {
        /// 配置文件路径（或 `<env:…>` 形式的环境变量标识）。
        path: camino::Utf8PathBuf,
        /// 脱敏后的解析错误摘要（不含原始配置源码、字段名或字段值）。
        message: String,
    },
}
