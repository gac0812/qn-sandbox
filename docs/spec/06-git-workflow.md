# 06 — Git 工作流与协作

状态：Draft · 2026-10-24 · 分支命名与评审底线遵循仓库 CONTRIBUTING.md（#2 所有），本文是其工程细化

## 1. 分支模型

```
main                 # 始终可发布；保护：PR +（CI 恢复后绿）+ ≥1 人类 review（CONTRIBUTING 要求）
└── <type>/<issue>-<slug>   # 短生命周期（≤3 天），从最新 main 切出
    feat/4-credential-broker
    fix/12-wsl-kernel-detect
    docs/8-architecture
    spike/15-nono-ffi
    deps/18-nono-bump        # nono 升级专用类型
```

- 合并用 **squash merge**（main 线性历史，一个 PR 一个语义提交）。
- 超期先 rebase 并拆小；不 force push 共享分支。

## 2. 提交（Conventional Commits，英文标题）

```
<type>(<scope>): <imperative summary>

[正文：动机与取舍，一段以内]
```

| type | 含义 | scope（crate 名去 `qn-` 前缀） |
| --- | --- | --- |
| feat | 新能力 | core / nono / broker / gateway / wsl / daemon / cli / ui / mcp / config / infra / profiles / sdk-ts / sdk-py |
| fix | 缺陷修复 | 同上；跨 crate 用 `core,daemon` |
| refactor / test / docs / chore | — | 依赖变更必用 `chore(deps)`；nono 升级用 `chore(deps)` + 分支类型 `deps` |

- 一个提交一件事。
- **AI 不 push `main`、不自行合并。**

## 3. PR 规范

### 3.1 通用要求
- 标题 = Conventional Commits；diff ≤ 400 行（生成物/lockfile 不计），超限拆分。
- 一事一 PR；早期用 Draft PR；特殊前缀 `security:`（触及 08 机制）、`deps:`（依赖变更附五问）。
- nono 升级单独成 PR（§5）。

### 3.2 模板（已入库 `.github/pull_request_template.md`）
做了什么/为什么 → 设计取舍（重大决策附 issue 链接）→ 验证证据（命令+平台+结果+未覆盖项）→ 新依赖五问 → AGENTS §6 自检清单。

### 3.3 合并门禁
- CI：GitHub Actions 当前因 #145 暂停；恢复前，PR 必须附 `fmt / clippy / test / strict doc / architecture / deny` 的新鲜本地证据 + **触及安全机制必须附攻击演示子集通过证据**。
  恢复 CI 需独立 PR、管理员确认资源并重新通过矩阵；恢复后以远程绿勾为准。
- Review：≥1 人类 approve；横切线相关 PR 需对应负责人看过（nono 跟版→A / 安全→B / 架构→C）。
- Review 重点顺序：**依赖方向与分层 → 安全边界 → 错误与边界 → 测试充分性 → 命名风格**。
- review 意见未解决不合并；AI 有分歧给论据，仍分歧交用户裁决。

## 4. 版本化与变更日志

- semver：`0.x` 阶段 minor 可含破坏性变更，CHANGELOG 标 BREAKING。
- CHANGELOG 按 [Keep a Changelog]，每优先级（P0/P1/P2）收口时整理。
- 契约（M3 起 proto）变更与 SDK 版本联动见 03 §5。

## 5. nono 上游跟版流程（对应 04 §6）

1. 新开 `deps/<issue>-nono-bump` 分支；改 qn-config 版本常量与安装清单。
2. PR 正文：上游 release notes 摘要、行为差异清单、影响面（qn-nono/broker/profiles）、
   回归验证证据（含攻击演示子集）。
3. 不兼容变化在 adapter 内适配，禁止把适配逻辑散进产品层。
4. 每个里程碑开工前主动检查上游是否有新 release（周同步议题）。

## 6. 敏感与杂项

- 禁止提交：任何凭证/token、内部材料（`材料/` 已在 .gitignore）、构建产物、`~/.qn` 状态。
- 大文件（测试用安装包）走 fixture 下载脚本，不进 git。
