#!/usr/bin/env bash
#
# btc-send-testnet-e2e.sh — L29 operator-driven live testnet E2E for
# `btc wallet send` (parent Issue #277 / sub-Issue #278).
#
# All inputs come from env vars (no config file). CI never runs this
# script — only the operator, against a real funded testnet wallet.
# Default invocation prints usage and exits 0 without network calls.
#
# Required env (under BTC_E2E_TESTNET=1):
#   BTC_E2E_TESTNET=1             master gate (otherwise usage + exit 0)
#   BTC_E2E_MNEMONIC              BIP-39 sender phrase (12/15/18/21/24 words)
#   BTC_E2E_MNEMONIC_FILE         alt: path to mnemonic, mode 0o600 enforced
#   BTC_E2E_RECIPIENT             testnet recipient address (BIP-21)
#   BTC_E2E_FAUCET_URL            testnet faucet landing page
#
# Optional env:
#   BTC_E2E_AMOUNT_SAT            default 10000
#   BTC_E2E_FEE_RATE_SAT_PER_VB   default unset → fetch via fee-estimates
#   BTC_E2E_FEE_TARGET_BLOCKS     default unset; if set, pick from
#                                 fee-estimates at closest target
#                                 (e.g. 1, 3, 6, 144, 1008)
#   BTC_E2E_ESPLORA_URL           default https://blockstream.info/testnet/api
#   BTC_E2E_SPKI_PIN              F20 SPKI pin (required for non-regtest)
#   BTC_E2E_POLL_TIMEOUT          default 600 (seconds; ~10 min for 1 conf)
#   BTC_E2E_MODE                  single | multi | drain | bumfee | all
#                                 default single
#
# Exit codes:
#   0  all enabled gates PASS
#   1  at least one gate FAIL
#   2  missing required env or invalid input
#   3  build failed
#
# L12 mnemonic handling: mnemonic flows through process env once
# (export) so `btc wallet send --mnemonic "$BTC_E2E_MNEMONIC"` does
# not echo it on the command line of subshells. Operator MUST set
# BTC_E2E_MNEMONIC_FILE (mode 0o600) and prefer that over the env var
# form in any environment where shell history is logged.

set -uo pipefail

usage() {
    cat <<'EOF'
btc-send-testnet-e2e.sh — L29 live-testnet E2E for btc wallet send.

Default (no env): print this usage, exit 0. No network calls.

Opt in:
  BTC_E2E_TESTNET=1 BTC_E2E_RECIPIENT=tb1q... \
    BTC_E2E_MNEMONIC_FILE=/path/to/mnem.txt \
    BTC_E2E_FAUCET_URL=https://coinfaucet.eu/btc/testnet/ \
    bash rust-wallet-app/scripts/btc-send-testnet-e2e.sh

Sub-gates (BTC_E2E_MODE): single | multi | drain | bumfee | all
EOF
}

# --- Default: usage + exit (CI-safe) ---
if [[ "${BTC_E2E_TESTNET:-0}" != "1" ]]; then
    usage
    exit 0
fi

# --- Color helpers ---
if [[ -t 1 ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'
    BLUE='\033[0;34m'; BOLD='\033[1m'; RESET='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; BLUE=''; BOLD=''; RESET=''
fi

# --- Validate required env ---
err_missing=()
for var in BTC_E2E_RECIPIENT BTC_E2E_FAUCET_URL; do
    if [[ -z "${!var:-}" ]]; then
        err_missing+=("$var")
    fi
done
if [[ -z "${BTC_E2E_MNEMONIC:-}" && -z "${BTC_E2E_MNEMONIC_FILE:-}" ]]; then
    err_missing+=("BTC_E2E_MNEMONIC (or BTC_E2E_MNEMONIC_FILE)")
fi
if [[ -n "${BTC_E2E_MNEMONIC:-}" && -n "${BTC_E2E_MNEMONIC_FILE:-}" ]]; then
    echo -e "${RED}BTC_E2E_MNEMONIC and BTC_E2E_MNEMONIC_FILE are mutually exclusive${RESET}" >&2
    exit 2
fi
if (( ${#err_missing[@]} > 0 )); then
    echo -e "${RED}Missing required env under BTC_E2E_TESTNET=1:${RESET}" >&2
    for v in "${err_missing[@]}"; do
        echo "  - $v" >&2
    done
    exit 2
fi

# --- Apply defaults ---
: "${BTC_E2E_AMOUNT_SAT:=10000}"
: "${BTC_E2E_ESPLORA_URL:=https://blockstream.info/testnet/api}"
: "${BTC_E2E_POLL_TIMEOUT:=600}"
: "${BTC_E2E_MODE:=single}"
# BTC_E2E_STEP controls incremental re-enable of gate_single stages.
#   0 = btc wallet balance (default; safe, no broadcast)
#   1 = + btc wallet send (captures txid; no poll)
#   2 = + poll_tx_confirmed
#   3 = + assert_recipient_paid (full original gate)
: "${BTC_E2E_STEP:=0}"

# --- Resolve mnemonic ---
resolve_mnemonic() {
    if [[ -n "${BTC_E2E_MNEMONIC:-}" ]]; then
        printf '%s' "$BTC_E2E_MNEMONIC"
        return 0
    fi
    local f="$BTC_E2E_MNEMONIC_FILE"
    if [[ ! -f "$f" ]]; then
        echo -e "${RED}BTC_E2E_MNEMONIC_FILE=$f does not exist${RESET}" >&2
        return 1
    fi
    # L12: enforce mode 0o600 on the mnemonic file. Refuse otherwise.
    local mode
    mode=$(stat -c '%a' "$f" 2>/dev/null || stat -f '%Lp' "$f" 2>/dev/null)
    if [[ "$mode" != "600" ]]; then
        echo -e "${RED}BTC_E2E_MNEMONIC_FILE=$f has mode $mode (expected 600). ${RESET}" >&2
        echo -e "${RED}Refusing to read; run: chmod 600 $f${RESET}" >&2
        return 1
    fi
    # Trim trailing newline.
    <"$f" tr -d '\n'
}

MNEMONIC_PHRASE=$(resolve_mnemonic) || exit 2

# --- Resolve fee rate (env override > fee-estimates) ---
resolve_fee_rate() {
    if [[ -n "${BTC_E2E_FEE_RATE_SAT_PER_VB:-}" ]]; then
        echo "$BTC_E2E_FEE_RATE_SAT_PER_VB"
        return 0
    fi
    if [[ -n "${BTC_E2E_FEE_TARGET_BLOCKS:-}" ]]; then
        local target="$BTC_E2E_FEE_TARGET_BLOCKS"
        echo -e "${BLUE}Fetching fee estimate${RESET} for target=$target blocks via $BTC_E2E_ESPLORA_URL" >&2
        local estimates
        estimates=$(btc fee-estimates \
            --network testnet \
            --esplora-url "$BTC_E2E_ESPLORA_URL" \
            ${BTC_E2E_SPKI_PIN:+--pin-spki "$BTC_E2E_SPKI_PIN"} \
            --json) || {
            echo -e "${RED}btc fee-estimates failed${RESET}" >&2
            return 1
        }
        # Pick the entry whose target is closest. JSON shape:
        # {"1": 5, "3": 4, "6": 3, "144": 2, "1008": 1}
        # Use jq if available, fall back to a python one-liner.
        local rate
        if command -v jq >/dev/null 2>&1; then
            rate=$(echo "$estimates" | jq -r --argjson t "$target" \
                '[. | to_entries[] | {diff: ((\(.key | tonumber) - $t) | fabs), rate: .value}] | min_by(.diff) | .rate')
        else
            rate=$(echo "$estimates" | python3 -c "
import json, sys
target = int(sys.argv[1])
est = json.load(sys.stdin)
print(min(est.values(), key=lambda v: abs(int([k for k in est if int(k) >= target][0]) - target)) if any(int(k) >= target for k in est) else est.get('1', est.get('144', 1)))
" "$target")
        fi
        if [[ -z "$rate" || "$rate" == "null" ]]; then
            echo -e "${RED}Could not extract fee rate from estimates${RESET}" >&2
            return 1
        fi
        echo "$rate"
        return 0
    fi
    # No fee source configured — default to 1 sat/vB (matches Story 5 default).
    echo "1"
}

FEE_RATE=$(resolve_fee_rate) || exit 2

# --- Build btc binary (skip if SKIP_BUILD=1 or BTC_BIN set) ---
build_btc() {
    if [[ -n "${BTC_BIN:-}" ]]; then
        return 0
    fi
    if [[ "${SKIP_BUILD:-0}" == "1" ]]; then
        return 0
    fi
    echo -e "${BLUE}Building btc...${RESET}" >&2
    ( cd "$(dirname "$0")/.." && cargo build -p btc ) >&2 || return 1
}

btc() {
    if [[ -n "${BTC_BIN:-}" ]]; then
        "$BTC_BIN" "$@"
    else
        ( cd "$(dirname "$0")/.." && cargo run --quiet -p btc -- "$@" )
    fi
}

build_btc || { echo -e "${RED}build failed${RESET}" >&2; exit 3; }

# --- Polling ---
poll_tx_confirmed() {
    local txid="$1"
    local deadline=$((SECONDS + BTC_E2E_POLL_TIMEOUT))
    while (( SECONDS < deadline )); do
        local status
        status=$(curl -fsS "$BTC_E2E_ESPLORA_URL/tx/$txid/status" 2>/dev/null \
            | grep -oE '"confirmed":(true|false)' || echo "")
        if [[ "$status" == *"true"* ]]; then
            return 0
        fi
        sleep 10
    done
    return 1
}

assert_recipient_paid() {
    local txid="$1" recipient="$2"
    # Esplora /tx/:txid/outspends returns an array; index 0 = vout 0.
    # For single-recipient sends, recipient is the vout 0 address.
    local outspends
    outspends=$(curl -fsS "$BTC_E2E_ESPLORA_URL/tx/$txid/outspends" 2>/dev/null) || return 1
    if [[ "$outspends" == "false"* ]] || [[ "$outspends" == *'"spent":true'* ]]; then
        # outspends endpoint reports spend status of outputs being spent LATER.
        # For our purposes, the recipient's vout is unspent at this point
        # (we just paid them) — what we want is confirmation + the tx
        # includes an output to recipient. Use /tx/:txid for output set.
        local tx_json
        tx_json=$(curl -fsS "$BTC_E2E_ESPLORA_URL/tx/$txid" 2>/dev/null) || return 1
        if [[ "$tx_json" == *"$recipient"* ]]; then
            return 0
        fi
    fi
    return 1
}

# --- Gate recorders ---
n_pass=0; n_fail=0; n_skip=0
record_step() {
    local status="$1" name="$2" detail="${3:-}"
    case "$status" in
        PASS) echo -e "  ${GREEN}✓ PASS${RESET} ${BOLD}$name${RESET} ${detail:+— $detail}"; n_pass=$((n_pass + 1)) ;;
        FAIL) echo -e "  ${RED}✗ FAIL${RESET} ${BOLD}$name${RESET} ${detail:+— $detail}"; n_fail=$((n_fail + 1)) ;;
        SKIP) echo -e "  ${YELLOW}○ SKIP${RESET} ${BOLD}$name${RESET} ${detail:+— $detail}"; n_skip=$((n_skip + 1)) ;;
    esac
}

# --- Gates ---
gate_single() {
    echo -e "${BOLD}=== Gate: single-recipient send (Story 5) ===${RESET}"
    echo "  BTC_E2E_STEP=$BTC_E2E_STEP (0=balance, 1=+send, 2=+poll, 3=+assert)"

    # STEP-0 (always): sender balance check — no broadcast.
    local balance
    balance=$(btc wallet balance \
        --mnemonic "$MNEMONIC_PHRASE" \
        --network testnet \
        --esplora-url "$BTC_E2E_ESPLORA_URL" \
        ${BTC_E2E_SPKI_PIN:+--pin-spki "$BTC_E2E_SPKI_PIN"} \
    ) || {
        record_step FAIL "balance check" "btc wallet balance returned non-zero"
        return 1
    }
    record_step PASS "balance check" "$balance sat"
    [[ "$BTC_E2E_STEP" -lt 1 ]] && return 0

    # STEP-1: send tx (captures txid; does NOT poll or assert yet).
    # STEP-1: re-enable for $BTC_E2E_STEP >= 1
    local txid
    txid=$(btc wallet send \
        --mnemonic "$MNEMONIC_PHRASE" \
        --network testnet \
        --address "$BTC_E2E_RECIPIENT" \
        --amount-sat "$BTC_E2E_AMOUNT_SAT" \
        --fee-rate "$FEE_RATE" \
        --esplora-url "$BTC_E2E_ESPLORA_URL" \
        ${BTC_E2E_SPKI_PIN:+--pin-spki "$BTC_E2E_SPKI_PIN"} \
    ) || {
        record_step FAIL "send" "btc wallet send returned non-zero"
        return 1
    }
    echo "$txid" > /tmp/btc-e2e-single-txid.txt
    echo "  txid: $txid"
    record_step PASS "send" "txid=$txid"
    [[ "$BTC_E2E_STEP" -lt 2 ]] && return 0

    # STEP-2: poll until confirmed.
    # STEP-2: re-enable for $BTC_E2E_STEP >= 2
    if poll_tx_confirmed "$txid"; then
        record_step PASS "confirmed" "txid=$txid"
    else
        record_step FAIL "confirmed" "txid=$txid (not confirmed within ${BTC_E2E_POLL_TIMEOUT}s)"
        return 1
    fi
    [[ "$BTC_E2E_STEP" -lt 3 ]] && return 0

    # STEP-3: assert recipient paid (full original gate).
    # STEP-3: re-enable for $BTC_E2E_STEP >= 3
    if assert_recipient_paid "$txid" "$BTC_E2E_RECIPIENT"; then
        record_step PASS "recipient received funds" "$BTC_E2E_RECIPIENT"
    else
        record_step FAIL "recipient NOT found in tx outputs" "$BTC_E2E_RECIPIENT"
        return 1
    fi
}

gate_multi() {
    echo -e "${BOLD}=== Gate: multi-recipient send (Story 13) ===${RESET}"
    record_step SKIP "multi-recipient send" "TODO: requires ≥2 funded testnet addresses; deferred to v1.0"
}

gate_drain() {
    echo -e "${BOLD}=== Gate: drain send (Story 14) ===${RESET}"
    record_step SKIP "drain send" "TODO: depends on gate_single succeeding first; deferred to v1.0"
}

gate_bumfee() {
    echo -e "${BOLD}=== Gate: RBF bump-fee (Story 17) ===${RESET}"
    record_step SKIP "RBF bump-fee" "TODO: depends on gate_single succeeding first; deferred to v1.0"
}

# --- Dispatch ---
echo -e "${BOLD}btc-send-testnet-e2e.sh${RESET} — L29 live testnet E2E"
echo "Mode:           $BTC_E2E_MODE"
echo "Step:           $BTC_E2E_STEP (0=balance, 1=+send, 2=+poll, 3=+assert)"
echo "Recipient:      $BTC_E2E_RECIPIENT"
echo "Amount (sat):   $BTC_E2E_AMOUNT_SAT"
echo "Fee rate:       $FEE_RATE sat/vB"
echo "Esplora:        $BTC_E2E_ESPLORA_URL"
echo "Mnemonic src:   $([[ -n "${BTC_E2E_MNEMONIC:-}" ]] && echo '<env>' || echo "file:$BTC_E2E_MNEMONIC_FILE")"
echo

case "$BTC_E2E_MODE" in
    single) gate_single ;;
    multi)  gate_multi ;;
    drain)  gate_drain ;;
    bumfee) gate_bumfee ;;
    all)
        gate_single
        gate_multi
        gate_drain
        gate_bumfee
        ;;
    *)
        echo "Unknown BTC_E2E_MODE: $BTC_E2E_MODE" >&2
        usage >&2
        exit 2
        ;;
esac

echo
echo -e "${BOLD}Summary:${RESET} pass=$n_pass fail=$n_fail skip=$n_skip"
if (( n_fail > 0 )); then
    exit 1
fi
exit 0