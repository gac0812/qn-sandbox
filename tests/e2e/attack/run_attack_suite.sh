#!/usr/bin/env bash
# 攻击演示剧本 v1 —— 安全不变式的可执行回归（spec 05 §1/§2；guardrails P0 第 6 项）。
#
# 用例从威胁模型（spec 08 §1）机械推导（U1–U8，见同目录 README.md）。
# 在装有 nono 的 Linux 环境直接运行：bash tests/e2e/attack/run_attack_suite.sh
# 可调项：NONO_BIN（默认 nono）、KEEP_TMP=1（保留现场，含 ~/.ssh/qn_attack_probe_key）。
#
# 断言纪律：每个"未观察到泄漏"型 PASS 必须先证明探测命令自身成功执行，
# 否则只证明"没看到"，不证明"被防住"（空转 PASS 是安全回归套件的命门）。
#
# 本套件不在 CI 中运行（runner 无 nono）；触及 08 机制代码的 PR 必须手动执行
# 并把汇总表粘贴进 PR（AGENTS §8.4、spec 05 §3）。
set -u

NONO_BIN="${NONO_BIN:-nono}"
umask 077
TMPROOT="$(mktemp -d /tmp/qn-attack.XXXXXX)" || { echo "FATAL: mktemp failed" >&2; exit 2; }
SSH_PROBE_KEY="$HOME/.ssh/qn_attack_probe_key"
SSH_DIR_CREATED=0
RESULTS=()
GAP_N=0
SENTINEL="QN_ATTACK_SENTINEL_zz9x"

cleanup() {
    rm -rf "$TMPROOT"
    rm -f "$SSH_PROBE_KEY"
    [ "$SSH_DIR_CREATED" = "1" ] && rmdir "$HOME/.ssh" 2>/dev/null
    return 0
}
trap cleanup EXIT
trap 'cleanup; exit 130' INT TERM
# KEEP_TMP=1：三个 trap 全清，保留现场（含探针输出与假密钥文件）。
if [ "${KEEP_TMP:-0}" = "1" ]; then
    trap - EXIT INT TERM
fi

record() { # status case detail
    RESULTS+=("$1|$2|$3")
    [ "$1" = "GAP" ] && GAP_N=$((GAP_N+1))
    printf '  [%s] %-6s %s\n' "$1" "$2" "$3"
}

# 在 nono 沙箱内执行（allowlist=$TMPROOT），回显完整会话输出供断言。
nono_session() { "$NONO_BIN" run --allow "$TMPROOT" -- "$@" 2>&1; }
# 经 sh -c（仅测试脚手架；生产路径无 shell）。
nono_shell() { "$NONO_BIN" run --allow "$TMPROOT" -- sh -c "$1" 2>&1; }

# ---------- 前置 ----------
if ! command -v "$NONO_BIN" >/dev/null 2>&1; then
    echo "FATAL: nono not found (NONO_BIN=$NONO_BIN)" >&2
    exit 2
fi
[ -d "$TMPROOT" ] || { echo "FATAL: tmproot missing" >&2; exit 2; }

if [ ! -d "$HOME/.ssh" ]; then
    mkdir -m 700 -p "$HOME/.ssh" && [ -d "$HOME/.ssh" ] && SSH_DIR_CREATED=1
fi
echo "FAKE-KEY sk-attack-probe-000000 (not a real credential)" > "$SSH_PROBE_KEY"
chmod 600 "$SSH_PROBE_KEY"

# ABI 探测：nono 会话面板打印能力版本；解析失败按未知处理（U4 分支见下）。
PROBE_OUT="$(nono_session true)"
if ABI_V="$(printf '%s' "$PROBE_OUT" | grep -oE 'Landlock V[0-9]+' | head -1 | grep -oE '[0-9]+')"; then
    :
else
    ABI_V="?"
fi
echo "== qn attack suite v1 | nono=$("$NONO_BIN" --version 2>/dev/null | tail -1) | Landlock ABI=V${ABI_V} =="

# ---------- U1: 凭证不进 fs（读 allowlist 外的 ~/.ssh 必须被拒且留痕） ----------
U1_OUT="$(nono_session cat "$SSH_PROBE_KEY")"
U1_RC=$?
if [ "$U1_RC" -ne 0 ] && printf '%s' "$U1_OUT" | grep -qi 'permission denied'; then
    record PASS U1 "fs denial: ~/.ssh read blocked (kernel Landlock)"
else
    record FAIL U1 "expected denial reading $SSH_PROBE_KEY, rc=$U1_RC"
fi

# ---------- U2: 凭证不进 env（父进程哨兵不得出现在沙箱 env） ----------
# 判定门 = 探测命令退出码：ENV_RC=0 才允许判"未泄漏"（stderr 不进证据文件，
# 错误文本非空不能冒充探测成功）。门失败一律 FAIL inconclusive（fail-closed）。
export SENTINEL_VALUE="$SENTINEL"
U2_SESS="$(nono_shell "env > '$TMPROOT/env.out'; echo ENV_RC=\$?")"
unset SENTINEL_VALUE
U2_ENVRC="$(printf '%s' "$U2_SESS" | grep -oE 'ENV_RC=[0-9]+' | grep -oE '[0-9]+')"
if [ "${U2_ENVRC:-none}" != "0" ]; then
    record FAIL U2 "probe inconclusive: env rc=${U2_ENVRC:-none} inside sandbox (no vacuous pass)"
elif grep -Fq "SENTINEL_VALUE=$SENTINEL" "$TMPROOT/env.out"; then
    record FAIL U2 "parent env sentinel LEAKS into sandbox env — violates guardrails invariant (credentials never enter sandbox env); qn-nono must scrub env at every spawn site (#27 env hygiene is a hard M1 prerequisite)"
else
    record PASS U2 "parent env sentinel does not leak into sandbox (probe rc=0 self-verified)"
fi

# ---------- U3: 凭证不进 argv（哨兵不得出现在沙箱内进程命令行） ----------
# 判定门 = PS_RC=0（ps 正常必有输出行，[ -s ] 仅作副证）；匹配用 [V] 括号技巧
# 避免自匹配。探测失败一律 FAIL inconclusive；仅"宿主无 ps"记 SKIP。
if command -v ps >/dev/null 2>&1; then
    export SENTINEL_VALUE="$SENTINEL"
    U3_SESS="$(nono_shell "ps -eww -o args > '$TMPROOT/ps.out'; echo PS_RC=\$?")"
    unset SENTINEL_VALUE
    U3_PSRC="$(printf '%s' "$U3_SESS" | grep -oE 'PS_RC=[0-9]+' | grep -oE '[0-9]+')"
    if [ "${U3_PSRC:-none}" != "0" ]; then
        record FAIL U3 "probe inconclusive: ps rc=${U3_PSRC:-none} inside sandbox (no vacuous pass)"
    elif grep -Eq "SENTINEL_[V]ALUE|$SENTINEL" "$TMPROOT/ps.out"; then
        record FAIL U3 "sentinel visible in sandbox argv scan"
    else
        record PASS U3 "sentinel not visible in sandbox process argv (probe rc=0 self-verified)"
    fi
else
    record SKIP U3 "ps not available in sandbox environment"
fi

# ---------- U4: 网络默认拒绝（未授权出站必须被内核/代理拒绝） ----------
# 判定纪律：curl rc=0 → ALLOWED；rc∈{6,7}（DNS 解析被断/连接被拒，spike① T4/T9
# 实测的强制拒绝形态）→ PASS；127（无 curl）/28（超时）等 → inconclusive FAIL。
U4_OUT="$(nono_shell 'curl -sI --max-time 8 -o /dev/null -w "%{http_code}" https://example.com; echo CURL_RC=$?')"
U4_RC="$(printf '%s' "$U4_OUT" | grep -oE 'CURL_RC=[0-9]+' | grep -oE '[0-9]+')"
U4_RC="${U4_RC:-none}"
case "$U4_RC" in
    0)
        if [ "$ABI_V" = "?" ]; then
            record FAIL U4 "outbound ALLOWED with unknown Landlock ABI (probe parse failed) — manual triage required"
        elif [ "$ABI_V" -lt 4 ]; then
            record GAP U4 "outbound ALLOWED on Landlock V$ABI_V — KNOWN-GAP: kernel <6.7 cannot enforce net default-deny (spike #16; decision #17); expected on stock WSL2"
        else
            record FAIL U4 "outbound ALLOWED despite Landlock V${ABI_V} — regression of net default-deny"
        fi
        ;;
    6|7)
        record PASS U4 "outbound enforced-denied (curl rc=$U4_RC, spike #16 T4/T9 denial form)"
        ;;
    *)
        record FAIL U4 "probe inconclusive: curl rc=$U4_RC (127=no curl, 28=timeout) — manual triage required"
        ;;
esac

# ---------- U5: 按 profile 放行 + 代理附加认证 ----------
record SKIP U5 "allowlisted-domain-with-auth needs qn-broker proxy integration (M4); nono --allow-domain groundwork noted in README"

# ---------- U6: 托管配置零明文（MCP 配置只允许 Handle/引用） ----------
record SKIP U6 "managed-config validation is qn-broker::write_managed_config territory (M4); revisit when broker lands"

# ---------- U7: 撤销立即断供 ----------
record SKIP U7 "grant/revoke lifecycle is qn-broker territory (M4); revisit when broker lands"

# ---------- U8: 拒绝必须留痕（会话输出包含否认记录） ----------
# v1 断言基于 nono 会话输出（防篡改审计呈现层属 qn-daemon/ui，M1 增强）。
if printf '%s' "$U1_OUT" | grep -qiE 'denial|denied'; then
    record PASS U8 "U1 denial is recorded in session output (audit source)"
else
    record FAIL U8 "U1 denial not traceable in session output — audit visibility gap"
fi

# ---------- 汇总 ----------
PASS_N=0; FAIL_N=0; SKIP_N=0
for r in "${RESULTS[@]}"; do
    s="${r%%|*}"
    case "$s" in
        PASS) PASS_N=$((PASS_N+1));;
        GAP|FAIL) FAIL_N=$((FAIL_N+1));;
        *) SKIP_N=$((SKIP_N+1));;
    esac
done
echo "== summary: PASS=$PASS_N FAIL=$FAIL_N SKIP=$SKIP_N GAP=$GAP_N | ABI=V${ABI_V} =="
# SKIP 不致败（依赖未建成组件）；GAP 计入 FAIL（fail-closed：升级内核后必须转 PASS，
# 防止真回归借 GAP 通道隐藏）；ABI 未知时 U4 已要求人工判读。
[ "$FAIL_N" -eq 0 ] && exit 0 || exit 1
