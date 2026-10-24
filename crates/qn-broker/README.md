# qn-broker
`provider::CommandProvider`（#101）通过现有 `qn-infra::process::spawn_managed` 执行可信宿主命令，
支持 nono 0.80.0 `credential_capture.command` 的 text 输出模式。nono 继续拥有代理、路由和凭证注入。

- 绝对命令路径/宿主 HOME、参数全部来自可信宿主配置，不能来自沙箱请求。参数只含非秘密引用。
- 清空继承环境；显式 HOME/cwd，其他只允许 PATH/LANG/LC_ALL/SystemRoot。真实凭证不通过环境传入。
- stdin 关闭，1–300 秒超时，stdout/stderr 各 16 KiB 上限；无缓存、无重试、无后台游离任务。
- 取消/超时/失败/截断/无效 UTF-8/空值/内部控制字符全部拒绝；首尾空白按上游 text capture 语义移除。
- Debug、错误和进程日志不包含输出。返回 `ProviderSecret` 使用 zeroize；infra 原有原始/lossy 输出副本
  仍为普通内存，不声称完整内存清零。只允许通过加密宿主通道送回 nono，绝不交给 Agent。
- 调用者在 provider IO 前验证 grant，交付前再次准入；此模块不负责路由授权、原生会话关联或持久审计。
- bootstrap 注入现有 ProcessPolicy；#93 全树清理限制仍然存在，命令结束不代表沙箱会话已停止。

`cargo test -p qn-broker --test host_provider` 启动真实子进程，使用合成凭证验证环境隔离、输出拒绝、
超时、事件触发取消和 tracing 脱敏；不读取开发者凭证。Windows 原生结果需单独验证。
