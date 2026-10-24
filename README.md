# qn-sandbox · Secure Agent Sandbox

Desktop-grade security sandbox for mainstream AI coding agents.
**桌面级 AI 安全沙箱**：让 Claude Code、Codex、dsh 等主流 Agent 应用在隔离的 Linux 环境里安全干活——安全第一，用户友好第二。

The product is designed around one primary promise: users can run agents such as Claude Code, Codex, and dsh without exposing API keys, passwords, OAuth tokens, or MCP credentials inside the sandbox. Security comes first; setup must remain approachable for users who do not want to write technical policy files.

对应承诺：长期凭证只保存在宿主机操作系统凭证库，由**宿主代答网关**在出站流量上附加认证、可随时撤销；凭证明文绝不进入沙箱的文件系统、环境变量、进程参数或 Agent 上下文。

## Status

This repository is under active development for the Qiniu 1024 Hackathon.

The repository starts from an empty, immutable `v0` baseline. The first deliverable, tracked as `P0`, is a working security path rather than a broad feature surface:

1. Sandbox adapter
2. Host-side credential broker
3. Host-side MCP gateway
4. Claude Code and Codex profiles
5. Minimal desktop control plane
6. Audit and attack-demonstration flow

`P0/P1/P2`（交付优先级标签）与工程里程碑 M0–M5 的映射见 [docs/spec/00-product-scope.md](./docs/spec/00-product-scope.md) §4。

## Security Invariants

Every implementation decision must preserve these invariants:

- Real credentials never enter the sandbox filesystem, environment, process arguments, or agent context.
- Agents receive only short-lived, scoped placeholders or broker handles.
- MCP secrets are injected on the host side and are never sent to the agent.
- Filesystem and network permissions default to deny and are granted per profile.
- Privileged actions produce an auditable event that is visible to the user.
- Revoking a session invalidates its placeholders and broker access.

## Target Architecture

The planned product layer is intentionally thin rather than a broad fork:

- **Desktop control plane:** Tauri-based UX for profiles, permissions, status, and audit review.
- **Sandbox engine:** `nono` as the security kernel and enforcement boundary.
- **Credential broker:** host-side exchange of scoped placeholders for real credentials, with explicit upstream allowlists.
- **MCP gateway:** host-side MCP mediation so agent configuration contains handles, not secrets.
- **Agent profiles:** tested, least-privilege defaults for supported agents.
- **Audit and approvals:** human-readable records of sandbox and credential activity.

crate 级拆分（C4 视图、port 语义、依赖矩阵）见 [docs/architecture-breakdown.md](./docs/architecture-breakdown.md)；
`nono` 与平台 driver 层（ADR-0002）的关系对齐记录于 [ADR-0005](./docs/adr/0005-sandbox-engine-enforcement.md)（Proposed，待评审）。

## Repository Scope

The internal hackathon materials and local credentials are not part of this repository. Proof-of-concept code is kept separate from product implementation and is reviewed through its own pull request.

See [CONTRIBUTING.md](CONTRIBUTING.md) for branch, review, and security rules.

## 文档地图（工程规范 · 中文）

| 文档 | 内容 | 读者 |
| --- | --- | --- |
| [AGENTS.md](./AGENTS.md) | **AI 协作总规范**：AI 在本仓库中如何思考、写代码、验证、汇报 | 所有 AI Agent（必读第一篇） |
| [docs/spec/00-product-scope.md](./docs/spec/00-product-scope.md) | 产品定位、范围、目标平台、P0/P1/P2 ↔ M0–M5 里程碑、非目标 | 所有人 |
| [docs/spec/01-architecture.md](./docs/spec/01-architecture.md) | 分层架构、crate 划分、依赖方向、平台后端 | 开发者 / AI |
| [docs/architecture-breakdown.md](./docs/architecture-breakdown.md) | **架构拆分**：C4 三级视图、组件五要素拆解、port 语义表、通道/数据/部署视图、依赖矩阵 | 开发者 / AI |
| [docs/spec/02-coding-standard-rust.md](./docs/spec/02-coding-standard-rust.md) | Rust 编码规范（整洁代码细则 + 正反例） | 开发者 / AI |
| [docs/spec/03-coding-standard-sdk.md](./docs/spec/03-coding-standard-sdk.md) | TypeScript / Python SDK 编码规范 | 开发者 / AI |
| [docs/spec/04-dependency-reuse.md](./docs/spec/04-dependency-reuse.md) | 依赖政策：不重复造轮子的流程与复用矩阵 | 开发者 / AI |
| [docs/spec/05-testing-quality.md](./docs/spec/05-testing-quality.md) | 测试策略、质量门禁、验收定义 | 开发者 / AI |
| [docs/spec/06-git-workflow.md](./docs/spec/06-git-workflow.md) | 分支、提交、PR 规范、版本化 | 开发者 / AI |
| [docs/spec/07-documentation-adr.md](./docs/spec/07-documentation-adr.md) | 文档规范与架构决策记录（ADR）流程 | 开发者 / AI |
| [docs/spec/08-security.md](./docs/spec/08-security.md) | 安全边界、威胁模型、安全编码要求 | 开发者 / AI |
| [docs/spec/09-team-split.md](./docs/spec/09-team-split.md) | 三人协作分工：主责领域、里程碑矩阵、接口共治、评审安排 | 所有人 |
| [docs/spec/glossary.md](./docs/spec/glossary.md) | 术语表 | 所有人 |
| [docs/adr/](./docs/adr/) | 架构决策记录（已定 / 待定） | 所有人 |
| [TODO.md](./TODO.md) | 工程待办：CI 与测试覆盖率门禁在 M0 骨架完成后接入（TODO-001~003） | 所有人 |

## 工程状态

规范制定阶段（2026-10）。尚未开始编码。待确认决策见 `docs/adr/` 中状态为 Proposed 的条目；
工程待办（CI、测试覆盖率门禁等，workspace 骨架完成后补上）见 [TODO.md](./TODO.md)。
