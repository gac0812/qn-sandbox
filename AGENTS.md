# AGENTS.md — qn-sandbox AI 协作规范

> 本文件是所有 AI Agent（以及人类协作者）在本仓库工作的**最高行为准则**。
> 与其他规范冲突时，以本文件为准；人类协作规则见 CONTRIBUTING.md（队友所有）。
> 每次会话开始时，AI 必须先读完本文件，再按 §1 的顺序加载上下文，然后才开始动手。

---

## 0. 项目一句话

**qn-sandbox**：桌面级 AI 安全沙箱。让 Claude Code、Codex、dsh 等主流 Agent 应用在内核强制的
最小权限下安全干活——安全第一（隔离 + 凭证不进沙箱），用户友好第二（零手工技术配置）。
产品是 **thin product on nono**：安全内核采用开源 [nono](https://github.com/nolabs-ai/nono)
（Landlock/Seatbelt 进程沙箱 + phantom token 凭证代理，Apache-2.0，锁版本跟版），
我们做适配、产品化（CLI/Tauri 控制面/MCP gateway/profiles/审计）与 Windows WSL2 供给链路。
平台：macOS（nono 原生）、Windows（qn 供给 WSL2 + nono）；VM 强隔离模式为 P2 可选项。

铁律（guardrails Security Invariants，无短期例外）：
**凭证明文绝不进入沙箱的文件系统、环境变量、进程参数或 Agent 上下文**；
fs/网络默认拒绝、按 profile 放行；特权动作必须留审计事件；撤销立即生效。

三条工程底线，所有规则都是它们的展开：

1. **不重复造轮子** —— nono 已实现的（进程沙箱/凭证代理/网络规则/审计）绝不自研第二份；
   先找现成库，找不到才自己写并写明依据。
2. **低耦合高内聚** —— 依赖只允许自上而下指向抽象；引擎差异封在 `qn-nono`，平台差异封在 `qn-wsl`。
3. **可验证** —— 每个变更附带可复现的验证证据（命令 + 结果），没有证据不算完成。

---

## 1. 上下文加载顺序（每次任务必做）

按需加载，禁止"什么都不读就开写"，也禁止"把所有文档读完才动第一步"：

```
1. 本文件（AGENTS.md）
2. docs/spec/00-product-scope.md          → 确认任务在哪个优先级/工作包内
3. docs/spec/01-architecture.md 相关章节   → 确认改动落在哪一层、依赖方向是否合法
4. 改动目标 crate 的 README + 现有代码     → 确认已有实现，避免重复
5. 与任务相关的决策 issue / spec 对应章节   → 确认不违背已定结论
6. 具体编码规范（02/03）+ 测试规范（05）    → 动手前最后过一遍
```

改动横跨多层、引入新模块/新依赖时，额外必读 `04-dependency-reuse.md` 与 `07-documentation.md`。

---

## 2. 硬性红线（违反即返工，不接受任何理由）

| # | 红线 | 为什么 |
| --- | --- | --- |
| R1 | **禁止在没有调研的情况下手写通用工具**（HTTP 客户端、配置解析、日志、压缩、路径处理、重试、ID 生成……）。动手前必须先搜：本仓库 → `04` 复用矩阵 → crates.io/npm/PyPI。**尤其禁止绕过 nono 手写 Landlock/seccomp/Seatbelt 调用或凭证代理。** | 重复造轮子是本项目第一禁忌 |
| R2 | **禁止引入新依赖而不说明**：PR 描述必须写清"解决了什么、为什么不自己写、license、活跃度、平台覆盖"。架构级依赖（安全内核、RPC 框架、GUI 框架）需三人在决策 issue 中确认。 | 依赖是长期负债 |
| R3 | **禁止跨层引用**：surface 层不得 import 具体适配器；core 不依赖适配器/surface；适配器互不依赖。nono 只能被 `qn-nono` 触碰，凭证代理只能被 `qn-broker` 触碰。 | 低耦合的强制保障 |
| R4 | **禁止 fork / subtree 引入上游源码**（nono 或任何项目）。外部能力一律以依赖或外部进程方式集成；缺失能力回馈上游，上游不可用则在 adapter 层吸收。 | 薄产品层原则；避免不可维护的 fork |
| R5 | **禁止 `unwrap()` / `expect()` 出现在非测试代码**（"构造即证明不会失败"的场合除外，且必须注释证明逻辑）。错误一律 `thiserror` 类型化并在边界转换。 | 安全软件，崩溃即失守 |
| R6 | **禁止平台相关代码散落**：`#[cfg(target_os)]` 只允许出现在 `qn-config`、`qn-bootstrap`、`qn-wsl`，业务逻辑必须面对 trait 编程。 | 平台差异只能存在于适配层 |
| R7 | **禁止合入无法验证的代码**：每个 PR 必须包含"验证证据"（命令、平台、结果）。CI 接入时以 CI 绿为最低要求；当前 Actions 暂停期间按 §8.4 的本地门禁记录执行。 | 可验证性是底线 |
| R8 | **禁止留无归属的 TODO/FIXME**：格式 `// TODO(<issue号或 owner>): 什么时候、怎么解决`。裸 TODO 视为缺陷。 | 债务必须可见可追踪 |
| R9 | **禁止手写生成物**：契约生成代码、lockfile、构建产物一律由工具生成，改法是"改源头重新生成"。 | 生成物手改必然漂移 |
| R10 | **禁止在文档/注释/日志/错误信息之外的地方写中文**（标识符、枚举值、路径用英文）；对外日志/错误英文；面向用户的 UI 文案可中文。 | 编码与检索一致性 |
| R11 | **禁止改动超出任务范围**：顺手重构、顺手格式化无关文件、顺手升级依赖都让 review 无法聚焦。相邻问题 → 记 TODO 或 issue。 | 变更最小化 |
| R12 | **禁止凭记忆写 API**：不确定的库用法必须查文档或读源码；不确定的文件内容必须先 Read 再 Edit；不确定 nono 行为必须实测（它 pre-1.0，文档可能滞后）。 | 幻觉 API 是事故之源 |

---

## 3. 标准工作流（六步，每步有出口条件）

### Step 1 — 理解（Understand）
- 复述任务：改什么、为什么、成功标准。
- 定位改动落在 01 分层图的哪一层。
- **出口条件**：能写出"本任务在 `<crate>` 实现 `<能力>`，经 `<port/接口>` 暴露，验证方式是 `<测试>`"。

### Step 2 — 调研（Research）
- 先搜本仓库；再查复用矩阵（04）与 crates.io/npm/PyPI；涉及 nono 能力先读其文档/源码/CHANGELOG。
- 候选依赖按"五问"评估（04 §2），记录进 PR 描述。
- **出口条件**：能回答"这个能力用现成的什么，或为什么必须自己写"。

### Step 3 — 设计（Design）
- 单 crate 内部改动 → PR 描述里写设计（数据结构、签名、错误路径、平台差异处理）。
- 跨 crate、新增/改 port、架构级依赖 → **先开决策 issue**（背景/选项≥2/结论/重审条件，07 §3），
  三人确认后同步修订 spec 再编码。
- **出口条件**：设计小到一屏说清，或大到有一个决策 issue；二者必居其一。

### Step 4 — 实现（Implement）
- 严格按 02（或 03）写；增量节奏：最小路径过测试 → 补错误处理与边界。
- 设计与现实冲突：停下开 issue / 修订 spec，不静默偏离。

### Step 5 — 验证（Verify）
- 本地跑过并贴证据：
  ```
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo deny check          # 依赖变更时
  ```
- 平台相关改动在目标平台跑 05 §4 最低矩阵，注明宿主机信息。
- **出口条件**：§6 自检清单全绿。

### Step 6 — 汇报（Report）
- 按 §7 模板汇报；没验证的明说"未验证，因为 XX"。

---

## 4. 代码整洁规则（AI 最常犯的错误逐条对照）

### 4.1 命名
- 类型/trait/crate `UpperCamelCase`；函数/变量/模块 `snake_case`；常量 `SCREAMING_SNAKE_CASE`。
- 名字读**意图**不读类型：`pending_sessions` 好，`session_vec` 坏。
- 布尔 `is_/has_/can_` 前缀；异步动词原形（`run_session`）；取值不加 `get_`。
- 领域词汇以 `glossary.md` 为准：运行单元叫 `Session`；引擎适配叫 `*Adapter`；凭证代答叫 `CredentialBroker`。

### 4.2 函数与结构
- 函数只做一层抽象的事：编排或执行，不混。
- 软上限：函数 50 行 / 参数 5 个 / 嵌套 3 层 / 单文件 400 行；结构体字段 ≤ 8。
- 禁止布尔陷阱参数（公开 API 用 enum）；禁止从不检查的防御性参数。

### 4.3 注释与文档
- 注释只写代码说不出来的事：为什么、约束来自哪、什么时候不能这么写。
- 每个公开项必须有 `///`：一句话用途 + 可运行例子（优先 doctest）。
- 对 nono 行为的假设必须注明来源（文档链接/版本/实测 issue），它 pre-1.0 易漂移。

### 4.4 错误处理
- 每 crate 一个 `thiserror::Error`；公共 API 禁 `anyhow`（应用层内部可用）。
- 错误信息给行动指引：`failed to run session {id}: profile {p} not found, run 'qn profile list' first`。
- 错误链保留来源（`#[from]`）；禁止 `map_err(|_| …)` 抹掉上下文。
- 拒绝（Denial）与错误（Error）是不同领域概念：内核拒绝的访问尝试进审计流，不是进程错误。

### 4.5 日志与可观测性
- 统一 `tracing`：入口建 span（`session_id`、`profile`、`engine`），结构化字段。
- 级别纪律：`error` 需人介入 / `warn` 自动降级 / `info` 生命周期 / `debug` 摘要 / `trace` 细节。
- **日志禁止出现任何凭证值、phantom token、Handle 以外的敏感内容。**

### 4.6 异步与并发
- tokio（multi-thread）统一；外部命令（nono、wsl.exe）一律经 `qn-infra::process`（超时/截断/退出码）。
- 后台任务必须有生命周期（JoinHandle 保存、显式 shutdown）；长任务可取消（CancellationToken）。
- 锁内禁止 `.await`；阻塞调用放 `spawn_blocking`。

### 4.7 安全编码专项（本项目特色）
- 任何把字符串拼进 shell 的路径都是缺陷：命令走参数数组。
- 托管配置写入前必须过 `qn-broker` 校验（明文凭证即拒绝）——不允许"先写后审"。
- 对 nono 的每次调用都要考虑失败模式：引擎不存在/版本不符/拒绝启动，全部类型化错误。

---

## 5. 依赖与复用纪律（细则见 04）

1. 写 `fn xxx_util()` 之前，先假设它存在——去找到它。
2. nono 是安全内核唯一入口：版本锁在 qn-config；升级 = 独立 PR + 回归验证（04 §6）。
3. 引入依赖"五问"全答才可用；架构级需三人决策。
4. 禁止复制来路不明的代码片段；>10 行第三方代码必须带 license 头并记录 NOTICE。

---

## 6. 完工自检清单（提交前逐项打勾）

```markdown
- [ ] 改动范围与任务声明一致，无顺手修改（R11）
- [ ] 没有绕过 nono/qn-nono 自研隔离或凭证机制（R1/R3）
- [ ] 新依赖五问已答（R2）；架构级依赖已经三人确认（决策 issue）
- [ ] 依赖方向合法（R3）；#[cfg(target_os)] 只在 qn-config/qn-bootstrap/qn-wsl（R6）
- [ ] 非测试代码无 unwrap/expect（R5）；错误类型化、上下文完整
- [ ] 公开 API 有 ///；注释只解释"为什么"；nono 行为假设注明来源
- [ ] 日志无凭证/token/敏感内容；拒绝事件走审计流
- [ ] 新增/修改行为有测试；cargo fmt/clippy/test 全绿并附证据（R7）
- [ ] TODO 带归属（R8）；生成物未手改（R9）
- [ ] 相关文档已同步（spec / crate README / glossary）
- [ ] 汇报按 §7 模板，未验证项如实标注
```

---

## 7. 汇报模板（每个任务结束时）

```markdown
## 结果
<一句话>

## 变更清单
- <crate/文件>: <做了什么，为什么>

## 验证证据
- 命令: …
- 平台: macOS 15 arm64（nono 1.x）/ Windows 11 + WSL2(内核 6.x)
- 结果: …
（未覆盖项如实列出）

## 设计取舍
<关键取舍；重大决策附 issue 链接>

## 遗留
- TODO(<issue>): <事项>
```

---

## 8. Git、PR 与提交流程（细则见 06，协作底线见 CONTRIBUTING）

GitHub Issue 是任务的唯一入口，`main` 是唯一集成分支。除仓库初始化外，所有变更
（代码、文档、配置、生成物）都必须经过分支、PR 和 Squash Merge，Review 按 §8.4 执行。
**AI 不 push `main`、不 approve 任何 PR、不启用 auto-merge。**

**合并授权（所有者例外）**：仓库所有者 `laitekin` 的 PR 由 AI 直接 Squash Merge，
不要求额外人类 Approve；其他人的 PR 仍必须由人类合并。该例外不豁免 §8.4 的本地门禁、
证据要求和合并后验证，也不允许跳过测试、安全检查或 secret scan。

### 8.1 开工前：Issue 与分支

1. 必须有一个可执行的 Issue，并写清：背景、目标、非目标、风险等级、验收标准、
   验证计划、关联 Milestone。缺少任一项时先补 Issue，不开始写代码。
2. Issue/PR 至少带一个 `P0` / `P1` / `P2` 优先级标签和适用风险标签。
   优先级标签不是版本号、分支名、文件名或 Git Tag。
3. 涉及新增/修改 Port、跨 crate 契约、架构级依赖或推翻既有结论时，先开决策 Issue；
   三人确认并同步 spec 后才能创建实现分支。
4. 创建分支前同步 `main`：

   ```bash
   git switch main
   git fetch origin --prune
   git pull --ff-only origin main
   ```

5. 从最新 `main` 创建短分支，命名格式：

   ```text
   <type>/<issue>-<slug>
   ```

   允许的 type：`feat`、`fix`、`refactor`、`test`、`docs`、`chore`、`spike`、`deps`。
   示例：`feat/12-core-ports`、`spike/13-nono-macos`。

6. 一个分支只对应一个 Issue；目标存活时间 ≤ 1 天，硬上限 3 天。超期先
   rebase 最新 `main` 并拆小，禁止把多个无关任务塞进同一分支。
7. 不在本地 `main` 上开发，不从过期分支继续切新分支，不建立长期共享集成分支。

### 8.2 Commit：一个提交一个完整意图

- Commit 标题使用英文 Conventional Commits：

  ```text
  <type>(<scope>): <imperative summary>
  ```

  scope 使用 crate 名去掉 `qn-` 前缀，例如
  `core`、`nono`、`broker`、`gateway`、`wsl`、`daemon`、`cli`、`ui`、`mcp`。
- 正文只在需要时说明动机、取舍和已知限制，不复述 diff。
- 每个提交必须能独立说明意图，并尽量保持可构建；禁止把格式化、无关重构和功能修改
  混在同一提交。
- 提交前必须完成：

  ```bash
  git diff --check
  git diff --cached
  ```

- staged diff 必须人工检查，确认没有真实凭证、Token、`auth.json`、Keychain 导出、
  `材料/`、宿主配置、构建产物或生成物手改。
- 只做与 Issue 验收标准直接相关的修改；相邻问题记录为 TODO 或新 Issue（R11）。
- Review 开始后不擅自改写公共历史。确需 rebase 时先确认没有其他协作者基于该分支工作；
  共享分支禁止普通 force push，只能使用 `--force-with-lease` 并在 PR 中说明。

### 8.3 PR：尽早暴露，保持可审查

- 第一个可评审提交后直接创建正式 PR，不使用 Draft PR；禁止完成后才一次性推送全部工作。
- PR 标题必须是英文，并遵循 Conventional Commits；PR 正文可使用中文。
- PR 必须关联 Issue，使用 `Closes #<issue>`；PR 范围必须与 Issue 一致。
- 创建 PR 时不要求作者手动邀请或指定 Reviewer；Review 由团队按分工自行认领。
- PR 的 diff 原则上 ≤ 400 行（lockfile、生成代码和机械迁移不计）。超限时拆 PR，
  不以“同属一个功能”为理由提交大 diff。
- PR 描述至少包含：
  - 做了什么、为什么；
  - 安全边界与依赖方向是否变化；
  - 验证命令、平台、实际结果与未覆盖项；
  - 新依赖五问（无依赖时删除）；
  - 兼容性、回滚方式与已知限制。
- 触及安全机制、凭证、网络策略或审计时，标题使用 `security:` 前缀，并提供攻击验证子集。
- 依赖升级使用 `deps:` 前缀；nono 升级必须独立 PR，附上游 release notes 与回归证据。
- 禁止在 PR 描述、评论、截图、日志或测试输出中出现真实凭证和内部赛题材料。

### 8.4 CI 与 Review：未验证不算完成

- 合并前必须通过仓库现有质量门禁：

  ```text
  fmt / clippy / test / cargo-deny / secret scan
  ```

  当前 GitHub Actions 暂停（#145）期间，PR 必须粘贴本地命令、平台、结果和未覆盖项；
  恢复 CI 前由人类 Reviewer 检查这些证据。恢复后再以远程绿勾为准，不能用手工结果替代已恢复的必需检查。
- 至少获得 1 名人类 Approve；作者不能作为自己 PR 的唯一 Review。
  所有者 `laitekin` 的 PR 按 §8 合并授权免 Approve，但 AI 仍不得 approve 任何 PR。
- 横切线由对应负责人自行认领 Review，不要求 PR 作者邀请：
  nono/上游跟版 → A，安全条款 → B，架构与依赖方向 → C。
- Review 优先检查：依赖方向与分层 → 安全边界 → 错误与边界 → 测试充分性 → 命名与风格。
- Review 意见标记为 `blocking`、`question`、`nit`、`fyi`；只有 `blocking` 阻塞合并。
  所有 `blocking` 必须解决，所有 Review conversation 必须 resolved。
- 新增/修改行为必须附带测试；安全路径必须包含拒绝场景和审计证据，不能只测成功路径。

### 8.5 Merge：只走 Squash，合并后仍需验证

合并必须同时满足：

- 关联 Issue 的验收标准全部满足；
- PR 与本 Issue 范围一致，无未说明的顺手修改；
- CI 必需检查全绿，Review conversation 全部 resolved；
- ≥ 1 名人类 Approve，必要的横切线负责人已 Review（所有者 `laitekin` 的 PR 按 §8
  合并授权免 Approve）；
- 分支已同步最新 `main`，且没有冲突。

合并规则：

1. 只使用 **Squash Merge**，保持 `main` 线性历史。
2. 合并后删除短生命周期分支。
3. `main` 合并后再从干净工作区执行最小 smoke/E2E；不要把本地测试通过当成远程 CI 结果。
4. 在 Issue/PR 中补充合并后验证结果；未验证项明确列出，不能口头声称完成。
5. 行为变化同步更新 spec、crate README、profile README、CHANGELOG 或 TODO。

### 8.6 Tag 与 Release

- `v0` 是不可移动的导入基线 Tag，禁止删除、重打或 force-push。
- `P0` / `P1` / `P2` 只作为 Label，不能作为 Tag、分支名或代码标识。
- 正式发布使用 `vMAJOR.MINOR.PATCH`，Tag 必须指向经过验证的 `main` Commit。
- Release 必须绑定源 Commit、构建产物和验证证据；Tag 创建后不可移动。

### 8.7 紧急修复

- 紧急修复也必须先有 Issue 和 PR，并在 PR 中说明受影响范围、恢复目标和回滚方式。
- 紧急路径只能压缩等待时间，不能跳过测试、安全检查和合并后验证。
- 合并后 24 小时内补齐复盘与后续改进 Issue。

---

## 9. 与 nono 上游的关系（R4 展开）

- **锁版本跟版**：版本号记在 qn-config；升级走独立 PR，正文列行为差异与回归证据。
- 不 fork、不改上游源码；缺失能力先提上游 issue/PR；确实等不及的在 `qn-nono` adapter 内吸收并注明原因。
- 每个里程碑开始时核对上游 release notes；pre-1.0 的 breaking 变化第一时间在 adapter 适配。
- 对 nono 行为的任何假设（如 Landlock 版本要求、phantom token 语义）都要有来源标注（R12）。
