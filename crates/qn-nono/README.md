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
| `audit::AuditReader` | 隔离 HOME/XDG、锁版探测、受管执行的 typed audit list/show | #68（原生记录，不等于完整 AuditEvent） |
| `version` | 显式绝对路径调用 `nono --version`，锁版比对、5 秒超时、单流 1 KiB 输出上限 | A-0.5 首片（#41） |
| `runner` | 经 `qn-infra::process`（#19）调用 nono CLI + 契约测试 | M1（未开始） |
| `profile::PreparedProfile` | 私有运行目录、物理路径重叠检查、离线 profile 渲染与命令构造 | #75 / #77（在线绑定待 broker） |
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
- `RunArgs::from_spec`（#72）消费 core `RunSpec`，profile 路径由 adapter 解析后提供；grant 句柄不进入 argv，
  实际授权及 provider 绑定必须由运行路径另行完成，不能把结构校验当作授权。
- `verify_version(&Utf8Path)` 是异步只读探测，全部经 `qn-infra::process`；
  不搜索 PATH，不拉起沙箱；通过 `env_clear()` 启动空环境子进程，避免继承宿主凭证（#56）。
  相对路径、非零退出、截断/异常输出、版本不符均拒绝。
  缺失二进制和超时保留原始进程错误；版本响应不会进入本 crate 的错误信息。
- 安装调用方可在 `install` 返回后显式调用 `verify_version`；通过 EngineSetup 接入 CLI/doctor，
  `install` 本身保持同步下载安装语义。版本字符串检查不能证明文件完整性。

- 安装完整性（#58）：每次复制缓存归档到独立临时目录，核对锁版 SHA-256 后重新提取并发布
  `nono`，恢复字节及归档权限。归档缓存位于 `<install_root>/<version>/<archive_name>`；
  缺失时下载并验证，损坏时拒绝返回成功。旧安装首次需要联网取得归档；有效缓存可离线修复。
  不同平台目标必须使用不同 install root；校验不抵御返回后宿主特权进程再次修改文件。
- 上游 `SHA256SUMS.txt` 是哈希清单而非签名；签名校验方案在 08 落地时评估。
- 宿主下载通过 ureq 读取代理环境配置（#60），仅接受 HTTPS 请求/重定向；归档仍须匹配
  锁版 SHA-256。代理配置只用于宿主下载，不写入 profile 或传给 nono 子进程。

## 只读安装校验（#115）

`installer::verify_installation(&InstallOptions)` 复用锁版 manifest/归档校验/提取，
再比较已装普通文件与归档二进制的字节；缺失、symlink、非普通文件、损坏和差异均拒绝。
不下载、执行、修复或修改受管根；仅在根外私有临时目录提取并自动清理。
宿主根必须可信，不抵御校验后同权限并发替换。同步磁盘 IO 无取消/截止时间承诺，
异步使用方应置于 blocking worker 并负责生命周期。完整性不等于版本或隔离就绪。
#114 的 NonoSetup 实现 core EngineSetup；bootstrap/CLI 消费现有安装、只读完整性和版本探测。

## 版本契约测试

默认单测不依赖网络。macOS / Linux 手动验证已校验 SHA-256 的锁版二进制：

```sh
QN_TEST_NONO=/absolute/path/to/nono cargo test --locked -p qn-nono --test version_contract -- --ignored
```

此测试只验证版本调用契约，不等同于沙箱隔离、凭证或 Agent E2E 验收。
`cargo test --locked -p qn-nono --test version_environment` 使用合成 canary 和自重执行的
Rust 测试程序验证实际探测路径不继承环境；不修改宿主环境，也不代表 #27 的 sandbox run 已接入。

macOS / Linux 离线验证已下载的锁版归档，安装器会自行校验 SHA-256 并修复合成损坏文件：

```sh
QN_TEST_NONO_ARCHIVE=/absolute/path/to/nono-release.tar.gz cargo test --locked -p qn-nono --lib installer::tests::it_repairs_and_probes_the_real_pinned_binary_offline -- --ignored --exact
```

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
审计 DTO 经 `audit` 模块公开，其他 wire 类型仍 crate-private；未接入的 engine 路径保留 #14 dead_code 豁免。

`AuditReader::new(binary, managed_root, policy)` 只接受绝对路径，调用方须先核对 binary 的安装完整性。
root 布局与 #46 探测一致（home/config/state/cache/tmp），须为受管目录；策略由 bootstrap 注入。
`list/show` 使用固定参数、空父环境、5 秒查询限时和单流 1 MiB 上限；show 核对完整 native ID。
默认 Debug/错误不回显载荷。DTO 仅呈现原生记录，不证明来源完整/可信，不推断 filesystem Denial，
也不表示 #66 的统一事件查询或攻击 U8 已验收。schema 样本见 `tests/fixtures/nono-0.80.0/audit-*.json`。

`PreparedProfile::prepare` 是阻塞准备步骤，异步调用方放入 `spawn_blocking`；`RuntimePaths` 必须来自可信装配层，
包含实际宿主 home、控制目录、已校验引擎及工具 PATH。工作目录不能覆盖这些位置、控制程序或保护路径（含 .netrc/.kube/.docker）。
生成策略仅支持两个官方引用的离线基线：网络阻断、禁用能力提升、无 hooks/明文凭证/bypass。
带 grant 请求直接拒绝，不能当作在线官方 profile 已完成。调用方保留准备对象直到执行结束，
或显式 `persist` 将审计目录交给注册表管理。Linux 的安全性不依赖 deny-within-allow；原生 canary 验收由 #75 跟踪。

真实只读 smoke：`QN_TEST_NONO=/verified/nono QN_TEST_ROOT=/isolated/probe-root cargo test --locked -p qn-nono --test audit_reader -- --native`
（只使用含合成审计记录的隔离根；默认不带 `--native` 时跑 Rust 子进程回归）。

## 原生会话关联（#89）

`session::SessionReader::find(qn_id)` 在私有 runtime root 中精确匹配名称，再以完整 native ID
执行 inspect 并重验两种身份。重复名称/ID、错误格式、身份变化或不一致状态均拒绝。
返回不可变 `NativeSession`；`Running/Paused/Exited` 与 nullable exit code 只是原生观察，
不推断 core Stopped、后代清理或会话所有权。名称不是认证，调用方须拥有启动句柄、确保
root 对沙箱不可见且每次启动唯一；`None` 也不证明进程退出。native ID 可传同根 AuditReader。
复用 audit 的空父环境、隔离 HOME/XDG、锁版检查、5 秒单次命令时限及 1 MiB 输出上限。
尚未接入生产 runner/core port；不会因读到记录而自行停止进程或扩大授权。

默认回归：`cargo test --locked -p qn-nono --lib session::tests` 和 `--test audit_reader`。
真实 smoke：在专用合成 HOME/XDG 下用锁版 nono 前台启动带 qn SessionId 名称的短时会话，
会话运行期间设置 `QN_TEST_NONO`、`QN_TEST_ROOT`、`QN_TEST_SESSION` 后执行：
`cargo test --locked -p qn-nono --test audit_reader -- --native-session`。
测试不负责启动/清理；只允许专用临时根，不得指向用户状态。2026-10-25 macOS 实测
8 秒前台 sleep：一个 Running 记录、find/inspect 成功、前台自然退出 0；不代表 Agent E2E。

## 启动前身份预留（#108）

LaunchBinding 在可信私有控制根中通过 create_new 原子预留 qn ID，使用 OS 随机数生成完整
native ID。预留文件在释放句柄、启动失败后仍保留；不得删除后重试同一 ID。不同控制根是
不同命名空间，装配层必须统一选择；目录项的崩溃持久化与保留清理由拥有者负责。
command 只允许相同 qn ID 的前台启动，清空父环境并设置 nono 0.80.0 私有启动身份变量；
调用方负责二进制完整性/版本、profile 校验、可信环境和唯一进程句柄。该变量由上游过滤，
不进入沙箱环境；此行为来自 launch_runtime / proxy_runtime / exec_strategy 并须原生回归。
verify 拒绝观察到的 qn/native 漂移；ID 不是认证、存活证明或后代清理证明（#93）。

bound_launch_probe 示例由宿主测试器在启动前接收关联，再通过有界 stdin 提交合成 child argv；
仅供测试协调，不是生产 runner。实际 runner/capture 组合在 #92/#107 的本地集成验证，
依赖的 PR 未合并。默认回归：cargo test --locked -p qn-nono --lib launch::tests。

## 固定原生代理端口（#137 / #136）

NonoArgs::run_with_proxy_port 和 LaunchBinding::command_with_proxy_port 接受
NonZeroU16；仅在 child 分隔符前增加原生 --proxy-port，保持原有路径/命令校验、
固定身份和空父环境，不增加网络许可或通用 flags。默认 run 参数保持不变。
Codex 0.147.0 的 MCP URL 不展开环境变量，因此管理配置须与原生端口一致。
空闲端口探测不等于所有权；原生绑定失败必须拒绝，不能连接已有服务或自动换端口。

macOS arm64 真机回归（私有测试父目录建议建在 /Users/Shared 下，权限 0700）：

    QN_TEST_NONO=/verified/nono QN_TEST_PROFILE_PARENT=/private/test-parent cargo test --locked -p qn-nono --test proxy_port_contract -- --ignored

测试验证锁版 binary SHA-256、严格 profile、隔离 HOME/XDG、空闲端口下真实 child
拿到精确代理地址；占用端口时 child sentinel 不存在，已有 listener 和合成上游均
无连接。provider 固定拒绝且无真实凭证，不调用模型。命令经 infra 限时/限量执行。
Windows/WSL/Intel 原生行为未验证；#93 全执行树清理、在线 profiles 与生产组合仍独立跟踪。

## 受管前台生命周期契约（#81）

macOS/Linux 安装 `/usr/bin/ruby`，先核对 nono 归档与安装器 manifest 的 SHA-256，
再确认二进制与归档一致。在 `/Users/Shared`（macOS）或专用测试根（Linux）创建私有空目录，运行：

```sh
QN_TEST_NONO=/verified/nono QN_TEST_PROFILE_PARENT=/private/probe-parent cargo test --locked -p qn-nono --test managed_contract -- --ignored
```

测试创建独立 HOME/XDG/project，严格校验策略，经 infra + process-wrap 前台运行；网络只给
合成后代一个 loopback 端口用于就绪/关闭证据。正常退出、取消、超时均检查；清理失败时仅通过
已建立连接让测试后代退出（另有 12 秒自退出期限），不补杀未经身份保护的 PID。
此结果不证明任意脱离进程组后代、detached supervisor、Windows/WSL 或完整 Agent E2E 的清理。

2026-10-25 集成 #92 进一步发现：即使后代未主动脱组，只要关闭继承的 stdio 并让父进程退出，
受管 wait 也可能返回 Exited(0)，后代仍可执行。新增
`it_demonstrates_parent_exit_is_not_descendant_cleanup` 在父进程成功退出后才向已建立的
合成后代连接发送探针，确认应答后关闭；通过表示复现此限制，**不是清理通过**。
因此生产 runner 不得把 ManagedOutput/原生 Exited 直接映射为 core Stopped；需先解决 #92 的
后代所有权/终止证明。测试不给沙箱额外文件权限、不按数字 PID 补杀；无真实凭证或外部网络。
## Official profile registry (#75 / #59)

`profiles/manifest.json` names the two embedded official assets (`claude-code.json`,
`codex.json`). `qn_nono::registry::OfficialCatalog::embedded` parses them at startup,
requires `extends: "default"`, a non-empty proxy allowlist, no whole-domain `*` and no
network block, and compares the asset meta version with the manifest version. The asset
name is resolved through a closed match, so no runtime path can select another file.

`HostProfileRegistry` stores installed versions under
`QN_HOME/profiles/<namespace>/<name>/versions/<MAJOR.MINOR.PATCH>/` with the exact asset,
a SHA-256 digest and one `current` selection. Reads reject symlinks, non-regular files,
oversized input, non-canonical versions and digest mismatches. Installing identical bytes
is idempotent; different bytes for the same version are refused; rollback selects the
previous verified version only. These operations never launch nono, resolve a credential
or widen a grant. Native validation of both assets uses the pinned binary:

    QN_TEST_NONO=/verified/nono cargo test --locked -p qn-nono --lib \
      registry::tests::it_validates_embedded_assets_with_the_pinned_engine -- --ignored

## Engine setup adapter (#159 / #114)

NonoSetup implements the existing core EngineSetup contract using the pinned installer
and clean-environment version probe. A trusted absolute cache namespace is partitioned
by native host target. Install reuses download/repair; Inspect checks cached archive and
installed bytes before execution, without downloading or repairing. The owned blocking
worker is awaited; dropping the future cannot promise cancellation of filesystem IO.
Original errors are retained privately behind redacted formatting/source chains.
No PATH lookup, package/version change or sandbox-readiness claim. Native Windows setup
returns Unsupported pending WSL composition. Bootstrap/CLI wiring remains in #114.

## Retained native audit (#121)

LaunchBinding::publish_audit stores an exclusive bounded qn/native/runtime association
before spawn. ManagedNativeAudit loads only the canonical qn session mapping, verifies
the installed binary before executing, then queries the exact native record with the
existing AuditReader. Mapping is retained after owner exit and grants no process authority.
Projection emits closed network decisions, optional wall times, native order and counts;
raw paths/targets/reasons/command text are withheld. Integrity stays unverified and
coverage stays network_only. Missing/invalid associations are errors, not empty reports.
Host roots must remain private and trusted; no same-user tamper authentication claim.

    cargo build --locked -p qn-cli
    cargo build --locked -p qn-nono --example bound_launch_probe
    python3 tests/e2e/macos/native_audit.py --workspace . --archive /verified/nono.tar.gz

This macOS arm64 fixture checks actual native network denial, retained identity, mismatched
mapping rejection and tampered-binary refusal before execution, then offline repair.
It does not prove filesystem-denial ingestion, complete process cleanup or Agent E2E.
