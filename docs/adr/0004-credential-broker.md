# ADR-0004 — 凭证代答网关：凭证明文永不进入沙箱（文件系统/环境变量/进程参数/Agent 上下文）

- 状态：Proposed（修订 v2：响应仓库 guardrails 不变式，**废弃原 v0 环境变量注入方案**）
- 日期：2026-10-24
- 决策人：AI 提议
- 关联：Issue #1（guardrails）、仓库 README「Security Invariants」

## 背景

产品承诺：**在沙箱内配置 Claude Code、Codex、dsh 等主流 Agent 应用及其任意 MCP 服务时，无需填写任何敏感信息**。
仓库基线（v0 / guardrails README / Issue #1）把安全不变式定为：

> Real credentials never enter the sandbox filesystem, **environment**, process **arguments**, or agent context.

本 ADR 原方案（v0 = exec 环境变量注入、v1 = 网关代答）**违反该不变式**，已废弃。
本修订版把"宿主代答"提前为唯一路径：从 M1 起就不让任何形式的凭证进入沙箱。

## 选项

### 选项 A（已否决）：exec 环境变量注入
宿主解析凭证引用注入目标进程 env。实现快，但凭证进入沙箱进程环境（`/proc/<pid>/environ` 可读），
直接违反 guardrails 不变式。**否决，不再作为任何阶段的方案。**

### 选项 B（采用）：宿主代答网关 + 沙箱内无凭证出口代理
- 宿主侧：`qn-daemon` 内运行**认证代答网关**（演进自 arcbox SOCKS5 代理）：按 (沙箱 × grant) 在
  出站流量的认证字段上附加凭证。明文的完整路径只有：OS 凭证库 → 宿主内存 → 出站流量认证字段。
- 沙箱侧：`qn-agent` 提供本地出口代理（**无任何凭证**的纯转发）。Agent 应用 / MCP 服务通过
  **非敏感配置**（代理地址，如 `HTTPS_PROXY=http://127.0.0.1:<port>`）走代理——代理地址不是凭证，允许。
- 托管配置（MCP 配置等）只写凭证引用 `${qn:secret/<name>}`，由网关在代答时解析。
- 短期 token 也**不下发**：沙箱内任何进程拿到的只有"可用的网络通路"，不是可提取的凭证。

### 选项 C（已否决）：凭证写入沙箱文件
违反不变式，永久禁止。

## 对比

| 维度 | A env 注入（否决） | B 代答网关（采用） | C 落盘（否决） |
| --- | --- | --- | --- |
| 不变式合规 | ✗（凭证进沙箱 env） | ✓（明文不出宿主） | ✗ |
| 通用性 | 高 | 中高（HTTPS 通用；非 HTTP 协议逐个适配） | 高 |
| 实现成本 | 低 | 中（复用 arcbox 代理底座，减轻） | 低 |
| 撤销/审计 | 中 | 好（网关单点，撤销即断流） | 差 |

## 结论

**选项 B 单轨制**，两个交付切片（不存在"先违反后修复"的阶段）：

- **M1 — HTTPS 出站代答**：覆盖 LLM API（Anthropic / OpenAI / GitHub 等）。arcbox SOCKS5 代理底座
  + 按 grant 附加认证；`qn-agent` 本地出口代理；Agent 应用模板默认走代理。
- **M4 — MCP 网关**：协议感知代答（stdio 类 MCP 服务器由宿主代认证），托管配置全面启用引用校验（明文即拒绝）。

选项 A、C 永久禁止；任何"为了兼容把凭证递进沙箱"的新提案必须推翻本 ADR + guardrails 不变式。

落地清单：
1. `qn-core`：`CredentialBroker` port（grant / write_managed_config / revoke——**无 resolve_env**）
   + `SecretRef` / `Grant` / `AgentAppCatalog`。
2. `qn-daemon`：代答网关实现（keyring 取值、grant 表、代答、审计——只记引用名）；撤销立即断流。
3. `qn-agent`：本地出口代理（无凭证纯转发）；托管配置校验。
4. 镜像/快照：凭证从未进入沙箱，天然无残留；网关侧明文仅存宿主内存，进程退出即消失。
5. 无法走代理的非 HTTP 形态（如本地 IPC 类 MCP）：设计评审逐个给出"宿主代执行"或"显式豁免（需 ADR）"，
   不允许默认放行。

## 后果

- 正面：与 guardrails 不变式完全一致；凭证生命周期（发放/撤销/审计）天然单点；
  "Agent 可用密钥而不可见密钥"成为可现场演示的硬承诺。
- 负面：M1 即需代理底座（复用 arcbox 减轻成本）；无法代理的 MCP 形态需逐个适配或豁免（M4 前出豁免清单）。
- 重审条件：guardrails 不变式被正式修订；或出现无法代理且无法豁免的主流 Agent/MCP 形态。
