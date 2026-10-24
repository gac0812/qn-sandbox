# ADR-0005 — 沙箱引擎：采用 nono（调研已完成，推荐选项 C，待三人确认）

- 状态：Proposed（rev2：调研完成，含推荐结论；**待评审拍板**）
- 日期：2026-10-24
- 决策人：AI 调研与提议，三人评审
- 关联：guardrails README「Target Architecture」、ADR-0001（arcbox 基线）、ADR-0002（平台后端）、ADR-0004（凭证网关）
- 调研来源：[nolabs-ai/nono](https://github.com/nolabs-ai/nono)（2026-10 查看）、[HF 官方博客](https://huggingface.co/blog/lukehinds/nono-agent-sandbox)（2026-02）

## 1. nono 是什么（调研事实）

**能力型安全 Shell**：用内核原语把进程锁进不可逆的沙箱——Landlock（Linux）/ Seatbelt（macOS），
"限制一经施加，没有 API 可以绕过，nono 自己也不行"；子进程继承限制；敏感路径（`~/.ssh`、`~/.aws`）默认阻断。

| 维度 | 事实 |
| --- | --- |
| 形态 | 单二进制，非特权运行；CLI + Rust/C FFI + Python 绑定 |
| 平台 | macOS 原生（Seatbelt，10.5+，fs+network）；Linux（Landlock：fs 需内核 5.13+，**fs+network 需 6.7+**）；Windows 原生 ✗（可在 WSL2 内以 Linux 形态运行） |
| 凭证 | **代理式注入**：API key 留在沙箱外，出站流量经代理附加认证；**phantom tokens**（短时占位符，限定只有获批的工具能用某凭证）——与 guardrails「scoped placeholders + upstream allowlists」措辞几乎逐字对应 |
| 网络 | 域名过滤 + L7 规则（HTTP method/path） |
| Profiles | 可组合 JSON 策略包（fs 范围/网络白名单/凭证/工具规则），支持 `--extends` 继承，有注册表 registry.nono.sh |
| 审计 | 防篡改会话/决策日志（可选签名）；Sigstore 技能/指令 attestation；文件快照回滚 |
| 活跃度 | 4.7k stars / 1.8k+ commits（2026-10），Sigstore 团队（Luke Hinds），pre-1.0（安全审计进行中） |
| License | Apache-2.0 |

**与 guardrails P0 清单的对照**：Sandbox adapter≈包装 nono；Credential broker≈nono 的 phantom-token 代理；Profiles≈nono profiles；Audit≈nono 防篡改日志。guardrails 作者显然是按 nono 的实际能力写的目标架构。

## 2. 选项（rev2 修订）

### 选项 A：纯自研平台隔离（原 ADR-0002 形态）
VM/WSL2 driver + qn-agent 内自研 enforcement（seccomp/landlock 手写）+ 自研凭证网关（演进 arcbox SOCKS5）。
- 优点：完全自主；VM 级隔离最强。
- 缺点：**重复造轮子规模巨大**——nono 已实现我们 P0 六项中的四项半；VM 路线开发周期远超黑客松窗口。

### 选项 B：VM driver + nono 作 guest 内策略内核
VZ/WSL2 提供虚拟化边界，nono 在 guest 内收紧权限（原 rev1 推荐方向）。
- 优点：双层防御（独立内核 + 内核原语），隔离强度最高；arcbox 复用保留。
- 缺点：VM 的全部代价（镜像、文件同步、启动慢、UX 割裂）；对 P0 而言过重。

### 选项 C：nono 作为 P0 主引擎（thin product on nono）★ 推荐
- **macOS（P0）**：nono 宿主直跑（Seatbelt）——`nono run --profile qn/claude-code -- claude`，agent 进程级受限运行在**真实项目目录**上（无 VM 文件同步问题）。
- **Windows（P0）**：qn 负责 WSL2 发行版供给，nono 在 WSL2 内运行 agent（Landlock）；qn 把"装好 WSL2+nono"封装成一键体验。
- **我们的产品层**（不可被 nono 替代的部分）：qn CLI + 最小 Tauri 控制面、claude-code/codex 官方 profiles 的编写与发布、**MCP gateway**（宿主侧 mediation）、审计/授权的呈现与撤销 UX、Windows WSL2 供给链路。
- 优点：P0 六项几乎全部落到"包装与产品化"上，黑客松窗口内可交付；平台承诺兑现（mac 原生 + win WSL2）；与 guardrails 意图一致（"intentionally thin rather than a broad fork"）。
- 缺点：进程级隔离弱于 VM（同一宿主内核；Landlock/Seatbelt 不可逃逸但内核漏洞不在防御面内）；nono pre-1.0。
- 风险与缓解：pre-1.0 → 锁版本 + 我们的 adapter 层隔离其 API（可替换）；WSL2 内核 6.7+ 才有 Landlock 网络限制（WSL2 原生内核常为 6.6.x）→ **P0 前置 spike：验证 WSL2 内核版本/自内核升级路径**；Seatbelt 已被 Apple 标记 deprecated（长期）→ 记录为平台风险，跟踪 nono 上游的 macOS 策略。

### 选项 D（作为 C 的演进路径，非独立选项）：P2 增设"强隔离模式"
对需要独立内核的合规场景，P2 提供 VZ VM（arcbox）+ guest 内 nono（即选项 B 形态）作为可切换的高档模式。
ADR-0001/0002 不废弃：降级为 P2 参考/实现路径。

## 3. 对比

| 维度 | A 自研 VM | B VM+nono | C thin-on-nono ★ |
| --- | --- | --- | --- |
| 复用度（不造轮子） | 低 | 中 | **高** |
| P0 交付风险（时间盒） | 高 | 高 | **低** |
| 隔离强度 | VM 级 | VM+进程双层 | 进程级（内核原语，不可逃逸） |
| 三平台覆盖 | 好 | 好 | mac 原生 / win=WSL2（spike 待验证） |
| 与 guardrails 对齐 | 部分 | 部分 | **完全** |

## 4. 结论（推荐，待确认）

**选项 C 为 P0/P1 引擎，选项 D（VM+nono 强隔离模式）留作 P2。**
若三人确认，连带修订：ADR-0001（arcbox subtree 降级为 P2 参考）、ADR-0002（后端策略改为"nono adapter + WSL2 供给；VZ/QEMU 转强隔离模式"）、ADR-0004（CredentialBroker 实现改为包装 nono phantom-token 代理，而非演进 arcbox SOCKS5——领域模型与不变式不变）、00/01/09 相应规格联动。

**前置 spike（P0 开工前，B 负责人即 gac0812）**：
1. WSL2 内核版本与 Landlock network（6.7+）可用性验证；
2. nono 在 macOS 上跑 claude-code 的 profile 实测（fs/网络/凭证代理行为）；
3. nono Rust FFI 集成方式确认（作为库链接 vs 子进程调用）。

## 5. 后果

- 正面：P0 从"自研四个子系统"变为"包装一个成熟内核 + 产品层"，交付概率大幅上升；与队友架构意图统一。
- 负面：接受进程级隔离的威胁模型（内核漏洞不在防御面——文档明示）；绑定 nono 上游节奏（pre-1.0）。
- 重审条件：nono 停止维护/License 变更；或 Windows 原生支持出现；或合规需求要求 VM 级隔离提前。
