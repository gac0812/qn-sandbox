# 02 — Rust 编码规范

本规范是 `AGENTS.md` §4 的展开与工具化。冲突时以 AGENTS.md 为准。
工具链强制项（fmt/clippy/deny）CI 会在门禁里兜底，但**规范的目标是 review 时少返工**，不是"混过 CI"。

## 1. 工具链与配置基线

- `rust-toolchain.toml` 锁定 stable 版本（起始 MSRV 记录在 workspace `Cargo.toml` 的 `rust-version = "1.85"`；调整需决策 issue 确认）。
- workspace 根 `Cargo.toml` 统一 `[workspace.lints]`，所有 crate `lints.workspace = true`，禁止 crate 自行放宽：

```toml
[workspace.lints.rust]
unsafe_code = "forbid"          # qn-nono 例外：若采用 FFI 集成，其绑定层允许 unsafe，块外必须注释 SAFETY:（子进程模式则无例外）
missing_docs = "warn"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
todo_without_issue = "warn"     # 需要自定义 lint 时允许去掉，裸 TODO 由 review 兜底
module_name_repetitions = "allow"   # qn-nono::nono_mod 这类允许
cognitive_complexity = "warn"
```

- `deny.toml`：licenses 只允许 `MIT|Apache-2.0|BSD-2|BSD-3|ISC|Unicode-3.0|Zlib`；`MPL-2.0` 仅限已批准依赖的传递依赖（规则见 04 §2，review 强制）；出现 `GPL` 系列依赖必须以"外部进程调用"方式使用并在 PR 说明（如 QEMU）。
- 提交前：`cargo fmt --all -- --check` 无 diff；`cargo clippy --workspace --all-targets -- -D warnings` 零告警。

## 2. 命名与词汇表

| 概念 | 统一词 | 禁用 |
| --- | --- | --- |
| 一次受限进程环境的运行实例 | `Session` | Sandbox、VM、Machine、Box、Container |
| 会话标识 | `SessionId`（`sxn-` 前缀 + uuid v7） | id、key 混用 |
| 引擎适配器 | `*Adapter`（如 `NonoAdapter`；P2 的 `VmAdapter`） | Driver/Backend/Provider/Engine 混用 |
| 凭证代答组件 | `CredentialBroker`（qn-broker） | injector（禁用词） |
| MCP mediation 产出 | `Handle` | token（禁用）、secret |
| 控制面后端 | `qn-daemon`（二进制 `qnd`） | server、service 混用 |
| 停止方式 | `StopMode::{Graceful, Kill}` | shutdown/terminate/force 混用 |

缩写规则：`Id`、`Url`、`Rpc`、`Tcp` 大写驼峰内全大写；crate 内部私有实现允许上下文清晰的短名。

## 3. 类型与 API 设计

- **新类型化**（newtype）一切领域 ID：`struct SessionId(Uuid)`，禁止 `String` 满天飞导致参数串位。
- 构造函数统一 `new` + 校验，失败返回 `Result`；提供 `TryFrom<&str>` 用于解析输入。
- 状态机显式化：会话状态 `enum SessionState { Starting, Running, Stopped, Failed }`，非法迁移在 core 集中定义 `fn transition(&self, next) -> Result<SessionState>`，适配器不得各自定义状态语义。
- 公共 API 参数 ≥ 4 个时引入参数结构体（`…Params`）；`SandboxSpec` 即此模式。
- trait 面向"最小必需"：先有调用方再定签名；为"以后可能"加的方法一律拒绝（记 TODO）。
- `Display`/`Debug`：领域类型必须实现；`Debug` 输出不得含敏感字段（密码/token 用 `#[derivative]` 或手写脱敏）。
- serde：对外结构 `deny_unknown_fields` + `#[serde(rename_all = "snake_case")]`；版本演进用 optional 字段，不做破坏性改名。

## 4. 错误处理（细则）

```rust
// ✅ 本 crate 错误：来源完整、信息可行动
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("session {id} is not running (state: {state}); check 'qn status'")]
    NotRunning { id: SessionId, state: SessionState },
    #[error("profile {profile} not found; run `qn profile list` first")]
    ProfileMissing { profile: ProfileRef },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// ❌ 禁止的模式
.map_err(|e| anyhow!("failed"))            // 丢上下文
Err(Error::Other("bad input".into()))      // 字符串错误类型（唯一逃生口：真正的动态来源错误才允许 Other，且必须带来源）
.unwrap()                                   // R5
```

- 库 crate（core/driver/config/infra）公共签名禁止 `anyhow::Result`；应用 crate（cli/daemon 内部）可用 `anyhow` 简化，但跨 daemon API 边界必须转回类型化错误。
- `?` 链上的每一步"语义转换点"才写 `map_err`，机械透传直接 `?`。
- 超时一律 `tokio::time::timeout` 包裹并在错误中标明时长；取消（cancel）与超时是不同错误变体，不得混用。

## 5. 并发

- 共享可变状态：优先 `mpsc` 消息驱动（状态机天然适合）；必须共享时 `Arc<StdMutex<T>>`（短临界区，锁内禁止 `.await`）或 `tokio::sync::Mutex`（跨 await 持锁）。
- 后台任务必须有生命周期：`JoinHandle` 保存、随父任务 Abort 或显式 shutdown；禁止"发射后不管"的 `tokio::spawn`。
- 所有外部 IO（进程、文件、网络、WSL 命令）有超时；驱动层操作可配置。
- 信号处理：daemon 必须处理 SIGINT/SIGTERM（Windows: CTRL_C/CTRL_CLOSE），优雅停止子资源后退出。

## 6. 平台代码（R6 展开）

- `unsafe` 只允许出现在 `qn-nono` 的 FFI 绑定层（若采用 FFI 集成；子进程模式则全仓 forbid）；每个 unsafe 块上方 `// SAFETY: <不变式>`。
- 外部命令调用统一走 `qn-infra::process`（记录命令行、超时、退出码、输出截断策略），禁止各处散落 `std::process::Command`。
- 路径处理统一 `camino::Utf8PathBuf`（跨 WSL/Windows 路径转换显式化），禁止字符串拼路径。
- WSL 桥接命令（`wsl.exe -d …`）的输出解析必须容忍本地化输出，只依赖退出码与结构化标记。

## 7. 测试代码规范（细则见 05）

- 单元测试与被测代码同文件 `#[cfg(test)] mod tests`，命名 `it_<行为>` 或中文场景注释 + snake_case 名。
- 测试允许 `unwrap()`/`expect()`；断言必须带信息：`assert_eq!(got, want, "sandbox {id} should be stopped after kill")`。
- 禁止 sleep-based 测试（用 tokio 时间冻结或事件等待）。

## 8. 快速正反例对照

```rust
// ❌ 平台分支泄漏进业务
pub async fn list_sandboxes() -> Vec<SandboxInfo> {
    if cfg!(target_os = "macos") { vf_list().await } else { wsl_list().await }
}

// ✅ registry 装配，业务只见 trait
pub async fn list_sandboxes(reg: &Registry) -> Result<Vec<SandboxInfo>> {
    reg.driver().list().await
}
```

```rust
// ❌ 魔法数字 + 字符串状态
if elapsed > 30 { status = "dead" }

// ✅ 命名常量 + 类型化状态
const GRACEFUL_STOP_TIMEOUT: Duration = Duration::from_secs(30);
if elapsed > GRACEFUL_STOP_TIMEOUT { state = SandboxState::Failed; }
```

```rust
// ❌ 重复实现别人写过的
fn retry_with_backoff<F>(f: F) -> … // 手写 40 行
// ✅ 用矩阵中已批准的依赖（04 §3）或 core 里已有的 retry
```
