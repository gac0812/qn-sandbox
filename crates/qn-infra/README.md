# qn-infra

被各层复用的通用机制（不含领域概念，spec 01 §2）。**禁止**在本 crate 出现
Sandbox/Session/Profile 等领域词汇。

## 模块

| 模块 | 职责 | 状态 |
| --- | --- | --- |
| `process` | 外部命令统一执行器：argv-only、超时（含 drain 阶段）、取消、有界输出、tracing | ✅ M0 |
| `error`（私有 `mod`，经 crate 根 `pub use` 导出） | crate 统一错误（`thiserror`） | ✅ M0 |
| `fs` / `lockfile` / `logging` / `retry` | 见 spec 01 §2 与 04 §3 | ⏳ 后续 issue |

## process 用法要点

- 任何外部二进制（nono、wsl.exe…）的调用**只**经 `process::run*`（AGENTS R1、spec 08 §5）。
- `CmdSpec` 是 argv 数组 builder，无隐式 shell 解释入口；默认超时 120s、单流捕获 1 MiB。
- `CmdOutcome` 同时提供 lossy 字符串与 `stdout_raw`/`stderr_raw` 原始字节——非 UTF-8 输出（如 wsl.exe 的 UTF-16）必须走原始字节自行解码。
- 需要取消传 `CancellationToken`；需要"必须成功"语义用 `run_checked`（非零码 → `Error::Failed`）。
- stdout/stderr 摘要会进 debug 级 tracing——输出可能含敏感内容的调用方须知此行为（08 §7）。
- `env_clear()` 清除环境继承及此前显式设置；之后用 `env()` 重建白名单。`env_remove()`
  移除继承/显式值，之后的 `env()` 可重新设置；按调用顺序执行，大小写语义交给系统。
- 默认仍继承宿主环境，安全调用方必须显式 `env_clear()`；优先传绝对程序路径，按需设置 PATH。
  环境编辑的 Debug 隐藏名称和值；这不改变 argv/输出现有的日志策略，不应向其中传入凭证。
- `spawn_managed(spec, policy)`（#65）返回拥有 stdin 和捕获任务的句柄；`take_stdin()` 交给调用方写入，
  `wait(cancel)` 等待结果，取消 token 即请求终止。输出默认 Debug、事件和错误不带载荷，解析器通过 `output()` 显式访问。
- 平台策略由 bootstrap 注入；取消/超时先终止组再回收，Drop 只作尽力清理并取消读任务。
  Unix 父进程已回收时拒绝用陈旧 PGID 补杀。进程组不能约束主动脱组的进程，不等同于安全沙箱。
- 已知边界（#27）：旧 `run*` 仍仅终止直接子进程；生产 adapter 的受管路径接入及 nono 清理验收尚未完成。

## 非目标

- 领域逻辑、平台供给（去 qn-wsl）、引擎调用（去 qn-nono）。
