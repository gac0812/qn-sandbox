//! qn-wsl crate 统一错误（spec 02 §4）。

/// 供给链路错误（health 探测以 `ProbeReport` 表达失败，不使用本错误；
/// 本错误保留给后续 distro/bootstrap 模块的执行性失败）。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// wsl.exe 存在但执行失败（退出码非零且非探测语义）。
    /// 行动指引：按 detail 中的 stderr 摘要排查（如虚拟化未启用）。
    #[error("wsl.exe invocation failed: {detail}")]
    WslInvocation {
        /// 失败摘要（命令、退出码、stderr 节选）。
        detail: String,
    },
}
