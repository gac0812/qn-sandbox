# qn-core

领域模型与规则，不依赖 adapter/surface。现有 `SessionId/SessionState` 保留唯一状态迁移规则。
`ProfileRef/GrantHandle/RunSpec`（#72）提供受管启动边界；只有语法校验，不代表文件授权或 grant 有效。
`RunSpec` 不开放任意环境、nono flags 或凭证明文字段；路径、argv、句柄默认 Debug 不回显。
生产 adapter 仍须校验 profile 来源、物理目录、grant 会话/作用域/期限。接口按 #14/#47 随消费者落地。

AuditQuery（#110）由首个 CLI 消费者推动：只读、单会话、显式 source/integrity/state，
不暴露 adapter 文件路径或自由文本。事件序号仅在单 journal 内排序，不伪造全局 monotonic。
ObservedClosed 不代表沙箱 Stopped、完整送达或防篡改；native/fs 事件与 follow 仍未实现。

EngineSetup（#114/#117）由实际 CLI install/doctor 消费者推动；本 PR 只发布契约，
实现仍在 #114 本地集成。Install 允许显式安装/修复，Inspect 禁止下载/修复，
完整性通过后才执行版本探测；结果不声明沙箱策略、凭证或完整执行树清理就绪。
调用方必须等待 future 完成；丢弃 future 不保证阻塞 IO 取消/回滚。adapter 必须对
错误 Display/Debug/来源链脱敏，终端呈现方必须转义路径。无新增依赖或平台类型。

`McpRouting`（#122 / #123）规划精确 HTTP 路由，供可信宿主组合使用。
`McpHttpRoute` 不含凭证或 grant，不代表注册/授权；adapter 负责端点语法验证。
契约源自 #122 已验证的本地 CLI/native 消费者；本切片不包含这些消费者。
`ProfileRegistry` (#75/#59) carries only closed profile DTOs: a canonical
`ProfileRef`, strict `MAJOR.MINOR.PATCH` version, source, bounded nono asset text,
allowlist hosts and one relative app state directory. Core never parses nono policy
syntax and never accepts a credential value. Install/rollback are local registry
operations; they do not launch an engine or widen authorization.
