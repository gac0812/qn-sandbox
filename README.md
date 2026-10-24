# qn-sandbox

**桌面级 AI 安全沙箱**：让 Claude Code、Codex、dsh 等主流 Agent 应用在隔离环境里安全干活。
核心是安全，其次是最终用户的友好性——安装即用，不做技术化的手工配置。

在 Windows（x64）与 macOS（Apple Silicon / Intel）上提供安全、可编程、可恢复的 Linux 隔离执行环境：

- **安全优先**：Agent 的代码、文件、网络都被限制在沙箱边界内；所有长期凭证（API key、密码等）
  只保存在宿主机操作系统凭证库，**沙箱内配置任何 Agent 应用、任何 MCP 服务均无需填写敏感信息**——
  由凭证代答网关按引用代持、在宿主侧代答、可随时撤销；凭证明文绝不进入沙箱的
  文件系统、环境变量与进程参数。
- **用户友好**：一条命令创建"开箱即跑 Claude Code"的沙箱；MCP 配置由宿主机代管；
  不要求用户理解 VM、WSL、代理等底层细节。
- **三平台一致**：CLI / MCP / SDK 一套 API，Windows 与 macOS 行为一致（仅安装方式不同）。

- 基线：[arcbox-custom](https://github.com/laitekin/arcbox-custom)（macOS 侧复用其 Rust 运行时与镜像/代理/隔离能力）
- 参考：E2B、microsandbox、OpenAI Codex sandbox、Microsoft execution containers

## 文档地图（阅读顺序）

| 文档 | 内容 | 读者 |
| --- | --- | --- |
| [AGENTS.md](./AGENTS.md) | **AI 协作总规范**：AI 在本仓库中如何思考、写代码、验证、汇报 | 所有 AI Agent（必读第一篇） |
| [docs/spec/00-product-scope.md](./docs/spec/00-product-scope.md) | 产品定位、范围、目标平台、里程碑、非目标 | 所有人 |
| [docs/spec/01-architecture.md](./docs/spec/01-architecture.md) | 分层架构、crate 划分、依赖方向、平台后端 | 开发者 / AI |
| [docs/architecture-breakdown.md](./docs/architecture-breakdown.md) | **架构拆分**：C4 三级视图、组件五要素拆解、port 语义表、通道/数据/部署视图、依赖矩阵 | 开发者 / AI |
| [docs/spec/02-coding-standard-rust.md](./docs/spec/02-coding-standard-rust.md) | Rust 编码规范（整洁代码细则 + 正反例） | 开发者 / AI |
| [docs/spec/03-coding-standard-sdk.md](./docs/spec/03-coding-standard-sdk.md) | TypeScript / Python SDK 编码规范 | 开发者 / AI |
| [docs/spec/04-dependency-reuse.md](./docs/spec/04-dependency-reuse.md) | 依赖政策：不重复造轮子的流程与复用矩阵 | 开发者 / AI |
| [docs/spec/05-testing-quality.md](./docs/spec/05-testing-quality.md) | 测试策略、质量门禁、验收定义 | 开发者 / AI |
| [docs/spec/06-git-workflow.md](./docs/spec/06-git-workflow.md) | 分支、提交、PR、版本化 | 开发者 / AI |
| [docs/spec/07-documentation-adr.md](./docs/spec/07-documentation-adr.md) | 文档规范与架构决策记录（ADR）流程 | 开发者 / AI |
| [docs/spec/08-security.md](./docs/spec/08-security.md) | 安全边界、威胁模型、安全编码要求 | 开发者 / AI |
| [docs/spec/09-team-split.md](./docs/spec/09-team-split.md) | 三人协作分工：主责领域、里程碑矩阵、接口共治、评审安排 | 所有人 |
| [docs/spec/glossary.md](./docs/spec/glossary.md) | 术语表 | 所有人 |
| [docs/adr/](./docs/adr/) | 架构决策记录（已定 / 待定） | 所有人 |
| [TODO.md](./TODO.md) | 工程待办：CI 与测试覆盖率门禁在 M0 骨架完成后接入（TODO-001~003） | 所有人 |

## 状态

规范制定阶段（2026-10）。尚未开始编码。待确认决策见 `docs/adr/` 中状态为 Proposed 的条目；
工程待办（CI、测试覆盖率门禁等，workspace 骨架完成后补上）见 [TODO.md](./TODO.md)。
