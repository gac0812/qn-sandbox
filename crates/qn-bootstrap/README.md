# qn-bootstrap

组合根：`process_policy()` 为 infra 的受管进程选择 Unix 进程组或 Windows Job Object（#65）。
平台分支只在此装配；复用 process-wrap 8.2.2，不自研 OS 信号/Job API，也不提供沙箱隔离。
用法见函数 doctest；`cargo test -p qn-bootstrap --test managed_process` 验证 stdin、捕获和组清理。
Windows 测试显式保留非敏感 `SystemRoot` 以支持系统 DLL/网络初始化（#69）；生产 `env_clear` 语义不变。
