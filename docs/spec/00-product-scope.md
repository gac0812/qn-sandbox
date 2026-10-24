# 00 — 产品范围与交付优先级

状态：Draft（对齐 guardrails：thin product on nono） · 2026-10-24

## 1. 问题定义

AI 编码代理（Claude Code、Codex、dsh 等）在用户机器上拥有读写文件、执行命令、联网的能力，
但当前没有顺手的安全边界：SSH/AWS 密钥裸露在 `~/.ssh`、`~/.aws`，一条提示词注入就可能让 agent
读密钥外传或删库。用户被迫在"效率"与"裸奔"之间二选一；就算想隔离，VM/容器方案又要镜像、
同步文件、逐个填 API key，对最终用户太技术化。

现有方案要么重（VM：导入镜像、文件同步、启动慢），要么散（手工配 seatbelt/landlock），
要么与凭证安全冲突（把密钥写进容器配置）。

## 2. 产品定位与设计原则

**桌面级 AI 安全沙箱**。定位词排序是刻意的：

1. **安全是核心**（guardrails Security Invariants 逐条落实）：
   - 真实凭证绝不进入沙箱的文件系统、环境变量、进程参数或 Agent 上下文；
   - Agent 只拿到短时、受限作用域的占位符 / broker 句柄（phantom tokens）；
   - MCP 凭证在宿主侧注入，永不下发给 agent；
   - 文件系统与网络权限默认拒绝、按 profile 放行；
   - 特权动作产生用户可见的审计事件；
   - 撤销会话即刻使占位符与 broker 访问失效。
2. **用户友好是第二目标**：一条命令创建"开箱即跑 Claude Code"的受限环境；profiles 与权限由
   宿主侧代管；不要求用户理解 Landlock/Seatbelt/WSL2——消化机制是产品的问题，不是用户的。
3. **薄产品层**（guardrails："intentionally thin rather than a broad fork"）：内核级隔离能力
   由 [nono](https://github.com/nolabs-ai/nono) 承担（Landlock / Seatbelt，进程级、不可逆、
   零 VM 开销），我们做适配、产品化与 gateway，不自研内核机制、不 fork 上游。

## 3. 目标平台与机制

| 平台 | 隔离机制 | 说明 |
| --- | --- | --- |
| macOS 13+（Apple Silicon 与 Intel 同等） | nono（Seatbelt，进程级 fs+网络） | 宿主直跑，无 VM；Seatbelt 与 CPU 架构无关 |
| Windows 11 / Win10 22H2（含 Home） | qn 供给 WSL2 发行版 + nono（Landlock） | Landlock fs 需内核 ≥5.13，**fs+网络需 ≥6.7**；WSL2 原生内核常为 6.6.x，供给链路负责探测/升级内核（P0 前置 spike） |
| （P2）强隔离模式 | 独立 VM（VZ / QEMU）+ guest 内 nono | 面向需要独立内核的合规场景；默认不开 |

非目标内的平台承诺：不做原生 Windows 进级沙箱（AppContainer 路线）、不做非 Linux guest、
不做跨架构模拟。

## 4. 交付优先级（P0/P1/P2，GitHub 标签）与工作包（M）

| 优先级 | 工作包 | 一句话 |
| --- | --- | --- |
| **P0** | M0 + M1 + M2 | 安全路径端到端（macOS + Windows），六项 guardrails 交付物 |
| **P1** | M3 | 统一 API/SDK 与 profile 生态 |
| **P2** | M4 | 高级能力（含强隔离 VM 模式） |

### M0 — 骨架与验证（P0 起点）
- cargo workspace 骨架、CI 四门接入（fmt/clippy/test/deny，TODO-001~003）。
- **nono 集成三 spike**（TODO-004，P0 开工硬前置）：① WSL2 内核 ≥6.7 与 Landlock 网络可用性验证及内核升级路径；② macOS 上 nono + claude-code profile 实测（fs/网络/凭证代理行为）；③ nono 集成形态确认（Rust FFI 链接 vs 子进程调用）。
- nono 版本锁定策略与安装器集成方案（bootstrap 安装 nono）。

### M1 — macOS 安全路径（P0 核心）
对应 guardrails 六项的实现映射：
1. **Sandbox adapter（qn-nono）**：包装 nono —— `qn run --profile qn/claude-code -- claude` 的产品化入口；状态查询、`nono why` 拒绝原因透传。
2. **Credential broker（qn-broker）**：包装 nono 的凭证代理（phantom tokens + 上游 allowlist），叠加 grant/revoke 生命周期与审计呈现；真实凭证只存宿主侧。
3. **MCP gateway（qn-gateway）**：宿主侧 MCP mediation——agent 配置里只有句柄没有密钥；MCP 服务器的凭证由网关在宿主侧代答。
4. **Agent profiles**：编写并发布 claude-code、codex 官方 profile（fs 白名单、网络域名/L7 规则、凭证绑定）。
5. **Minimal desktop control plane**：qn CLI + 最小 Tauri 壳（profiles/权限/状态/审计四视图，复用同一控制面后端）。
6. **Audit & attack-demonstration**：审计事件的用户可读呈现；攻击演示剧本（读 `~/.ssh`、越权出站、环境变量探测——全部被拦截且留痕）。
- 验收：干净 Mac 上 5 分钟内"安装 → `qn run --agent claude-code` → 真实项目目录受限运行（全程零明文凭证）"；`qn revoke` 立即断供；攻击演示全拦截。

### M2 — Windows 安全路径（P0 收口）
- **qn-wsl 供给链路**：WSL2 发行版安装/内核版本探测与升级（Landlock 网络 ≥6.7）/nono 引导/代理打通，全部封装成一键体验。
- M1 六项在 Windows 上等价落地（profile 同一套）。
- 验收：Windows 11 Home 同一 E2E 剧本跑通（含攻击演示）。

### M3 — 统一 API 与生态（P1）
- 控制面后端稳定 HTTP API + TS/Python SDK（契约生成）；profile 注册表发布流程（registry）；MCP gateway 协议通用化；沙箱即 MCP 工具（供 Agent 框架调用）。

### M4 — 高级能力（P2，范围再议）
- **强隔离 VM 模式**（VZ/QEMU + guest 内 nono，作为可切换的高档模式）、网络策略 UI、资源配额、企业审计报表、快照/回滚产品化。

## 5. 非目标（明确不做）

- ❌ 云端托管服务（本地软件）。
- ❌ 自研内核沙箱机制或 fork nono/上游（薄产品层原则；需求回馈上游）。
- ❌ VM 作为默认形态（P0/P1 全部进程级；VM 仅 P2 可选强隔离模式）。
- ❌ 凭证以任何形式进入沙箱（env/argv/文件/短时真凭证下发）——不变式，非取舍。
- ❌ 凭证云同步/团队凭证库（多设备同步交给用户自己的密码管理器）。
- ❌ Windows 原生进程级沙箱、非 Linux guest、跨架构模拟。

## 6. 参考/基础项目的取舍

| 项目 | 角色 | 说明 |
| --- | --- | --- |
| [nono](https://github.com/nolabs-ai/nono)（Apache-2.0） | **安全内核基座**（非竞品） | Landlock/Seatbelt 进程沙箱、phantom token 凭证代理、L7 网络规则、profiles、防篡改审计——我们包装与产品化，锁版本跟进 |
| [arcbox-custom](https://github.com/laitekin/arcbox-custom) | P2 强隔离模式参考 | VZ 集成与 guest agents 的实现参考（届时评估依赖或仅参考） |
| E2B / microsandbox | API 形态与交互参考 | 其 VM 栈不采用（与薄产品层/进程级路线冲突） |
| Codex sandbox、Microsoft execution containers | 权限模型分层思路 | 机制不同，仅设计参考 |

## 7. 成功标准（产品级）

1. macOS 与 Windows 同一套 CLI/控制面行为一致（差异仅在底层供给）。
2. 从安装到 Agent 受限运行第一个任务 ≤ 5 分钟，无手工技术配置。
3. **沙箱内配置并运行主流 Agent 应用及其任意 MCP 服务，全程零明文凭证；
   凭证不进沙箱 fs/env/argv/agent 上下文；宿主可随时撤销并立即生效。**
4. 攻击演示剧本（读密钥/越权出站/env 探测）全部被拦截且产生用户可见审计事件。
5. 引擎可替换：nono 之上始终有 adapter 层，更换/升级引擎不动产品层。
