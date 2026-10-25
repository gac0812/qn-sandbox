# Spike① 报告：WSL2 内核与 Landlock 网络可用性验证（TODO-004a）

- 日期：2026-10-24 · 执行：成员 B（gac0812）· 状态：**实验完成，两项裁决待决策 issue（见 §6）**
- 环境：Windows 11（10.0.26200.9711）/ WSL 2.6.3.0 / Ubuntu-24.04（`--no-launch` 安装）/ nono v0.80.0（deb，SHA256SUMS 校验通过）
- 复现说明：附录 A 为内核探针完整源码；§2 表内为命令要点，完整命令与原始输出见附录 B。

## 1. 假设验证结论

| # | 假设 | 结论 | 关键证据 |
| --- | --- | --- | --- |
| H1 | WSL2 默认内核 ≥6.7 | **✗ 证伪（本机当前）** | `uname -r` = `6.6.87.2-microsoft-standard-WSL2`。注意：这不是 stable 通道天花板——**stable WSL 3.0.3 已附带内核 6.18.54.1**（观察日期 2026-10-24，来源 microsoft/WSL release 3.0.3 notes，PR #41822）；本机停留 2.6.3.0 且 `wsl --update` 因网络返回 HTTP 403 无法触达（§4-E4） |
| H2 | Landlock 网络限制在默认内核可用 | **✗ 证伪（6.6 内核）** | C 探针（附录 A）：`ABI version: 3`；net ruleset 创建失败；fs ruleset 创建成功；`CONFIG_SECURITY_LANDLOCK=y` 且 landlock 位于 LSM 链首位（多源互证） |
| H3 | 存在可行内核升级路径 | **✓ 成立（端到端已验证，2026-10-24）** | MSI 提权安装 WSL 3.0.3.0 成功：内核 `6.18.54.1-microsoft-standard-WSL2`，探针 ABI **3→7**、net ruleset 创建成功；nono 面板报 Landlock V6（内核宣告 7，nono 用至其支持级 V6）。**兜底** = 自编译 `linux-msft-wsl-6.18.y` + `.wslconfig kernel=`（离线/锁版场景）。渠道障碍实测见 §4-E4 |
| H4 | nono 在 WSL2 内正常工作 | **◐ 部分成立** | fs 强制 ✓（T1）；网络 per-domain 过滤在 6.6 上 ✗——nono 自报 `Landlock V3, degraded: per-port filtering… unavailable on WSL2`（其引用 `nono.sh/docs/cli/internals/wsl2`）；代理相关旗标族存在（E1），phantom 代理启用方式待确认（已列 M2 遗留） |

## 2. 行为测试记录（nono v0.80.0，Ubuntu-24.04，root；完整命令见附录 B）

| # | 场景 | 结果 | 判读 |
| --- | --- | --- | ---|
| T1 | fs 越界读 `~/.ssh`（假密钥） | **Permission denied，exit 1** | 内核级 fs 拒绝生效 ✓ |
| T2 | fs 白名单内读 `/tmp` | 正常输出 | ✓ |
| T7 | 默认网络出站（curl example.com） | **CURL_EXIT=0**，capabilities 面板明示 `net outbound allowed` | 6.6 上默认网络全开 |
| T4 | `--block-net` 出站 | CURL_EXIT=6（DNS 失败） | 被断 ✓ |
| T9 | `--block-net` 直连 IP（http://1.1.1.1/，绕过 DNS） | **CURL_EXIT=7** | 连接级真实阻断，排除"仅 DNS 把戏" ✓ |
| T5c/d | 回环对照（127.0.0.1:8765 本地服务） | 无 block-net：exit 0；有 block-net：exit 7 | **block-net 断 TCP 含回环** |
| E2 | **AF_UNIX 通路在 block-net 下** | `UNIX_SOCKET_OK`（unix socket 双向通信成功） | **block-net 只断 TCP 族，AF_UNIX 存活**——降级方案的关键新事实 |
| T8 | 沙箱内代理相关 env | 无 HTTP(S)_PROXY 注入（仅 `NONO_CAP_FILE`、BROWSER 包装） | v0.80 默认 run 不以 env 方式启代理 |
| E1 | nono 网络旗标枚举 | `--block-net`、`--network-profile`、`--upstream-proxy <HOST:PORT>`、`--proxy-port`、`--proxy-ca-validity`、`--network-denial-audit-rate/burst` | 代理与网络审计能力在 CLI 层存在；`--block-net` 语义为"经代理按域名阻断，先于 allowlist，与 --allow-net 互斥" |

## 3. 对架构的影响（供决策 issue 引用，不构成单方定论）

1. **6.6 内核无法兑现 guardrails「网络默认拒绝、按 profile 放行」**：per-domain/L7 过滤需要 Landlock ABI v4+；6.6 上 nono 只能 fs 强制 + 网络全开或粗粒度 block-net。spec 00 §3 既有"供给链路负责探测/升级内核"的 P0 前置职责由此**确认为硬前置**，且主路径从"自编译"修正为"**官方 stable 升级（WSL ≥3.0.3）**"。
2. **降级路线（fs-only + block-net + 代理）初步判断 TCP 形态不可行、AF_UNIX 形态未被排除**：block-net 断 TCP 含回环（T5d/T9）使 TCP 代理通路死亡；但 AF_UNIX 存活（E2）且 nono 具备 upstream-proxy 旗标族（E1）——若 nono 代理可绑定 unix socket，降级方案仍有生机。**该结论不单方定案**，按 09 §6 走 B+A 评估 + 三人裁决（决策 issue，含"Windows 提前启用 P2 VM 模式"备选分支）。
3. **升级失败用户的降级档与 doctor 呈现**需与 spec 08 §2"不达标显式降级告知（fs-only）"对齐修订：现证据表明 fs-only 档的网络形态只剩"全断（block-net）"，即 **offline-only profile**。同入决策 issue。
4. **供给链路工程事实**：Ubuntu-24.04 `--no-launch` 静默安装与 deb 装 nono（SHA256 校验）全程可自动化；WSL 内直连 GitHub 停滞（NAT 模式宿主代理不透传，反复出现 localhost 代理警告）→ 下载步骤应在宿主侧完成后经 `/mnt/c` 递入；镜像加速（如 ghfast.top）可用但**经镜像分发必须保留 SHA256/digest 校验**（08 §6）。M2 评估 `networkingMode=mirrored` 替代。

## 4. 升级路径实测（E4，观察日期 2026-10-24）

| 渠道 | 结果 | 对 qn-wsl 的含义 |
| --- | --- | --- |
| `wsl --update`（stable） | **HTTP 403**（`Wsl/UpdatePackage/0x80190193`，本机网络环境拦截微软 CDN） | 供给链路不能只依赖该命令；需探测+多渠道回退 |
| GitHub 官方 `wsl.3.0.3.0.x64.msi`（367MB） | 安装返回 **1603**（当前 shell 未提权；msi 需管理员） | 用户级自动化首选非 msi，或引导 UAC |
| GitHub 官方 msixbundle（711MB） | 下载/`Add-AppxPackage` 用户级安装可行（本次进行中被中止，**未完成端到端验证**；本机升级涉及用户现有 Docker/WSL 环境，已提交所有者批准后再执行） | qn-wsl kernel 模块的候选主路径之一 |
| 自编译 `linux-msft-wsl-6.18.y` | 未执行（M2 任务） | 兜底路径（离线/锁版场景） |

**待办**：所有者批准后在本机完成 WSL 3.0.3 升级 → `wsl --shutdown` → 重跑附录 A 探针复验 ABI ≥4 → nono 网络过滤行为复测。这是 spike 的最后一环（H3 端到端）。

## 4b. 升级后验证与新增发现（2026-10-24，H3 收口）

| 验证项 | 结果 |
| --- | --- |
| `wsl --version` | 3.0.3.0（MSI 提权安装，exit 0） |
| `uname -r` | `6.18.54.1-microsoft-standard-WSL2` |
| Landlock 探针（附录 A） | `ABI version: 7`（原 3）；`NET ruleset: fd=3` 创建成功（原 E2BIG） |
| nono 能力面板 | `Landlock V6`（内核宣告 ABI 7，nono 使用至其支持级），不再报 degraded |
| docker-desktop 发行版 | 完好（`wsl -l -v` 确认） |
| 攻击套件重跑（tests/e2e/attack） | PASS=3 FAIL=2 SKIP=3 **GAP=0**：U4 从 GAP 转为普通违规（语义正确：内核已能强制，是调用姿态问题） |

**新增发现（改变 #17 决策输入）**：

1. **nono 默认网络姿态 = outbound allowed，与内核能力无关**（ABI V3 与 V6 实测一致，
   面板明示 `net outbound allowed`）。升级前"WSL2 degraded"的解释不完整：
   默认拒绝不能指望内核升级自动带来，必须由 qn 调用层构造。
2. **默认拒绝 + 白名单今天即可构造**：仅传 `--allow-domain example.com` 即激活代理
   过滤且隐含默认拒绝——白名单域名 HTTP 200 放行（P7）、白名单外域名 curl rc=56
   被拒（P6）。空白名单 = 完全离线（offline-only 降级档的实现形态）。
3. **`--block-net` 与 `--allow-domain` 互斥**（官方报错：domain filtering requires
   proxy mode）——全断模式与代理过滤模式二选一，profile 层不可混用。
4. 附带发现的新旗标：`--deny-domain`、`--upstream-bypass`、`--extends`（profile 继承）、
   `--proxy-ca-validity`——qn profiles 的策略 DSL 比预想丰富（nono v0.80 `run --help`，
   2026-10-24）。

结论：kernel 升级必要但不充分；guardrails 网络不变式的落地公式 =
**内核 ≥6.7（ABI v4+）× qn 调用层强制传 profile 白名单（`--allow-domain`）**。
M4 之前唯一做不到的只剩 U5 的凭证附加（代理认证注入）。

## 5. 升级方案初版风险清单（TODO-004a DoD 要求）

1. 用户已有 WSL 发行版/Docker Desktop 与 WSL 3.x 的兼容性未在本机验证（升级被暂缓的原因之一）。
2. `.wslconfig kernel=` 注入自编译内核时，错配可能导致 WSL 无法启动——需要 doctor 提供"恢复默认内核"一键路径。
3. 网络受限环境（如本机 403）下官方渠道不可达——多渠道（update/msi/msix/镜像+校验）与离线包方案必须并存。
4. msi 需提权、msixbundle 体积大（711MB）——供给链路的下载与安装体验需分级设计（doctor 探测 → 建议 → 用户确认）。
5. 长期跟版：WSL stable 内核随 6.18.y 演进，qn 需锁最低版本（≥3.0.3）并跟踪 nono 对 ABI 版本的要求变化。
6. Landlock ABI 前提写死为 ≥v4；若 nono 未来要求更高 ABI（如 scoped v6），探测逻辑需读 `landlock_create_ruleset` 返回值而非仅内核版本号。

## 6. 遗留与去向（全部转 M2 / qn-wsl kernel 模块，关联决策 issue 见 PR）

- [x] 本机 WSL 3.0.3 升级 + ABI ≥4 复验 + nono 网络过滤行为复测（已完成，见 §4b）
- [ ] 决策 issue：kernel 硬前置 + 降级档（offline-only）+ doctor 呈现 + P2 VM 备选分支（三人裁决）
- [ ] AF_UNIX 形态的代理连通验证（若 nono 代理可绑 unix socket，降级路线复活）
- [ ] mirrored 网络模式下宿主代理连通性验证（M2 / qn-wsl bootstrap 模块）
- [ ] nono phantom 代理启用方式确认（T8/E1 之后的文档级证据）——与 qn-broker 集成设计对齐

## 附录 A：内核探针源码（gcc 编译，输出见 §1-H2）

```c
#include <linux/landlock.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <stdio.h>
#include <errno.h>
#include <string.h>
#include <stdint.h>
int main(void) {
    errno = 0;
    int abi = syscall(SYS_landlock_create_ruleset, NULL, 0, LANDLOCK_CREATE_RULESET_VERSION);
    printf("ABI version: %d (errno=%d %s)\n", abi, errno, strerror(errno));
    struct landlock_ruleset_attr attr = {0};
    attr.handled_access_net = (uint64_t)LANDLOCK_ACCESS_NET_BIND_TCP | (uint64_t)LANDLOCK_ACCESS_NET_CONNECT_TCP;
    errno = 0;
    int fd = syscall(SYS_landlock_create_ruleset, &attr, sizeof(attr), 0);
    printf("NET ruleset: fd=%d errno=%d (%s)\n", fd, errno, strerror(errno));
    attr.handled_access_net = 0;
    attr.handled_access_fs = (uint64_t)LANDLOCK_ACCESS_FS_READ_FILE;
    errno = 0;
    fd = syscall(SYS_landlock_create_ruleset, &attr, sizeof(attr), 0);
    printf("FS ruleset:  fd=%d errno=%d (%s)\n", fd, errno, strerror(errno));
    return 0;
}
```

本机输出：`ABI version: 3` / `NET ruleset: fd=-1 errno=7 (E2BIG)`（按 man 2 landlock_create_ruleset：传入的 ruleset_attr 尺寸超过该内核支持，即此内核的结构体尚无 `handled_access_net` 字段）/ `FS ruleset: fd=3`。

## 附录 B：完整命令与原始输出（摘要）

```text
# T1  fs 拒绝（假密钥）
$ wsl -d Ubuntu-24.04 -u root -- nono run --allow /tmp -- cat /root/.ssh/fake_key
/usr/bin/cat: /root/.ssh/fake_key: Permission denied
Command exited with code 1.        # capabilities 面板另示: kernel Landlock V3, degraded ... unavailable on WSL2

# T7/T4/T9/T5c/T5d 网络（在沙箱内取 curl 真实退出码）
$ nono run --allow /tmp -- bash -c "curl -sI --max-time 6 -o /dev/null https://example.com; echo CURL_EXIT=$?"   # CURL_EXIT=0
$ nono run --allow /tmp --block-net -- bash -c "curl -sI --max-time 6 -o /dev/null https://example.com; echo CURL_EXIT=$?"  # CURL_EXIT=6
$ nono run --allow /tmp --block-net -- bash -c "curl -s --max-time 5 -o /dev/null http://1.1.1.1/; echo CURL_EXIT=$?"      # CURL_EXIT=7
$ nono run --allow /tmp -- curl -s --max-time 4 -o /dev/null http://127.0.0.1:8765/   # 对照组 exit 0（python3 -m http.server 8765 --bind 127.0.0.1）
$ nono run --allow /tmp --block-net -- bash -c "curl -s --max-time 4 -o /dev/null http://127.0.0.1:8765/; echo CURL_EXIT=$?" # CURL_EXIT=7

# E2 AF_UNIX 存活（block-net 下 unix socket 双向通信）
$ nono run --allow /tmp --block-net -- python3 /tmp/uxt.py     # UNIX_SOCKET_OK
#   uxt.py: AF_UNIX SOCK_STREAM bind(/tmp/ux.sock)+listen → 同进程 connect → accept → ping/pong

# E1 网络旗标枚举
$ nono run --help | grep -iE "net|proxy"
#   --block-net / --network-profile <PROFILE> / --upstream-proxy <HOST:PORT>
#   --proxy-port <PORT> / --proxy-ca-validity <DAYS>
#   --network-denial-audit-rate <PER_SEC> / --network-denial-audit-burst <COUNT>
#   --block-net 语义原文（同 help 输出，2026-10-24，nono v0.80.0）：
#   "Block a domain through the proxy. Evaluated before the allowlist.
#    Supports wildcards (e.g. *.ads.example.com). Incompatible with --allow-net"

# E2 附：uxt.py 源码（AF_UNIX 探针，随本报告一并入库于 docs/dev-notes/）
#   bind(/tmp/ux.sock)+listen → 同进程 connect → accept → send "ping"/recv 校验 → 打印 UNIX_SOCKET_OK

# T8 代理 env
$ nono run --allow /tmp -- bash -c "env | grep -iE 'proxy|nono'"   # 仅 NONO_CAP_FILE=... 与 BROWSER=...（PATH 另含 nono 包装目录）

# E4 升级渠道
$ wsl --update          # 错误(403): Wsl/UpdatePackage/0x80190193（本机仍 2.6.3.0）
$ msiexec /i wsl.3.0.3.0.x64.msi /qn /norestart    # MSI EXIT=1603（未提权）
# msixbundle 用户级安装路径已识别（Add-AppxPackage），端到端验证待所有者批准
```

## 附录 C：外部参考（均观察于 2026-10-24）

- [Landlock 内核文档（ABI 版本表）](https://docs.kernel.org/userspace-api/landlock.html)
- [landlock_create_ruleset(2) — E2BIG 语义](https://man7.org/linux/man-pages/man2/landlock_create_ruleset.2.html)
- [microsoft/WSL release 3.0.3（kernel 6.18.54.1，stable）](https://github.com/microsoft/WSL/releases/tag/3.0.3)
- [microsoft/WSL2-Linux-Kernel（分支 linux-msft-wsl-6.18.y；releases 无预编译产物）](https://github.com/microsoft/WSL2-Linux-Kernel)
- nono CLI 自报降级说明 URL：`https://nono.sh/docs/cli/internals/wsl2`
