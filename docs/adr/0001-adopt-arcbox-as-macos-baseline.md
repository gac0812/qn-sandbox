# ADR-0001 — 以 arcbox-custom 作为 macOS 侧基线，经 git subtree 引入并视为只读上游

- 状态：Proposed（待用户确认）
- 日期：2026-10-24
- 决策人：AI 提议

## 背景

目标产品需要 macOS（Apple Silicon 起步）上的本地 Linux 沙箱。
[laitekin/arcbox-custom](https://github.com/laitekin/arcbox-custom) 已实现：Virtualization.framework 集成、
Linux guest agents（exec/文件 RPC）、镜像导入、SOCKS5 代理与网络策略、隔离机器等——与本项目 macOS 需求高度重合。
自研等价能力需要数月；引入则需要决定引入方式（源码依赖、fork、subtree）。

## 选项

### 选项 A：git subtree 引入 `arcbox/`，只读上游 + 适配层
- 优点：仓库自包含（clone 即全量可编译）；上游更新可控（显式 pull）；适配层（qn-driver-vf）隔离上游 API 变化；许可（MIT/Apache-2.0）随源保留。
- 缺点：仓库体积增大；subtree 命令对协作者有学习成本。
- 风险与缓解：上游断更 → 我们的适配层已隔离，最坏情况冻结使用；上游 API 变动 → 同步 PR 单独评审。

### 选项 B：以 arcbox 为 git fork 直接二次开发
- 优点：起步最快。
- 缺点：项目身份与上游历史强绑定；跨平台代码与上游 Swift 桌面端搅在一起；Windows 扩展无处安放；不符合"低耦合"目标。
- 风险：fork 越改越偏，最终既回不去也跟不上。

### 选项 C：不引入源码，仅参考重写
- 优点：零上游包袱。
- 缺点：重写 VZ 集成与 guest agents = 最大的"重复造轮子"，直接违反项目第一原则。

## 对比

| 维度 | A subtree | B fork | C 重写 |
| --- | --- | --- | --- |
| 复用度 | 高（源码级） | 高 | 无 |
| 平台扩展（win/intel） | 好（qn-* 独立扩展） | 差（搅在 fork 里） | 好但成本极高 |
| 维护成本 | 中（同步纪律） | 高 | 最高 |

## 结论

**选项 A**。落地：
1. `git subtree add --prefix=arcbox https://github.com/laitekin/arcbox-custom.git <默认分支> --squash`
2. `qn-driver-vf` 只做"arcbox 能力 → qn-core port"的适配；修改上游需符合 AGENTS.md §9。
3. 保留上游 LICENSE/NOTICE；仓库根 NOTICE 声明来源。

## 后果

- 正面：M1 里程碑的工作量从"实现 macOS 沙箱"降为"包装 + CLI"。
- 负面：接受仓库体积与同步纪律成本。
- 重审条件：上游 6 个月无维护且出现必须大改的缺口。
