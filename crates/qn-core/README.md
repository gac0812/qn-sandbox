# qn-core

领域模型与规则，不依赖 adapter/surface。现有 `SessionId/SessionState` 保留唯一状态迁移规则。
`ProfileRef/GrantHandle/RunSpec`（#72）提供受管启动边界；只有语法校验，不代表文件授权或 grant 有效。
`RunSpec` 不开放任意环境、nono flags 或凭证明文字段；路径、argv、句柄默认 Debug 不回显。
生产 adapter 仍须校验 profile 来源、物理目录、grant 会话/作用域/期限。接口按 #14/#47 随消费者落地。
