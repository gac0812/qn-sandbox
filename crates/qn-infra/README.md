# qn-infra

被各层复用的通用机制（不含领域概念，spec 01 §2）。**禁止**在本 crate 出现
Sandbox/Session/Profile 等领域词汇。

## 模块

| 模块 | 职责 | 状态 |
| --- | --- | --- |
| `process` | 外部命令统一执行器：argv-only、超时、取消、有界输出、tracing | ✅ M0 |
| `error` | crate 统一错误（`thiserror`） | ✅ M0 |
| `fs` / `lockfile` / `logging` / `retry` | 见 spec 01 §2 与 04 §3 | ⏳ 后续 issue |

## process 用法要点

- 任何外部二进制（nono、wsl.exe…）的调用**只**经 `process::run*`（AGENTS R1、spec 08 §5）。
- `CmdSpec` 是 argv 数组 builder，无 shell 字符串入口；默认超时 120s、单流捕获 1 MiB。
- 需要取消传 `CancellationToken`；需要"必须成功"语义用 `run_checked`（非零码 → `Error::Failed`）。

## 非目标

- 领域逻辑、平台供给（去 qn-wsl）、引擎调用（去 qn-nono）。
