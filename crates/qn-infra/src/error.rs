//! qn-infra crate 统一错误。
//!
//! 按 spec 02 §4：`thiserror` 类型化、保留来源、信息可行动；
//! 各模块的具体错误语义见其文档。

/// 外部命令执行错误（`process` 模块）。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// 子进程无法启动（程序不存在、权限不足等）。
    /// 行动指引：检查程序路径与安装状态（如 `qn doctor` 的探测项）。
    #[error("failed to spawn `{program}`: {source}")]
    Spawn {
        /// 无法启动的程序名。
        program: String,
        /// 底层 IO 错误。
        #[source]
        source: std::io::Error,
    },

    /// 子进程已启动，但等待其退出时发生 IO 错误。
    /// 行动指引：多为系统级异常，重试或查看系统日志。
    #[error("failed to wait for `{program}`: {source}")]
    Wait {
        /// 命令的程序名。
        program: String,
        /// 底层 IO 错误。
        #[source]
        source: std::io::Error,
    },

    /// 命令超过时限被强制终止。
    /// 行动指引：放宽 timeout 或排查该命令为何挂起。
    #[error("command `{label}` timed out after {timeout_ms}ms and was killed")]
    TimedOut {
        /// 命令标签（默认为程序名）。
        label: String,
        /// 生效的超时毫秒数（亚秒精度，避免 400ms 被显示为 0s）。
        timeout_ms: u64,
    },

    /// 命令被调用方取消并终止（与超时是不同的错误变体，spec 02 §4）。
    #[error("command `{label}` was cancelled and killed")]
    Cancelled {
        /// 命令标签（默认为程序名）。
        label: String,
    },

    /// 命令以非零退出码结束（仅 `run_checked` 路径产生）。
    #[error(
        "command `{label}` failed with exit code {code}; stderr: {stderr_summary}\
         (full output is in tracing at debug level)"
    )]
    Failed {
        /// 命令标签（默认为程序名）。
        label: String,
        /// 退出码。
        code: i32,
        /// stderr 摘要（截断到固定长度，避免错误信息爆量）。
        stderr_summary: String,
    },
}
