# AGENTS.md — qn-sandbox AI 协作规范

> 本文件是所有 AI Agent（以及人类协作者）在本仓库工作的**最高行为准则**。
> 与其他规范冲突时，以本文件为准；本文件未覆盖的细节，见 `docs/spec/` 下各专项规范。
> 每次会话开始时，AI 必须先读完本文件，再按 §1 的顺序加载上下文，然后才开始动手。

---

## 0. 项目一句话

**qn-sandbox**：桌面级 AI 安全沙箱。让 Claude Code、Codex、dsh 等主流 Agent 应用在隔离的
Linux 环境里安全干活——安全第一（强隔离 + 凭证不落沙箱），用户友好第二（零手工技术配置）。
基于 arcbox-custom 的 macOS 实现，扩展出 Windows（WSL2 优先）与 Intel Mac 支持，
对上层提供统一 API（CLI / HTTP / MCP / SDK）。
铁律：沙箱内配置任何 Agent 应用或 MCP 服务一律通过凭证代答网关引用宿主凭证；
**明文敏感信息（API key、password）绝不进入沙箱的文件系统、环境变量、进程参数、镜像或托管配置**
（guardrails 不变式，无短期例外，见 08 §4 / ADR-0004）。

三条不可动摇的工程底线，所有规则都是它们的展开：

1. **不重复造轮子** —— 先找现成的、 licenses 干净的、活跃维护的库；找不到才自己写，且必须写明依据。
2. **低耦合高内聚** —— 依赖只允许自上而下指向抽象；平台差异被隔离在 driver 层，绝不外泄。
3. **可验证** —— 每个变更都附带能复现的验证证据（命令 + 结果），没有证据不算完成。

---

## 1. 上下文加载顺序（每次任务必做）

按需加载，禁止"什么都不读就开写"，也禁止"把所有文档读完才动第一步"：

```
1. 本文件（AGENTS.md）
2. docs/spec/00-product-scope.md          → 确认要做的事在范围内、在哪个里程碑
3. docs/spec/01-architecture.md 的相关章节 → 确认改动落在哪一层、依赖方向是否合法
4. 改动目标 crate 的 README.md + 现有代码  → 确认已有实现，避免重复
5. docs/adr/ 中与任务相关的决策记录        → 确认不违背已定决策
6. 具体编码规范（02/03）+ 测试规范（05）    → 动手前最后过一遍对应条款
```

改动横跨多层、或引入新模块/新依赖时，额外必读 `04-dependency-reuse.md` 与 `07-documentation-adr.md`。

---

## 2. 硬性红线（违反即返工，不接受任何理由）

| # | 红线 | 为什么 |
| --- | --- | --- |
| R1 | **禁止在没有调研的情况下手写通用工具**（HTTP 客户端、配置解析、日志、压缩、tar、路径处理、重试、ID 生成……）。动手前必须先搜：本仓库 → `docs/spec/04-dependency-reuse.md` 的复用矩阵 → crates.io/npm/PyPI。 | 重复造轮子是本项目第一禁忌 |
| R2 | **禁止引入新依赖而不说明**：PR 描述必须写清"解决了什么、为什么不自己写、license 是什么、是否活跃维护、带来了多少体积"。 | 依赖是长期负债 |
| R3 | **禁止跨层引用**：surface 层不得 import 具体 driver 类型；core 不得 import 任何 driver/surface；driver 之间禁止互相引用。违规方向见 `01-architecture.md` §3 的依赖方向图。 | 低耦合的强制保障 |
| R4 | **禁止在 subtree 目录（`arcbox/`）内直接修改上游代码**。需要改时：小改走"补丁文件 + 覆盖说明"，大改在 `qn-*` crate 中包装/组合。 | 保持可同步上游的能力 |
| R5 | **禁止 `unwrap()` / `expect()` 出现在非测试代码**（`const` 初始化与"构造即证明不会失败"的场合除外，且必须注释证明逻辑）。错误一律用 `thiserror` 定义本 crate 错误类型并在边界转换。 | 沙箱软件，崩溃即失守 |
| R6 | **禁止平台相关代码散落各处**：`#[cfg(target_os)]` 只允许出现在 driver crate 的模块入口、`qn-config` 与 `qn-bootstrap`（装配根）中，业务逻辑必须面对 trait 编程。 | Windows/macOS 差异只能存在于 driver 层 |
| R7 | **禁止合入无法验证的代码**：每个 PR 必须包含"验证证据"小节（跑了什么命令、什么平台、结果如何）。CI 绿是最低要求，不是全部。 | 可验证性是底线 |
| R8 | **禁止留无归属的 TODO/FIXME**：格式必须是 `// TODO(<issue号或 owner>): 什么时候、怎么解决`。裸 TODO 视为缺陷。 | 债务必须可见可追踪 |
| R9 | **禁止手写生成物**：protobuf/Connect/OpenAPI 生成的代码、lockfile、`dist/` 一律由工具生成，改动方式是"改源头重新生成"，不是手编辑。 | 生成物手改必然漂移 |
| R10 | **禁止在文档、注释、日志、错误信息之外的地方写中文**（标识符、字符串枚举值、路径等用英文）。对外日志/错误信息用英文，面向用户 UI 文案可中文。 | 编码与检索一致性 |
| R11 | **禁止改动超出任务范围**：顺手重构、顺手格式化无关文件、顺手升级依赖，都会让 review 无法聚焦。发现相邻问题 → 记 TODO 或新 issue，不要顺手改。 | 变更最小化 |
| R12 | **禁止凭记忆写 API**：不确定的库用法必须查文档或读源码；不确定的文件内容必须先 Read 再 Edit。 | 幻觉 API 是事故之源 |

---

## 3. 标准工作流（六步，每步有出口条件）

### Step 1 — 理解（Understand）
- 用自己的话复述任务：改什么、为什么、成功标准是什么。
- 定位改动将落在 `01-architecture.md` 分层图中的哪一层（或多层）。
- **出口条件**：能写出一句"本任务在 `<crate>` 中实现 `<能力>`，通过 `<接口>` 暴露，验证方式是 `<测试>`"。

### Step 2 — 调研（Research）
- 先搜本仓库：`grep` 关键能力词，看是否已有实现或半成品。
- 再查复用矩阵（`04-dependency-reuse.md`）与 crates.io/npm/PyPI。
- 候选库按"五问"评估（license / 维护活跃度 / 平台覆盖 / 体积与依赖树 / MSRV 与工具链兼容），过程记录进 PR 描述。
- **出口条件**：能回答"这个能力，用现成的什么，还是为什么必须自己写"。

### Step 3 — 设计（Design）
- 改动局限在单个 crate 内部 → 直接在 PR 描述里写设计（数据结构、函数签名、错误路径）。
- 改动跨 crate、新增模块、新增 trait、改变依赖方向 → **必须先写 ADR**（`07-documentation-adr.md`），状态 Proposed，待确认后再编码。
- 设计必须包含：错误如何传播、平台差异如何隔离、如何测试。
- **出口条件**：设计小到能在一屏内说清，或大到有一份 ADR；二者必居其一。

### Step 4 — 实现（Implement）
- 严格按 `02-coding-standard-rust.md`（或 03）写代码。
- 遵循"先让最小路径通过测试，再补全错误处理与边界"的增量节奏。
- 遇到设计与现实冲突：停下来更新设计/ADR，而不是让代码悄悄偏离设计。

### Step 5 — 验证（Verify）
- 本地依次跑过并贴出证据（命令 + 结果摘要）：
  ```
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo deny check          # license / 依赖审计，涉及依赖变更时
  ```
- 涉及平台后端的改动：在目标平台跑 `05-testing-quality.md` 规定的最低平台测试矩阵，注明宿主机信息（OS 版本 / 芯片）。
- **出口条件**：§6 自检清单全绿。

### Step 6 — 汇报（Report）
- 按 §7 模板汇报。禁止把"应该能跑"当结论；没验证的就明说"未验证，因为 XX"。

---

## 4. 代码整洁规则（AI 最常犯的错误逐条对照）

### 4.1 命名
- 类型/ trait / crate：`UpperCamelCase`；函数/变量/模块：`snake_case`；常量：`SCREAMING_SNAKE_CASE`。
- 名字要能读出**意图**而不是**类型**：`pending_sandboxes` 好，`sandbox_vec` 坏；`connect_with_retry` 好，`do_connect2` 坏。
- 布尔用 `is_/has_/can_` 前缀；异步动作用动词原形（`spawn_sandbox`），取值不加 `get_` 前缀（`config()` 不是 `get_config()`）。
- 领域词汇以 `glossary.md` 为准，全仓库统一（例：宿主进程里的运行单元一律叫 `Sandbox`，不混用 VM / Machine / Box）。

### 4.2 函数与结构
- 函数只做一层抽象的事：要么编排（调用其他函数），要么执行（自己干活），不混在同一个函数里。
- 软上限：函数 50 行 / 参数 5 个 / 嵌套 3 层 / 单文件 400 行。超了先想拆分，确实合理再写明原因。
- 结构体字段 ≤ 8 个，多了拆子结构或用 builder（`derive_builder` 或手写，二选一按 crate 内先例）。
- 禁止"防御性参数"（传了却从不检查的 flag）、禁止布尔陷阱参数（公开 API 用 `enum` 表达模式）。

### 4.3 注释与文档
- 注释只写**代码本身说不出来的事**：为什么这么做、约束来自哪里、什么情况下不能这么写。
  ```rust
  // 好：解释约束
  // WSL2 的 tar 不支持 --sparse，镜像大于 4GiB 时必须分卷。
  // 坏：复述代码
  // 用 tar 解压文件
  ```
- 每个公开（`pub`）类型、函数、trait 必须有 `///` 文档：一句话说清用途，然后给一个可复制的例子（能用 `doc(test)` 验证的优先）。
- `pub(crate)` 及以下不强制文档，但结构怪异处必须有"为什么"注释。

### 4.4 错误处理
- 每个 crate 一个 `Error` 枚举（`thiserror`），对外边界转换成统一 `qn-core::Error`，禁止把 `anyhow` 泄漏到公共 API 签名上（应用层 `qn-cli` 内部可以用）。
- 错误信息给**行动指引**：`failed to start sandbox {id}: image {img} not found, run `qn image pull {img}` first` 好；`start failed` 坏。
- 错误链必须保留来源（`#[from]`），禁止 `map_err(|_| ...)` 抹掉上下文。
- 用户输入/外部数据一律显式校验，返回类型化错误，不 panic。

### 4.5 日志与可观测性
- 统一 `tracing`：入口建 span（字段：`sandbox_id`、`image`、`backend`），事件带结构化字段，不用字符串拼接。
- 级别纪律：`error`=需要人介入；`warn`=自动降级/重试了；`info`=生命周期事件（sandbox 启停等）；`debug`=请求/响应摘要；`trace`=协议细节。禁止 `info` 打印循环体内逐条数据。
- 禁止在日志输出密钥、token、用户文件内容。

### 4.6 异步与并发
- 运行时统一 tokio（multi-thread），禁止混用其他异步运行时。
- 长任务（镜像导入、VM 启动）必须有进度事件与取消路径（`CancellationToken`），不可取消的操作必须在文档标注。
- 共享状态优先消息传递（channel）或 `Arc<Mutex<…>>` 粒度最小化；禁止全局 `static mut`、禁止在 async 上下文用 std 阻塞锁——阻塞调用放 `spawn_blocking`。

### 4.7 平台隔离写法（本项目特色，违反是 R6）
```rust
// ✅ 正确：cfg 只出现在模块入口
// qn-driver-wsl/src/lib.rs
#[cfg(target_os = "windows")]
mod wsl_backend;
#[cfg(target_os = "windows")]
pub use wsl_backend::WslBackend;

// ❌ 错误：业务函数里长出平台分支
fn start(sandbox: &Sandbox) -> Result<()> {
    if cfg!(target_os = "windows") { /* …30 行 wsl 逻辑… */ }
    else { /* …30 行 vf 逻辑… */ }
}
```
- 每个平台特有行为先升级成 core trait 的一个方法或一个配置维度，再在 driver 里实现。发现"两个 driver 复制了同一段逻辑"时，把公共部分下沉到 `qn-core` 或公共 util crate。

---

## 5. 依赖与复用纪律（细则见 04 文档，这里只有行为准则）

1. **每次想写 `fn xxx_util()` 之前**，先假设它已经存在——去找到它。
2. 引入依赖的决策权：单个小依赖（无传递依赖膨胀）AI 可自决并记录；架构级依赖（运行时、框架、协议栈、GUI）必须走 ADR。
3. 候选依赖必须"五问"全部有答案才可用（license 宽松、近 6 个月有提交、覆盖目标平台、依赖树可控、MSRV 兼容）。
4. 禁止复制粘贴网上/AI 记忆里的代码片段——要么用依赖，要么自己写并注明出处思路；复制超过 10 行的第三方代码必须连同 license 头一起引入并记录在 `NOTICE`。
5. subtree（`arcbox/`）内的能力优先直接使用；包装新接口前先确认上游没有等价物。

---

## 6. 完工自检清单（提交前逐项打勾）

```markdown
- [ ] 改动范围与任务声明一致，无顺手修改（R11）
- [ ] 没有引入任何"先搜一下就肯定有"的重复实现（R1）
- [ ] 新依赖：五问答案已写进 PR 描述（R2）；架构级依赖已有 ADR
- [ ] 依赖方向合法：core 不依赖 driver/surface；surface 不 import 具体 driver（R3）
- [ ] `#[cfg(target_os)]` 只出现在允许的位置（R6）
- [ ] 无 unwrap/expect（非测试）；错误类型化、上下文完整（R5）
- [ ] 公开 API 全部有 /// 文档；注释只解释"为什么"
- [ ] 日志用 tracing 结构化字段；无敏感信息
- [ ] 新增/修改行为有对应测试；`cargo fmt/clippy/test` 全绿并附证据（R7）
- [ ] TODO 带归属（R8）；生成物未被手改（R9）
- [ ] 相关文档已同步：crate README / spec / ADR / glossary
- [ ] 汇报按 §7 模板给出，未验证项如实标注
```

---

## 7. 汇报模板（每个任务结束时）

```markdown
## 结果
<一句话：完成了什么>

## 变更清单
- <crate/文件>: <做了什么，为什么>

## 验证证据
- 命令: `cargo test -p qn-driver-wsl`
- 平台: Windows 11 23H2 x64 / WSL2 2.1.5
- 结果: 23 passed, 0 failed
（未覆盖的平台/场景如实列出：macOS 未测，因为本机无 macOS）

## 设计取舍
<一两句关键取舍；若做了架构级决定，附 ADR 链接>

## 遗留
- TODO(<issue>): <事项>
```

---

## 8. Git 与提交（细则见 06 文档）

- Conventional Commits：`feat(driver-wsl): support sparse image import`。scope 用 crate 名去掉 `qn-` 前缀。
- 一个提交一件事；提交前 rebase；禁止 `git commit -m "fix"`。
- 分支名：`feat/<主题>`、`fix/<主题>`、`spike/<主题>`（调研性）。
- **AI 不得自行 push 到 `main`**；默认在工作分支提交，由用户确认后合并。

---

## 9. 与 arcbox 上游的关系（R4 展开）

- `arcbox/` 目录通过 git subtree 引入，**视为只读上游**。
- 上游 bug/缺失：优先判断"上游将来会不会接受这个修复"——会 → 准备可回灌上游的补丁并记录 `docs/patches/`；不会 → 在 `qn-*` 侧包装。
- 同步上游是显式任务：`git subtree pull` 单独提交，绝与其他变更混合。
- 复用上游能力前先读 `arcbox/` 内对应模块的 README 与现有调用点，禁止凭印象假设其行为。
