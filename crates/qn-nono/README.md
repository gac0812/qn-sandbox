# qn-nono — engine adapter for the nono sandbox kernel

全仓唯一允许触碰 [nono](https://github.com/nolabs-ai/nono) 的 crate（01 §3）。
以**子进程方式驱动锁定版本的 `nono` CLI**（spike ③，决策 issue #28），只解析
`--json` / `--diagnostics-json` 输出；待 #14 冻结 ports v1 后实现 `SandboxEngine`。

## 模块

| 模块 | 职责 | 状态 |
| --- | --- | --- |
| `installer` | 锁版 manifest（v0.80.0 + 各平台 SHA-256）、下载、校验、解包安装到 `<install_root>/<version>/nono` | M0 骨架（本期） |
| `args` | version / profile validate / run 的受限参数构造；固定诊断开关和命令分隔符 | A-0.5（#43） |
| `args::session` | 本地 ps/inspect/stop/audit list/show 参数；完整 native ID 与固定选项边界 | A-0.5（#52） |
| `version` | 显式绝对路径调用 `nono --version`，锁版比对、5 秒超时、单流 1 KiB 输出上限 | A-0.5 首片（#41） |
| `runner` | 经 `qn-infra::process`（#19）调用 nono CLI + 契约测试 | M1（未开始） |
| `profile_gen` | core Profile → nono profile JSON | M1（未开始） |
| `audit_tail` / `why` | 审计流采集、拒绝原因透传 | M1（未开始） |

## 安全红线

- 只允许 `--credential`（代理注入）；**禁止生成 `--env-credential` / `--env-credential-map`**
  ——它们把凭证注入沙箱环境变量，违反铁律（spike ③ F8，#28）。
- 不自研任何隔离机制（R1）；不链接 `nono` / `nono-proxy` 库（R4，#28）。

## 非目标

- 策略决策在 nono CLI 与 profiles，本 crate 只做参数组装与结构化解析。
- 凭证存储与生命周期在 `qn-broker`；本 crate 不触碰真实凭证。

## 已知限制

- `NonoArgs` 只生成参数、不启动进程；`RunArgs` 复用 core `SessionId`，profile/workdir
  必须是绝对 UTF-8 路径，命令保持数组，空程序/NUL 被拒绝。`Debug` 不输出路径或参数值。
- 不提供追加 nono flags 的 API；`--env-credential*`、`--bypass-protection`、`--no-audit`
  等不能成为 nono 选项。分隔符后的同名参数属于 Agent，按原样保留，不代表凭证可放进 argv。
- 参数构造不校验 profile 内容或来源；runner 在执行前仍须验证 profile 的默认拒绝、环境白名单、
  凭证约束与来源，并完成 #27 环境清理。此片未实现 runner 或冻结 #14 ports。
- `verify_version(&Utf8Path)` 是异步只读探测，全部经 `qn-infra::process`；
  不搜索 PATH，不拉起沙箱；通过 `env_clear()` 启动空环境子进程，避免继承宿主凭证（#56）。
  相对路径、非零退出、截断/异常输出、版本不符均拒绝。
  缺失二进制和超时保留原始进程错误；版本响应不会进入本 crate 的错误信息。
- 安装调用方可在 `install` 返回后显式调用 `verify_version`；尚未接入 CLI/doctor，
  `install` 本身保持同步下载安装语义。版本字符串检查不能证明文件完整性。

- 安装幂等：目标路径已存在 `nono` 时直接复用，不重新校验（完整性复核由
  `qn doctor` 在 runner 落地后承担；当前版本探测不替代缓存完整性检查）。
- 上游 `SHA256SUMS.txt` 是哈希清单而非签名；签名校验方案在 08 落地时评估。

## 版本契约测试

默认单测不依赖网络。macOS / Linux 手动验证已校验 SHA-256 的锁版二进制：

```sh
QN_TEST_NONO=/absolute/path/to/nono cargo test --locked -p qn-nono --test version_contract -- --ignored
```

此测试只验证版本调用契约，不等同于沙箱隔离、凭证或 Agent E2E 验收。
`cargo test --locked -p qn-nono --test version_environment` 使用合成 canary 和自重执行的
Rust 测试程序验证实际探测路径不继承环境；不修改宿主环境，也不代表 #27 的 sandbox run 已接入。

## macOS 协议实测

Issue #46 的隔离探测命令、结果和限制见 [spike ②](../../docs/dev-notes/spike-02-nono-macos.md)。
仅用于合成数据研究；examples/probe 不可作为生产 runner，生产前置仍包括 #14 / #27。

## 会话命令契约（#52）

`NonoArgs` 的 `sessions/inspect/stop/audit_list/audit_show` 只构造 argv；查询固定 JSON，
`sessions` 包含 exited 记录且不支持 remote。`args::NonoSessionId` 仅接收当前锁版生成的
16 位小写 hex ID；它不是 core `SessionId`，runner 须从受管记录关联并验证归属，不接受名称、
前缀或旧式日期审计 ID。停止宽限期只接受 1–300 整秒，不静默舍入；退出码不证明后代已清理。

`session_contract` 是默认忽略的 macOS 真机测试，先编译并从 JSON 取得测试可执行文件：

```sh
cargo test --locked -p qn-nono --test session_contract --no-run --message-format=json
```

在 `/Users/Shared` 下新建 0700 空目录作为 `QN_TEST_ROOT`，创建 `home/config/state/cache/tmp`，
以清空后重建的环境启动测试二进制 `--ignored`：`HOME/XDG_CONFIG_HOME/XDG_STATE_HOME/
XDG_CACHE_HOME/TMPDIR` 分别指向这些子目录，`PATH=/usr/bin:/bin`、`LANG=en_US.UTF-8`，
`QN_TEST_NONO` 指向已核对归档及二进制 SHA-256 的锁版 nono。测试会创建两次 sleep 会话；
结束或失败后检查 `cleanup.json` 中的 PID 并清理仍存活的测试进程，不能只看 ps 的 exited。
本轮复现 #47 的 force 残留，并观察到强杀后审计元数据缺失；审计详情契约仅在正常停止后断言。
此测试只验证 CLI 契约，不代表 #14/#27/#47 或生产 Agent、安全撤销 E2E 已完成。

## 内部协议解析（#48）

wire 模块按 #46 的 nono 0.80.0 实测 fixture 解析 profile、ps/inspect、audit list/show 和混合 stderr 诊断。
字段未知可兼容；必需字段缺失/错型、截断、超限、歧义拒绝。错误和 Debug 不回显载荷。
解析不判断退出成功、不推断 Denial、不将 wire 状态映射到 core；未来 runner 仍须处理进程结果。
诊断只保证结构，不保证来源：child stderr 可伪造 JSON，#47 确认审计可信来源前不能把它当授权依据。
模块暂时 crate-private、无生产调用；限定 dead_code 豁免待 #14 首个 consumer 接入后移除。
