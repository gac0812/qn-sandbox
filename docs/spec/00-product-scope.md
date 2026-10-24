# 00 — 产品范围与里程碑

状态：Draft（待用户确认范围） · 2026-10-24 · 最近更新：2026-10-24（补充桌面级定位与凭证代理，见 ADR-0004）

## 1. 问题定义

AI Agent（编码代理、自动化流程）需要一个地方安全地执行代码：不能污染宿主机、不能裸奔联网、
崩了要能恢复、并能被程序化地驱动（exec / 文件传输 / 状态查询）。

真实用户跑 Claude Code、Codex、dsh 这类 Agent 应用时还有一层凭证难题：
为了让 Agent 应用及其 MCP 服务用上 LLM API、GitHub 等外部服务，用户被迫把 API key、密码
明文写进沙箱/容器的配置文件——隔离环境反而成了密钥泄露面，且逐个应用逐台机器重复配置。

现有方案要么只支持单一平台（E2B 依赖 KVM、arcbox 依赖 Apple Silicon），
要么按 OS 各自为政（Seatbelt / AppContainer / namespaces），没有统一 API，更没有统一的凭证方案。

## 2. 产品定位与设计原则

**桌面级 AI 安全沙箱**。两个定位词的排序是刻意的：

1. **安全是核心**。Agent 的代码执行、文件访问、网络出入都被限制在沙箱边界内；
   长期凭证（API key、password、token）只保存在宿主机操作系统凭证库，
   **沙箱内配置任何 Agent 应用、任何 MCP 服务，均无需填写任何敏感信息**——
   由凭证代理（Credential Broker）按引用代持、在宿主网关代答、可随时撤销——
   凭证明文绝不进入沙箱的文件系统、环境变量与进程参数（guardrails 不变式，见 ADR-0004 / 08 §4）。
2. **用户友好是第二目标**。面向最终用户（而不只是会用终端的工程师）：
   - 安装后一条命令创建"开箱即跑 Claude Code"的沙箱（Agent 应用模板）；
   - MCP 配置在宿主机侧代管，沙箱内只出现凭证引用，不出现明文；
   - 默认安全（无默认挂载、网络受控）通过引导式交互放行，不要求手写网络/挂载/代理配置；
   - 不要求用户理解 VM、WSL、Hyper-V 等底层机制——消化平台差异是本项目的问题，不是用户的问题。

## 3. 目标平台（本项目的"跨平台"定义）

| 平台 | 隔离技术 | 状态 |
| --- | --- | --- |
| macOS 13+ Apple Silicon (M1–M4) | Hypervisor.framework / Virtualization.framework，arm64 Linux guest | ✅ 继承 arcbox-custom，主力路径 |
| macOS 13+ Intel | Virtualization.framework，x86_64 Linux guest | 🆕 新增（VZ 本身支持 Intel，但 arcbox 镜像与流水线需适配双架构） |
| Windows 11 / Win10 22H2（含 Home） | WSL2 内运行 Linux 沙箱（容器优先，嵌套 microVM 备选） | 🆕 新增，Windows 主力路径 |
| Windows（Pro 且启用 Hyper-V） | Hyper-V VM 或 QEMU+WHPX | ⏳ 备选后端，非首期 |

架构约束（写死，不做支持矩阵之外的承诺）：
- guest 一律是 **Linux**（与 arcbox 的 agents、与 E2B 生态一致）。
- guest 架构跟随宿主：Apple Silicon → aarch64，Intel Mac → x86_64，Windows (x64) → x86_64。
  跨架构 guest（如 Apple Silicon 上跑 x86_64）属非目标。
- Windows 基线 = WSL2 可用（`wsl --version` 正常）。这是 Windows Home 的唯一可行路径，也是 Pro 的推荐路径。

## 4. 交付优先级与里程碑（P0/P1/P2 ↔ M0–M5）

> `P0/P1/P2` 是仓库级交付优先级（GitHub 标签，见 CONTRIBUTING.md，**不是版本号/分支名**）；
> `M0–M5` 是工程工作包编号，供各规范交叉引用。原则：**P0 = 先打通安全路径**（单平台即可交付）、
> P1 = 平台广度与统一 API、P2 = 高级能力。

| 交付优先级 | 工作包 | 一句话 |
| --- | --- | --- |
| **P0** | M0 + M1 | 安全路径端到端（macOS Apple Silicon） |
| **P1** | M2 + M3 + M4 | 三平台覆盖 + 统一 API 面 |
| **P2** | M5 | 高级能力 |

### P0 — 安全路径（黑客松核心交付）

**M0 — 规范与骨架（当前）**
- 规范文档集、ADR、cargo workspace 骨架、CI 骨架接入（fmt/clippy/test/deny，TODO-001~003）。
- 复用引入：arcbox-custom 以 subtree 进入。
- 沙箱引擎/enforcement 层的架构对齐拍板（ADR-0005，不阻塞 M1 启动）。

**M1 — macOS 安全路径**（含自 M4 提前的 MCP 网关最小切片，与审计/攻击演示）
- 沙箱适配器（Sandbox adapter）：`qn-driver-vf` 完成 create/start/stop/rm/exec/文件进出；exec 支持 **PTY/交互式流**（跑交互式 Agent CLI 的硬前提）。
- **凭证代答网关 v0**（Credential broker）：宿主凭证库（Keychain）+ 认证代答网关（复用 arcbox SOCKS5 代理底座）；沙箱内只有无凭证出口代理与短时占位符/句柄，凭证明文不进沙箱。
- **MCP 网关最小切片**：覆盖 claude-code / codex 两个 profile 所需的宿主侧 MCP 代答（协议通用化延后至 M4）。
- **Agent profile v0**：`qn create --agent claude-code`（含 codex）一条命令创建沙箱，凭证自动接入。
- **文件系统/网络默认拒绝**，按 profile 显式放行（guardrails 不变式；细则见 08 §2/§3）。
- **最小桌面控制面**：Tauri 壳复用 qn-daemon——状态、授权、审计可见与撤销（完整桌面体验属 P2）。
- **审计与攻击演示**：`qn audit` 查询；攻击演示剧本（尝试读取凭证/越权出站/环境变量探测，全部被拦截且产生审计事件）。
- 验收：干净 Mac 上 5 分钟内完成"安装 → `qn create --agent claude-code` → 沙箱内 Claude Code 可用（全程零明文凭证）"；`qn revoke` 立即断流；攻击演示全拦截。

### P1 — 平台广度与统一 API 面

**M2 — Intel Mac**
- 镜像体系支持双架构（`--arch aarch64|x86_64`），VZ 后端按宿主架构选择 guest；Agent profile 出双架构版本。
- 验收：同一套 CLI 命令在 Intel Mac 上跑通 M1 的 E2E。

**M3 — Windows (WSL2)**
- `qn-driver-wsl`：WSL2 发行版作为 Linux 载体，其上以容器（crun/runc 或 Docker 引擎可选）提供每沙箱隔离。
- exec / 文件走 WSL 的互操作或 guest agent；代答网关在 Windows 凭证库（Credential Manager）上等价落地。
- 验收：Windows 11 Home，`winget install` 后跑通同一 E2E（含 `--agent claude-code` 全流程零手工凭证）。

**M4 — 统一 API 面**
- 守护进程 + HTTP API（ConnectRPC，与 arcbox 协议对齐），TS/Python SDK（契约生成）。
- **MCP 网关协议通用化**：M1 最小切片扩展为协议感知代答（stdio 类 MCP 服务器由宿主代认证）。
- **MCP 配置代管**：`qn mcp` 在宿主机侧管理沙箱内 Agent 应用的 MCP 配置，只写凭证引用（`${qn:secret/<name>}`），写明文即拒绝；宿主侧 **qn-mcp server**（沙箱即 MCP 工具，供 Agent 框架调用）。
- 验收：同一份契约生成的 SDK 在三平台驱动同一 E2E；沙箱内 MCP 服务器配置全程零明文凭证。

### P2 — 高级能力（M5，范围再议）
- 快照/恢复、网络白名单管理 UI、端口转发、资源配额、镜像仓库、凭证授权审计报表、QEMU 兜底后端、完整桌面体验。

## 5. 非目标（明确不做，防止范围蔓延）

- ❌ 云端托管服务（本项目是本地软件；"self-host 的多租户云"不是目标）。
- ❌ 跨架构 guest 模拟（不在 Apple Silicon 上跑 x86_64 VM）。
- ❌ Windows 原生进程级沙箱（AppContainer/Seatbelt 路线）——那是另一个产品形态，与"Linux 沙箱 + 统一 API"冲突。若需要，作为独立后端再立项。
- ❌ 非 Linux guest（BSD / Windows guest）。
- ❌ 凭证云同步/团队凭证库——凭证只存本机 OS 凭证库；多设备同步交给用户自己的密码管理器（导入导出作为显式功能，M5 再议）。
- ❌ 完整 GUI 桌面体验（P0 仅含**最小 Tauri 控制面**：状态/授权/审计可见与撤销；完整桌面体验属 P2，基于 Tauri 复用 qn-daemon，不移植 SwiftUI）。
- ❌ 凭证以任何形式进入沙箱（环境变量、argv、文件、短时 token 下发）——这是不变式，不是取舍（ADR-0004 / 08 §4）。

## 6. 竞品/参考项目的取舍结论

| 项目 | 借什么 | 不借什么，为什么 |
| --- | --- | --- |
| [arcbox-custom](https://github.com/laitekin/arcbox-custom) | macOS 全栈：VZ 集成、guest agents、镜像导入、SOCKS5 代理、隔离机器、ConnectRPC 协议 | SwiftUI 桌面端（Windows 不可用；M5 再议 Tauri）；其凭证处理不满足"零明文进沙箱"目标，需自建凭证代理 |
| [E2B](https://github.com/e2b-dev/E2B) | API 契约形态（exec/文件/生命周期）、SDK 用户体验、envd 思路 | Firecracker 栈（KVM-only，Windows/mac 不可用） |
| [microsandbox](https://github.com/microsandbox/microsandbox) | 微 VM 管理面交互设计、单二进制分发思路 | 其自研 VM 栈依赖平台 KVM/HVF，Windows 支持不完整 |
| Codex sandbox（OpenAI） | 权限模型分层（读/写/网络的策略分档）思路 | Seatbelt/AppContainer 是进程级方案，与本项目 VM 级路线不同 |
| Microsoft execution containers | 跨平台策略抽象的描述方式 | 实现绑定其自家栈 |

## 7. 成功标准（产品级）

1. 一套 CLI/SDK 命令，三平台行为一致（差异仅在安装方式）。
2. 从安装到 Agent 应用跑通第一个任务 ≤ 5 分钟，全程无手工技术配置。
3. **在沙箱内配置并运行主流 Agent 应用（Claude Code、Codex 等）及其任意 MCP 服务时，
   全程无需在沙箱内填写任何 API key/密码，且凭证明文不进入沙箱的文件系统、环境变量与进程参数；
   宿主机可随时撤销某沙箱的凭证授权并立即生效。**
4. Agent 在沙箱内的任何操作都不能逃逸到宿主机（见 `08-security.md` 威胁模型）。
5. 新增一个平台后端不需要改动 core / surfaces 任何代码（只新增一个 crate + feature）。
