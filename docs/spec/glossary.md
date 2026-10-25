# 术语表（Glossary）

全仓库（代码、文档、日志、CLI 输出）必须统一使用本表词汇。
新增领域词先进本表再使用；发现不一致按本表修正。

| 术语 | 含义 | 禁用同义词 |
| --- | --- | --- |
| **nono** | 我们采用的开源安全内核（[nolabs-ai/nono](https://github.com/nolabs-ai/nono)，Apache-2.0）：Landlock/Seatbelt 进程沙箱、phantom token 凭证代理、L7 网络规则、防篡改审计；只经 `qn-nono` 适配器使用，锁版本跟版 | 引擎/沙箱（泛指时） |
| **Session** | 一次受限进程环境的运行实例（`SandboxEngine::run` 产物） | Sandbox/VM/Box（旧模型词，不再用于进程级形态） |
| **ProfileRef** | 受管策略的规范 `namespace/name` 标识；不是文件路径，也不代表策略已授权 | profile path |
| **RunSpec** | core 的已校验启动输入：SessionId、ProfileRef、argv、workdir 与可选 GrantHandle | raw nono args |
| **GrantHandle** | 不含凭证明文的授权记录标识；有效性、作用域与撤销由 broker 检查 | API key / authentication proof |
| **Profile（Agent profile）** | 预装某 Agent CLI 与最小权限默认值（fs/网络/凭证绑定）的策略包（nono profile，JSON 可组合）；仓库维护官方 profile 于 `profiles/` | docker image / template |
| **Profile Registry（profile 注册表）** | 宿主侧版本化存储：安装副本 + SHA-256 + `current`，支持幂等安装与回滚；不授予运行或凭证权限 | docker registry / package manager |
| **Seatbelt / Landlock** | nono 所用的内核强制原语：macOS Seatbelt（10.5+）；Linux Landlock（fs ≥5.13，fs+网络 ≥6.7） | 沙箱（混用） |
| **强隔离模式（Strong-isolation mode）** | P2 可选高档模式：独立 VM + guest 内 nono | 默认沙箱 |
| **占位符 / broker 句柄（placeholder / handle / phantom token）** | 可进沙箱的**非凭证**引用形态：短生命周期、受限作用域、撤销即失效 | token（禁用）、密钥 |
| **凭证代答（Credential Broker）** | 宿主侧组件：包装 nono phantom-token 代理，grant/revoke 生命周期 + 审计；凭证明文绝不进沙箱 | 注入（禁用——注入意味着凭证进入沙箱） |
| **MCP Gateway（qn-gateway）** | 宿主侧 MCP mediation：agent 配置只有 Handle，真实凭证由网关代答 | MCP proxy（口语可用） |
| **Grant / Revoke** | 凭证使用权的授予/撤销；撤销立即断供 | — |
| **Denial** | 一次被内核/策略拒绝的访问尝试（含 `nono why` 原因），审计事件的核心载荷 | error（混用） |
| **qn-wsl 供给链路（provisioning）** | Windows 上 WSL2 发行版安装、内核版本探测/升级（Landlock ≥6.7）、nono 引导、代理打通的一键化流程 | installer（口语可用） |
| **qn / qnd** | CLI 二进制 `qn`；控制面后端 daemon `qnd` | server/service 混用 |
| **Port** | qn-core 中的 trait（SandboxEngine / CredentialBroker / McpGateway） | interface（口语可用） |
| **qn-bootstrap** | 装配根（组合根）：唯一同时可见 core 与全部适配器的 crate | registry |
| **五问** | 引入新依赖前的五项检查（04 §2） | 依赖评估 |
| **P0 / P1 / P2** | 仓库交付优先级（GitHub 标签）：P0 安全路径、P1 统一 API 与生态、P2 高级能力；不是版本号或分支名 | 版本号/分支名 |
| **v0** | 空基线上的不可变 git tag（不得移动/重建/强推） | 起始版本号 |
| **Host / Guest** | 宿主（用户机器）/ VM 内环境（仅强隔离模式语境使用 Guest） | 本地/客户端 |
