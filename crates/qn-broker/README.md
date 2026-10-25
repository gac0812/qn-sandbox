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

## Claude MCP configuration (#125)

claude_mcp_template supplies one fixed qn_mcp HTTP route. write_claude_mcp_config
accepts only that exact closed schema, validates before IO and atomically creates
.claude.json without replacing existing targets. Unknown/duplicate fields, altered
URLs, headers, environment references, transports and oversized input are refused.
Existing routing.json replacement semantics remain unchanged.

Only QN_MCP_BASE_URL and QN_MCP_TOKEN variable references are written; no credential,
phantom value or upstream address. The host must provide a fresh private absolute
directory, give the sandbox read-only access to it, and configure the corresponding
native route and live audited broker. Call blocking publication via spawn_blocking.
Config preparation grants no authorization or listener ownership.

Real Claude 2.1.304 JSON/SSE connection and revoke acceptance passed in the #125
local integration with hash-verified nono 0.80.0; commands and evidence are recorded
in that Issue. The broader native fixtures and service composition remain tracked
in #122/#107 and are not bundled into this renderer PR. No model call, multi-route,
OAuth, existing user config merge, Windows/WSL or production lifecycle claim.

## Codex MCP configuration (#136)

write_codex_mcp_config takes a private absolute root and NonZeroU16 port, then
atomically creates config.toml. The fixed qn_mcp server contains only
http://127.0.0.1:<port>/qn_mcp/mcp and bearer_token_env_var = QN_MCP_TOKEN.
No arbitrary URL/header/env, credential or phantom value is accepted. Existing
files/directories are never replaced. Serialization reuses the already-approved
toml dependency; async callers must put this blocking publication in spawn_blocking.

Codex 0.147.0 does not expand URL environment references, so the host must match
the port with LaunchBinding::command_with_proxy_port from #137/#139. An available
port probe grants neither ownership nor authorization; native bind must succeed
before child execution. Config.toml and its parent stay read-only to the sandbox.

Real Codex app-server JSON/SSE list/call/revoke and occupied-port refusal passed
in the local #136 integration, with no model call or real user authentication.
Fresh state is provisioned separately: only tmp/.tmp, the individual installation_id
and a separate SQLite directory were writable; host configuration is not imported.
Native fixture/service publication, production profiles and #93 remain independent.

## Scoped grant lifecycle (#79)

`GrantRegistry`（#79）复用 core ID，实现不含秘密值的宿主授权生命周期；尚未接入 capture/provider/IPC/托管配置。

- Grant 绑定会话、引用、精确 origin/method/path 和单调时钟到期点；无通配符授权。
- Capture adapter 负责路由规范化/来源认证；scope 不解析 URL，句柄不等于认证，agent 不能自报会话。
- Provider IO 前 `authorize`，入队前 `with_authorization` 再检查；闭包持短锁，禁止阻塞/await/回调 registry。
- 撤销完成后不再准入新响应；在途响应不能追溯撤销。capture 必须零缓存，集成后另做原生验证。
- `revoke_session` 撤销已有授权；新的显式 grant 仍需控制面验证会话存活。
- 注册表容量有限，新增 grant 时清除过期项；重启后所有旧句柄失效，无磁盘授权缓存。
- Mutex poisoning 后拒绝全部操作；Debug/错误不包含路由或 provider 载荷。

`cargo test --locked -p qn-broker` 以合成数据验证作用域/到期/撤销/容量/并发；不代表原生 E2E 已完成。

## Authenticated host capture (#97)

`capture`（#96）为宿主 provider 提供有界、带认证的本机传输；不实现凭证代理或存储。

- listener 只绑定 IPv4 loopback，每 listener OS 随机 256-bit capability；snow 的 Noise NNpsk0（25519/ChaChaPoly/SHA256）双向认证、加密及完整性保护；grant 句柄不是认证。
- 私有 `CaptureBinding` 只能交给宿主 provider；禁止进入沙箱 fs/env/argv、公开配置或日志。部署方须提供私有根和文件权限。
- 每个帧 1–16 KiB；accepted peer 的认证/读取/回复共用 deadline；无缓存、自动重试或后台任务。
- consumer 必须认证绑定的会话，并在 provider IO 前和响应准入时验证 live grant；已准入响应属于在途请求。
- `CaptureBytes`/binding 的 Debug 脱敏，内存 drop 时清零；调用方不得日志化原始载荷或 serde 错误。
- `cargo test -p qn-broker` 覆盖认证、加密帧界限、超时及关闭；relay/原生重放由 #96 后续测试交付。
- 尚未接入生产 runner/CLI、持久 provider、审计或 WSL 桥；本机同用户未受限恶意进程不在此传输隔离保证内。
- 原生撤销验证复用 #80 的授权表在隔离集成工作区执行；#93 的完整进程树清理仍未解决。
