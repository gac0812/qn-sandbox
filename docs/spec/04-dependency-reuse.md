# 04 — 依赖政策与复用矩阵（不重复造轮子）

状态：Draft · 2026-10-24 · 本文档是 AGENTS.md R1/R2/R5 的展开

## 1. 决策流程：想写一个"通用能力"时的固定动作

```
1. 本仓库已有？  grep 能力词 + 看 qn-infra / qn-core / arcbox(上游)
2. 复用矩阵有？  下表 §4/§5 已批准的依赖直接用
3. 矩阵没有？    搜 crates.io / npm / PyPI，按 §2 "五问"评估
4. 全部不合格？  自研 —— 在 PR 中写明 1-3 步都查过什么、为何不合适
```

## 2. 引入新依赖的"五问"（答案写进 PR 描述）

| # | 问题 | 通过线 |
| --- | --- | --- |
| 1 | License 是什么？ | MIT / Apache-2.0 / BSD / ISC / Unicode-3.0 / Zlib；GPL 系仅允许"外部进程"方式 |
| 2 | 维护活跃吗？ | 近 6 个月有提交或已宣布稳定；issue 响应不沉寂 |
| 3 | 覆盖我们三个平台吗？ | macOS(arm64+x64) / Windows x64 至少覆盖目标后端所需 |
| 4 | 依赖树多大？ | 传递依赖 ≤ 30 个（heavy 框架类另议）；无互相冲突版本 |
| 5 | MSRV / 工具链兼容？ | 不迫使 workspace 降低 rust-version；无 nightly 强制 |

架构级依赖（运行时、RPC 框架、MCP SDK、虚拟化绑定）额外走 ADR。
**引入后**：写入本文矩阵并标注版本与理由；删除依赖同样要记录原因。

## 3. 已批准的 Rust 依赖矩阵

> 约定：标注 ⭐ 的为"该能力唯一入口"——任何 crate 禁止为同一能力引入第二个依赖。

### 基础设施
| 能力 | 依赖 | 备注 |
| --- | --- | --- |
| 异步运行时 ⭐ | `tokio`（full） | 禁止混入 async-std/smono 等第二运行时 |
| 异步 trait ⭐ | `async-trait` | 与 arcbox 上游一致 |
| 错误定义 ⭐ | `thiserror` | 库 crate；应用层可加 `anyhow` |
| 日志/追踪 ⭐ | `tracing` + `tracing-subscriber`（json + rolling file） | |
| 序列化 ⭐ | `serde` + `serde_json` + `toml` | 配置用 toml，协议用 json |
| CLI ⭐ | `clap`（derive） | 仅 qn-cli |
| ID ⭐ | `uuid`（v7） | SandboxId/镜像层 ID；时间有序利于排序 |
| 时间 ⭐ | `jiff` 或 `chrono`（二选一，M1 拍板） | 禁止混用 |
| 路径 ⭐ | `camino` | 跨 WSL/Win 路径显式 UTF-8 |
| 目录定位 | `dirs` | `qn-config` 独占 |
| 重试/退避 | `backon`（或 `tower` retry layer，M1 拍板其一） | 禁止手写 backoff 循环 |
| 取消 ⭐ | `tokio_util::sync::CancellationToken` | |
| OS 凭证库 ⭐ | `keyring` | 凭证代理（ADR-0004）唯一访问入口，仅 `qn-daemon` 允许依赖；沙箱侧/guest 禁止引入 |
| 文件锁 ⭐ | `fd-lock` | 沙箱注册表并发控制 |
| 压缩/tar ⭐ | `tar` + `flate2`（或复用上游 arcbox 已有实现，若其已含则以上游为准） | |

### 协议与 API 面
| 能力 | 依赖 | 备注 |
| --- | --- | --- |
| RPC/契约 ⭐ | `connect-rpc` rust 生态（`connect-rpc`/`prost`） | 与 arcbox 上游协议同源；ADR-0003 |
| MCP server ⭐ | 官方 `modelcontextprotocol/rust-sdk`（crate 名以 M4 落地时为准） | ADR-0003 |
| HTTP（daemon 对外） | Connect 之上的默认传输即可，不引入 axum 除非必要 | 必要时走 ADR |
| 校验 ⭐ | `garde` 或 `validator`（M1 拍板其一） | |

### 平台驱动
| 能力 | 依赖/来源 | 备注 |
| --- | --- | --- |
| macOS 虚拟化 | **arcbox subtree**（上游已封装 Virtualization.framework） | qn-driver-vf 只做适配，不直接引入第二套 VZ 绑定；上游缺什么优先回灌 |
| Windows WSL | `wsl.exe` / `wsl.exe --manage` 外部命令（qn-infra::process） | 无成熟 crate，视为"外部工具调用"而非依赖 |
| Windows 容器运行时（WSL 内） | 沙箱内用 Linux 侧标准工具（runc/crun via 发行版包），宿主只下发命令 | |
| QEMU 兜底 | 外部 qemu-system 进程（qn-infra::process） | GPL 以外部进程方式使用，合规 |
| 硬件加速探测 | 平台命令（`sysctl hw.hv_support` / WHPX API via `windows` crate，M3 时定） | `windows` crate 仅 qn-driver-qemu/qn-config 允许 |

## 4. 明确"自研清单"（少数必须自己写的，防止误引入或误造）

| 能力 | 为什么自研 | 放哪 |
| --- | --- | --- |
| 沙箱注册表（状态文件 + 锁） | 业务状态机，无现成通用件 | qn-core + qn-infra 锁 |
| 领域状态机 | 业务规则 | qn-core |
| `qn` CLI 交互层 | 产品本体 | qn-cli |
| WSL2 编排 | 组合逻辑（发行版+容器+agent），现成库不存在 | qn-driver-wsl |
| 镜像双架构解析 | 领域规则 | qn-core |

## 5. 非 Rust 侧依赖

| 端 | 能力 | 依赖 | 备注 |
| --- | --- | --- | --- |
| TS SDK | 生成/传输 | `@connectrpc/connect` + `@bufbuild/protoc-gen-connect-es` | 与契约同源 |
| TS SDK | 测试 | `vitest` | |
| Python SDK | 生成/传输 | `connect` 官方 python 生态（落地时定版本） | |
| Python SDK | 质量 | `ruff` + `mypy` + `pytest` | |
| CI 工具 | 静态检查/审计 | `cargo-deny`、`cargo-audit`、`taplo`(toml) | |

## 6. 上游复用（arcbox）的原则

- **先上游，后包装**：能力在 `arcbox/` subtree 中已存在 → 直接调用/复用其类型；需要新接口形态才在 `qn-*` 包装。
- 上游缺口的三种处理（按优先级）：可回灌的通用修复（提 patch，记录 `docs/patches/`）＞ 在 qn 侧组合扩展 ＞ fork 内改（禁止，除非 ADR 批准）。
- 上游协议文件（proto/Connect 定义）视为契约源之一；不复制粘贴其内容到 qn 侧，保持单一来源。

## 7. 复用反例（review 直接拒）

- 自己写 `fn parse_duration(s: &str)` → 应使用已批准依赖或 `humantime` 类（引入前过五问）。
- 自己写 base64/hex/UUID 生成/路径拼接/环境探测 → 同上。
- 为了"少一个依赖"手写 30 行 tar 流处理 → 违反 R1；反之"为了一个函数引入巨大全家桶"违反五问第 4 条。
- 把 Stack Overflow / LLM 记忆里的工具函数抄进来（无出处、无测试）→ 必须改为依赖或自研并补测试与出处说明。
