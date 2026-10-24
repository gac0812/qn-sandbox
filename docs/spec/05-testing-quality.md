# 05 — 测试与质量门禁

状态：Draft · 2026-10-24 · **实施状态**：本文为目标态——CI 门禁（§3）与覆盖率统计当前未接入，待 workspace 骨架（M0）搭建完毕后补上，跟踪见根目录 [TODO.md](../../TODO.md)（TODO-001 / TODO-002）。

## 1. 测试金字塔与各层职责

```
        ┌─────────────┐
        │  E2E（每平台）│  tests/e2e/  ── 真实 VM 起停，跑用户剧本；慢、数量少、只在对应平台跑
        ├─────────────┤
        │  集成（crate）│  crates/*/tests/ ── trait 实现对着假 guest/假命令跑；分钟级
        ├─────────────┤
        │  单元（模块内）│  #[cfg(test)] ── 秒级，无外部资源；覆盖规则/状态机/解析
        └─────────────┘
```

| 层 | 覆盖对象 | 环境 | CI 中 |
| --- | --- | --- | --- |
| 单元 | core 状态机、config 解析、错误映射、路径转换 | 纯内存 | 每平台矩阵都跑 |
| 集成 | driver 的 trait 实现逻辑（用 mock 外部命令）、daemon API（起内存实例） | 临时目录 + stub 命令 | 每平台矩阵都跑 |
| E2E | 用户剧本：创建→exec→传文件→停止→删除 | **真实平台**（macOS runner / Windows runner + WSL2） | 每日 + 手动触发（PR 可选跑） |

## 2. 各层写法要求

### 单元测试
- 与代码同文件；一个行为一个测试；表驱动（`fn_cases`/手写循环）优先于复制粘贴。
- 不依赖执行顺序、不依赖真实文件系统（用 `tempfile`）、不 sleep（tokio `start_paused = true` 或事件等待）。
- 核心状态机（`SandboxState::transition`）必须有**穷举迁移表**测试。

### 集成测试
- driver 测试针对 trait 写，不针对具体平台写死：mock `wsl.exe`/`qemu` 的可执行 stub 放 `tests/fixtures/bin/`，通过环境变量注入。
- daemon 测试用真实 daemon + 假 driver（`mockall` 或手写 fake），断言 HTTP 行为与错误码。
- 断言错误也要断言**错误信息内容**（可行动性是规范的一部分）。

### E2E（每个里程碑的验收就是它的剧本）
```yaml
剧本 1（M1/M2/M3 通用，参数化平台）:
  - qn image import <fixtures 镜像>
  - qn create --image ubuntu --name t1
  - qn exec t1 -- echo hello        → stdout == "hello"
  - qn cp ./main.py t1:/tmp/ && qn exec t1 -- python /tmp/main.py → 退出码 0
  - qn stop t1 && qn rm t1          → 清理干净（list 不含 t1）
剧本 2（M4）: 同剧本 1 改用 Python SDK 走 daemon API
```
- E2E 失败必须输出：平台信息、qn 日志尾部、状态文件内容（自动收集进 CI artifact）。

## 3. 质量门禁（CI stages）

| 门 | 命令 | 时机 |
| --- | --- | --- |
| fmt | `cargo fmt --all -- --check` | 每 PR |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 每 PR |
| test | `cargo test --workspace`（三平台矩阵） | 每 PR |
| deny | `cargo deny check licenses bans advisories` | 每 PR + 每周定时 |
| doc | `cargo doc --no-deps`（broken links = fail） | 每 PR |
| gen-drift | SDK 生成物与契约一致性 diff | 契约/SDK 变更 PR |
| coverage | `cargo llvm-cov` —— 目标：core ≥ 90%，driver 逻辑层 ≥ 70%（FFI/外部命令壳不计入） | 每日，低于线开 issue |

## 4. 平台测试矩阵

| runner | 跑什么 |
| --- | --- |
| ubuntu-latest | 编译 + 单元（跨平台公共逻辑的快速反馈） |
| macos-14 (arm64) | 全套 + E2E 剧本 1（VZ 路径） |
| macos-13 (x64, Intel) | 全套 + E2E 剧本 1（Intel 路径）——M2 起启用 |
| windows-2022 | 全套 + E2E 剧本 1（WSL2 路径，workflow 里先 `wsl --install`）——M3 起启用 |

本地无对应平台时：AI 必须在汇报中如实标注"未在 X 平台验证"（AGENTS.md §7），不得只凭 ubuntu 编译通过宣称跨平台完成。

## 5. 验收定义（DoD，每个任务完成必须全部满足）

1. AGENTS.md §6 自检清单全勾。
2. 新行为有测试覆盖（单元/集成至少其一；里程碑级功能必有 E2E 剧本行）。
3. 验证证据（命令+平台+结果）写入 PR/汇报。
4. 相关文档同步（spec/ADR/glossary/crate README）。
5. 无新增告警、无新增依赖红线。

## 6. 缺陷分级与回归

- P0：沙箱逃逸迹象、数据损坏、宿主机系统文件被改 → 立即修复 + 补回归测试 + ADR 复盘。
- P1：核心命令不可用、跨平台行为不一致 → 修复版本内完成。
- P2/P3：体验与文档问题 → issue 跟踪。
- 每个修复 bug 的 PR 必须包含"先复现（红）再修复（绿）"的测试提交顺序。
