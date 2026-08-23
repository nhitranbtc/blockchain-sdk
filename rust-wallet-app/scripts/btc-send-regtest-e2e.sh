#!/usr/bin/env bash
#
# btc-send-regtest-e2e.sh — Local-regtest end-to-end smoke for
# `btc wallet send` (sub-task of Issue #283 / #284).
#
# Validates the full operator flow on a real local Bitcoin network:
#   bitcoind + Esplora (regtest)  →  fund sender  →  send  →
#   mine 1 block  →  poll confirmation  →  assert recipient paid
#
# NO testnet, NO faucet, NO SPKI pin (F20 waived on regtest).
# NO external network beyond the operator's localhost.
#
# Default invocation (no env) prints usage + exits 0; no side effects.
# CI-safe: never invokes `bitcoin-cli` or `btc` without opt-in.
#
# Required env under BTC_E2E_REGTEST=1:
#   BTC_E2E_REGTEST=1              master gate
#   BTC_RPC_URL                    default http://localhost:18443
#   BTC_RPC_USER                   default foo
#   BTC_RPC_PASS                   default bar
#   BTC_RPC_CLI                    default `bitcoin-cli` (on PATH)
#   BTC_ESPLORA_URL                default http://localhost:50001/regtest/api
#   BTC_E2E_MNEMONIC               OR
#   BTC_E2E_MNEMONIC_FILE          path to file (mode 0o600 enforced)
#   BTC_E2E_RECIPIENT              default: sender address (self-send)
#   BTC_E2E_AMOUNT_SAT             default 100000 (= 0.001 BTC)
#   BTC_E2E_NETWORK                default regtest
#   BTC_E2E_FEE_RATE_SAT_PER_VB    default 1
#   BTC_E2E_POLL_TIMEOUT           default 60 (seconds)
#   BTC_E2E_FUND_MIN_CONFIRMATIONS default 1
#   SKIP_BUILD=1                   use pre-built btc binary
#   BTC_BIN=/path/to/btc           override btc binary location
#
# Exit codes:
#   0  all steps PASS
#   1  at least one step FAIL
#   2  missing required env or invalid input
#   3  bitcoind unreachable / regtest not synced
#   4  build failed

set -uo pipefail

usage() {
    cat <<'EOF'
btc-send-regtest-e2e.sh — Local-regtest E2E smoke for btc wallet send.

Default (no env): print this usage, exit 0. No side effects.

Opt in:
  BTC_E2E_REGTEST=1 BTC_E2E_MNEMONIC="abandon ... about" \
    bash rust-wallet-app/scripts/btc-send-regtest-e2e.sh

Or with a mnemonic file:
  BTC_E2E_REGTEST=1 BTC_E2E_MNEMONIC_FILE=/path/to/mnem.txt \
    bash rust-wallet-app/scripts/btc-send-regtest-e2e.sh
EOF
}

# Auto-source the operator env file if present. Lets the operator
# run `bash btc-send-regtest-e2e.sh` without manually running
# `set -a; source e2e-regtest.env; set +a` first. Skipped silently
# when the file doesn't exist. Runs BEFORE the opt-in check so the
# env's BTC_E2E_REGTEST=1 is honored.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [[ -f "$SCRIPT_DIR/e2e-regtest.env" ]]; then
    set -a
    # shellcheck disable=SC1091
    source "$SCRIPT_DIR/e2e-regtest.env"
    set +a
fi

# --- Default: usage + exit (CI-safe) ---
if [[ "${BTC_E2E_REGTEST:-0}" != "1" ]]; then
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

# --- Apply defaults ---
: "${BTC_RPC_URL:=http://localhost:18443}"
: "${BTC_RPC_USER:=foo}"
: "${BTC_RPC_PASS:=bar}"
: "${BTC_RPC_CLI:=bitcoin-cli}"
: "${BTC_ESPLORA_URL:=http://localhost:50001/regtest/api}"
: "${BTC_E2E_AMOUNT_SAT:=100000}"
: "${BTC_E2E_NETWORK:=regtest}"
: "${BTC_E2E_FEE_RATE_SAT_PER_VB:=1}"
: "${BTC_E2E_POLL_TIMEOUT:=60}"
: "${BTC_E2E_FUND_MIN_CONFIRMATIONS:=1}"

# --- Validate required env ---
err_missing=()
if [[ -z "${BTC_E2E_MNEMONIC:-}" && -z "${BTC_E2E_MNEMONIC_FILE:-}" ]]; then
    err_missing+=("BTC_E2E_MNEMONIC (or BTC_E2E_MNEMONIC_FILE)")
fi
if [[ -n "${BTC_E2E_MNEMONIC:-}" && -n "${BTC_E2E_MNEMONIC_FILE:-}" ]]; then
    echo -e "${RED}BTC_E2E_MNEMONIC and BTC_E2E_MNEMONIC_FILE are mutually exclusive${RESET}" >&2
    exit 2
fi
if (( ${#err_missing[@]} > 0 )); then
    echo -e "${RED}Missing required env under BTC_E2E_REGTEST=1:${RESET}" >&2
    for v in "${err_missing[@]}"; do echo "  - $v" >&2; done
    exit 2
fi

# --- Resolve mnemonic (env or file, mode 600 enforced) ---
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
    local mode
    mode=$(stat -c '%a' "$f" 2>/dev/null || stat -f '%Lp' "$f" 2>/dev/null)
    if [[ "$mode" != "600" ]]; then
        echo -e "${RED}BTC_E2E_MNEMONIC_FILE=$f has mode $mode (expected 600). Refusing.${RESET}" >&2
        return 1
    fi
    <"$f" tr -d '\n'
}

MNEMONIC_PHRASE=$(resolve_mnemonic) || exit 2

# --- Helpers ---
n_pass=0; n_fail=0; n_skip=0
record_step() {
    local status="$1" name="$2" detail="${3:-}"
    case "$status" in
        PASS) echo -e "  ${GREEN}✓ PASS${RESET} ${BOLD}$name${RESET} ${detail:+— $detail}"; n_pass=$((n_pass + 1)) ;;
        FAIL) echo -e "  ${RED}✗ FAIL${RESET} ${BOLD}$name${RESET} ${detail:+— $detail}"; n_fail=$((n_fail + 1)) ;;
        SKIP) echo -e "  ${YELLOW}○ SKIP${RESET} ${BOLD}$name${RESET} ${detail:+— $detail}"; n_skip=$((n_skip + 1)) ;;
    esac
}

# Per-step log line. Surfaces every action (including intermediate btc
# invocations) so an operator can see exactly which subcommand failed
# when a gate FAILs. Timestamp prefix helps correlate with Docker
# container logs when BTC_DOCKER_CONTAINER is set.
log_step() {
    local msg="$1"
    printf '  %s %s\n' "$(date '+%H:%M:%S')" "$msg" >&2
}

# bitcoin-cli wrapper. L12: pass user/pass via --rpcuser/--rpcpassword
# flags, NOT userinfo-in-URL (see Issue #35).
#
# If `BTC_DOCKER_CONTAINER` is set, route the call through
# `docker exec -i <container>` — useful when bitcoind runs in a
# container but the script runs on the host (no host bitcoin-cli).
bitcoin_cli() {
    local prefix=()
    if [[ -n "${BTC_DOCKER_CONTAINER:-}" ]]; then
        prefix=(docker exec -i "$BTC_DOCKER_CONTAINER" )
    fi
    # Parse host:port from BTC_RPC_URL. -rpcconnect takes host only;
    # -rpcport takes the port.
    local rpc_host rpc_port
    rpc_host=$(echo "$BTC_RPC_URL" | sed -E 's#^https?://##; s#:[0-9]+$##')
    rpc_port=$(echo "$BTC_RPC_URL" | sed -E 's#^https?://##; s#.*:##')
    [[ "$rpc_port" == "$rpc_host" ]] && rpc_port=18443  # no port in URL
    # When running via docker exec, force loopback (the container's
    # own bitcoind) — `localhost` may not resolve in some containers.
    if [[ -n "${BTC_DOCKER_CONTAINER:-}" ]]; then
        rpc_host=127.0.0.1
    fi
    "${prefix[@]}" "$BTC_RPC_CLI" \
        -regtest \
        -rpcuser="$BTC_RPC_USER" \
        -rpcpassword="$BTC_RPC_PASS" \
        -rpcconnect="$rpc_host" \
        -rpcport="$rpc_port" \
        "$@"
}

# btc CLI wrapper (resolves to prebuilt binary or cargo run).
btc() {
    if [[ -n "${BTC_BIN:-}" ]]; then
        "$BTC_BIN" "$@"
    else
        ( cd "$(dirname "$0")/.." && cargo run --quiet -p btc -- "$@" )
    fi
}

# Wait for bitcoin-cli to return a valid response. Fails the script
# (exit 3) if the daemon is unreachable or regtest isn't ready.
wait_for_bitcoind() {
    local deadline=$((SECONDS + 30))
    while (( SECONDS < deadline )); do
        if bitcoin_cli getblockchaininfo >/dev/null 2>&1; then
            return 0
        fi
        sleep 2
    done
    echo -e "${RED}bitcoind unreachable at $BTC_RPC_URL after 30s${RESET}" >&2
    exit 3
}

# Ensure a default wallet is loaded. bitcoind 24+ no longer creates one
# automatically; without a wallet, getnewaddress / generatetoaddress fail.
ensure_wallet() {
    local wallets
    wallets=$(bitcoin_cli listwallets 2>/dev/null | grep -oE '"[^"]+"' | tr -d '"' | head -1)
    if [[ -n "$wallets" ]]; then
        return 0
    fi
    bitcoin_cli createwallet "default" >/dev/null 2>&1 || {
        echo -e "${RED}createwallet default failed${RESET}" >&2
        return 1
    }
    echo "  created wallet: default"
}

# Mine `n` blocks to a fresh address (for confirmations).
mine_blocks() {
    local n="$1"
    local new_addr
    new_addr=$(bitcoin_cli getnewaddress) || return 1
    bitcoin_cli generatetoaddress "$n" "$new_addr" >/dev/null || return 1
    echo "$new_addr"
}

# Poll Esplora until txid is confirmed (or timeout).
poll_tx_confirmed() {
    local txid="$1"
    local deadline=$((SECONDS + BTC_E2E_POLL_TIMEOUT))
    while (( SECONDS < deadline )); do
        local status
        status=$(curl -fsS "$BTC_ESPLORA_URL/tx/$txid/status" 2>/dev/null \
            | grep -oE '"confirmed":(true|false)' || echo "")
        if [[ "$status" == *"true"* ]]; then
            return 0
        fi
        sleep 2
    done
    return 1
}

# Assert tx output set contains the recipient address.
assert_recipient_paid() {
    local txid="$1" recipient="$2"
    local tx_json
    tx_json=$(curl -fsS "$BTC_ESPLORA_URL/tx/$txid" 2>/dev/null) || return 1
    [[ "$tx_json" == *"$recipient"* ]]
}

# --- Pre-flight ---
echo -e "${BOLD}btc-send-regtest-e2e.sh${RESET} — local regtest E2E"
echo "Network:    $BTC_E2E_NETWORK"
echo "RPC:        $BTC_RPC_URL"
echo "Esplora:    $BTC_ESPLORA_URL"
echo "Amount:     $BTC_E2E_AMOUNT_SAT sat"
echo "Fee rate:   $BTC_E2E_FEE_RATE_SAT_PER_VB sat/vB"
echo "Mnemonic:   $([[ -n "${BTC_E2E_MNEMONIC:-}" ]] && echo '<env>' || echo "file:$BTC_E2E_MNEMONIC_FILE")"
echo

log_step "wait_for_bitcoind: probing $BTC_RPC_URL via ${BTC_DOCKER_CONTAINER:-host}"
wait_for_bitcoind
log_step "ensure_wallet: bitcoind 24+ doesn't auto-create; checking listwallets"
ensure_wallet
log_step "preflight complete (Esplora at $BTC_ESPLORA_URL must be reachable; bring it up out-of-band)"

# --- Steps ---

# Step 0: Derive sender address + initial balance
echo -e "${BOLD}=== Step 0: sync + balance ===${RESET}"
log_step "0.1 btc wallet sync (network=$BTC_E2E_NETWORK, esplora=$BTC_ESPLORA_URL)"
sync_out=$(btc wallet sync \
    --mnemonic "$MNEMONIC_PHRASE" \
    --network "$BTC_E2E_NETWORK" \
    --esplora-url "$BTC_ESPLORA_URL" 2>&1) || {
    record_step FAIL "btc wallet sync" "see stderr above"
    exit 1
}
SENDER_ADDR=$(echo "$sync_out" | grep -oE 'tb1q[a-z0-9]{38,}|bc1q[a-z0-9]{38,}|2[A-Za-z0-9]{33,}|m[A-Za-z0-9]{33,}|n[A-Za-z0-9]{33,}' | head -1)
[[ -z "$SENDER_ADDR" ]] && {
    record_step FAIL "derive sender address" "no address in sync output"
    exit 1
}
log_step "0.2 derived sender address: $SENDER_ADDR"
record_step PASS "sync + derive sender address" "$SENDER_ADDR"

log_step "0.3 btc wallet balance"
BALANCE=$(btc wallet balance \
    --mnemonic "$MNEMONIC_PHRASE" \
    --network "$BTC_E2E_NETWORK" \
    --esplora-url "$BTC_ESPLORA_URL" 2>&1) || {
    record_step FAIL "btc wallet balance" "see stderr above"
    exit 1
}
BALANCE_SAT=$(echo "$BALANCE" | grep -oE '[0-9]+' | head -1)
[[ -z "$BALANCE_SAT" ]] && BALANCE_SAT=0
log_step "0.4 initial balance: ${BALANCE_SAT} sat"
record_step PASS "initial balance" "${BALANCE_SAT} sat"

# Default recipient to sender (self-send) when unspecified
: "${BTC_E2E_RECIPIENT:=$SENDER_ADDR}"
log_step "0.5 recipient: $BTC_E2E_RECIPIENT (defaulted from sender if unset)"
echo "Recipient: $BTC_E2E_RECIPIENT"

# Step 1: Fund sender if balance insufficient
echo -e "${BOLD}=== Step 1: fund sender (if needed) ===${RESET}"
REQUIRED=$((BTC_E2E_AMOUNT_SAT + BTC_E2E_FEE_RATE_SAT_PER_VB * 200))  # rough upper bound
log_step "1.1 balance=${BALANCE_SAT} sat, required=${REQUIRED} sat (amount + ~200 vbytes fee)"
if (( BALANCE_SAT >= REQUIRED )); then
    record_step SKIP "fund sender" "balance ${BALANCE_SAT} >= required ${REQUIRED}"
else
    log_step "1.2 mining 101 blocks to mature coinbase"
    mine_blocks 101 >/dev/null || {
        record_step FAIL "mine 101 blocks" "bitcoin-cli generatetoaddress failed"
        exit 1
    }
    log_step "1.3 coinbase matured (101 confirmations)"
    record_step PASS "mine 101 blocks" "coinbase matured"

    FUND_AMT_BTC=$(awk "BEGIN { printf \"%.8f\", ${BTC_E2E_AMOUNT_SAT}/100000000 + 0.001 }")
    log_step "1.4 bitcoin-cli sendtoaddress $SENDER_ADDR $FUND_AMT_BTC"
    TXID_FUND=$(bitcoin_cli sendtoaddress "$SENDER_ADDR" "$FUND_AMT_BTC") || {
        record_step FAIL "sendtoaddress" "bitcoin-cli sendtoaddress failed"
        exit 1
    }
    log_step "1.5 funding tx: $TXID_FUND"
    record_step PASS "sendtoaddress" "txid=$TXID_FUND amount=$FUND_AMT_BTC"

    log_step "1.6 mining $BTC_E2E_FUND_MIN_CONFIRMATIONS confirmation block(s)"
    mine_blocks "$BTC_E2E_FUND_MIN_CONFIRMATIONS" >/dev/null || {
        record_step FAIL "mine confirmation block" "bitcoin-cli generatetoaddress failed"
        exit 1
    }
    log_step "1.7 funding confirmed"
    record_step PASS "mine 1 confirmation block" "funded"
fi

# Step 2: Send (capture txid)
echo -e "${BOLD}=== Step 2: send ===${RESET}"
log_step "2.1 btc wallet send (recipient=$BTC_E2E_RECIPIENT, amount=$BTC_E2E_AMOUNT_SAT sat, fee=$BTC_E2E_FEE_RATE_SAT_PER_VB sat/vB)"
TXID=$(btc wallet send \
    --mnemonic "$MNEMONIC_PHRASE" \
    --network "$BTC_E2E_NETWORK" \
    --address "$BTC_E2E_RECIPIENT" \
    --amount-sat "$BTC_E2E_AMOUNT_SAT" \
    --fee-rate "$BTC_E2E_FEE_RATE_SAT_PER_VB" \
    --esplora-url "$BTC_ESPLORA_URL" 2>&1) || {
    record_step FAIL "btc wallet send" "see stderr above"
    exit 1
}
# Capture only the txid (last 64-hex line)
TXID=$(echo "$TXID" | grep -oE '[0-9a-f]{64}' | tail -1)
[[ ${#TXID} -ne 64 ]] && {
    record_step FAIL "parse txid" "got: $TXID"
    exit 1
}
log_step "2.2 captured txid: $TXID"
echo "  txid: $TXID"
record_step PASS "send" "txid=$TXID"

# Step 3: Mine 1 block (confirm) + poll
echo -e "${BOLD}=== Step 3: mine + poll confirmation ===${RESET}"
log_step "3.1 mining 1 confirmation block"
mine_blocks 1 >/dev/null || {
    record_step FAIL "mine 1 block" "bitcoin-cli generatetoaddress failed"
    exit 1
}
log_step "3.2 block mined; polling Esplora /tx/$TXID/status (timeout=${BTC_E2E_POLL_TIMEOUT}s)"
record_step PASS "mine 1 block" "block mined"

if poll_tx_confirmed "$TXID"; then
    log_step "3.3 tx confirmed"
    record_step PASS "tx confirmed" "txid=$TXID"
else
    record_step FAIL "tx confirmed" "not confirmed within ${BTC_E2E_POLL_TIMEOUT}s"
    exit 1
fi

# Step 4: Assert recipient paid
echo -e "${BOLD}=== Step 4: assert recipient paid ===${RESET}"
log_step "4.1 GET $BTC_ESPLORA_URL/tx/$TXID (assert recipient in vout set)"
if assert_recipient_paid "$TXID" "$BTC_E2E_RECIPIENT"; then
    log_step "4.2 recipient found in tx outputs"
    record_step PASS "recipient received funds" "$BTC_E2E_RECIPIENT"
else
    record_step FAIL "recipient NOT in tx outputs" "$BTC_E2E_RECIPIENT"
    exit 1
fi

# Step 5: Sanity check fee-estimates
echo -e "${BOLD}=== Step 5: btc fee-estimates ===${RESET}"
log_step "5.1 btc fee-estimates --network $BTC_E2E_NETWORK"
if btc fee-estimates --network "$BTC_E2E_NETWORK" --esplora-url "$BTC_ESPLORA_URL" 2>&1 | grep -qE '[0-9]+'; then
    log_step "5.2 fee-estimates returned numeric output"
    record_step PASS "btc fee-estimates" "see output above"
else
    record_step FAIL "btc fee-estimates" "no numeric output"
fi

echo
echo -e "${BOLD}Summary:${RESET} pass=$n_pass fail=$n_fail skip=$n_skip"
echo "  txid: $TXID"
echo "  sender: $SENDER_ADDR"
echo "  recipient: $BTC_E2E_RECIPIENT"
echo "  amount: $BTC_E2E_AMOUNT_SAT sat"
[[ -n "${TXID_FUND:-}" ]] && echo "  funding tx: $TXID_FUND"
if (( n_fail > 0 )); then
    exit 1
fi
exit 0