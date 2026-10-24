//! 外部命令统一执行器。
//!
//! 仓库唯一允许触达外部二进制（nono、`wsl.exe`、内核安装工具等）的路径：
//! - **argv 数组 only**：不存在任何 shell 字符串拼接（spec 08 §5）；
//! - **可观测**：spawn 与结束均有 tracing 结构化事件；
//! - **可超时**：默认 [`DEFAULT_TIMEOUT`]，可按命令覆盖；
//! - **可取消**：[`run_with_cancel`] 接受 `CancellationToken`，取消即 kill；
//! - **输出可信**：stdout/stderr 有界捕获（[`DEFAULT_MAX_OUTPUT_BYTES`]），
//!   超出标记 truncated，绝不无界吃内存。
//!
//! # Examples
//!
//! ```no_run
//! # async fn demo() -> Result<(), qn_infra::Error> {
//! use qn_infra::process::{self, CmdSpec};
//! use std::time::Duration;
//!
//! let spec = CmdSpec::new("nono")
//!     .args(["--version"])
//!     .timeout(Duration::from_secs(10))
//!     .label("nono --version");
//! let outcome = process::run_checked(&spec).await?;
//! println!("{}", outcome.stdout);
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use tokio::process::{Child, Command};
use tokio_util::sync::CancellationToken;

use crate::error::Error;

/// 默认超时。调用方应经 qn-config 的用户配置覆盖（M0 阶段先用常量）。
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// 单流（stdout/stderr）默认捕获上限：1 MiB。超出即截断并置 truncated 标记。
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 1 << 20;

/// stderr 写入 `Error::Failed` 摘要的最大字节数。
const STDERR_SUMMARY_BYTES: usize = 512;

/// 外部命令描述（不可变 builder）。
///
/// 只接受 argv 数组与显式 env 追加，不接受 shell 字符串——这是
/// spec 08 §5 的强制约束，类型层面不提供违规入口。
#[derive(Debug, Clone)]
#[must_use = "CmdSpec is a builder; call run/run_checked on it"]
pub struct CmdSpec {
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    cwd: Option<camino::Utf8PathBuf>,
    timeout: Option<Duration>,
    max_output_bytes: Option<usize>,
    label: Option<String>,
}

impl CmdSpec {
    /// 以程序名创建（如 `"wsl"`、`"nono"`）。参数一律经 [`CmdSpec::arg`]/[`CmdSpec::args`] 追加。
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            env: Vec::new(),
            cwd: None,
            timeout: None,
            max_output_bytes: None,
            label: None,
        }
    }

    /// 追加单个参数（原值透传，不做任何 shell 解释）。
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// 追加多个参数。
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// 追加（或覆盖）一个环境变量；其余环境继承自当前进程。
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// 设置工作目录（UTF-8 路径，spec 02 §6）。
    pub fn cwd(mut self, cwd: impl Into<camino::Utf8PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// 覆盖默认超时（[`DEFAULT_TIMEOUT`]）。
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 覆盖单流输出捕获上限（[`DEFAULT_MAX_OUTPUT_BYTES`]）。
    pub fn max_output_bytes(mut self, max: usize) -> Self {
        self.max_output_bytes = Some(max);
        self
    }

    /// 设置日志与错误信息中使用的命令标签（默认为程序名）。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    fn effective_label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.program)
    }
}

/// 命令结果：退出码可信、输出有界。
#[derive(Debug, Clone)]
pub struct CmdOutcome {
    /// 退出码；进程被外部信号杀死时为 `None`。
    pub exit_code: Option<i32>,
    /// 是否成功（退出码 0）。
    pub success: bool,
    /// stdout（有界，UTF-8 lossy）。
    pub stdout: String,
    /// stderr（有界，UTF-8 lossy）。
    pub stderr: String,
    /// stdout 是否因超出捕获上限被截断。
    pub stdout_truncated: bool,
    /// stderr 是否因超出捕获上限被截断。
    pub stderr_truncated: bool,
    /// 实际运行时长。
    pub duration: Duration,
}

impl CmdOutcome {
    /// 非零退出码转 [`Error::Failed`]；成功返回 `Ok(self)`。
    ///
    /// 退出码为 `None`（被信号杀死）同样视为失败，code 记为 `-1`。
    pub fn check(self, label: &str) -> Result<Self, Error> {
        if self.success {
            return Ok(self);
        }
        let code = self.exit_code.unwrap_or(-1);
        Err(Error::Failed {
            label: label.to_owned(),
            code,
            stderr_summary: summarize(&self.stderr),
        })
    }
}

fn summarize(stderr: &str) -> String {
    let bytes = stderr.as_bytes();
    if bytes.len() <= STDERR_SUMMARY_BYTES {
        stderr.to_owned()
    } else {
        format!("{}…[truncated]", String::from_utf8_lossy(&bytes[..STDERR_SUMMARY_BYTES]))
    }
}

/// 执行命令，无论退出码如何都返回 [`CmdOutcome`]。
///
/// 超时（默认 [`DEFAULT_TIMEOUT`]）即 kill 并返回 [`Error::TimedOut`]。
/// 取消能力见 [`run_with_cancel`]。
///
/// # Errors
/// - [`Error::Spawn`] 无法启动；
/// - [`Error::TimedOut`] 超过时限被终止。
pub async fn run(spec: &CmdSpec) -> Result<CmdOutcome, Error> {
    run_inner(spec, None).await
}

/// 同 [`run`]，但支持取消：`cancel` 触发即 kill 并返回 [`Error::Cancelled`]。
///
/// # Errors
/// 除 [`run`] 的错误外，还可能返回 [`Error::Cancelled`]。
pub async fn run_with_cancel(
    spec: &CmdSpec,
    cancel: CancellationToken,
) -> Result<CmdOutcome, Error> {
    run_inner(spec, Some(cancel)).await
}

/// 执行命令并把非零退出码转为 [`Error::Failed`]（常见于“必须成功”的调用点）。
///
/// # Errors
/// 见 [`run`] 与 [`CmdOutcome::check`]。
pub async fn run_checked(spec: &CmdSpec) -> Result<CmdOutcome, Error> {
    let label = spec.effective_label().to_owned();
    run(spec).await?.check(&label)
}

async fn run_inner(spec: &CmdSpec, cancel: Option<CancellationToken>) -> Result<CmdOutcome, Error> {
    let timeout = spec.timeout.unwrap_or(DEFAULT_TIMEOUT);
    let max_output = spec.max_output_bytes.unwrap_or(DEFAULT_MAX_OUTPUT_BYTES);
    let label = spec.effective_label().to_owned();

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }

    tracing::info!(
        label = %label,
        program = %spec.program,
        args = ?spec.args,
        timeout_secs = timeout.as_secs(),
        "spawning external command"
    );

    let mut child = command
        .spawn()
        .map_err(|source| Error::Spawn {
            program: spec.program.clone(),
            source,
        })?;

    let (stdout_rx, stderr_rx) = spawn_readers(&mut child, max_output);
    let started = std::time::Instant::now();

    let status = tokio::select! {
        status = child.wait() => status.map_err(|source| Error::Spawn {
            program: spec.program.clone(),
            source,
        })?,
        _ = tokio::time::sleep(timeout) => {
            let _ = child.start_kill();
            return Err(Error::TimedOut {
                label,
                timeout_secs: timeout.as_secs(),
            });
        }
        _ = async {
            match cancel {
                Some(token) => token.cancelled().await,
                None => std::future::pending().await,
            }
        } => {
            let _ = child.start_kill();
            return Err(Error::Cancelled { label });
        }
    };

    let (stdout, stdout_truncated) = stdout_rx.await.unwrap_or_else(|_| (Vec::new(), false));
    let (stderr, stderr_truncated) = stderr_rx.await.unwrap_or_else(|_| (Vec::new(), false));
    let duration = started.elapsed();

    let outcome = CmdOutcome {
        exit_code: status.code(),
        success: status.success(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        stdout_truncated,
        stderr_truncated,
        duration,
    };

    tracing::info!(
        label = %label,
        exit_code = outcome.exit_code,
        success = outcome.success,
        duration_ms = duration.as_millis() as u64,
        stdout_truncated = outcome.stdout_truncated,
        stderr_truncated = outcome.stderr_truncated,
        "external command finished"
    );
    tracing::debug!(
        label = %label,
        stdout = %summarize(&outcome.stdout),
        stderr = %summarize(&outcome.stderr),
        "external command output"
    );

    Ok(outcome)
}

type ReaderResult = (Vec<u8>, bool);

fn spawn_readers(child: &mut Child, max_output: usize) -> (tokio::task::JoinHandle<ReaderResult>, tokio::task::JoinHandle<ReaderResult>) {
    // SAFETY-free: take() 把管道所有权移给读任务，管道在子进程退出后关闭，读任务随之结束。
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    (
        tokio::spawn(read_bounded(stdout, max_output)),
        tokio::spawn(read_bounded(stderr, max_output)),
    )
}

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(
    reader: Option<R>,
    max: usize,
) -> ReaderResult {
    let Some(mut reader) = reader else {
        return (Vec::new(), false);
    };
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        let room = max.saturating_sub(buf.len());
        if room == 0 {
            // 已达上限：再读一字节以确认是否真的还有更多输出。
            let mut probe = [0u8; 1];
            match tokio::io::AsyncReadExt::read(&mut reader, &mut probe).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    truncated = true;
                    break;
                }
            }
        }
        let want = chunk.len().min(room);
        match tokio::io::AsyncReadExt::read(&mut reader, &mut chunk[..want]).await {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    (buf, truncated)
}
