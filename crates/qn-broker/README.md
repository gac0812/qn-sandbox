# qn-broker

`identity::CaptureIdentity`（#103）验证 nono 0.80.0 的反向代理 capture 元数据。
启动编排必须先固定完整 native ID 和 route，再接受已认证宿主通道的请求；不从首个请求学习身份。

- 严格 schema：所有字段必需；未知/重复字段拒绝；config/interaction 只接受空对象。
- native、credential、route、request_host、cache_scope 必须与固定绑定完全匹配；16 KiB 输入上限。
- 输出只有精确 method/path；canonical origin、qn SessionId、grant 和 provider 来自可信宿主状态。
- 此模块不是认证或授权：调用者先认证通道，再验证身份、live grant 和 scope；provider IO 后再次准入。
- 不放宽路径/方法、不接受通配符、不自选上游；Debug/错误链不回显请求内容。
- schema 依据锁版 proxy_runtime.rs；request_host/cache_scope 的 route 值已经 #99 原生重放验证。

`cargo test -p qn-broker` 覆盖绑定、完整 schema、重复键、控制字符、超长输入与错误脱敏。
生产 serving、启动身份关联、持久审计和 Windows 原生验证仍需集成；#93 全树清理限制不变。
