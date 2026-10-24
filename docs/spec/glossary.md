# 术语表（Glossary）

全仓库（代码、文档、日志、CLI 输出）必须统一使用本表词汇。
新增领域词先加进本表再使用；发现不一致按本表修正（允许在 refactor PR 中顺带统一）。

| 术语 | 含义 | 禁用同义词 |
| --- | --- | --- |
| **Sandbox** | 宿主上一个隔离的 Linux 执行环境（VM 或 WSL2 内容器） | VM / machine / box / container（泛指时） |
| **SandboxId** | 沙箱唯一标识，`sbx-` 前缀 + UUIDv7 | handle / key |
| **Image** | 创建沙箱的只读模板（根文件系统 + 元数据），支持多架构 | template / snapshot |
| **ImageRef** | 镜像引用：`name:tag@arch` | 镜像路径字符串 |
| **Driver** | 平台后端实现（`Vf` / `Wsl` / `Qemu`），实现 `SandboxDriver` port | backend / provider / platform |
| **Guest** | 沙箱内的 Linux 环境（区别于 Host） | client |
| **GuestAgent** | guest 内的 RPC 服务，提供 exec / 文件 / 状态能力 | helper / runner |
| **Host** | 运行 qn-sandbox 的用户机器 | 本地/宿主机（口语可用，文档统一 Host） |
| **Port** | qn-core 中定义的 trait（六边形架构端口） | interface（口语可用） |
| **Registry** | 宿主侧沙箱注册表（状态文件 + 文件锁） | database / store |
| **daemon (qnd)** | 宿主常驻进程，暴露 Connect/HTTP API | server / service |
| **ExecRequest / ExecResult / ExecEvent** | 一次命令执行的请求/汇总结果/流式事件 | run / job |
| **StopMode** | `Graceful`（先 TERM 后 KILL）/ `Kill` | shutdown / force |
| **SandboxState** | `Creating → Running → Stopped / Failed → Removing`（迁移规则在 qn-core） | status 字符串 |
| **Mount** | 显式授权的宿主↔沙箱路径映射（默认不存在任何挂载） | shared folder |
| **NetPolicy** | 沙箱出/入站网络策略（默认拒绝，白名单放行） | firewall（口语可用） |
| **凭证代答网关（Credential Broker / Gateway）** | 宿主侧组件：从 OS 凭证库代持秘密，按 grant 在出站流量上代答（附加认证），支持撤销与审计（ADR-0004）；凭证明文绝不进入沙箱 | secret manager / vault / 注入（禁用） |
| **代答（on-the-wire substitution）** | 宿主网关在出站流量的认证字段上附加凭证的动作；沙箱内进程全程不见凭证 | 注入（禁用——注入意味着凭证进入沙箱） |
| **凭证引用（SecretRef）** | 沙箱配置中对宿主秘密的命名引用（`${qn:secret/<name>}`）；沙箱内出现明文密钥即违规 | key / 密钥 |
| **Agent 应用模板（Profile）** | 预装某主流 Agent CLI（Claude Code、Codex、dsh 等）及其运行时与**最小权限默认值**（fs/网络按 profile 放行）的官方模板，`qn create --agent <app>` 直接使用；EN: agent profile | docker image |
| **占位符 / broker 句柄（placeholder / handle）** | 可进沙箱的**非凭证**引用形态：短生命周期、受限作用域（guardrails 措辞 placeholders or broker handles）；撤销会话即失效 | token（禁止）、密钥 |
| **P0 / P1 / P2** | 仓库交付优先级（GitHub 标签）：P0 安全路径、P1 平台广度与统一 API、P2 高级能力；不是版本号或分支名 | 版本号 / 分支名 |
| **v0** | 空基线上的不可变 git tag（贡献规范：不得移动、重建或强推） | 起始版本号 |
| **AgentAppCatalog** | Agent 应用模板与所需 SecretRef 清单的登记表（qn-core 领域对象） | 模板列表 |
| **subtree (arcbox/)** | 以 git subtree 引入的只读上游代码 | vendor / fork |
| **契约** | RPC 接口定义文件（proto 等），API 面的唯一事实源 | 接口文档 |
| **Surfaces** | 协议适配层：qn-cli / qn-daemon / qn-mcp / SDK | 前端 / 入口层 |
| **五问** | 引入新依赖前的五项检查（04 §2） | 依赖评估 |
