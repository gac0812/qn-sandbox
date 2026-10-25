# qn-broker

宿主侧凭证代答适配器；真实凭证存储与 nono proxy 集成仍待 #59。

## 托管路由配置（#84）

docs/spec/08 的封闭配置边界在本 crate 先实现为内部 routing.json：
只接受 version=1、core SessionId、GrantHandle 和非零回环 proxy_port，
并逐项对照调用方已授权的 ManagedConfigBinding；不接受 env/header/URL/任意路径。
serde 拒绝未知与重复字段，输入限 4 KiB。校验和序列化完成后才创建临时文件，
复用 tempfile 原子替换固定文件名；普通失败路径清理临时文件并保留旧目标。
所有错误链与 Debug 不回显输入。没有新增代理、凭证存储或 core port。

调用方须提供**沙箱不可访问的私有宿主目录**、已认证的会话、实时授权的 grant 与批准的
回环监听端口；本 API 不验证目录 ACL、端口实际归属或 grant 有效性。
目录及路径组件不可被非可信进程改写；不支持在 Agent 可写 HOME 中直接发布此文件。
这是 qn 内部路由元数据，**不是 Claude Code/Codex/MCP 的应用配置格式**；
后续 renderer/gateway/provider 接入及实时撤销仍按 #59/#82 验收。
没有目录 fsync 承诺，不保证断电后的目录项持久性；异步调用方须用 spawn_blocking。

验证：cargo test --locked -p qn-broker；仅合成句柄与临时目录，不需要真实凭证。
