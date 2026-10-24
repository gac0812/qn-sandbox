# 04 — 依赖政策与复用矩阵（不重复造轮子）

状态：Draft · 2026-10-24 · 架构基座：thin product on nono

## 1. 决策流程

```
1. 本仓库已有？  grep 能力词 + 看 qn-infra / qn-core
2. 复用矩阵有？  §3/§4 已批准的直接用
3. 矩阵没有？    搜 crates.io / npm / PyPI，按 §2 五问评估
4. 全不合格？    自研 —— PR 写明 1-3 步查过什么、为何不合适
```

## 2. 引入新依赖的"五问"（答案写进 PR 描述）

| # | 问题 | 通过线 |
| --- | --- | --- |
| 1 | License？ | MIT / Apache-2.0 / BSD / ISC / Unicode-3.0 / Zlib；MPL-2.0 仅限已批准依赖的传递依赖（弱 copyleft、文件级不传染，如 dirs → option-ext），新增**直接**依赖不得使用 |
| 2 | 维护活跃？ | 近 6 个月有提交或宣布稳定 |
| 3 | 平台覆盖？ | macOS（arm64+x64）与 Windows（经 WSL2）所需场景 |
| 4 | 依赖树多大？ | 传递依赖 ≤ 30（重框架另议），无版本冲突 |
| 5 | 工具链兼容？ | 不压低 rust-version，不强制 nightly |

架构级依赖（安全内核、RPC/契约框架、GUI 框架）额外需**三人在 issue/PR 中确认**（07 §3）。

## 3. 已批准的 Rust 依赖矩阵（⭐ = 该能力唯一入口）

### 安全与产品基座
| 能力 | 依赖 | 备注 |
| --- | --- | --- |
| 安全内核（进程沙箱/凭证代理/L7 网络规则/审计） ⭐ | **nono**（外部单二进制，Apache-2.0） | 只经 `qn-nono` 适配器调用（子进程或 FFI，M0 spike 定）；**锁版本**，升级走独立 PR（06 §5）；五问：Apache-2.0 ✓ / 4.7k stars·1.8k commits·Sigstore 团队 ✓ / macOS 原生+Windows WSL2 ✓ / 单二进制 ✓ / Rust 生态 ✓ |
| 桌面壳 ⭐ | **Tauri**（v2） | guardrails 指定；qn-ui 唯一使用；后端逻辑全在 qn-daemon |
| OS 凭证库 | `keyring` | 仅控制面自身 token/配置用；**Agent 凭证的真实存储跟随 nono 的宿主侧机制**，不建第二套 |

### 基础设施
| 能力 | 依赖 |
| --- | --- |
| 异步运行时 ⭐ / 异步 trait ⭐ / 取消 ⭐ | `tokio` / `async-trait` / `tokio_util::CancellationToken` |
| 错误定义 ⭐ / 日志 ⭐ | `thiserror`（库层）+`anyhow`（应用层）/ `tracing` + `tracing-subscriber` |
| 序列化 ⭐ / CLI ⭐ | `serde`+`serde_json`+`toml` / `clap`(derive) |
| ID ⭐ / 路径 ⭐ / 目录 | `uuid`(v7) / `camino` / `dirs`（qn-config 独占） |
| 重试 / 文件锁 ⭐ | `backon` / `fd-lock` |
| RPC/契约（M3 起） | ConnectRPC 生态（`connect-rpc`/`prost`；与契约生成同源） |
| 校验 / 时间 | `garde` / `jiff`（M1 拍板其一，禁止混用） |

### 平台适配
| 能力 | 来源 | 备注 |
| --- | --- | --- |
| WSL2 操作 | `wsl.exe` 外部命令（qn-infra::process 统一封装） | 无成熟 crate；输出解析只依赖退出码与结构化标记 |
| WSL2 内核升级（Landlock ≥6.7） | 微软官方内核 msix/脚本 | qn-wsl 供给链路实现，M0 spike 验证路径 |

## 4. 明确"自研清单"（无现成通用件，防止误造误引）

| 能力 | 为什么自研 | 放哪 |
| --- | --- | --- |
| nono 之上的产品化适配（安装/锁版/状态/why 透传） | 产品本体 | qn-nono |
| grant/revoke 生命周期与审计呈现 | 业务规则 | qn-core + qn-broker |
| MCP gateway（宿主 mediation） | 产品核心差异化 | qn-gateway |
| 官方 agent profiles 内容 | 领域资产 | profiles/ |
| WSL2 一键供给链路 | 组合编排 | qn-wsl |
| 控制面（CLI/Tauri/daemon） | 产品本体 | qn-cli/qn-ui/qn-daemon |

## 5. 非 Rust 侧

| 端 | 依赖 |
| --- | --- |
| TS SDK | `@connectrpc/connect` + 生成器；测试 `vitest` |
| Python SDK | connect python 生态；`ruff`+`mypy`+`pytest` |
| CI 工具 | `cargo-deny`、`cargo-audit`、`taplo` |

## 6. 上游（nono）跟版纪律

- **锁版本**：版本号记录在 qn-config 常量与安装器清单；升级 = 独立 PR（标题 `chore(deps): bump nono to <ver>`），正文列行为差异与回归验证证据。
- 不 fork、不改上游源码；缺失能力先提 issue/PR 回馈上游；上游不可用时由 adapter 层吸收（禁止把补丁散进产品层）。
- pre-1.0 风险：每个里程碑开始时核对上游 release notes；发现 breaking 变化第一时间在 adapter 内适配。

## 7. 复用反例（review 直接拒）

- 手写 landlock/seccomp/Seatbelt 调用（绕过 nono）——违反"引擎唯一入口"。
- 手写 base64/hex/uuid/路径拼接/重试循环/duration 解析。
- 为一个函数引入全家桶；或"少一个依赖"为由手写 30 行通用件。
- 抄来路不明的工具函数（无出处无测试）。
