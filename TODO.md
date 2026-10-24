# TODO — 工程待办

> 格式遵循 AGENTS.md R8：每项有触发条件与验收标准。完成后移入「已完成」区并注明 PR 链接。
>
> 进度（2026-10-24）：TODO-004a 实验完成，报告见 `docs/dev-notes/spike1-wsl2-landlock.md`（PR 关联 #16）；
> kernel 硬前置与降级档裁决在决策 issue #17，收口前 004a 保持开放。

## 待办

| # | 待办 | 触发条件 | 验收标准 | 认领 |
| --- | --- | --- | --- | --- |
| TODO-001 | 接入 CI 工作流：`fmt / clippy / test（平台矩阵）/ cargo-deny` 四道门；Actions 锁定 commit SHA（08 §6） | **cargo workspace 骨架（M0）搭建完毕** | 任一 PR 四门全绿；ubuntu / macos / windows+WSL2 矩阵生效 | 待认领 |
| TODO-002 | 测试覆盖率门禁：`cargo llvm-cov` 报告 + 阈值阻断（core ≥ 90%、适配器逻辑层 ≥ 70%，05 §3） | TODO-001 完成 | 报告随 CI 产出；阈值阻断生效 | 待认领 |
| TODO-003 | 启用分支保护与 PR 模板：`main` 需 PR + CI 必过 + ≥1 人类 approve；模板自动加载 | 仓库推送 GitHub 且 TODO-001 完成 | main 直推被拒；新 PR 自动加载模板 | 待认领 |
| TODO-004a | **spike①：WSL2 内核 ≥6.7 与 Landlock 网络可用性验证**（含内核升级路径：微软官方 msix/脚本） | **M0 启动即做（M1/M2 前置硬条件）** | dev-note 记录：本机 WSL2 内核版本实测、Landlock 网络限制可用性结论、升级方案与风险；若不可行给出降级方案（fs-only + 代理）供团队裁决 | B / gac0812 |
| TODO-004b | **spike②：macOS 上 nono + claude-code 实测**（fs/网络/凭证代理行为、phantom token 语义确认） | M0 启动即做 | dev-note 记录：nono 锁定版本、claude-code 受限运行可行、代理行为与审计输出样例 | A + C |
| TODO-004c | **spike③：nono 集成形态定稿**（Rust FFI 链接 vs 子进程调用：稳定性/体积/升级成本对比） | M0 启动即做 | 决策 issue（背景/选项/结论）+ 01/04 相应条文更新 | A |

## 已完成

（暂无）
