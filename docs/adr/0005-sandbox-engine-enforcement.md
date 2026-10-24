# ADR-0005 — 沙箱引擎与 enforcement 层：`nono` 与平台 driver 的关系

- 状态：Proposed（**资料待补**：`nono` 的上游链接、license、活跃度、平台矩阵由 #2 / guardrails 作者补充后再拍板）
- 日期：2026-10-24
- 决策人：待三人评审
- 关联：guardrails README「Target Architecture: Sandbox engine: `nono` as the security kernel」↔ ADR-0002（平台后端策略）

## 背景

guardrails（PR #2）把 `nono` 指定为"security kernel and enforcement boundary"（沙箱引擎）；
本仓库 ADR-0002 已定的平台后端是 VZ / WSL2 / QEMU 三 driver（继承 arcbox）。
两份架构叙述目前并存且未对齐——按 07 规范，架构冲突不得静默偏离，必须显式决策。

## 选项

### 选项 A：仅平台级隔离（维持 ADR-0002 原样）
- 做法：隔离边界完全由 driver（VM/WSL2 容器）提供；profile 的 fs/网络最小权限由 qn-agent 内标准 Linux 机制（mount namespace / cgroup / seccomp / landlock）实现。
- 优点：不引入新依赖；复用 arcbox 全部既有能力。
- 缺点：enforcement 逻辑散在 qn-agent 各处；若无统一内核层，"default deny + per profile"审计粒度较粗。

### 选项 B：driver 之上引入 in-guest enforcement kernel（`nono` 作为 guest 内策略内核）
- 做法：driver 提供平台隔离基座（不变）；`nono` 运行在 guest 内作为策略执行内核（按 profile 收紧 fs/网络/进程），qn-agent 调用之。
- 优点：两层防御（VM 边界 + guest 内最小权限）；与 guardrails "security kernel and enforcement boundary" 的定位吻合；若 nono 是成熟项目则复用大于自研。
- 缺点：引入新架构级依赖（需过 04 §2 五问）；P0 工作量增加。
- 风险与缓解：nono 资料不明 → 本 ADR 阻塞在此，先按 A 实现不阻塞 M1。

### 选项 C：`nono` 作为唯一跨平台引擎（替代 driver 层）
- 做法：抛弃 VZ/WSL2/QEMU 三 driver，全部由 nono 承载。
- 优点：单一引擎，行为最一致。
- 缺点：若 nono 是 Linux-only 方案（大概率），macOS/Windows 宿主无从谈起——不可行性很高，列出仅为完备。

## 对比

| 维度 | A 纯平台隔离 | B driver + nono 内核 | C nono 唯一引擎 |
| --- | --- | --- | --- |
| 复用度 | 高（arcbox） | 中高（arcbox + nono） | 取决于 nono（存疑） |
| 平台覆盖 | 三平台 | 三平台（nono 在 guest 内，Linux） | 存疑 |
| enforcement 精度 | 中 | 高（in-guest 最小权限） | 高（若可行） |
| 维护成本 | 低 | 中 | 未知 |

## 结论

**待评审**。倾向：若 `nono` 通过五问 → 选项 B；否则选项 A（把 guardrails 的 "security kernel" 职责由 qn-agent 的 enforcement 模块承担，并在 README/ADR 中记录该替代关系）。
**阻塞项**：`nono` 是什么（上游仓库、license、最近提交、平台矩阵、是否 Linux-only）——请 #2 作者在本 ADR 的 issue/PR 评论中补充。
**不阻塞**：M0/M1 按 A 路径启动（vf 适配、代答网关均不依赖本决策）；B 若采纳，nono 以 qn-agent 内可选模块接入，不动 driver 层。

## 后果

- 正面：架构分叉被显式记录，两条叙述（guardrails / ADR-0002）不再静默并存。
- 负面：拍板前 "Sandbox adapter"（P0 第 1 项）的实现深度存在两种解读（纯 driver 适配 vs + nono 集成）。
- 重审条件：nono 资料补充即重开评审；或 P0 结束时按实际实现回填结论。
