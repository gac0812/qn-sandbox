# qn-bootstrap

组合根：`process_policy()` 为 infra 的受管进程选择 Unix 进程组或 Windows Job Object（#65）。
平台分支只在此装配；复用 process-wrap 8.2.2，不自研 OS 信号/Job API，也不提供沙箱隔离。
用法见函数 doctest；`cargo test -p qn-bootstrap --test managed_process` 验证 stdin、捕获和组清理。
Windows 测试显式保留非敏感 `SystemRoot` 以支持系统 DLL/网络初始化（#69）；生产 `env_clear` 语义不变。

engine_setup(&QnPaths) 装配 qn-nono 的 core EngineSetup 实现；缓存根由 qn-config
统一提供，目标平台/版本由 adapter 追加。CLI 不直接引用 adapter。

native_audit_query 通过 core NativeAuditQuery 暴露 retained qn/native 关联查询；
注入既有 process_policy 与受管缓存/审计目录，不改变 broker 源时钟与语义。

`run_session` / `status_session` / `stop_session` 组合托管引擎、已解析 profile、
`LaunchBinding` 与受管前台进程。run 在私有 `QN_HOME/runtime/<session>` 写入官方资产、
发布审计关联并继承终端；status 读取 owner 记录并可选查询原生 `ps --all`；stop 只按记录中的
预定 native ID 调用锁版 `nono stop`，对已退出记录也执行 reaper（后代可能晚于父进程退出）。

会话清理由 `cleanup` 模块单独证明（#93）：以会话私有 HOME/XDG/TMPDIR 路径为标记，
持续记录受管父进程的后代；收尾时 SIGSTOP 候选、复验 `(pid, start_time)` 身份、
SIGKILL，并要求连续多轮无标记/无候选才写 `cleanup=confirmed`。父进程退出、`ps` 消失或
`stop` 返回 0 单独都不构成证明。作用域残余：在首次观察前清空继承环境并脱离的后代
不可见（macOS 无 subreaper/cgroup），需 VM 或上游内核边界；该场景不在本证明内。
