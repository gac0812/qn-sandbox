# 06 — Git 工作流与协作

状态：Draft · 2026-10-24

## 1. 分支模型

```
main                # 始终可发布；保护分支：PR + CI 绿 + 1 review
├── feat/<主题>     # 功能
├── fix/<主题>      # 缺陷
├── spike/<主题>    # 调研（产出 ADR 或结论文档，通常不合并代码）
└── subtree-sync    # arcbox 上游同步专用分支
```

- 分支从最新 `main` 切出；合并用 **squash merge**（保持 main 历史线性、一个 PR 一个语义提交）。
- 生命周期 ≤ 3 天；超期先 rebase 并拆小。

## 2. 提交（Conventional Commits）

```
<type>(<scope>): <祈使句摘要>

[正文：动机与取舍，一段以内]
```

| type | 含义 | scope（= crate 名去 `qn-` 前缀） |
| --- | --- | --- |
| feat | 新能力 | core / driver-vf / driver-wsl / driver-qemu / agent / daemon / cli / mcp / config / infra / sdk-ts / sdk-py / docs / spec |
| fix | 缺陷修复 | 同上；跨 crate 用 `core,daemon` |
| refactor | 行为不变的重构 | |
| test | 只加测试 | |
| chore | 工具链/CI/依赖 | deps（依赖变更必标） |
| docs | 文档 | |

示例：
```
feat(driver-wsl): support sparse image import via wsl --import
fix(core): reject SandboxState transition Running->Creating
chore(deps): add fd-lock 4.0 (five-questions in PR body)
```

- AI 提交署名：正文中追加 `Co-authored-by: <agent标识>`；不得伪造人类作者。
- **AI 不直接 push main**；不 force push 共享分支。

## 3. PR 规范

### 3.1 通用要求

- **标题**遵循 Conventional Commits（§2）。
- **尺寸**：diff ≤ 400 行（生成物/lockfile 不计）。超限必须拆分；确实拆不动时在 PR 顶部说明理由。
- **一事一 PR**：功能、重构、依赖升级不混在同一 PR（对应 AGENTS R11）。
- 早期征求意见用 **Draft PR**；转 ready 前模板必须填写完整。
- 特殊前缀：`security:`（触及 08 机制）、`deps:`（依赖变更，正文附五问）；subtree 同步单独成 PR（§5）。

### 3.2 模板

模板已入库（`.github/pull_request_template.md`），核心小节：

| 小节 | 必填内容 |
| --- | --- |
| 做了什么 / 为什么 | 两三句 + 关联 issue |
| 设计取舍 | 关键决定与备选；架构级变更附 ADR 链接 |
| 验证证据（R7） | 命令与结果、平台信息、未覆盖项（如实标注，不得空） |
| 新依赖（R2） | 五问答案 |
| 自检清单 | AGENTS §6 逐项勾选 |

### 3.3 合并门禁

| 门 | 要求 |
| --- | --- |
| CI | `fmt / clippy / test（三平台矩阵）/ deny` 全绿 —— **CI 工作流待 M0 骨架搭建完毕后接入（TODO-001），接入前以 PR 中粘贴的本地运行记录代替** |
| Review | ≥1 名非作者 approve；触及横切线的 PR 需对应负责人看过（许可→A / 安全→B / 架构→C） |
| 模板 | 各小节填写完整，验证证据不得为空 |
| 自检 | AGENTS §6 清单全勾 |

- **AI 不自行合并、不 push `main`**；分支保护与模板自动加载在仓库上 GitHub 且 CI 接入后启用（TODO-003）。
- Review 重点顺序：**依赖方向与分层 → 错误与边界 → 测试充分性 → 命名与可读性 → 风格**。
- Review 意见未解决不得合并；AI 收到意见先理解再改：不机械服从也不固执，有分歧给论据，仍分歧交用户裁决。

## 4. 版本化与变更日志

- 宿主端 semver：`0.x` 阶段（M4 前）minor 即可含破坏性变更，但必须在 CHANGELOG 标 BREAKING。
- `CHANGELOG.md` 按 [Keep a Changelog]：每个里程碑一节，人为/AI 整理自 squash 提交。
- 契约（proto）变更与 SDK 版本联动规则见 `03-coding-standard-sdk.md` §5。

## 5. subtree 同步流程（R4）

```
git subtree pull --prefix=arcbox <upstream-url> <ref> --squash
```
- 单独 PR（title: `chore(subtree): sync arcbox @ <日期/版本>`），正文记录：同步了什么、本地补丁是否仍适用、行为影响。
- `docs/patches/*.md` 每个补丁一页：针对上游哪个文件、为什么、回灌状态。

## 6. 敏感与杂项

- 禁止提交：镜像文件、密钥/token、`~/.qn` 状态、构建产物（`.gitignore` 维护清单）。
- 大文件（测试镜像 >5MB）走 fixture 下载脚本（`tests/fixtures/fetch.sh`），不进 git。
- 提交信息与 PR 一律英文；代码注释遵循 02 §4.3。
