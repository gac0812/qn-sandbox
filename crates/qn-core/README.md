# qn-core

领域模型与规则，不依赖 adapter/surface。现有 `SessionId/SessionState` 保留唯一状态迁移规则。
`ProfileRef/GrantHandle/RunSpec`（#72）提供受管启动边界；只有语法校验，不代表文件授权或 grant 有效。
`RunSpec` 不开放任意环境、nono flags 或凭证明文字段；路径、argv、句柄默认 Debug 不回显。
生产 adapter 仍须校验 profile 来源、物理目录、grant 会话/作用域/期限。接口按 #14/#47 随消费者落地。

`McpRouting`（#122 / #123）规划精确 HTTP 路由，供可信宿主组合使用。
`McpHttpRoute` 不含凭证或 grant，不代表注册/授权；adapter 负责端点语法验证。
契约源自 #122 已验证的本地 CLI/native 消费者；本切片不包含这些消费者。
