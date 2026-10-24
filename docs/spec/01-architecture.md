# 01 — 架构规范

状态：Draft（与 ADR-0001/0002/0003 联动） · 2026-10-24

## 1. 总体形态

```
┌──────────────────────────  使用者  ──────────────────────────┐
│   AI Agent (MCP 客户端)      人 (终端)        SDK 应用 (TS/Py) │
└──────┬──────────────────┬───────────────────┬───────────────┘
       │ MCP              │ CLI              │ HTTP/Connect
┌──────▼─────────┐ ┌──────▼──────┐ ┌─────────▼───────────────┐
│   qn-mcp       │ │   qn-cli    │ │  qn-daemon (HTTP/Connect)│   ← surfaces：协议适配，无业务规则
└──────┬─────────┘ └──────┬──────┘ └─────────┬───────────────┘
       └──────────────────┼───────────────────┘
                          │  只依赖 qn-core 的 trait
┌─────────────────────────▼───────────────────────────────────┐
│                        qn-core                              │   ← 领域层：模型 + ports
│   Sandbox / Image / ExecRequest / SandboxDriver / ImageStore │      （零平台依赖）
└──────┬──────────────────┬───────────────────┬───────────────┘
       │                  │                   │     ← 运行期按平台装配唯一实现
┌──────▼───────┐  ┌───────▼────────┐  ┌───────▼────────┐
│ qn-driver-vf │  │ qn-driver-wsl  │  │ qn-driver-qemu │   ← drivers：平台差异全部关在这里
│ (macOS, VZ)  │  │ (Windows,WSL2) │  │ (兜底, 跨平台)  │
└──────┬───────┘  └───────┬────────┘  └───────┬────────┘
       │ 复用 arcbox subtree  │ WSL2 + 容器/guest agent │ QEMU 进程 (HVF/WHPX/KVM)
┌──────▼───────────────────▼───────────────────▼──────────────┐
│  qn-infra: 配置/路径/锁/进程工具/tracing 初始化（被各层复用）  │
└──────────────────────────────────────────────────────────────┘
```

## 2. Crate 划分（cargo workspace）

| crate | 职责 | 禁止 |
| --- | --- | --- |
| `qn-core` | 领域模型（`Sandbox`、`Image`、`ExecRequest/Result`、`Snapshot`）、**ports**（`SandboxDriver`、`ImageStore`、`GuestAgent`、`NetPolicy`）、统一 `Error`、`Config` 结构 | 依赖任何 driver/surface/平台 API；直接做 IO 的业务函数除外 |
| `qn-driver-vf` | macOS 后端：Virtualization.framework；**起自 arcbox subtree 的适配层**（把上游能力包装成 `SandboxDriver`） | 依赖其他 driver；含任何 Windows 代码路径 |
| `qn-driver-wsl` | Windows 后端：WSL2 生命周期 + 其上容器/agent 管理 | 依赖其他 driver；含任何 macOS 代码路径 |
| `qn-driver-qemu` | 兜底后端：外部 QEMU 进程（HVF/WHPX/KVM 加速自动探测） | 依赖其他 driver |
| `qn-agent` | 沙箱内 guest agent（Linux，静态链接），提供 exec/文件/快照 RPC 与**无凭证本地出口代理**（纯转发，凭证由宿主网关在流量上代答）；**优先复用 arcbox 的 Linux agents**，本 crate 只做差量 | 链接任何宿主端依赖 |
| `qn-bootstrap` | 装配根（组合根）：按目标平台选择唯一 driver，构造 `Arc<dyn SandboxDriver>` 并组装 core 服务；daemon 与 CLI 直连模式共用 | 除装配外的任何逻辑 |
| `qn-daemon` | 宿主常驻守护进程：暴露 ConnectRPC/HTTP；经 `qn-bootstrap` 装配 driver；管理沙箱注册表；**凭证代答网关**（访问 OS 凭证库、发放/撤销授权、流量代答） | 含协议解析之外的业务规则（规则在 core）；明文凭证写入日志/状态文件 |
| `qn-cli` | `qn` 命令行（clap）：人机入口，薄壳 | 业务逻辑（只做参数解析 → 调 daemon/core → 呈现） |
| `qn-mcp` | MCP server：把 core 能力映射为 MCP tools（复用官方 Rust SDK） | 旁路实现任何 core 已有能力 |
| `qn-config` | 配置文件、路径解析、平台常量（**唯一允许 `#[cfg(target_os)]` 的非 driver crate**） | 运行时逻辑 |
| `qn-infra` | tracing 初始化、目录/锁文件、进程执行工具、重试（自研部分） | 领域概念 |
| `arcbox/`（subtree） | 上游只读代码 + 其 Linux agents 与协议定义 | 除 docs/patches/ 记录的补丁外任何修改 |

> SDK（`sdks/typescript`、`sdks/python`）由契约生成（ADR-0003），不在此 workspace 内。

## 3. 依赖方向（强制，CI 用 cargo-deny ban 规则校验）

```
qn-cli / qn-daemon / qn-mcp  ──►  qn-core  ◄──  qn-driver-{vf,wsl,qemu}
qn-cli / qn-daemon  ──►  qn-bootstrap  ──►  qn-core + qn-driver-*（唯一同时可见二者的组合根）
qn-driver-* / qn-core / surfaces ──►  qn-infra, qn-config
qn-driver-vf ──► arcbox (subtree)
```

- **core 不回头**：`qn-core` 的 Cargo.toml 禁止出现 driver、surface、平台 crate。
- **surfaces 不认识具体 driver**：surfaces 从 `qn-bootstrap`（装配根，唯一同时可见 core 与 drivers 的 crate）拿到 `Arc<dyn SandboxDriver>`，不直接 `use qn_driver_vf::…`。
- **driver 互不依赖**：共同逻辑下沉 `qn-core`（属领域）或 `qn-infra`（属机制）。
- **禁止环形依赖**：crate 间新增依赖必须让方向图仍是 DAG；违反即架构缺陷，回退。
- 违规检测：`cargo modules generate graph`（评审用）+ `cargo-deny` ban 段（CI 强制）。

## 4. Ports（core 中的关键 trait 草案）

trait 是耦合控制的核心工具。首批 port 及其不变式：

```rust
/// 沙箱生命周期。实现者：vf/wsl/qemu 三个 driver。
/// 不变式：所有方法可重入（幂等或明确报"状态不符"）；失败不留半开状态。
#[async_trait::async_trait]
pub trait SandboxDriver: Send + Sync {
    fn name(&self) -> &'static str;                       // "vf" | "wsl" | "qemu"
    fn supported_arch(&self) -> Arch;                     // 宿主决定的 guest 架构
    async fn create(&self, spec: SandboxSpec) -> Result<Sandbox>;
    async fn start(&self, id: SandboxId) -> Result<()>;
    async fn stop(&self, id: SandboxId, mode: StopMode) -> Result<()>;
    async fn remove(&self, id: SandboxId) -> Result<()>;
    async fn status(&self, id: SandboxId) -> Result<SandboxStatus>;
    async fn list(&self) -> Result<Vec<SandboxInfo>>;
    /// 打通到 guest agent 的通道；driver 负责寻址，协议由 GuestAgent port 定义。
    async fn connect(&self, id: SandboxId) -> Result<Box<dyn GuestAgent>>;
}

/// 沙箱内的执行面。实现者：复用 arcbox agents 适配（vf）、qn-agent（wsl/qemu）。
#[async_trait::async_trait]
pub trait GuestAgent: Send + Sync {
    async fn exec(&self, req: ExecRequest) -> Result<ExecResult>;        // 带超时与取消
    fn exec_stream(&self, req: ExecRequest) -> Result<ExecStream>;      // stdout/stderr 流式
    async fn upload(&self, req: FileTransfer) -> Result<()>;
    async fn download(&self, path: FilePath) -> Result<FileContent>;
}

/// 凭证代答网关 port。宿主侧实现（qn-daemon，OS 凭证库为唯一存储）。
/// 硬不变式（guardrails README / 08 §4）：凭证明文绝不进入沙箱的文件系统、
/// 环境变量、进程参数或 Agent 上下文——沙箱内只有凭证引用与无凭证出口代理。
/// 注：GuestAgent 的 exec 需支持 PTY/交互流——跑 Claude Code 等交互式 Agent CLI 的硬前提。
#[async_trait::async_trait]
pub trait CredentialBroker: Send + Sync {
    /// 授予某沙箱一组凭证引用的使用权（带 TTL，可撤销）；
    /// 网关据此在出站流量上代答，沙箱内进程自始至终见不到凭证。
    async fn grant(&self, id: SandboxId, refs: Vec<SecretRef>, ttl: Duration) -> Result<Grant>;
    /// 校验并写入沙箱内 Agent 应用的 MCP 配置——配置中出现明文即拒绝，只允许引用。
    async fn write_managed_config(&self, id: SandboxId, app: AgentAppId, cfg: McpConfig) -> Result<()>;
    /// 撤销：立即断流，网关对该沙箱的代答即刻停止。
    async fn revoke(&self, grant: Grant) -> Result<()>;
}
```

规则：
- trait 方法参数/返回值全部是 core 领域类型，**禁止泄漏平台类型**（如 `VZVirtualMachineConfiguration`、`wsl::Distro`）。
- trait 演进：加默认方法可以；改签名必须评估三个实现与所有 surface。
- 新 port 先写使用方（surface）的调用示例，再定签名——接口由需求倒推，不臆测。

## 5. 数据流（两个代表性场景，评审新功能时套用）

**场景 A：`qn exec sbx-1 -- python main.py`**
```
qn-cli 解析参数 ─► qn-core::Command(领域对象) ─► bootstrap 拿 driver
  ─► SandboxDriver::connect(id) ─► GuestAgent::exec(ExecRequest)
  ─► driver 内部: (vf) arcbox ConnectRPC / (wsl) WSL 互操作+agent
  ─► ExecResult ─► qn-cli 呈现；daemon 侧 tracing span 全程贯穿
```

**场景 B：`qn create --image ubuntu-24.04 --arch auto`**
```
qn-cli ─► SandboxSpec ─► ImageStore::resolve(镜像本地存在？→ 拉取/导入)
  ─► SandboxDriver::create(spec) ─► driver 产出 Sandbox(含 backend 细节字段, 私有)
  ─► registry 注册（文件锁防并发, qn-infra）─► 返回 SandboxInfo（不含平台类型）
```

**场景 C：`qn create --agent claude-code`（凭证不进沙箱，产品核心卖点）**
```
qn-cli ─► AgentAppId("claude-code") ─► AgentAppCatalog（镜像模板 + 所需 SecretRef 清单）
  ─► CredentialBroker::grant(sbx, refs, ttl)     // 宿主 OS 凭证库取值，审计只记引用名
  ─► SandboxDriver::create(agent 模板 spec)      // 网络默认指向 qn-agent 本地出口代理（无凭证）
  ─► GuestAgent::exec(启动 Claude Code)          // 环境变量只含代理地址，不含任何凭证
  ─► 出站 HTTPS ─► qn-agent 代理 ─► 宿主代答网关 ─► 附加认证 ─► LLM API
  ─► M4 起：MCP 网关协议感知代答；托管配置只写引用，明文即拒绝
```

## 6. 关键横切决策

| 决策点 | 结论 | 依据 / ADR |
| --- | --- | --- |
| 语言 | 宿主端 Rust（workspace）；guest agent Rust（musl 静态）；SDK TS/Python 生成 | 与 arcbox 同栈，复用最大化；ADR-0001 |
| RPC/协议 | ConnectRPC（arcbox 同款），契约文件为唯一事实源；MCP 由官方 rust-sdk | ADR-0003 |
| 异步运行时 | tokio（multi-thread） | 生态默认；arcbox 一致 |
| 错误模型 | 各 crate `thiserror` + core 统一 `Error`；CLI 边界转人话 | AGENTS.md §4.4 |
| 配置 | `~/.qn/config.toml` + 环境变量覆盖（`QN_*`），`qn-config` 独占解析 | 平台路径差异集中在这一处 |
| 并发控制 | 沙箱注册表 = 文件锁 + JSON 状态文件（qnd-*）；daemon 单写者 | 跨平台文件锁是少数需要自研的点，见 04 复用矩阵 |
| 日志 | tracing → 滚动文件（`~/.qn/logs/`）+ `--verbose` 转发 stdout | 排障需要跨进程关联 |
| 凭证托管 | 宿主 OS 凭证库（`keyring`：Keychain / Credential Manager）为唯一存储；沙箱内只允许引用，明文即拒绝 | ADR-0004 |
| 镜像格式 | 沿用 arcbox 镜像导入（M1），M3 起加 OCI 容器镜像作为 WSL 内容源 | ADR-0002 |

## 7. 目录结构（仓库根）

```
qn-sandbox/
├── AGENTS.md                 # AI 协作规范（入口）
├── README.md
├── Cargo.toml                # workspace 根
├── rust-toolchain.toml
├── deny.toml                 # cargo-deny: licenses/bans/advisories
├── arcbox/                   # git subtree（只读上游, R4）
├── crates/
│   ├── qn-core/
│   ├── qn-config/
│   ├── qn-infra/
│   ├── qn-bootstrap/
│   ├── qn-driver-vf/
│   ├── qn-driver-wsl/
│   ├── qn-driver-qemu/
│   ├── qn-agent/
│   ├── qn-daemon/
│   ├── qn-cli/
│   └── qn-mcp/
├── sdks/
│   ├── typescript/           # 生成 + 手写薄封装
│   └── python/
├── docs/
│   ├── spec/                 # 本规范集
│   ├── adr/                  # 决策记录
│   └── patches/              # 对 arcbox 上游的补丁及说明
├── tests/
│   └── e2e/                  # 跨 crate E2E（按平台 gate）
└── .github/workflows/        # CI: fmt/clippy/test/deny + 平台矩阵
```

## 8. 架构评审检查表（每次 PR 涉及结构变化时过一遍）

- [ ] 新代码落在正确的层？（协议细节→surface；业务规则→core；平台调用→driver；机制→infra）
- [ ] 依赖方向仍是无环 DAG？core 是否被"顺手"加了对平台的依赖？
- [ ] 平台差异没有泄漏？（grep `cfg!(target_os)` 与 `#[cfg(target_os` 应只出现在白名单位置）
- [ ] 新能力是先有 port 再有实现，还是 driver 间先复制了一份？
- [ ] trait 是否最小（有没有使用者已经需要的方法才存在）？
- [ ] 数据流里有没有"绕过 core 直接调 driver"的捷径？
