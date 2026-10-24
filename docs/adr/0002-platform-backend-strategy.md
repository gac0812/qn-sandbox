# ADR-0002 — 平台后端策略：macOS 用 VZ（继承 arcbox），Windows 用 WSL2（容器化沙箱），QEMU 作跨平台兜底

- 状态：Proposed（待用户确认）
- 日期：2026-10-24
- 决策人：AI 提议

## 背景

目标平台：macOS Apple Silicon、macOS Intel、Windows 10 22H2/11（含 Home 版）。
约束：guest 一律 Linux；用户安装成本要低（≤5 分钟跑通）；与 arcbox 复用最大化；不依赖云。

各平台可用技术的事实核对（2026-10）：
- Virtualization.framework：macOS 11+ 同时支持 Apple Silicon（aarch64 guest）与 Intel（x86_64 guest），同架构约束。
- WSL2：Win10 1903+/Win11（含 Home）自带安装路径，真实 Linux 内核；容器运行时（runc/crun/Docker 引擎）可在其发行版内运行；嵌套 KVM 可用但脆弱。
- QEMU：跨平台，macOS HVF / Windows WHPX / Linux KVM 加速；GPL-2.0，作为外部进程使用许可合规。
- Hyper-V：仅 Win Pro+，Home 版不可用 → 不能作为 Windows 唯一路径。

## 选项

### 选项 A（推荐）：每平台一条主力路径 + QEMU 兜底
| 平台 | 主力 | 兜底 |
| --- | --- | --- |
| macOS arm64 | VZ（arcbox 继承） | QEMU+HVF |
| macOS x64 | VZ（x86_64 guest，双架构镜像） | QEMU+HVF |
| Windows x64 | WSL2 + 容器级隔离（每沙箱一容器） | QEMU+WHPX（VM 级） |

- 优点：每平台用系统原生最优解；Home 版覆盖；QEMU 兜底保底"总能跑"。
- 缺点：三个 driver 的实现与测试成本；WSL2 内容器隔离强度弱于 VM（缓解见 08 §2）。
- 风险与缓解：WSL2 环境碎片化 → 版本探测 + 明确最低版本 + 引导安装流程。

### 选项 B：全平台统一 QEMU（一套 driver 走天下）
- 优点：driver 只写一个，行为最一致。
- 缺点：macOS 上放弃 arcbox 全部复用（VZ 集成、agents 适配全重写）；性能/电池/体验差于 VZ；QEMU 管理面代码量大。违反"复用最大化"。

### 选项 C：Windows 上 WSL2 内跑嵌套 microVM（KVM）
- 优点：隔离强度对齐 macOS VM 级。
- 缺点：嵌套虚拟化依赖宿主 BIOS/Windows 版本，大量用户机器不可用；Firecracker 类栈在嵌套环境问题多；失败模式不可控。可作为 M5+ 的 `qn-driver-wsl-kvm` 增强，不做主力。

## 对比

| 维度 | A | B | C |
| --- | --- | --- | --- |
| 复用度 | 高（mac 全复用） | 低 | 中 |
| 平台覆盖 | 全覆盖（含 Win Home） | 全覆盖 | Win 覆盖不稳定 |
| 维护成本 | 3 个 driver | 1 个 driver 但很厚 | 3+ 嵌套问题 |
| 隔离强度 | mac VM / win 容器 | VM 一致 | VM 一致 |

## 结论

**选项 A**。落地顺序对齐里程碑：M1 mac arm64（Vf）→ M2 intel（Vf 双架构镜像）→ M3 WSL2 → QEMU 兜底后端按需排期（M5 前不启动，除非 WSL2 路径出现不可逾越的覆盖缺口）。
隔离强度差异（win 容器 < VM）作为已接受代价记录，安全条款按各平台基线执行（08 §2）。

## 后果

- 正面：每个平台用该平台最成熟、安装成本最低的机制；复用最大化。
- 负面：三 driver 的行为一致性要靠 core port 的严格测试保证（05 §4 矩阵）。
- 重审条件：微软发布原生轻量 VM API（如 OpenVMM/WHP 稳定易用）且覆盖 Home 版，可重评 Windows 主力路径。
