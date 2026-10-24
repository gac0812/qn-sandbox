# 攻击演示剧本（Attack-Demonstration Suite）

安全声明的唯一可信形式是系统性枚举攻击并观察全部被拒且留痕（spec 05 §1）。
用例从威胁模型（spec 08 §1 / guardrails Security Invariants）**机械推导**，
不靠灵感；任何一条放行 = P0 缺陷。

## 用法

```bash
# 需要：装有 nono 的 Linux 环境（WSL2 发行版内或 macOS 的 Linux 侧）
bash tests/e2e/attack/run_attack_suite.sh
NONO_BIN=/path/to/nono bash tests/e2e/attack/run_attack_suite.sh   # 指定二进制
KEEP_TMP=1 bash tests/e2e/attack/run_attack_suite.sh               # 保留现场排障
```

退出码：`0` = 无 FAIL；`1` = 存在 FAIL；`2` = 前置缺失（如无 nono）。
**SKIP 不致败**（依赖未建成组件）。

## 用例 ↔ 不变式映射

| # | guardrails 不变式 | 攻击动作 | 预期 | v1 状态 |
| --- | --- | --- | --- | --- |
| U1 | 凭证不进 fs | 读 allowlist 外的 `~/.ssh` 假密钥 | 拒 + 留痕 | ✅ |
| U2 | 凭证不进 env | 父进程设置哨兵 env，沙箱内 `env` 扫描 | 哨兵不可见 | ✅（若 FAIL = 真实缺口，关联 #27） |
| U3 | 凭证不进 argv | 沙箱内 `ps -eww -o args` 扫哨兵 | 不可见 | ✅（无 ps 则 SKIP） |
| U4 | 网络默认拒绝 | 未授权 `curl https://example.com` | 非 0 退出码 | ✅（ABI<4 时标注 KNOWN-GAP，见下） |
| U5 | 按 profile 放行 + 代理附加认证 | allowlist 域名经代理 | 200 + 认证头 | ⏳ M4（需 qn-broker） |
| U6 | 托管配置零明文 | 写入含明文密钥的 MCP 配置 | 拒绝写入 | ⏳ M4（`write_managed_config`） |
| U7 | 撤销立即断供 | revoke 后再出站 | 立即拒 | ⏳ M4（grant/revoke） |
| U8 | 拒绝留痕（审计） | 检查 U1 的会话否认记录 | 可检索 | ✅ |

## KNOWN-GAP 语义

Landlock ABI <4（内核 <6.7，如 stock WSL2 6.6）时内核无法强制网络默认拒绝——
U4 的失败会被标注为 KNOWN-GAP（spike① #16；裁决在 #17），与"回归"区分。
这不是放行：升级内核（WSL ≥3.0.3 自带 6.18）后该标注自动消失，U4 必须 PASS。

## 与 CI 的关系

CI runner 无 nono，本套件**不进 CI**；触及 08 机制代码的 PR 必须手动执行并把
汇总表粘进 PR（AGENTS §8.4）。执行环境记录：nono 版本（`NONO_PINNED_VERSION`
对齐）、Landlock ABI、宿主平台。

## 扩展点（按里程碑）

- M2：Windows lane 形态 = 在 qn-wsl 供给的 WSL2 发行版内跑同一脚本（无 macOS 分支需要写）。
- M4：补 U5/U6/U7（对接 qn-broker/qn-gateway 的真实接口）。
