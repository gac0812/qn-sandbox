#!/usr/bin/env bash
# 攻击演示剧本 v1 —— 安全不变式的可执行回归（spec 05 §1/§2；guardrails P0 第 6 项）。
#
# 用例从威胁模型（spec 08 §1）机械推导（U1–U8，见同目录 README.md）。
# 在装有 nono 的 Linux 环境（WSL2 / macOS 本机 Linux 侧）直接运行：
#   bash tests/e2e/attack/run_attack_suite.sh
# 可调项：NONO_BIN（默认 nono）、KEEP_TMP=1（保留现场）。
#
# 本套件不在 CI 中运行（runner 无 nono）；触及 08 机制代码的 PR 必须手动执行
# 并把汇总表粘贴进 PR（AGENTS §8.4、spec 05 §3）。
set -u

NONO_BIN="${NONO_BIN:-nono}"
TMPROOT="$(mktemp -d /tmp/qn-attack.XXXXXX)"
SSH_PROBE_KEY="$HOME/.ssh/qn_attack_probe_key"
SSH_DIR_CREATED=0
RESULTS=()   # "STATUS|case|detail"
SENTINEL="QN_ATTACK_SENTINEL_zz9x"

cleanup() {
    rm -rf "$TMPROOT"
    rm -f "$SSH_PROBE_KEY"
    [ "$SSH_DIR_CREATED" = "1" ] && rmdir "$HOME/.ssh" 2>/dev/null
    return 0
}
trap cleanup EXIT
[ "${KEEP_TMP:-0}" = "1" ] && trap - EXIT

record() { # status case detail
    RESULTS+=("$1|$2|$3")
    printf '  [%s] %-6s %s\n' "$1" "$2" "$3"
}

# 在 nono 沙箱内执行（allowlist=$TMPROOT），回显完整会话输出供断言。
nono_session() { # command...
    "$NONO_BIN" run --allow "$TMPROOT" -- "$@" 2>&1
}

nono_shell() { # snippet —— 经 sh -c（仅测试脚手架；生产路径无 shell）
    "$NONO_BIN" run --allow "$TMPROOT" -- sh -c "$1" 2>&1
}

# ---------- 前置 ----------
if ! command -v "$NONO_BIN" >/dev/null 2>&1; then
    echo "FATAL: nono not found (NONO_BIN=$NONO_BIN)" >&2
    exit 2
fi

mkdir -p "$TMPROOT"
if [ ! -d "$HOME/.ssh" ]; then mkdir -p "$HOME/.ssh"; SSH_DIR_CREATED=1; fi
echo "FAKE-KEY sk-attack-probe-000000 (not a real credential)" > "$SSH_PROBE_KEY"

# ABI 探测：nono 每次会话都打印能力面板，解析 Landlock 版本号。
PROBE_OUT="$(nono_session true)"
if ABI_V="$(printf '%s' "$PROBE_OUT" | grep -oE 'Landlock V[0-9]+' | head -1 | grep -oE '[0-9]+')"; then
    :
else
    ABI_V="?"
fi
echo "== qn attack suite v1 | nono=$("$NONO_BIN" --version 2>/dev/null | tail -1) | Landlock ABI=V${ABI_V} =="

# ---------- U1: 凭证不进 fs（读 ~/.ssh 之外的 allowlist 必须被拒） ----------
U1_OUT="$(nono_session cat "$SSH_PROBE_KEY")"
U1_RC=$?
if [ "$U1_RC" -ne 0 ] && printf '%s' "$U1_OUT" | grep -qi 'permission denied'; then
    record PASS U1 "fs denial: ~/.ssh read blocked (kernel Landlock)"
else
    record FAIL U1 "expected denial reading $SSH_PROBE_KEY, rc=$U1_RC"
fi

# ---------- U2: 凭证不进 env（父进程哨兵不得出现在沙箱 env） ----------
U2_OUT="$(SENTINEL_ENV_TMP=1; export SENTINEL_VALUE="$SENTINEL"; nono_shell 'env | grep -c SENTINEL_VALUE || true')"
if printf '%s' "$U2_OUT" | grep -qE '^0$'; then
    record PASS U2 "parent env sentinel does not leak into sandbox"
else
    record FAIL U2 "parent env sentinel LEAKS into sandbox env — violates guardrails invariant (credentials never enter sandbox env); nono v0.80 has no env-scrub flag (--help survey), so qn-nono must scrub env at every spawn site (#27 env hygiene is a hard M1 prerequisite)"
fi

# ---------- U3: 凭证不进 argv（哨兵不得出现在沙箱内进程命令行） ----------
# 注：grep 模式用 [S] 括号技巧避免自匹配（ps 会看到 grep/sh 自己的 argv）。
if command -v ps >/dev/null 2>&1; then
    export SENTINEL_VALUE="$SENTINEL"
    U3_OUT="$(nono_shell 'ps -eww -o args 2>/dev/null | grep -c "SENTINEL_[V]ALUE" || true')"
    unset SENTINEL_VALUE
    if printf '%s' "$U3_OUT" | grep -qE '^0$'; then
        record PASS U3 "sentinel not visible in sandbox process argv"
    else
        record FAIL U3 "sentinel visible in sandbox argv scan"
    fi
else
    record SKIP U3 "ps not available in sandbox environment"
fi

# ---------- U4: 网络默认拒绝（未授权出站必须被拒） ----------
U4_OUT="$(nono_shell 'curl -sI --max-time 8 -o /dev/null -w "%{http_code}" https://example.com; echo CURL_RC=$?')"
U4_RC_LINE="$(printf '%s' "$U4_OUT" | grep -oE 'CURL_RC=[0-9]+')"
if printf '%s' "$U4_RC_LINE" | grep -q 'CURL_RC=0'; then
    if [ "$ABI_V" != "?" ] && [ "$ABI_V" -lt 4 ]; then
        record FAIL U4 "outbound ALLOWED on Landlock V$ABI_V — KNOWN-GAP: kernel <6.7 cannot enforce net default-deny (spike #16; decision #17); expected on stock WSL2"
    else
        record FAIL U4 "outbound ALLOWED despite Landlock V${ABI_V} — regression of net default-deny"
    fi
else
    record PASS U4 "outbound denied by default (non-zero curl rc: $U4_RC_LINE)"
fi

# ---------- U6: 托管配置零明文（MCP 配置只允许 Handle/引用） ----------
record SKIP U6 "managed-config validation is qn-broker::write_managed_config territory (M4); revisit when broker lands"

# ---------- U7: 撤销立即断供 ----------
record SKIP U7 "grant/revoke lifecycle is qn-broker territory (M4); revisit when broker lands"

# ---------- U8: 拒绝必须留痕（会话输出包含否认记录） ----------
if printf '%s' "$U1_OUT" | grep -qiE 'denial|denied'; then
    record PASS U8 "U1 denial is recorded in session output (audit source)"
else
    record FAIL U8 "U1 denial not traceable in session output — audit visibility gap"
fi

# ---------- 汇总 ----------
PASS_N=0; FAIL_N=0; SKIP_N=0
for r in "${RESULTS[@]}"; do
    s="${r%%|*}"
    case "$s" in PASS) PASS_N=$((PASS_N+1));; FAIL) FAIL_N=$((FAIL_N+1));; *) SKIP_N=$((SKIP_N+1));; esac
done
echo "== summary: PASS=$PASS_N FAIL=$FAIL_N SKIP=$SKIP_N | ABI=V${ABI_V} =="
# SKIP 不致败：SKIP 的用例依赖未建成组件；FAIL 才是回归/缺口信号。
[ "$FAIL_N" -eq 0 ] && exit 0 || exit 1
