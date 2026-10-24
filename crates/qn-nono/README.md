# qn-nono — engine adapter for the nono sandbox kernel

全仓唯一允许触碰 [nono](https://github.com/nolabs-ai/nono) 的 crate（01 §3）。
以**子进程方式驱动锁定版本的 `nono` CLI**（spike ③，决策 issue #28），只解析
`--json` / `--diagnostics-json` 输出；待 #14 冻结 ports v1 后实现 `SandboxEngine`。

## 模块

| 模块 | 职责 | 状态 |
| --- | --- | --- |
| `installer` | 锁版 manifest（v0.80.0 + 各平台 SHA-256）、下载、校验、解包安装到 `<install_root>/<version>/nono` | M0 骨架（本期） |
| `args` | version / profile validate / run 的受限参数构造；固定诊断开关和命令分隔符 | A-0.5（#43） |
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

- 安装幂等：目标路径已存在 `nono` 时直接复用，不重新校验（完整性复核由
  `qn doctor` 在 runner 落地后承担，TODO #19）。
- 上游 `SHA256SUMS.txt` 是哈希清单而非签名；签名校验方案在 08 落地时评估。
