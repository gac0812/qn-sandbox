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

## 3. 依赖方向（强制，CI architecture 校验）

```
qn-cli / qn-ui / qn-mcp / qn-daemon ──► qn-core ◄── qn-nono / qn-broker / qn-gateway / qn-wsl
qn-cli / qn-daemon ──► qn-bootstrap ──► qn-core + 全部适配器（唯一组合根）
适配器 / core / surfaces ──► qn-infra, qn-config
qn-nono / qn-wsl ──► 外部：nono 二进制、wsl.exe（经 qn-infra::process）
```

- core 不回头依赖任何适配器；surfaces 不认识具体适配器（经 qn-bootstrap）；
  适配器互不依赖（公共逻辑下沉 core/infra）；无环 DAG。
  **唯一例外（#188 裁决）**：`qn-wsl → qn-nono` 允许——安装器 staging 复用（锁版 + SHA256 + Linux target 工件，上游模块文档明示 qn-wsl 为消费方）；仅限 installer 家族，qn-nono 其余模块（runner/wire/audit）对 qn-wsl 仍禁止。
  **唯一例外（#188 裁决）**：`qn-wsl → qn-nono` 允许——安装器 staging 复用（锁版 + SHA256 + Linux target 工件，上游模块文档明示 qn-wsl 为消费方）；仅限 installer 家族，qn-nono 其余模块（runner/wire/audit）对 qn-wsl 仍禁止。
- **引擎可替换**是硬要求：`qn-nono` 是唯一驱动 nono CLI 的 crate；
  P2 的 qn-vm 将实现同一 port 实现强隔离模式切换。

### 3.1 自动门禁（#132）

`python3 scripts/check_architecture.py` 使用 Cargo metadata 检查所有 workspace
成员的声明依赖，不按宿主平台或 feature 过滤；普通、dev、build、optional、平台条件
依赖均服从同一边界。重命名和 workspace 继承按 Cargo 解析后的真实包名与路径检查。

| 来源 | 允许依赖的内部 crate |
| --- | --- |
| `qn-config` / `qn-infra` | 基础层中的另一个 crate |
| `qn-core` | `qn-config`、`qn-infra` |
| 适配器 `qn-nono/broker/gateway/wsl/vm` | `qn-core`、基础层 |
| `qn-bootstrap` | `qn-core`、适配器、基础层 |
| `qn-cli` / `qn-daemon` | `qn-core`、`qn-bootstrap`、基础层 |
| `qn-ui` / `qn-mcp` | `qn-core`、基础层 |

新 workspace crate 必须显式归类；`qn-` 内部依赖必须来自对应 workspace 路径。
第三方依赖仍由 cargo-deny 检查。门禁检查依赖声明，不证明 Rust 源码/外部进程调用
满足安全边界，也不代替代码审查。策略修改须同步本节与检查器测试。

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
2026-10-25 的 #92 集成实测进一步确认：即使后代未主动脱组，关闭继承 stdio 后也可在
父进程 Exited(0) 后继续执行。因此 ManagedOutput、原生 Exited 与输出 EOF 均不能单独
映射为 core Stopped；生产接入必须提供整个受管执行树的清理证据。可复现反例和实验范围见
[受管 runner 集成验证](../dev-notes/owned-runner-integration.md)。

证据到实现的对应关系：#46 的用户 default 覆盖 → 隔离配置根；#52 的强杀残留 → 停止清理验证；
#46 的零缓存撤销 → 按请求检查 grant；#48 的 stderr 歧义 → 显式证据来源与完整性。
这些前期实测不是完整 Agent、broker 或跨平台 E2E 的完成证明。

宿主 capture 传输（[#96](https://github.com/gac0812/qn-sandbox/issues/96)）采用 snow 的标准
Noise NNpsk0/25519/ChaChaPoly/SHA256，PSK 仅在宿主私有 binding；grant 句柄不作认证。
请求及秘密响应均认证加密，consumer 仍负责会话/精确作用域与准入时撤销检查。
该 broker 内部传输不新增 core Port，不代表生产 provider、审计或 WSL 桥已完成。

### 4.2 首个审计查询消费者（#110）

CLI 的 audit show/list --session 经 bootstrap 装配 core AuditQuery；qn-broker 实现固定会话
文件的有界读取。core 只包含封闭呈现类型和查询接口，不依赖 adapter/文件系统。日志源明确
为 broker、完整性 unverified；区分 empty/open/partial/observed_closed，关闭事件不证明
沙箱 Stopped、完整送达或防篡改。未知字段、重复字段、错会话、序号缺失/倒退均拒绝；
只允许尾部未完成行作为 partial 展示。native/fs 拒绝不从 broker 推断，跨源顺序与 follow
待真实消费者实现。沿用 #14/#47 用户委托授权，不声称 B/C 个人审批；决定见 #110。

#120 的 audit follow 按 250ms 轮询已有 AuditQuery，检查完整已读前缀后仅输出新增匹配。
过滤在全部快照验证后应用；原序号与源状态保留。GrantIssued/GrantRevoked 按 #119 投影。
沿用 #152 的 schema_version=1 与 timeline JSON 包装，保留人读展示；不恢复授权。

### 4.3 引擎安装与诊断契约（#114/#117）

按 #14/#47 用户委托，由已本地验证的 install/doctor 消费者推动最小 EngineSetup port：
`run(SetupOperation::{Install,Inspect}) -> SetupFuture`；future 输出
`Result<EngineInstallation,SetupError>`。core 仅承载 std/camino 类型，不依赖 adapter 或运行时。
Install 显式下载/修复；Inspect 不下载/修复，须先验证完整性再运行版本探测。
成功只证明托管引擎字节/版本，不证明隔离、凭证、完整执行树清理或 WSL 就绪。
调用方等待操作完成，丢弃 future 不保证阻塞 IO 取消；adapter 脱敏全部公开错误链，
surface 转义路径控制字符。#114 经 QnPaths::engine_cache → bootstrap → NonoSetup 接线；
默认 doctor 先完整性后版本探测，--config-only 保持原配置诊断契约。

### 4.4 MCP HTTP 路由规划（#122 / #123）

core `McpRouting` 定义宿主路由规划，`qn-gateway::HttpMcpRouting` 实现规范端点校验。
契约源自 #122 本地集成中的 CLI 与原生 MCP 测试消费者；本切片仅发布规划能力，
消费者、bootstrap 装配及原生测试另行发布。HTTP/SSE 转发与凭证代答仍复用 nono/broker，
不新增代理或 adapter 互引。路由保留 name/origin/path 和 POST/GET/DELETE，不含凭证。
仅接受规范 HTTPS 或数字回环 HTTP；拒绝 userinfo/query/fragment、控制空白、别名、
百分号路径、反斜线和前导双斜线路径。复用已锁 url 解析器，错误/Debug 不回显输入。
路由只供可信宿主使用，不是 Agent 配置、注册、授权或存活证明；生产注册、stdio、
OAuth、Windows 和 #93 的平台残余仍待完成；会话级 reaper 见 §4.7。
按 #14/#47 委托实施，不声明 B/C 个人审批。

### 4.5 原生审计消费者（#121）

NativeAuditQuery 单独承载 native network-only report；不向 broker 单调时钟投影。
启动前 LaunchBinding 发布不可覆盖的 qn/native/runtime 关联；关联仅用于查询，不授予
进程或授权能力。读取严格限长、拒绝非普通文件、错会话及非规范 native ID/runtime 路径。
先校验安装完整性再复用 AuditReader 执行；只输出封闭 decision、原序号、可选 wall time、
原生总数/exit_code，省略可能携带秘密的 target/path/reason/command。
CLI audit native 通过 bootstrap 消费；JSON schema_version=1/report，source=nono_audit、
integrity=unverified、coverage=network_only。宿主目录可信前提与目录持久化责任不变；
会话清理见 §4.7 的作用域与残余。按 #14/#47 委托实施，未声明 B/C 个人审批。

### 4.6 官方 profile 与版本化注册表（#75 / #59）

`profiles/` 保存两份版本化 nono profile 资产（`claude-code.json`、`codex.json`）与
`manifest.json`。资产以 `extends: "default"` 保留强制 deny 组，显式列出 fs 白名单、
代理 `allow_domain` 与 `network.credentials` 服务名；不授予 Keychain/credential 路径，
不使用 `allow_domain: ["*"]`，不含任何秘密或宿主路径。

`qn-nono::registry` 在编译期嵌入这两份资产，运行期只接受 manifest 中列出的已知资产名，
因此不存在任意路径发布入口。宿主注册表位于 `QN_HOME/profiles`：
`<namespace>/<name>/versions/<version>/` 保存 `profile.json` 与含 SHA-256 的
`manifest.json`，`current` 选择一个已验证版本。安装对相同字节幂等、对不同字节拒绝，
回滚只选择上一已验证版本。core `ProfileRegistry` 仅暴露封闭的 summary/resolved DTO
与类型化错误；`qn-bootstrap` 装配，CLI `qn profile list/show/install/rollback` 呈现。
这些操作不启动引擎、不解析凭证、不扩大沙箱授权。`qn profile validate` 先校验托管引擎
归档/二进制完整性与版本，再把已解析资产写入 `QN_HOME` 私有临时文件并执行锁版
`nono profile validate --json --strict`；不下载/修复、不启动沙箱，成功只证明引擎接受该
资产语法与引用。`qn profile publish` 只接受编译期官方身份（app state 目录由目录决定），
读取绝对、限长、普通 UTF-8 文件，先经同一引擎校验，再以显式 MAJOR.MINOR.PATCH 发布并
切换 `current`；资产 `meta.version` 必须与参数一致，同版本不同字节拒绝，rollback 仍可
回到上一已验证版本。资产发布不等于在线 Agent 认证 E2E、跨平台验收；#93 的会话级
reaper 与残余见 §4.7。
按 #14/#47 委托实施，未声明 B/C 个人审批。

### 4.7 前台会话运行、状态与停止（#92 / #93）

`qn run --profile <id> -- <argv...>` 经 bootstrap 组合：解析并校验 profile、校验托管引擎
归档/二进制完整性与版本、在 `QN_HOME/runtime/<session>` 建立私有隔离 HOME/XDG、预留
`LaunchBinding`、发布 qn/native/runtime 审计关联，然后以继承 stdio 的前台进程启动。
profile 资产只含句柄/代理地址，不含凭证明文；grant 仍由 broker 逐请求核验。运行目录必须
位于 profile 系统组授予路径之外（例如 macOS 上不要在 `/private/var` 下设置 `QN_HOME`），
否则 nono 会拒绝初始化。

`qn status <session>` 读取 owner 记录并可选查询原生 `ps --all` 观察；`qn stop <session>`
用记录中的预定 native ID 调用锁版 `nono stop`，绝不使用数字 PID/PGID；对已退出记录也执行
同样的 reaper，因为后代可能晚于受管父进程退出。

会话清理由 `qn-bootstrap` 的 reaper 单独证明（#93）：以会话私有 HOME/XDG/TMPDIR 路径为
标记，持续记录受管父进程的后代；收尾时 SIGSTOP 候选、复验 `(pid, start_time)` 身份、
SIGKILL，并要求连续多轮无标记/无候选才写 `cleanup=confirmed`。父进程退出、`ps` 消失或
`stop` 返回 0 单独都不构成证明，也不映射为 core `Stopped`。

证明作用域：在首次观察前清空继承环境并脱离（含 `setsid` 后再 `env -i`）的后代不可见；
macOS 无 subreaper/cgroup，关闭该残余需要 VM 或上游内核边界。该场景明确不在当前证明内，
不能把 `confirmed` 读作任意进程树的内核级无幸存者证明。身份复验与 `kill(2)` 是两个系统调用
（macOS 无 `pidfd`），复验后、发送前被回收的 PID 无法识别；之后回收的 PID 均被 start_time
拒绝。Windows/WSL 前台路由尚未接线（非 unix 构建固定返回 `unconfirmed`）。

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
