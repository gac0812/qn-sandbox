# 09 — 三人协作分工（thin product on nono）

状态：Draft（成员 A、B、C 均已认领） · 2026-10-24

## 1. 分工原则

1. 每人独占一条主路径，避免三人改同一处代码。
2. 每人守一条横切线，互不重叠。
3. 接口（qn-core ports）共治，变更走固定流程（§4）。
4. 无空窗期：主里程碑启动前负责前置 spike。
5. 任何人不做自己代码的唯一评审（§5）。

## 2. 角色总览

| | 成员 A | 成员 B | 成员 C |
| --- | --- | --- | --- |
| **认领（GitHub id）** | **[laitekin](https://github.com/laitekin)** | **[gac0812](https://github.com/gac0812)** | **[Weiki886](https://github.com/Weiki886)** |
| **主责领域** | 引擎适配 + macOS + profiles | Windows 供给链路 + 安全演示 | 控制面 + MCP gateway + SDK |
| **主要 crate** | `qn-nono`、`profiles/`、`qn-broker`（phantom 代理包装） | `qn-wsl`、攻击演示与审计剧本 | `qn-daemon`、`qn-cli`、`qn-ui`、`qn-mcp`、`qn-gateway`、SDK |
| **横切线** | nono 上游跟版与回馈（04 §6 锁版纪律） | 安全条款（08）落实与评审 | 架构与依赖方向守门（R3/R6、deny ban） |
| **硬件前提** | Mac（Apple Silicon 或 Intel 均可） | Windows 11 实机 + WSL2 | 无特殊硬件 |

## 3. 按工作包的分工矩阵

> 对应 00 §4：P0 = M0（骨架+spike）+ M1（macOS 安全路径）+ M2（Windows）；P1 = M3；P2 = M4。

（主导 = 交付负责人；支援 = 协作并承担对应评审）

| 工作包 | 成员 A（引擎/macOS/profiles） | 成员 B / gac0812（Windows/安全） | 成员 C（控制面/gateway） |
| --- | --- | --- | --- |
| **M0 骨架与验证** | spike ③：nono 集成形态（FFI vs 子进程）定稿；`qn-nono` 骨架与安装器锁版方案 | spike ①：WSL2 内核 ≥6.7 + Landlock 网络验证与升级路径；spike ②协测；Windows CI 矩阵；`qn-infra::process`/`qn-config` | 主导：workspace/deny/PR 模板/`qn-core` 初版（模型+ports）；spike ②：macOS nono+claude-code 实测 |
| **M1 macOS 安全路径（P0）** | 主导：`qn-nono`（run/status/why/审计采集）、`qn-broker`（phantom 代理包装）、claude-code/codex profiles、macOS E2E | 攻击演示剧本 v1（读密钥/越权出站/env 探测全拦截留痕）；审计事件可读呈现需求 | 主导：`qn-cli`（run/audit/doctor）、`qn-ui` 最小 Tauri 壳、`qn-gateway` MCP mediation 最小版 |
| **M2 Windows（P0 收口）** | profiles 在 WSL2 环境的兼容验证 | 主导：`qn-wsl` 供给链路（发行版/内核/nono 引导/代理打通一键化）、Windows E2E、攻击演示 Windows 版 | 控制面 Windows 适配与打包（winget/msi）；review |
| **M3 统一 API 与生态（P1）** | profile 注册表发布流程；nono 升级回归 | Windows GA 兼容性矩阵（Win10 22H2/11 各版本） | 主导：daemon HTTP API、契约、TS/Python SDK、`qn-mcp`（沙箱即工具） |
| **M4 高级能力（P2）** | 强隔离 VM 模式（`qn-vm`：VZ/QEMU + guest 内 nono） | 资源配额与 Windows 安装引导打磨 | 网络策略 UI、企业审计报表 |

## 4. 接口（port）变更流程

ports（`SandboxEngine` / `CredentialBroker` / `McpGateway`）是三人公共契约：

| 变更类型 | 所需同意 | 记录 |
| --- | --- | --- |
| 加带默认实现的方法 | C + 受影响适配器负责人 | PR 描述 |
| 改签名 / 新增 / 删除 port | **三人一致** | 决策 issue（背景/选项≥2/结论）+ spec 同步修订 |
| 适配器内部实现 | 该负责人 | PR |

- M0 结束时 ports v1 冻结；M1 内允许小修（第一级），M2 起签名改动需三人一致。
- 实现与 port 语义冲突：停下提 issue，不静默绕过。

## 5. 评审与值班

- 互审矩阵：A↔C 互审引擎适配与控制面；B↔C 互审架构与 Windows 代码；A↔B 互审 profiles 与安全演示。横切线相关 PR 必须由横切线负责人看（上游跟版→A，安全→B，架构→C）。
- 周同步（30 分钟）：平台 E2E 红绿、nono 上游动态、阻塞项、下周主导项。
- 轮值：CHANGELOG 与每周 `cargo deny` 巡检三人轮值。

## 6. 再平衡预案

- **spike ① 失败**（WSL2 内核无法达 6.7+）：B 与 A 评估降级方案（fs-only Landlock + nono 网络代理仍可用？）或 Windows 路径改走 P2 VM 模式提前，团队裁决。
- **C 成为瓶颈**（M1/M3 期）：A/B 按互审矩阵分担 01 §8 检查表，C 只保留 port 与依赖方向终审。
- **成员连续缺席 > 1 周**：横切线临时移交互审对成员，写入周同步记录。
