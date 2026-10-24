# ADR-0003 — API 契约先行：单一契约源生成多端（daemon/CLI/MCP/SDK），guest 协议优先复用 arcbox

- 状态：Proposed（待用户确认）
- 日期：2026-10-24
- 决策人：AI 提议

## 背景

同一套沙箱能力要暴露给四种消费者：人（CLI）、Agent 框架（MCP）、应用（TS/Python SDK）、内部（daemon↔driver）。
API 面各自手写会必然漂移（低耦合的敌人是重复定义）。
arcbox 上游已带 ConnectRPC 协议定义与 guest agents；E2B 的 envd 证明"沙箱内统一 RPC 服务"的形态可行。

## 选项

### 选项 A（推荐）：契约文件 = 唯一事实源（proto/Connect），guest 协议复用 arcbox
- proto 定义仓库内唯一一份 → 生成：daemon 服务端、guest agent 存根、TS/Python SDK 类型、（M4）MCP tool 描述。
- guest 侧协议直接复用 arcbox 的 agents/协议；不足处在同源 proto 上扩展（不复制另起一份）。
- 优点：改一处全端一致；与上游同源便于同步；TS/Python 生成质量高。
- 缺点：proto 工作流（buf）引入构建复杂度。
- 风险与缓解：buf 工具链锁版本进 CI；契约评审作为 PR 硬检查。

### 选项 B：REST + OpenAPI 先行，SDK 手写类型
- 优点：调试直观，curl 可用。
- 缺点：流式 exec（双向）在 OpenAPI 表达别扭；TS/Python 类型生成质量参差；与 arcbox 现成协议不复用。

### 选项 C：各端各自定义模型，文档对齐
- 直接违反项目低耦合原则，仅列出作为对照。

## 对比

| 维度 | A proto/Connect | B OpenAPI | C 各自为政 |
| --- | --- | --- | --- |
| 复用度 | 高（arcbox 同源） | 低 | 无 |
| 流式支持 | 原生 | 差 | 不一致 |
| SDK 生成质量 | 高 | 中 | 无生成 |
| 工具复杂度 | 中（buf） | 低 | 低 |

## 结论

**选项 A**。落地：
1. M1 起 proto 目录即仓库契约目录（`crates/qn-core/proto/` 或顶层 `proto/`，随骨架定）；M4 前服务端先以进程内 trait 驱动 CLI，M4 起 daemon 上线 Connect。
2. MCP（M4）：官方 Rust SDK 实现 `qn-mcp`，tool 集合 = 沙箱生命周期 + exec + 文件，映射自 core port，不另立语义。
3. SDK（M4）：生成层 + 薄人体工学层（03 规范）。

## 后果

- 正面：四端一致由机制保证，而非靠人肉同步；上游协议演进可跟随。
- 负面：契约变更多端重生成，PR 体积显大（接受，CI gen-drift 检查兜底）。
- 重审条件：若 MCP/Connect 生态出现破坏性变化或 arcbox 协议停滞，重评传输层。
