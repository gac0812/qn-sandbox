//! qn-wsl crate 统一错误（spec 02 §4）。
//!
//! health 探测以 `ProbeReport` 表达失败（失败是数据）；本枚举供**执行性动作**
//! 的真实失败使用（当前唯一使用方：upgrade；distro/bootstrap 各有自有错误类型）。

/// 升级动作错误（当前唯一使用方：upgrade）。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// 内核升级未能使内核达标（步骤失败或复验不达标）。
    /// 行动指引：按 detail 排查；降级档 = offline-only（#17）；手动渠道见 remediation。
    #[error("kernel upgrade incomplete: {detail}; remediation: {remediation}")]
    UpgradeIncomplete {
        /// 失败摘要。
        detail: String,
        /// 行动指引（手动渠道）。
        remediation: String,
    },
}
