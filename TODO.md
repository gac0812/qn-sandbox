# TODO — 工程待办

> 格式遵循 AGENTS.md R8：每项有触发条件与验收标准。完成后移入「已完成」区并注明 PR 链接。

## 待办

| # | 待办 | 触发条件 | 验收标准 | 认领 |
| --- | --- | --- | --- | --- |
| TODO-001 | 接入 CI 工作流：`fmt / clippy / test（三平台矩阵）/ cargo-deny` 四道门；Actions 锁定 commit SHA（08 §5） | **cargo workspace 骨架（M0）搭建完毕** | 任一 PR 上四门全绿；ubuntu / macos-14(arm64) / windows+WSL2 矩阵生效；对应 workflow 入库 | 待认领 |
| TODO-002 | 测试覆盖率门禁：`cargo llvm-cov` 出报告，随后按 05 §3 目标（core ≥ 90%、driver 逻辑层 ≥ 70%）设置阻断阈值 | TODO-001 完成 | 覆盖率报告随 CI 产出（artifact）；阈值在 CI 阻断；05 §3 表格标注"已生效" | 待认领 |
| TODO-003 | 启用分支保护与 PR 模板：`main` 需 PR + CI 必过 + ≥1 approve，禁止直推；`.github/pull_request_template.md` 自动生效 | 仓库推送 GitHub 且 TODO-001 完成 | main 直推被拒；新建 PR 自动加载模板；门禁与 06 §3.3 一致 | 待认领 |

## 已完成

（暂无）
