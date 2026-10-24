# qn-broker

`GrantRegistry`（#79）复用 core ID，实现不含秘密值的宿主授权生命周期；尚未接入 capture/provider/IPC/托管配置。

- Grant 绑定会话、引用、精确 origin/method/path 和单调时钟到期点；无通配符授权。
- Capture adapter 负责路由规范化/来源认证；scope 不解析 URL，句柄不等于认证，agent 不能自报会话。
- Provider IO 前 `authorize`，入队前 `with_authorization` 再检查；闭包持短锁，禁止阻塞/await/回调 registry。
- 撤销完成后不再准入新响应；在途响应不能追溯撤销。capture 必须零缓存，集成后另做原生验证。
- `revoke_session` 撤销已有授权；新的显式 grant 仍需控制面验证会话存活。
- 注册表容量有限，新增 grant 时清除过期项；重启后所有旧句柄失效，无磁盘授权缓存。
- Mutex poisoning 后拒绝全部操作；Debug/错误不包含路由或 provider 载荷。

`cargo test --locked -p qn-broker` 以合成数据验证作用域/到期/撤销/容量/并发；不代表原生 E2E 已完成。
