//! qn-config crate 统一错误（spec 02 §4：类型化、保留来源、可行动）。

/// 配置加载/解析错误。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
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

    /// 配置文件不是合法 TOML 或字段类型不对。
    /// 行动指引：按错误位置修正 config.toml；schema 见 qn-config 文档。
    #[error("failed to parse config file `{path}`: {message}")]
    Parse {
        /// 配置文件路径。
        path: camino::Utf8PathBuf,
        /// 解析错误摘要（含行列位置）。
        message: String,
    },
}
