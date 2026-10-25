# qn-core

领域模型与规则，不依赖 adapter/surface。现有 `SessionId/SessionState` 保留唯一状态迁移规则。
`ProfileRef/GrantHandle/RunSpec`（#72）提供受管启动边界；只有语法校验，不代表文件授权或 grant 有效。
`RunSpec` 不开放任意环境、nono flags 或凭证明文字段；路径、argv、句柄默认 Debug 不回显。
生产 adapter 仍须校验 profile 来源、物理目录、grant 会话/作用域/期限。接口按 #14/#47 随消费者落地。

AuditQuery（#110）由首个 CLI 消费者推动：只读、单会话、显式 source/integrity/state，
不暴露 adapter 文件路径或自由文本。事件序号仅在单 journal 内排序，不伪造全局 monotonic。
ObservedClosed 不代表沙箱 Stopped、完整送达或防篡改；native/fs 事件与 follow 仍未实现。
