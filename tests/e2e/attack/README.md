# 攻击演示剧本（Attack-Demonstration Suite）

安全声明的唯一可信形式是系统性枚举攻击并观察全部被拒且留痕（spec 05 §1）。
用例从威胁模型（spec 08 §1 / guardrails Security Invariants）**机械推导**，
不靠灵感；任何一条 FAIL（含 KNOWN-GAP）= 缺口信号，SKIP 才是"暂不可判"。

## 用法

```bash
# 需要：装有 nono 的 Linux 环境（WSL2 发行版内；macOS 上为 nono 原生路径，Seatbelt）
bash tests/e2e/attack/run_attack_suite.sh
NONO_BIN=/path/to/nono bash tests/e2e/attack/run_attack_suite.sh   # 指定二进制
KEEP_TMP=1 bash tests/e2e/attack/run_attack_suite.sh               # 保留现场排障
```

退出码：`0` = 无 FAIL/GAP；`1` = 存在 FAIL 或 GAP（**GAP 计入失败**，fail-closed：
升级内核后必须转 PASS，防止真回归借 GAP 通道隐藏）；`2` = 前置缺失（无 nono/mktemp 失败）。
SKIP 不致败（依赖未建成组件）。

`KEEP_TMP=1` 会保留：`/tmp/qn-attack.*`（含 env/ps 探针输出）与
`~/.ssh/qn_attack_probe_key`（内容为显式标注的假密钥）。

## 断言纪律（本套件的信任根基）

每个"未观察到泄漏"型 PASS **必须以探测命令的退出码 = 0 为判定门**（stderr 不进
证据文件——错误文本非空不能冒充探测成功），否则只证明"没看到"，不证明"被防住"。
探测命令失败一律 **FAIL inconclusive**（fail-closed）；仅"宿主侧探测工具缺失"记
SKIP。U4 同理：curl rc=0 → ALLOWED（按 ABI 三分支：未知 → manual triage、V<4 →
GAP、否则 regression）；rc∈{6,7}（spike① T4/T9 实测的强制拒绝形态）→ PASS；
127（无 curl）/28（超时）等 → inconclusive FAIL。

## 用例 ↔ 不变式映射

| # | guardrails 不变式 | 攻击动作 | 预期 | 实现 | 实跑(v1) |
| --- | --- | --- | --- | --- | --- |
| U1 | 凭证不进 fs | 读 allowlist 外的 `~/.ssh` 假密钥 | 拒 + 留痕 | ✅ | PASS |
| U2 | 凭证不进 env | 父进程设置哨兵 env，沙箱内 `env` 扫描 | 哨兵不可见 | ✅ | **FAIL（真实发现）** |
| U3 | 凭证不进 argv | 沙箱内 `ps -eww -o args` 扫哨兵 | 不可见 | ✅ | PASS |
| U4 | 网络默认拒绝 | 未授权 `curl https://example.com` | rc∈{6,7} | ✅（ABI 感知） | GAP（KNOWN-GAP） |
| U5 | 按 profile 放行 + 代理附加认证 | allowlist 域名经代理 | 200 + 认证头 | ⏳ M4 | SKIP |
| U6 | 托管配置零明文 | 写入含明文密钥的 MCP 配置 | 拒绝写入 | ⏳ M4 | SKIP |
| U7 | 撤销立即断供 | revoke 后再出站 | 立即拒 | ⏳ M4 | SKIP |
| U8 | 拒绝留痕（审计） | 检查 U1 的会话否认记录 | 可检索 | ✅（会话输出层代理） | PASS |

### v1 未覆盖面（推导完备性的账面清单）

- **agent context**（inv1 第四面，提示注入上下文）——无对应用例；需 guest 内
  行为级探针，与 qn-agent/qn-nono 集成后评估。
- **inv2（短时占位符/broker 句柄）**——属 qn-broker 域，随 U5/U7 一并（M4）。
- **威胁模型"控制面（qnd/CLI）"行**——qnd 尚未实现（M1 后补"本机恶意进程"用例）。

## KNOWN-GAP 语义

Landlock ABI <4（内核 <6.7，如 stock WSL2 6.6）时内核无法强制网络默认拒绝——
U4 记为 `GAP`（引用 spike① #16；裁决 #17），并**计入失败**。ABI 探测失败（`?`）时
U4 输出 "manual triage required"，不断言回归。升级内核（WSL ≥3.0.3 自带 6.18）后
GAP 标注消失，U4 必须 PASS。

## 与 CI 的关系

CI runner 无 nono，本套件**不进 CI**；触及 08 机制代码的 PR 必须手动执行并把
汇总表（含 `GAP=N` 计数）粘进 PR（AGENTS §8.4）。执行环境记录：nono 版本
（与 `NONO_PINNED_VERSION` 对齐）、Landlock ABI、宿主平台。

## 扩展点（按里程碑）

- M2：Windows lane 形态 = 在 qn-wsl 供给的 WSL2 发行版内跑同一脚本。
- M4：补 U5/U6/U7（对接 qn-broker/qn-gateway 真实接口）。nono v0.80.0 已具备
  `--allow-domain <DOMAIN_OR_URL>`（支持 URL+路径 glob，如 `https://github.com/org/**`）
  与 `--network-profile`（来源：`nono run --help` 实测输出，2026-10-24）——U5 的
  代理白名单断言可直接建立在其上。

## v1 实测记录（2026-10-24，WSL2 Ubuntu-24.04 + nono v0.80.0 + Landlock V3）

- PASS：U1（fs 拒绝）、U3（argv 扫描）、U8（否认留痕）
- **FAIL（真实发现）U2**：父进程 env 哨兵出现在沙箱内 `env` 输出中——nono 默认
  透传父进程环境（`run --help` 未见 env 清理旗标）；guardrails"凭证不进 env"在
  引擎层不设防 → **qn-nono 适配器必须在每次拉起时剥离宿主 env（#27 env_clear
  升级为 M1 硬前置）**。该用例留作永久回归门：适配器补上 env 剥离后应转 PASS。
- **GAP U4**：ABI V3 无法内核级拒绝出站（#16/#17；升级内核后应转 PASS）。
- SKIP：U5/U6/U7（M4 组件依赖）。
- 判别力自证：U3 首跑曾出现假阳性（`ps | grep SENTINEL_VALUE` 自匹配 grep 自己的
  argv），以 `[V]` 括号技巧修复后转 PASS——假阳性被套件自身的重跑机制捕获。
