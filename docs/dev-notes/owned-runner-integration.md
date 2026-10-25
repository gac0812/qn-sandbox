# 受管离线 runner 集成验证（#92）

状态：实验完成，生产接入未完成 · 2026-10-25

## 结论

准备 profile、严格校验、前台启动、原生身份查询、受管等待/取消可以组成实际运行路径。
macOS 真机已验证项目写入成功、合成宿主密钥和父环境不可见、未授权网络连接被拒绝。
但 **ManagedOutput 的 Exited(0) 不能证明后代已清理**：关闭继承 stdio 的同组后代可在父进程
成功退出后继续执行。不能据此生成 core Stopped；必须先解决终止所有权和证明问题。

## 精确基线与隔离方式

实验在独立 detached worktree 进行；未修改 main、队友分支或既有 PR 的公开历史。

| 输入 | Commit |
| --- | --- |
| canonical main（#76） | b045a98bae0cc290691d763e6fa9bf43d2b166b4 |
| #78 profile preparation | 3940d12680210b35689852ccafbfb728206f4535 |
| #91 native session reader | b08ef993547714b9becd641b7a569b2cf46e0638 |
| #83 managed lifecycle（初始） | 1d62ad20448debf8829449062df450d4246febca |
| #88 portable rustdoc | 4b3481a1d779ea063819e9e3f8ff8d1e46a041b6 |

仅有 lib.rs 的 profile/session 模块声明与 README 追加章节冲突，均保留两侧内容。
本地合成基线为 34640103f592436b8c7e8d4b7af1c0cf771dd25b；实验实现提交为
be1e4194d61271a8dc5277c973ae25d87d0231c5（仅本地，不作为远程可获取的发布承诺）。
原始实现保存在当前任务的附属工作树，生成的 patch 另有本地副本；这份文档不启用该实验 API。

## 已实现并执行的组合

实验性的 OfflineRun::start 接收 RunSpec、RuntimePaths、进程策略、期限和取消 token：

1. 取消/期限检查；spawn_blocking 执行 PreparedProfile::prepare。
2. 核对锁版，执行 5 秒严格 profile validation，解析 valid/errors。
3. 创建同一私有根的 SessionReader，经 spawn_managed 启动前台 nono。
4. 立即保存审计布局和 waiter 的 JoinHandle，使期限从 spawn 开始生效。
5. observe 精确关联 native ID；finish 保留 waiter 至完成，取消该等待 future 可重试。
6. cancel 请求终止并等待真实结果；错误保留为错误，未映射到 core Stopped。

配置和参数复用既有实现，未复制上游沙箱代码。没有新增包；tokio 从既有 dev 依赖扩展为
运行依赖。原始输出与 panic 载荷不进入 Debug/错误摘要。退出后审计目录保留，生产注册表的
保留/删除策略仍未实现。Drop 仅尽力取消，不声称完成异步清理。

## 原生证据

宿主：macOS 26.4（25E246）arm64，Rust 1.85.0，nono 0.80.0。

- 归档 SHA-256：ae824167b3d90531b830429ba92b7781379b1c2d79f9b11fdd88b0fd267e4e32。
- 二进制 SHA-256：e95479a2ef64b19e3245a90a60024bb248e96e8ec234ffeb25e062ddd45b1417。
- 与锁版 manifest 和归档解出的字节一致；实验前重新计算哈希。
- 测试根在 /Users/Shared 新建，权限 0700；数据、环境 canary、目录和连接全部合成。

| 验证项 | 直接证据 |
| --- | --- |
| 项目写入 | Ruby 在明确授权的 project 创建 edited.txt，内容等于 restricted-write-ok |
| 文件隔离 | 读取 project 外合成 host/.ssh/canary 得到 EACCES/EPERM |
| 环境隔离 | 父环境带合成 QN_RUNNER_CANARY，沙箱中该变量不存在 |
| 网络拒绝 | 宿主已绑定的 loopback listener 无放行规则，连接得到 EACCES/EPERM |
| 身份与取消 | sleep 会话精确 find/inspect 为 Running；取消返回 ProcessEnd::Cancelled |
| 时间限制 | 12 秒 sleep 在 500 毫秒期限触发 ProcessEnd::TimedOut |
| 启动前取消 | 返回 RunCancelled，control 目录没有创建会话布局 |
| 后代反例 | wait 返回 Exited(0) 后，后台后代仍响应新的宿主探针 |

前两项组合测试：2 passed / 2.97s；单独后代探针：1 passed / 1.66s。
这些是特定路径的证据，不覆盖恶意脱组、所有取消竞态、真实 Agent 或完整攻击套件。

## 可独立复现的后代探针

反例已转入 #83 的附加提交 535ea67b58146b25985e11beaf3097d79e48b93d，不依赖实验 runner。
使用该 PR 分支，在已验证 nono 的 macOS 主机执行（测试父目录须预先创建为 0700）：

~~~sh
QN_TEST_NONO=/absolute/verified/nono \
QN_TEST_PROFILE_PARENT=/absolute/private/probe-parent \
cargo test --locked -p qn-nono --test managed_contract -- --ignored --nocapture
~~~

结果：2 passed / 6.85s。一个测试验证合作式父/子进程的取消与超时；另一个测试**成功复现缺口**。
后代关闭 stdio、连接测试端口、通过 pipe 通知父进程就绪，父进程正常退出。宿主在 wait 返回后
才向原有连接发送 P；后代回答 A 后关闭连接并退出，有 12 秒自退出兜底，不按数字 PID 补杀。
因此结果不能解释为“后代清理测试全部通过”。

本机锁定 process-wrap 8.2.2 的 ProcessGroupChild::wait 先回收直接子进程，再用 waitpid
尝试回收可等待的同组子进程；ECHILD 不意味着进程组不存在。它无法通过此途径回收已被系统
接管的后代。原生 Exited、管道 EOF 和成功返回值均不足以证明整个执行树终止。

## 质量门禁与后续

合成工作树的 cargo fmt --all -- --check、cargo clippy --locked --workspace --all-targets -- -D warnings、
cargo test --locked --workspace、RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps 全部通过。
#83 的附加提交重新通过 fmt、clippy、workspace test 和两项原生测试。无远程 CI 绿灯推断。

生产接入前仍需：完整后代所有权/清理证明（#93）、注册表与重复 ID 处理、核心 port、宿主 capture/provider、
审计呈现与事件完整性、CLI/控制面、在线官方 Agent 和 Windows/WSL 验收。P1/P2 全范围仍在 #82。
不通过增加权限、猜测 PID 或把清理未知包装成成功来缩小这些要求。
