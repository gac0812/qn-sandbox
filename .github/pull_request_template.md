<!-- 标题遵循 Conventional Commits，如 feat(driver-wsl): support sparse image import -->

## 做了什么 / 为什么

<两三句；关联 issue>

## 设计取舍

<关键决定与备选方案；架构级变更附 ADR 链接>

## 验证证据（R7，不得空）

- 命令与结果：
- 平台：<!-- macOS 15 arm64 / Windows 11 23H2 + WSL2 / ubuntu CI -->
- 未覆盖项：<!-- 如实标注：哪些平台/场景未验证及原因 -->

## 新依赖（无则删除本节，R2）

- 解决什么 / 为什么不自研：
- 五问：license / 近 6 个月活跃度 / 平台覆盖 / 传递依赖规模 / MSRV：

## 自检清单（AGENTS §6）

- [ ] 改动范围与任务一致，无顺手修改（R11）
- [ ] 无可复用的既有实现被绕过（R1）；新依赖五问已答（R2）
- [ ] 依赖方向合法（R3）；`#[cfg(target_os)]` 只在白名单位置（R6）
- [ ] 非测试代码无 unwrap/expect（R5）；错误类型化且上下文完整
- [ ] 公开 API 有 `///` 文档；注释只解释"为什么"
- [ ] TODO 带归属（R8）；生成物未手改（R9）
- [ ] 相关文档已同步（spec / ADR / glossary / crate README）
