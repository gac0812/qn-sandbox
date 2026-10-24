# 01 — 架构规范（thin product on nono）

状态：Draft · 2026-10-24 · 与 guardrails Target Architecture 对齐

M1 执行边界已按 [#14](https://github.com/gac0812/qn-sandbox/issues/14) /
[#47](https://github.com/gac0812/qn-sandbox/issues/47) 的用户授权接受（§4.1）。
这是实现约束；当前交付状态见各 crate README 与 [成员 A 清单 #59](https://github.com/gac0812/qn-sandbox/issues/59)。

## 1. 总体形态

**一套产品层，一个安全内核，一条凭证主线。**

```
┌───────────────────────────  使用者  ───────────────────────────┐
│  人（qn CLI / Tauri 壳）      Agent 框架（MCP）      应用（SDK） │
└───────┬──────────────────────────┬──────────────────┬─────────┘
        │                          │                  │ HTTP（M3 起）
┌───────▼──────────────────────────▼──────────────────▼─────────┐
│              qn 控制面后端（qn-daemon：薄，无业务规则）          │  ← surfaces
│   qn-cli · qn-ui(Tauri) · qn-mcp · SDK                          │
└───────────────────────────────┬────────────────────────────────┘
                                │ 只依赖 qn-core 的 port
┌───────────────────────────────▼────────────────────────────────┐
│                            qn-core                              │  ← 领域层
│  Sandbox/Profile/Grant/AuditEvent 模型 + 状态机 + 统一 Error    │
│  ports：SandboxEngine · CredentialBroker · McpGateway · AuditLog│
└───────┬──────────────┬────────────────┬───────────────┬────────┘
        │              │                │               │  运行期装配（qn-bootstrap）
┌───────▼─────┐ ┌──────▼──────┐ ┌───────▼───────┐ ┌────▼─────────┐
│  qn-nono    │ │  qn-broker  │ │  qn-gateway   │ │   qn-wsl     │  ← 适配层
│ 引擎适配器  │ │ 凭证代答    │ │ MCP mediation │ │ Win 供给链路  │
└───────┬─────┘ └──────┬──────┘ └───────┬───────┘ └────┬─────────┘
        │ 子进程(CLI)   │ phantom 代理    │              │ wsl.exe
┌───────▼──────────────▼────────────────▼──────────────▼─────────┐
│  nono（外部单二进制，锁版本）      OS 凭证库      WSL2 发行版     │
│  Landlock(Linux)/Seatbelt(macOS) · phantom tokens · 审计日志    │
└─────────────────────────────────────────────────────────────────┘
（P2 强隔离模式：qn-vm（VZ/QEMU）+ guest 内 nono，作为可切换引擎，不进 P0/P1）
```

## 2. Crate 划分

| crate | 职责 | 禁止 |
| --- | --- | --- |
| `qn-core` | 领域模型（`Sandbox`/`Profile`/`Grant`/`AuditEvent`、状态机）、ports、统一 `Error` | 依赖任何适配器/surface/平台 API |
| `qn-nono` | **引擎适配器**：nono 安装/版本锁定、`run --profile` 拉起、状态与 `why` 透传、审计日志采集；实现 `SandboxEngine` | 绕过 nono 自行实现隔离；链接第二套沙箱机制 |
| `qn-broker` | **凭证代答**：包装 nono phantom-token 代理 + 上游 allowlist；grant/revoke 生命周期；实现 `CredentialBroker` | 明文进日志/状态文件；自建第二套凭证存储（真实凭证存放跟随 nono 的宿主侧机制） |
| `qn-gateway` | **MCP mediation**：宿主侧代答 MCP 服务器的认证，agent 配置只有句柄；实现 `McpGateway` | 把任何密钥下发进沙箱 |
| `qn-wsl` | **Windows 供给链路**：WSL2 发行版安装、内核版本探测/升级（Landlock 网络 ≥6.7）、nono 引导、代理打通 | 业务规则；非 Windows 平台代码 |
| `qn-daemon` | 控制面后端：装配 adapters、HTTP API（M3 起对外）、状态与审计查询 | 业务规则；直连 nono 细节（经 port） |
| `qn-cli` | `qn` 命令行：人机入口（run/exec/status/audit/doctor） | 业务逻辑 |
| `qn-ui` | 最小 Tauri 壳：profiles/权限/状态/审计四视图 | 自建后端逻辑（全部走 qn-daemon） |
| `qn-mcp` | 沙箱即 MCP 工具（供 Agent 框架调用，M3） | 旁路实现 core 能力 |
| `qn-config` | 配置/路径/平台常量（`#[cfg]` 白名单位置之一） | 运行时逻辑 |
| `qn-infra` | process/fs/lockfile/logging/retry 通用机制 | 领域概念 |
| `qn-bootstrap` | 装配根：构造 adapters 并注入 core 服务（`#[cfg]` 白名单位置之一） | 除装配外的逻辑 |
| `qn-vm`（P2 占位） | 强隔离模式：VZ/QEMU VM + guest 内 nono | P0/P1 期间任何实现代码 |
| `sdks/typescript` `sdks/python` | 契约生成 + 薄人体工学层（M3） | 手写平行类型 |

## 3. 依赖方向（强制，CI 用 cargo-deny ban 校验）

```
qn-cli / qn-ui / qn-mcp / qn-daemon ──► qn-core ◄── qn-nono / qn-broker / qn-gateway / qn-wsl
qn-cli / qn-daemon ──► qn-bootstrap ──► qn-core + 全部适配器（唯一组合根）
适配器 / core / surfaces ──► qn-infra, qn-config
qn-nono / qn-wsl ──► 外部：nono 二进制、wsl.exe（经 qn-infra::process）
```

- core 不回头依赖任何适配器；surfaces 不认识具体适配器（经 qn-bootstrap）；
  适配器互不依赖（公共逻辑下沉 core/infra）；无环 DAG。
- **引擎可替换**是硬要求：`qn-nono` 是唯一驱动 nono CLI 的 crate；
  P2 的 qn-vm 将实现同一 port 实现强隔离模式切换。

## 4. Ports（core 中的关键 trait）

```rust
/// 沙箱引擎：拉起/查询/停止一个受限进程环境。实现者：qn-nono（P0/P1）、qn-vm（P2）。
/// 不变式：限制一经施加不可逆（内核保证）；失败不留半开状态；默认拒绝、按 profile 放行。
#[async_trait::async_trait]
pub trait SandboxEngine: Send + Sync {
    fn name(&self) -> &'static str;                       // "nono" | "vm"
    async fn run(&self, spec: RunSpec) -> Result<Session>; // profile + 命令 + 工作目录
    async fn status(&self, id: SessionId) -> Result<SessionStatus>;
    async fn explain(&self, id: SessionId) -> Result<ExplainReport>; // 历史观察及证据完整性
    async fn stop(&self, id: SessionId, mode: StopMode) -> Result<()>;
}

/// 凭证代答：真实凭证只在宿主侧；沙箱内只有短时占位符/句柄。
#[async_trait::async_trait]
pub trait CredentialBroker: Send + Sync {
    async fn grant(&self, id: SessionId, refs: Vec<SecretRef>, scope: GrantScope) -> Result<Grant>;
    async fn write_managed_config(&self, id: SessionId, app: AgentAppId, cfg: McpConfig) -> Result<()>; // 明文即拒绝
    async fn revoke(&self, grant: Grant) -> Result<()>;   // 立即断供
}

/// MCP mediation：宿主侧代答 MCP 认证。
#[async_trait::async_trait]
pub trait McpGateway: Send + Sync {
    async fn register(&self, server: McpServerRef) -> Result<Handle>;   // agent 侧只见 Handle
    async fn mediate(&self, handle: Handle, req: McpRequest) -> Result<McpResponse>;
    async fn revoke(&self, handle: Handle) -> Result<()>;
}
```

规则：trait 参数/返回全部是 core 领域类型；平台/nono 类型不外泄；新 port 先写使用方再定签名。

### 4.1 M1 生产实现约束（已接受，#14 / #47）

2026-10-24 用户授权成员 A 自主定案、同步 spec 并实施本方案，豁免本任务的三人确认前置；
B/C 的个人审批不作已取得处理。采用 #14 的选项 2：领域模型和 port 随首个真实使用方增量落地，
上面的签名是契约示意，不代表当前 crate 已提供完整 API。安全不变式与验证要求保持不变。

| 边界 | 实现要求 | 成功判定 |
| --- | --- | --- |
| 启动输入 | `RunSpec` 包含 qn `SessionId`、受管 `ProfileRef`、argv、workdir、可选 grant 句柄；不含任意 env、nono flags 或凭证明文 | profile、grant 归属、输入及环境验证通过后才拉起；同一 ID 不重复启动 |
| 环境与 profile | qn 管理会话的 HOME/config/state/cache；清空父环境后显式重建白名单；锁版内置 base 不受用户 default 覆盖 | 恶意宿主环境/用户 default 不影响受管运行；官方 profile 无 Keychain bypass 或 env-credential |
| 生命周期 | 私有映射关联 qn ID 与完整 native ID；由 infra 管理可取消的进程生命周期，保留所有后台任务句柄 | 启动失败不遗留资源；Starting 期间取消经确认清理后进入 Stopped |
| 停止 | 验证所拥有的进程及后代已结束；不得仅凭 `stop` 返回 0、`ps` 消失或未经保护的 PID 判成功 | 未确认清理则返回类型化失败，不标记为 Stopped，不误杀复用 PID 的其他进程 |
| 历史解释 | `ExplainReport` 返回观察到的 Denial、来源及完整性；`why` 仅用于策略预检 | 失败退出不合成 Denial；无法归属的 stderr 诊断标记为不确定，不能充当授权证据 |
| 宿主代答 | grant 绑定会话、引用、作用域及到期；capture 每次核验存活 grant，缓存 TTL 为 0 | 撤销后新请求不能取得认证；在途请求语义有单独测试和审计 |
| 适配器协作 | bootstrap 注入 core port；适配器不直接 import 其他适配器 | 真实凭证只在宿主 provider/代理边界内，公共沙箱配置只包含句柄和代理地址 |

`AuditLog` 在总图中表示审计能力方向，待具体事件写入/读取使用方确定后再引入 port；
不为补齐图示而添加无使用方接口。生产 provider 后端与进程清理机制必须经锁版实测再启用。
受管进程（#65）由 infra 暴露 `ProcessPolicy` 与拥有子进程/读任务的句柄，bootstrap 装配
Unix 进程组或 Windows Job Object。取消/超时先终止再回收；父进程已回收且无稳定组句柄时
拒绝用数字 PID/PGID 补杀，返回清理未确认。该机制不声称限制主动脱离进程组的后代。

证据到实现的对应关系：#46 的用户 default 覆盖 → 隔离配置根；#52 的强杀残留 → 停止清理验证；
#46 的零缓存撤销 → 按请求检查 grant；#48 的 stderr 歧义 → 显式证据来源与完整性。
这些前期实测不是完整 Agent、broker 或跨平台 E2E 的完成证明。

### 4.2 MCP HTTP 路由规划（#122 / #123）

core `McpRouting` 定义宿主路由规划，`qn-gateway::HttpMcpRouting` 实现规范端点校验。
契约源自 #122 本地集成中的 CLI 与原生 MCP 测试消费者；本切片仅发布规划能力，
消费者、bootstrap 装配及原生测试另行发布。HTTP/SSE 转发与凭证代答仍复用 nono/broker，
不新增代理或 adapter 互引。路由保留 name/origin/path 和 POST/GET/DELETE，不含凭证。
仅接受规范 HTTPS 或数字回环 HTTP；拒绝 userinfo/query/fragment、控制空白、别名、
百分号路径、反斜线和前导双斜线路径。复用已锁 url 解析器，错误/Debug 不回显输入。
路由只供可信宿主使用，不是 Agent 配置、注册、授权或存活证明；生产注册、stdio、
OAuth、Windows 和 #93 整树清理仍待完成。按 #14/#47 委托实施，不声明 B/C 个人审批。

## 5. 数据流（两个代表场景）

**场景 A：`qn run --agent claude-code`（P0 主路径）**
```
qn-cli ─► Profile(qn/claude-code) 解析 ─► CredentialBroker::grant（宿主授权与 capture 入口就绪）
  ─► SandboxEngine::run（qn-nono 拉起 nono，Seatbelt/Landlock 生效，默认拒绝）
  ─► claude-code 在真实项目目录受限运行：fs 白名单内可写、~/.ssh 等默认阻断
  ─► 出站经 nono 代理：allowlist 域名放行并附加认证，越权即拒 + Denial 留痕
  ─► qn revoke ─► Grant 失效 ─► 后续出站即刻断供
```

**场景 B：MCP 服务器接入（P0 第 3 项）**
```
qn mcp add github ─► McpGateway::register（真实 token 存宿主侧，产出 Handle）
  ─► agent 配置文件里只有 Handle（write_managed_config 校验：明文即拒绝）
  ─► agent 调 MCP ─► 网关在宿主侧代答认证 ─► agent 全程见不到 token
```

## 6. 关键横切决策

| 决策点 | 结论 | 依据 |
| --- | --- | --- |
| 安全内核 | nono（锁版本，Apache-2.0）；不自研、不 fork | guardrails Target Architecture；五问全过（04 §3） |
| 隔离级别 | 进程级内核原语（Seatbelt/Landlock）为默认；VM 为 P2 强隔离模式 | 薄产品层原则；内核漏洞不在 P0 威胁模型（08 §1 明示） |
| 凭证 | 包装 nono phantom-token 代理；真实凭证宿主侧存放 | guardrails invariants 2/3/6 |
| 决策记录 | 重大决策以 GitHub issue + spec 修订承载，不设独立 ADR 目录 | 与队友贡献流程一致（07 §3） |
| 错误/日志/并发 | thiserror 分层 + tracing + tokio；规范见 02 | — |

## 7. 目录结构（仓库根）

```
qn-sandbox/
├── AGENTS.md / README.md / CONTRIBUTING.md（#2）
├── Cargo.toml（workspace）/ rust-toolchain.toml / deny.toml
├── crates/
│   ├── qn-core/ qn-config/ qn-infra/ qn-bootstrap/
│   ├── qn-nono/ qn-broker/ qn-gateway/ qn-wsl/
│   ├── qn-daemon/ qn-cli/ qn-ui/ qn-mcp/ qn-vm/  （qn-vm 仅 P2 占位）
├── profiles/                    # 官方 agent profiles（claude-code、codex…）
├── sdks/typescript/ sdks/python/
├── docs/spec/                   # 本规范集（无 ADR 目录）
├── tests/e2e/                   # 跨 crate E2E（按平台 gate）
└── .github/workflows/
```

## 8. 架构评审检查表

- [ ] 新代码落在正确的层？（协议→surface；规则→core；引擎/平台调用→adapter；机制→infra）
- [ ] nono 只被 qn-nono 触碰？凭证只被 qn-broker 触碰？
- [ ] `#[cfg(target_os)]` 只在 qn-config / qn-bootstrap / qn-wsl？
- [ ] 新能力是先有 port 再有实现？适配器间没有复制逻辑？
- [ ] 沙箱内可能出现明文凭证的路径为零（grep 校验 + 08 §4）？
