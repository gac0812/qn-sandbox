# spike ②：nono 0.80.0 macOS 协议与安全边界

2026-10-24 · A · Issue #46 · macOS 26.4 arm64 · Rust 1.85.0。
本文是实测结果，不冻结 core port，也不是完整 M1 E2E 验收。

## 复现

~~~sh
cargo build --locked -p qn-nono --example probe
python3 tests/e2e/macos/probe.py \
  --nono /absolute/path/to/nono \
  --archive /absolute/path/to/nono-v0.80.0-aarch64-apple-darwin.tar.gz \
  --probe target/debug/examples/probe
~~~

脚本先核对安装清单中 arm64 归档 SHA-256，再比对归档内可执行文件与传入文件的 SHA。
合成 HOME/config/state/cache 位于权限 0700 的 /Users/Shared/qn-nono-probe-*；
父进程环境显式重建，不读取宿主凭证或代理变量；所有 nono 调用经 Rust probe →
qn-infra::process，无 tracing subscriber。原始输出只保存在该独立目录，不提交仓库。
测试提供者和假上游仅使用固定合成值；不打印 phantom 或收到的认证头。
/private/tmp 不适合作为此测试 HOME：default 的 system_read_macos 包含 /private，
与其下受保护的 .nono 状态根冲突，nono 在启动前拒绝。这不是拦截用例通过。

## 实测结果

| 检查 | 观测与结论 |
| --- | --- |
| profile 校验 | 正常文件 exit 0 / valid=true；未知字段 exit 1 / valid=false；errors 是字符串数组 |
| 文件拒绝 | 合成 HOME 下 .ssh/canary 读取 exit 1；诊断 session.denials 含 path/access/reason，violations 含 operation/target |
| env 白名单 | 宿主合成 QN_FAKE_TOKEN 被过滤；printenv exit 1 且 stdout 为空 |
| 网络 block | profile 的 network.block=true；直连 loopback curl exit 7，并有 Seatbelt network-outbound violation。单凭连接失败不足以证明拒绝 |
| 代理拒绝 | allow_domain 只含 example.invalid；blocked.invalid 请求 exit 22；audit show 记录 decision=deny、mode=connect、denial_category=host_denied |
| detached 关联 | --name sxn-… --detached exit 0；ps/inspect 按 name 找回 nono session_id |
| ps 与 audit 不同 | ps/inspect 用 status=running/paused/exited、attachment=detached/attached；is_alive/is_stale 属于 audit list，不可混用 |
| stop | 普通 stop --timeout 3 与 --force 均 exit 0；再次 stop exit 1。停止幂等须由未来 adapter 实现 |
| 清理 | ps 空不代表 PID 已退出；一次强制停止在 8 秒内仍有 PID，其他轮次均消失。重复验证不稳定，阻塞 runner 清理验收 |
| 用户覆盖 | synthetic user profiles/default.json 的 set_vars 标记出现在继承 default 的沙箱内；绝对路径传入子 profile 也无法阻止 base 被覆盖 |
| 零缓存撤销 | 同一会话两次请求；首请求 200，假上游收到正确合成认证头并撤销；次请求 503，假上游计数仍为 1；audit 有 capture exit 1 与 managed_credential_unavailable |
| 提供者隔离 | 沙箱通过 ruby 读取/执行宿主 provider.rb 被拒 exit 1，stdout 空；提供者仅在 supervisor 运行 |
| Agent 编辑 | 未验证：本片未接入经过安全评审的真实 API 凭证路径；不使用宿主 OAuth/Keychain 绕过 |

诊断 JSON 实际形状为 {session:{exit_code,denials,violations,diagnostics,…}}，
嵌在 stderr 人类文本之间。成功命令也可能有 command_failed_* 建议，不可据建议合成拒绝。
原始网络阻断的 audit network_events 可为空；代理层拒绝才产生本次观察到的 network_events。
macOS 诊断偶见其他探测命令的拒绝路径，后续会话归属必须进一步验证，不应默认每条记录归属准确。
workdir.access 在本次非交互模式仍提示未授权 CWD，因此 fixture 显式把项目目录放入 filesystem.allow。

## 官方 pack 比较

使用 nono pull nolabs-ai/claude@0.1.4（exit 0），不运行 pack hooks。
实测安装后 profile 位于 package 内的 profiles/claude.json（meta.version=1.0.0）。
它继承 default、包含多个 runtime/cache 组；network.block=false；允许 HOME/.claude，
并对 macOS HOME/Library/Keychains 设置 allow 与 bypass_protection，且没有 env 白名单。
所以不得直接作为 qn 官方安全 profile 发布；只可据锁版资产评估复用必要组。

## 后续决策与验证

- #14：保持 port 未冻结；explain 必须区分历史证据与 why 的策略预检。
- #27：production spawn 环境清理仍为硬前置；测试清洁环境不等于该产品能力完成。
- #47 跟踪 profile base 隔离、诊断归属、stop 残留与 provider 宿主明文环节；待 A/B/C 决策。
- 零缓存实验仅证明 nono 提供者失败后的请求断供，不证明 qn grant/RPC 身份认证已实现。
- fmt、workspace clippy -D warnings、workspace test、rustdoc -D warnings、cargo-deny 通过。
- 真 macOS harness 完整轮次通过，但复跑出现强制停止残留失败；保留失败结论；未验证 Linux、Windows、真实 Claude 编辑及远端 CI。

## 来源与复用

锁版源码：nolabs-ai/nono tag v0.80.0 下 profile-authoring-guide.md、nono-profile.schema.json、
session_commands.rs、audit_commands.rs、proxy_runtime.rs、nono-proxy/server.rs；未复制上游实现。
JSON 使用 04 已批准 serde_json；测试网络用 Python 标准库 http.server、Ruby 标准库 Net::HTTP。

实测 fixture 由 tests/e2e/macos/fixtures.py <evidence-dir> <destination> 生成；
仅导出预设的状态/拒绝/校验样例，脱敏根路径、用户路径、会话 ID 和时间，不导出 capture 输出。
