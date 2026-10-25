# 05 — 测试与质量门禁

状态：Draft · 2026-10-25 · **实施状态**：fmt/clippy/三平台 test/deny/architecture
已接入；严格 doc 三平台门禁由 #134 接入。每日覆盖率、每周 deny 与每日/PR E2E
尚未接入；不得以通用单测/文档构建通过替代真实平台安全验收。nono 前置 spike 见 TODO-004。

## 1. 测试金字塔与各层职责

```
        ┌──────────────────┐
        │ E2E（每平台）      │  tests/e2e/ ── 真实 nono + 真实 Agent CLI，跑用户剧本与攻击演示
        ├──────────────────┤
        │ 集成（crate）      │  crates/*/tests/ ── 适配器对真 nono 二进制（CI 缓存）/ stub wsl.exe
        ├──────────────────┤
        │ 单元（模块内）      │  #[cfg(test)] ── 秒级，无外部资源
        └──────────────────┘
```

| 层 | 覆盖对象 | 环境 | CI 中 |
| --- | --- | --- | --- |
| 单元 | core 状态机、profile 生成、grant 表、托管配置校验、错误映射 | 纯内存 | 全矩阵 |
| 集成 | `qn-nono` 对真 nono 的契约（run/status/why/审计）；`qn-wsl` 对 stub `wsl.exe`；`qn-gateway` mediation；daemon API | 临时目录 + 缓存的 nono 二进制 / stub | 全矩阵 |
| E2E | 用户剧本 + **攻击演示**（安全回归） | 真实平台 | 每日 + PR 可选 |

### 安全回归的特殊地位
**攻击演示剧本是安全条款的回归测试**（08 全部机制），任何触及隔离/凭证/策略的 PR 必须跑：
读 `~/.ssh`、`~/.aws` → 被拒 + Denial 留痕；越权域名出站 → 被拒；环境变量探测 → 无凭证；
revoke 后出站 → 立即断供；托管配置写明文 → 拒绝写入。任何一条放行 = P0 缺陷（§6）。

## 2. E2E 剧本（P0 验收即它的通过）

```
剧本 1（M1 macOS / M2 Windows 参数化）:
  - qn doctor                          → nono 就绪（版本=锁定值；机制可用）
  - qn run --agent claude-code -- <真项目目录>   → 会话进入 Running，受限运行
  - 沙箱内读 ~/.ssh、越权出站            → 全部 Denial 且 qn audit 可见
  - qn mcp add <server>                → agent 配置只有 Handle
  - qn revoke <session>                → 出站立即断供
  - qn stop / 清理                      → 无残留进程
剧本 2（M3）: 同剧本 1 改用 Python SDK 走控制面 API。
```
- E2E 失败自动收集：平台信息、qn 日志尾部、审计事件尾部（CI artifact）。

## 3. 质量门禁（CI stages）

| 门 | 命令 | 时机 |
| --- | --- | --- |
| architecture | `python3 scripts/check_architecture.py` + 检查器 unittest | 每 PR；声明依赖分层（#132） |
| fmt | `cargo fmt --all -- --check` | 每 PR |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 每 PR |
| test | `cargo test --workspace`（平台矩阵） | 每 PR |
| deny | `cargo deny check licenses bans advisories` | 每 PR + 每周 |
| doc | `RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps` | 每 PR / main push；Ubuntu/macOS/Windows（#134） |
| 攻击演示 | 剧本 1 安全子集（读密钥/越权出站/revoke 断供） | 每 PR（触及 08 机制的必跑） |
| coverage | `cargo llvm-cov`：core ≥ 90%，适配器逻辑层 ≥ 70% | 每日，低于线开 issue |

`doc` 作业沿用锁定 Rust 1.85 和只读权限，在各宿主上原生构建 cfg 对应的文档；
任一 rustdoc 警告（含失效链接）即失败。文档不上传、不发布。三个作业独立保留失败
结果，不设置 continue-on-error；文档构建不运行真实 Agent 或 WSL2 E2E。

## 4. 平台测试矩阵

| runner | 跑什么 |
| --- | --- |
| ubuntu-latest | 编译 + 单元（快速反馈） |
| macos-latest | 全套 + 剧本 1（nono 原生路径）——M1 起启用 |
| windows-2022 | 全套 + 剧本 1（WSL2 路径：workflow 先装 WSL2 并校验内核 ≥6.7，Landlock 网络可用性探测）——M2 起启用 |

本地无对应平台时：AI 必须在汇报中如实标注"未在 X 平台验证"（AGENTS §7）。

## 5. 验收定义（DoD）

1. AGENTS §6 自检清单全勾。
2. 新行为有测试；触及安全机制的必须有攻击演示子集通过。
3. 验证证据（命令+平台+结果）写入 PR。
4. 相关文档同步（spec/crate README/glossary/profiles）。
5. 无新增告警、无新增依赖红线。

## 6. 缺陷分级

- **P0**：任何攻击演示子集放行（凭证可见/越权出站/撤销不生效/审计缺失）→ 立即修复 + 回归 + 决策 issue 复盘。
- P1：核心命令不可用、跨平台行为不一致。
- P2/P3：体验与文档 → issue 跟踪。
- 修 bug 的 PR 必须包含"先复现（红）再修复（绿）"的测试提交顺序。
