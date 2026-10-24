# spike ③ — nono 集成形态：Rust 库链接（FFI）vs 子进程调用

状态：调研完成，结论待决策 issue 三人确认 · 2026-10-24 · 负责人：A（laitekin） · 对应 TODO-004c

> 本笔记是决策 issue 的素材（背景 / 选项 / 结论 / 重审条件，07 §3）。
> 观察对象：crates.io 上 `nono` / `nono-cli` / `nono-proxy` **0.80.0**（2026-10-13 发布），
> 源码取自 crates.io 发布包，观察日期 2026-10-24。

## 1. 背景

01 §1 与 04 §3 把 nono 的接入方式留作"子进程或 FFI，M0 spike 定"。`qn-nono` 的 `runner`
模块、安装器锁版方案、以及 `qn-wsl` 在 WSL2 内驱动 nono 的方式都依赖这个结论，是 M1 开工前置。

## 2. 事实（源码与发布物观察）

| # | 事实 | 来源 |
| --- | --- | --- |
| F1 | 上游拆为三个 crate：`nono`（库）、`nono-proxy`（网络过滤/凭证代理库）、`nono-cli`（二进制 `nono`），三者同版本号发布，均 Apache-2.0 | 各 crate `Cargo.toml` |
| F2 | `nono` 库自述是 **"pure sandboxing primitive … without imposing any security policy"**：只提供 `CapabilitySet` + `Sandbox::apply*`；profiles、pack 注册表、supervisor、凭证代理接线、审计账本、`why`、`ps/stop/logs/inspect` 全在 `nono-cli` 内 | `nono/src/lib.rs` 模块文档；`nono-cli/src/` 文件列表 |
| F3 | `Sandbox::apply*` 作用于**调用进程本身且不可逆**；要沙箱化子进程必须 fork 后在子进程里 apply，`nono-cli` 自身即用 `unsafe { fork() }` 实现 | `nono/src/lib.rs` 示例注释；`nono-cli/src/exec_strategy.rs` |
| F4 | 三个 crate 的 `rust-version = "1.95"`；本仓库工作区为 1.85（`rust-toolchain.toml` / CI `RUSTUP_TOOLCHAIN`） | `Cargo.toml` |
| F5 | 库仅在 Linux（Landlock）/ macOS（Seatbelt）有实现，其他平台返回 `UnsupportedPlatform`；依赖按 `cfg(target_os = "linux"/"macos")` 引入 | `nono/src/lib.rs` "Platform Support"；`Cargo.toml` target 段 |
| F6 | CLI 子命令覆盖 port 全部语义：`run`/`wrap`（拉起）、`ps`/`inspect`（状态）、`stop`（停止）、`logs`/`audit list|show|verify`（审计）、`why`（策略判定）、`profile list|show|validate|diff`、`pull`（签名 pack） | `nono-cli/src/cli.rs` `enum Commands` |
| F7 | 机读输出：`why`、`ps`、`inspect`、`logs`、`audit list/show/verify/status`、`profile list/show/diff/validate`、`list/search/outdated` 均有 `--json`；`run` 有 `--diagnostics-json` | `nono-cli/src/cli.rs` 各 `*Args.json` |
| F8 | 凭证两条路径：`--credential <service>`（反向代理注入，phantom token）与 `--env-credential` / `--env-credential-map`（**把凭证作为环境变量注入沙箱**） | `nono-cli/src/cli.rs` L1267–L1304 |
| F9 | `nono why` 回答的是"在某 profile/参数下，某路径/主机/命令**会**被允许还是拒绝"（策略查询），不是"某会话**已发生**的拒绝列表"；后者在 `logs` / `audit show` / `inspect` | `WhyArgs` 字段（path/op/host/profile…，无 session id） |
| F10 | 官方分发：Homebrew（macOS/Linux）、`cargo install nono-cli`、源码构建；官方 claude-code profile 经 `nono pull nolabs-ai/claude` 获取 | `nono-cli/README.md` |

## 3. 选项

### 选项 A：链接 `nono` 库（进程内 FFI / Rust 依赖）——**否决**

| 否决理由 | 依据 |
| --- | --- |
| 库没有策略层；profiles、凭证代理接线、审计、会话管理都得我们在 `qn-nono` 里重写一份，正是 R1/R4 禁止的"自研第二份" | F2 |
| apply 不可逆且作用于调用者：qnd 只能 fork 后在子进程里 apply，需要 `unsafe`（工作区 `unsafe_code = "forbid"`），且 fork 多线程 tokio 进程本身就是已知风险 | F3 |
| 强制把工作区 MSRV 从 1.85 抬到 1.95，并且此后每次 nono 升级都可能牵动全仓工具链 | F4 |
| Windows 宿主上 nono 跑在 WSL2 里，宿主进程无法链接；Windows 路径无论如何都要走 `wsl.exe` 子进程，选 A 会造成 macOS / Windows 两套集成方式 | F5；00 §3 |
| 传递依赖面大（sigstore-verify、keyring、landlock、nix、regress…；`nono-proxy` 还带 hyper/rustls/aws-sdk 系），cargo-deny 许可证/重复版本治理成本高 | 各 `Cargo.toml` |

### 选项 B：子进程调用 `nono` CLI（锁版本外部二进制）——**采纳**

| 优点 | 依据 |
| --- | --- |
| 复用上游完整策略层（profiles、pack 签名、代理、审计账本、supervisor），`qn-nono` 只做参数组装与 JSON 解析，符合"薄产品层" | F2、F6 |
| 状态/审计/诊断都有 `--json`，可以做结构化解析与契约测试，不靠抓人类可读文本 | F7 |
| macOS（直跑）与 Windows（`wsl.exe -- nono …`）同一调用模型，差异只在 `qn-wsl` 提供的命令前缀 | F5 |
| 无 `unsafe`、不牵动工作区 MSRV；升级 = 换锁定二进制 + 契约测试回归 | F3、F4 |
| 隔离失败面：nono 崩溃不拖垮 qnd，退出码 + `--diagnostics-json` 类型化为 `EngineUnavailable` 等错误 | 01 §4 错误变体 |

代价与对策：

| 代价 | 对策 |
| --- | --- |
| CLI 参数与 JSON schema 是上游 pre-1.0 接口，可能漂移 | 版本锁死；`qn-nono` 契约测试跑真二进制（arch-breakdown §3.2）；升级 PR 必须附契约测试结果（04 §6） |
| 需要安装与校验外部二进制 | 见 §5 安装器锁版方案 |
| 每次调用有进程启动开销 | 仅发生在会话生命周期操作上（run/stop/状态轮询），非热路径 |

## 4. 结论（待三人确认）

1. **`qn-nono` 以子进程方式驱动锁定版本的 `nono` CLI**，不链接 `nono` / `nono-proxy` 库；
   全部外部调用经 `qn-infra::process`（超时/截断/退出码，AGENTS §4.6），参数数组传参，禁止拼 shell。
2. 解析只依赖 `--json` / `--diagnostics-json` 输出与退出码；人类可读输出只透传给用户，不做解析。
3. Windows：`qn-wsl` 提供"在受管发行版内执行"的命令前缀（`wsl.exe -d <distro> --`），`qn-nono`
   经 `qn-bootstrap` 装配获得该前缀，自身不出现平台分支（R6）。
4. **安全约束（新增，供 08 采纳）**：`qn-nono` / `qn-broker` 只允许使用 `--credential`（代理注入）；
   **禁止生成 `--env-credential` / `--env-credential-map` 参数**——它们把凭证放进沙箱环境变量，
   直接违反铁律"凭证不进 env"（F8）。需在 `qn-nono` 参数构造处加拒绝测试。

## 5. 安装器锁版方案（M0 草案，`qn-nono::installer` 落地）

| 项 | 方案 |
| --- | --- |
| 锁定版本 | `qn-config` 常量 `NONO_VERSION = "0.80.0"`（本 spike 观察版本）；升级走独立 `deps:` PR |
| 获取方式 | 首选上游 GitHub Release 预编译产物 + 上游签名校验；`brew install nono` 不可锁精确版本，只作开发者便利 |
| 校验 | 安装清单记录各平台产物 SHA-256；安装后执行 `nono --version` 比对锁定版本，不符即 `EngineUnavailable`（带 `qn doctor` 指引） |
| 位置 | `~/.qn/cache/` 下按版本分目录（arch-breakdown §7），不污染用户 PATH，不复用用户自装的 nono |
| Windows | 同一清单，由 `qn-wsl::bootstrap` 在发行版内安装 Linux 产物 |

## 6. 对现有 spec 的影响

| 文档 | 修改 |
| --- | --- |
| 01 §1 分层图 | "子进程/FFI" → "子进程（nono CLI）" |
| 04 §3 nono 行 | "子进程或 FFI，M0 spike 定" → 子进程调用锁版 CLI，不链接库 |
| architecture-breakdown §3.2 / §6 | `runner` 定为子进程；通道表同步 |
| 01 §4 `explain` 语义（**需另议**） | 当前写"透传 nono why"，但 `why` 是策略查询（F9）。会话已发生的拒绝应来自 `logs`/`audit show --json`；`why` 更适合 `qn why <path>` 类预检。属 port 语义，按 09 §4 由 C 牵头另开决策 issue，本 spike 不改 |

## 7. 实测记录

macOS Seatbelt 下的行为实测属 spike ②（A + C），本笔记不覆盖；WSL2 内核实测属 spike ①（B）。

### 7.1 Linux 实测（验证子进程 + `--json` 路线可行）

环境：Linux 6.18（云端开发容器，非 WSL2），Rust 1.97，`cargo install nono-cli --version 0.80.0 --locked`
构建成功，二进制约 56 MB，`nono --version` → `nono 0.80.0`。

| 命令（`./proj` 为授权目录） | 结果 | 对 `qn-nono` 的含义 |
| --- | --- | --- |
| `nono why --path /etc/shadow --op read --allow ./proj --json` | `{"status":"denied","reason":"path_not_granted","suggested_flag":"--read-file /etc/shadow",…}`，**退出码 0** | 判定结果在 JSON `status` 字段，不能用退出码区分 allow/deny |
| `nono why --path ./proj/a.txt --op read --allow ./proj --json` | `{"status":"allowed","reason":"granted_path","access":"read+write",…}`，退出码 0 | 同上 |
| `nono run --allow ./proj -- cat ./proj/a.txt` | 输出 `hi`，退出码 0 | 授权路径可用 |
| `nono run --allow ./proj -- cat <授权目录外文件>` | `Permission denied`，退出码 1（透传子命令退出码） | Landlock 拦截生效 |
| `nono run --allow ./proj --block-net -- curl https://example.com` | curl 退出码 7（连接失败） | 网络拦截生效 |
| `nono run --diagnostics-json …`（越权读） | stderr 输出 JSON：`session.exit_code=1`，`denials=[]`，`diagnostics=[command_failed_likely_sandbox,…]`；其后跟人类可读文本 | JSON 与人类文本混在 stderr，解析需定界；见下方风险 |
| `nono ps --json` / `nono audit list --json` | `[]` / 会话数组（`session_id`、`command`、`exit_code`、`network_event_count`、`is_alive`…） | 状态与审计可结构化读取 |

**实测发现的风险（需 spike ① / ② 跟进，不影响本结论）**：在该 Linux 环境默认模式下，
越权读被内核拒绝，但 nono 报告 `denials: []` 和 "No path denials were observed"——
Landlock 拒绝没有被 nono 捕获为 Denial。M1 验收要求"攻击演示全拦截**且留痕**"，因此需确认：
macOS Seatbelt 下是否有 denial 记录（spike ②）、WSL2 内核下是否需要 nono 的 supervised/诊断模式
或更高 Landlock ABI 才能产出 denial（spike ①）。

## 8. 重审条件

- 上游把 profiles/代理/审计下沉进库并提供稳定 API（1.0）；或
- 子进程模型无法满足某项 M1 验收（例如审计事件延迟不可接受）；或
- CLI 的 `--json` 输出在两个连续版本中出现破坏性变化且上游不承诺稳定。
