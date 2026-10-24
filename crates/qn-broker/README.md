# qn-broker

`capture`（#96）为宿主 provider 提供有界、带认证的本机传输；不实现凭证代理或存储。

- listener 只绑定 IPv4 loopback，每 listener OS 随机 256-bit capability；snow 的 Noise NNpsk0（25519/ChaChaPoly/SHA256）双向认证、加密及完整性保护；grant 句柄不是认证。
- 私有 `CaptureBinding` 只能交给宿主 provider；禁止进入沙箱 fs/env/argv、公开配置或日志。部署方须提供私有根和文件权限。
- 每个帧 1–16 KiB；accepted peer 的认证/读取/回复共用 deadline；无缓存、自动重试或后台任务。
- consumer 必须认证绑定的会话，并在 provider IO 前和响应准入时验证 live grant；已准入响应属于在途请求。
- `CaptureBytes`/binding 的 Debug 脱敏，内存 drop 时清零；调用方不得日志化原始载荷或 serde 错误。
- `cargo test -p qn-broker` 覆盖认证、加密帧界限、超时及关闭；relay/原生重放由 #96 后续测试交付。
- 尚未接入生产 runner/CLI、持久 provider、审计或 WSL 桥；本机同用户未受限恶意进程不在此传输隔离保证内。
- 原生撤销验证复用 #80 的授权表在隔离集成工作区执行；#93 的完整进程树清理仍未解决。
