# qn-nono — engine adapter for the nono sandbox kernel

全仓唯一允许触碰 [nono](https://github.com/nolabs-ai/nono) 的 crate（01 §3）。
以**子进程方式驱动锁定版本的 `nono` CLI**（spike ③，决策 issue #28），只解析
`--json` / `--diagnostics-json` 输出；待 #14 冻结 ports v1 后实现 `SandboxEngine`。

## 模块

| 模块 | 职责 | 状态 |
| --- | --- | --- |
| `installer` | 锁版 manifest（v0.80.0 + 各平台 SHA-256）、下载、校验、解包安装到 `<install_root>/<version>/nono` | M0 骨架（本期） |
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

- `verify_version(&Utf8Path)` 是异步只读探测，全部经 `qn-infra::process`；
  不搜索 PATH，不拉起沙箱。相对路径、非零退出、截断/异常输出、版本不符均拒绝。
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
