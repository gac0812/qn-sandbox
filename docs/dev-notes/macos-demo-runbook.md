# qn-sandbox macOS 最小演示流程

这份流程用于黑客松联调，目标是在一台干净的 macOS 机器上展示“安装 → profile → 受限运行 → audit → stop”的控制面链路。真实 nono、Agent 和 MCP 凭证验收必须按文中的前置条件执行；没有真实证据的步骤不得标记为通过。

## 前置条件

- macOS 13+，Apple Silicon 或 Intel；
- Rust/Cargo 与仓库锁定工具链可用；
- 仓库已检出，工作区干净；
- 已准备与宿主架构匹配的锁定 nono archive；
- Claude Code 或 Codex 已安装；
- 演示使用专用测试项目目录；
- 所有凭证使用临时测试凭证或由宿主侧 broker 提供，禁止把真实 API key 写入项目、环境变量、命令参数、截图或日志。

## 1. 构建和基础诊断

```bash
cargo build --locked -p qn-cli
cargo run --locked -p qn-cli -- --version
cargo run --locked -p qn-cli -- --help
```

准备隔离的 qn home：

```bash
export QN_HOME="$(mktemp -d /Users/Shared/qn-demo-home.XXXXXX)"
mkdir -p "$QN_HOME"
```

配置-only 诊断不需要引擎，适合先确认 CLI：

```bash
cargo run --locked -p qn-cli -- doctor --config-only
cargo run --locked -p qn-cli -- doctor --config-only --json
```

JSON 应包含 `schema_version: 1`、`configuration: "ok"` 和 `sandbox_engine: "not_checked"`。该结果只证明配置有效。

## 2. 安装和完整性检查

把锁定 archive 放入 qn 管理的缓存目录，或按安装器支持的方式提供测试 archive，然后执行：

```bash
cargo run --locked -p qn-cli -- install
cargo run --locked -p qn-cli -- install --json
cargo run --locked -p qn-cli -- doctor
cargo run --locked -p qn-cli -- doctor --json
```

成功结果必须说明 archive integrity 和 version 已验证。`doctor --json` 中 `sandbox_engine` 应为 `binary_verified`，同时 `runtime_checks` 保持 `not_checked`。缺少 archive、错误 digest、替换 binary 或非 regular file 都必须失败，且不输出部分成功 JSON。

## 3. Profile 检查

```bash
cargo run --locked -p qn-cli -- profile list --json
cargo run --locked -p qn-cli -- profile show qn/claude-code --json
cargo run --locked -p qn-cli -- profile show qn/codex --json
cargo run --locked -p qn-cli -- profile validate qn/claude-code --json
```

确认 profile 的 source、版本、state directory 和 network allowlist 可见；不要把 `nono_profile` 中的凭证值、宿主密钥路径或临时 token 展示到演示输出。profile validate 必须使用已验证的 pinned engine。

## 4. 受限 Agent 运行

在专用测试项目目录运行一个无敏感数据的最小任务：

```bash
export QN_DEMO_PROJECT="$(mktemp -d /Users/Shared/qn-demo-project.XXXXXX)"
cd "$QN_DEMO_PROJECT"
printf '# qn demo\n' > README.md
cargo run --locked -p qn-cli -- run --profile qn/claude-code -- claude
```

也可以使用非交互的短命令验证 session 生命周期：

```bash
cargo run --locked -p qn-cli -- run --profile qn/claude-code -- /bin/sh -c 'printf demo-session'
```

记录输出中的 `session`、`profile`、`native`、`end` 和 `cleanup`。`cleanup: unconfirmed` 是预期安全语义，不能改写成成功清理。

## 5. Status、stop 和 audit

使用上一步产生的 canonical session ID：

```bash
cargo run --locked -p qn-cli -- status "$SESSION_ID"
cargo run --locked -p qn-cli -- status "$SESSION_ID" --json
cargo run --locked -p qn-cli -- audit show "$SESSION_ID"
cargo run --locked -p qn-cli -- audit show "$SESSION_ID" --json
cargo run --locked -p qn-cli -- audit native "$SESSION_ID" --json
cargo run --locked -p qn-cli -- stop "$SESSION_ID" --json
```

Audit 输出必须保留 source、integrity 和 `partial/open/observed_closed` 状态。native audit 只声称它实际覆盖的 network evidence；不能推断 filesystem denial、凭证成功或完整 cleanup。

## 6. 安全验收清单

每项都要记录真实命令、平台和输出位置：

- [ ] Agent 不能读取 `~/.ssh`、`~/.aws` 或测试密钥文件；拒绝事件有 audit 证据；
- [ ] 越权域名出站被拒；
- [ ] Agent 环境变量、argv、工作目录和 stdout/stderr 中没有真实凭证；
- [ ] managed MCP/Agent config 中不存在 secret、password、token、api key 的值；
- [ ] revoke 后新的受保护请求被拒；
- [ ] stop/取消后只报告观察到的 native 状态，cleanup 仍按实际证据标记；
- [ ] JSON 输出可解析，错误退出时 stdout 为空；
- [ ] 截图、日志和 PR 评论没有内部材料或真实凭证。

## 7. 未覆盖项

本流程不证明 Windows WSL2、Tauri 安装包、daemon HTTP/RPC、authenticated MCP transport、真实 provider credential mediation 或执行树完整清理。缺少对应平台或真实 Agent 证据时，应明确标记为未验证，并转到对应 Issue。
