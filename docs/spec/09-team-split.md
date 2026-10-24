# 09 — 三人协作分工

状态：Draft（待三人各自认领角色） · 2026-10-24

## 1. 分工原则

1. **按平台 + 按层切主责**：每人独占一条主路径（macOS / Windows / 架构与 API 面），避免三人改同一处代码。
2. **每人守一条横切线**：横切线与其主责天然呼应，且互不重叠。
3. **接口共治**：`qn-core` 的 port（trait）属于公共契约，任何变更走固定流程（§4），不允许单人拍板。
4. **没有空窗期**：每人在自己主里程碑启动前，负责其前置依赖或技术验证（spike），保证里程碑不互相等待。
5. 任何人不做自己代码的**唯一评审**；review 义务见 §5。

## 2. 角色总览

> 认领状态：**成员 B 已认领（GitHub: [gac0812](https://github.com/gac0812)）**；成员 A / 成员 C 待认领。

| | 成员 A | 成员 B | 成员 C |
| --- | --- | --- | --- |
| **认领（GitHub id）** | 待认领 | **gac0812** | 待认领 |
| **主责领域** | macOS 后端 + arcbox 上游 | Windows 后端 + guest agent | 架构、API 契约、CI 质量门禁 |
| **主要 crate** | `qn-driver-vf`、镜像体系 | `qn-driver-wsl`、`qn-agent` | `qn-core`、`qn-config`、`qn-infra`、`qn-daemon`、`qn-cli`、`qn-mcp`、SDK |
| **横切线** | 许可合规：LICENSE/NOTICE、subtree 同步、`docs/patches/` | 安全条款（08 规范）在实现中的落实与评审 | 架构与依赖方向：R3 检查、deny ban 规则、ADR 守门 |
| **硬件前提** | Apple Silicon Mac（必需）；Intel Mac 可先用 CI macos-13 替代 | Windows 11 物理机（或支持嵌套虚拟化的环境）+ WSL2 | 无特殊硬件；需三平台 CI runner 管理权限 |

## 3. 按里程碑的分工矩阵

（主导 = 交付负责人；支援 = 协作并承担对应评审）

| 里程碑 | 成员 A（macOS/上游） | 成员 B / gac0812（Windows/agent） | 成员 C（架构/API） |
| --- | --- | --- | --- |
| **M0 骨架** | 引入 arcbox subtree（ADR-0001 落地）；复核上游能力清单与许可 | 搭 Windows CI 矩阵（wsl --install workflow）；实现 `qn-infra::process`、文件锁、`qn-config` 平台路径 | 主导：cargo workspace、deny.toml/clippy 配置、PR/issue 模板、`qn-core` 初版（领域模型 + ports + registry） |
| **M1 mac 基线** | 主导：`qn-driver-vf` 适配层、镜像导入链路、macOS E2E 剧本 | 完成 WSL2 路线 spike（手工验证容器方案可行性），产出结论文档（影响 ADR-0002 复审）；`qn-agent` 调研 | 定稿 ports v1 并**冻结**；`qn-cli` 初版；对 A 的实现做 port 语义 review |
| **M2 Intel Mac** | 主导：双架构镜像体系（`--arch`）、Intel E2E | 预研 WSL2 内嵌套 KVM 备选（只 spike 不实现，供 M5 决策） | 支援 CLI 收尾；文档与 glossary 同步 |
| **M3 Windows** | 支援：E2E 剧本库、镜像构建脚本复用到 x86_64 | 主导：`qn-driver-wsl`（发行版编排、容器隔离、互操作桥接）、Windows E2E | review + 按需扩展 core；陪跑安全横切评审（B 主守） |
| **M4 统一 API 面** | 支援：VZ 侧协议适配验证 | 支援：WSL 侧协议适配验证 | 主导：`qn-daemon`、契约（proto/buf）、`qn-mcp`、TS/Python SDK、gen-drift CI |
| **M5 高级能力** | 主导：快照/恢复（VZ 能力）、网络策略（复用上游 SOCKS5 代理） | 主导：资源配额、Windows 安装引导与兼容性 | 主导：契约演进、SDK/文档配套 |

## 4. 接口（port）变更流程

ports 属于三人公共契约，变更分级：

| 变更类型 | 所需同意 | 记录 |
| --- | --- | --- |
| 给已有 trait 加**带默认实现**的方法 | C + 受影响的 driver 负责人 | PR 描述 |
| 改 trait 签名 / 新增 port / 删除 port | **三人一致** | ADR |
| driver 内部实现细节 | 该负责人 | PR |

- 冻结点：M0 结束时 ports v1 冻结；M1 内允许小修（上述第一级），M3 起 trait 改动默认需要 ADR。
- 任何人发现实现与 port 语义冲突：停下提 issue，不静默绕过。

## 5. 评审与值班安排

- **互审矩阵**：A↔C 互审架构与 mac 代码；B↔C 互审架构与 win 代码；A↔B 互审 E2E 剧本与文档。任何 PR 至少一人 review（非作者），横切线相关 PR 必须由横切线负责人看一眼（许可→A，安全→B，架构→C）。
- **周同步**（30 分钟）：三平台 E2E 矩阵红绿、arcbox 上游动态、阻塞项、下周主导项。
- **轮值**：CHANGELOG 整理与每周 `cargo deny` 巡检，三人轮值（周次表建在本文件下方追加）。

## 6. 再平衡预案

- **B 的 spike 失败**（WSL2+容器隔离不足）：B 与 A 一起评估把 QEMU+WHPX 兜底提前，必要时 A 暂停 M2 支援（Intel Mac 影响小于 Windows 主路径）。
- **C 成为评审瓶颈**（M4 期）：A/B 按互审矩阵分担架构检查表（01 §8），C 只保留 port 与依赖方向终审。
- **某成员连续缺席 > 1 周**：其横切线临时移交同互审对的成员，移交内容写入周同步记录。
