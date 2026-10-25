# qn-gateway

宿主 MCP HTTP 路由规划（#123）；HTTP 转发和真实认证仍属于 nono / qn-broker。
本 crate 只依赖 core 与 URL 解析器，不导入其他 adapter，不创建 listener 或 grant。

`HttpMcpRouting` 实现 core `McpRouting`；`plan(name, endpoint)` 返回可信宿主使用的
name/origin/path 和 POST/GET/DELETE 方法集。规划不是注册、授权或 Agent 配置。

端点必须是规范 HTTPS 或数字回环 HTTP，长度上限 4096 字节；拒绝 userinfo、query、
fragment、控制/空白、非规范别名、百分号编码路径、反斜线及路径开头双斜线。
HTTP 不接受 localhost 等名称，避免回环例外依赖名称解析。复用已锁 url 2.5.8。
路由名以小写字母开头，1–64 字符，只含小写字母、数字、下划线和连字符。
错误和 Debug 不回显端点；路径仍可能被调用方放入秘密，因此只能作为宿主配置。
实际出站策略和每次请求的存活 grant 仍须由 nono / broker 执行。

验证：`cargo test --locked -p qn-core -p qn-gateway`。覆盖规范路由、凭证与歧义输入
拒绝、长度边界及错误脱敏。官方 SDK 的原生 JSON/SSE 协议证据见
[#122](https://github.com/gac0812/qn-sandbox/issues/122)；该集成不在本切片中。

未完成：bootstrap / CLI 接线、生产注册和存储、Agent 托管配置、stdio、OAuth、
Windows/WSL 和真实 Agent 验收。#93 的会话级 reaper 已交付（01 §4.7）；生产 runner 仍受
其它接线项限制。
