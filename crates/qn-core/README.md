# qn-core

领域模型与规则，不依赖 adapter/surface。现有 `SessionId/SessionState` 保留唯一状态迁移规则。
`ProfileRef/GrantHandle/RunSpec`（#72）提供受管启动边界；只有语法校验，不代表文件授权或 grant 有效。
`RunSpec` 不开放任意环境、nono flags 或凭证明文字段；路径、argv、句柄默认 Debug 不回显。
生产 adapter 仍须校验 profile 来源、物理目录、grant 会话/作用域/期限。接口按 #14/#47 随消费者落地。

EngineSetup（#114/#117）由实际 CLI install/doctor 消费者推动；本 PR 只发布契约，
实现仍在 #114 本地集成。Install 允许显式安装/修复，Inspect 禁止下载/修复，
完整性通过后才执行版本探测；结果不声明沙箱策略、凭证或完整执行树清理就绪。
调用方必须等待 future 完成；丢弃 future 不保证阻塞 IO 取消/回滚。adapter 必须对
错误 Display/Debug/来源链脱敏，终端呈现方必须转义路径。无新增依赖或平台类型。
