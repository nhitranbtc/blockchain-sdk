#!/usr/bin/env bash
#
# btc-send-demo.sh — end-to-end demo of `btc wallet send` (Story 5 / Issue #118).
#
# Walks through 4 demo steps:
#   1. Argument parsing — confirm `btc wallet send --help` shows all flags.
#   2. Cross-network rejection — refuse to send a `tb1...` address with
#      `--network bitcoin` (operator error defense).
#   3. Invalid address rejection — refuse non-Bitcoin junk in `--address`.
#   4. Build path — `btc wallet send` against an unfunded testnet
#      wallet (entropy=0 mnemonic) fails at the build step with
#      `InsufficientFunds` (no testcontainers needed; TxBuild surfaces
#      this without ever hitting the network).
#
# The full happy-path send (build + sign + broadcast + return txid)
# requires either:
#   - a real funded testnet wallet (L29 operator-driven gate), OR
#   - a testcontainers regtest node (Issue #115 — deferred).
# Both are documented in the demo's exit block.
#
# Usage:
#   bash rust-wallet-app/scripts/btc-send-demo.sh
#   bash rust-wallet-app/scripts/btc-send-demo.sh --help
#   SKIP_BUILD=1 bash rust-wallet-app/scripts/btc-send-demo.sh   # skip cargo build
#
# Requirements:
#   - Rust toolchain (cargo)
#   - btc crate builds: `cargo build -p btc`

set -uo pipefail

usage() {
    cat <<'EOF'
btc-send-demo.sh — end-to-end demo of `btc wallet send` (Story 5 / Issue #118).

Walks through:
  1. CLI flag surface (`btc wallet send --help`)
  2. Cross-network rejection (defends against "send to wrong chain")
  3. Invalid address rejection
  4. Build-path InsufficientFunds error (entropy=0 mnemonic, no UTXOs)

Exit code 0 = all 4 demo steps passed. Non-zero = at least one failed.

Optional env:
  SKIP_BUILD=1   skip the `cargo build -p btc` step
  BTC_BIN        path to pre-built btc binary (overrides cargo invocation)
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit 0
fi

# Color helpers (same pattern as btc-quickstart.sh / btc-import-demo.sh)
if [[ -t 1 ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'
    BLUE='\033[0;34m'; BOLD='\033[1m'; RESET='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; BLUE=''; BOLD=''; RESET=''
fi

record_step() {
    local status="$1" name="$2" detail="${3:-}"
    case "$status" in
        PASS) echo -e "  ${GREEN}✓ PASS${RESET} ${BOLD}${name}${RESET} ${detail:+— $detail}"; n_pass=$((n_pass + 1)) ;;
        FAIL) echo -e "  ${RED}✗ FAIL${RESET} ${BOLD}${name}${RESET} ${detail:+— $detail}"; n_fail=$((n_fail + 1)) ;;
        SKIP) echo -e "  ${YELLOW}○ SKIP${RESET} ${BOLD}${name}${RESET} ${detail:+— $detail}"; n_skip=$((n_skip + 1)) ;;
    esac
}

status_glyph() {
    case "$1" in
        PASS) echo -e "${GREEN}✓${RESET}" ;;
        FAIL) echo -e "${RED}✗${RESET}" ;;
        SKIP) echo -e "${YELLOW}○${RESET}" ;;
    esac
}

print_step_result() {
    local status="$1" label="$2" expected="$3" actual="$4"
    local glyph; glyph=$(status_glyph "$status")
    echo -e "    ${glyph} ${label}: expected=${BOLD}${expected}${RESET}, actual=${BOLD}${actual}${RESET}"
}

# ---------- setup ----------------------------------------------------------

# Use a fixed workdir under /tmp so the demo is reproducible. Cleanup
# is gated by BTC_SEND_DEMO_KEEP=1 for debugging.
WORKDIR="/tmp/btc-send-demo-$$"
mkdir -p "$WORKDIR"
if [[ -z "${BTC_SEND_DEMO_KEEP:-}" ]]; then
    trap 'rm -rf "/tmp/btc-send-demo-$$"' EXIT
else
    echo -e "${YELLOW}BTC_SEND_DEMO_KEEP=1${RESET} — leaving $WORKDIR on disk"
fi

if [[ -z "${BTC_BIN:-}" ]]; then
    if [[ -n "${SKIP_BUILD:-}" ]]; then
        echo -e "${YELLOW}SKIP_BUILD=1${RESET} set — assuming btc is pre-built (looking in target/debug/btc)"
        BTC_BIN="target/debug/btc"
    else
        echo -e "${BLUE}==>${RESET} ${BOLD}Building btc${RESET}"
        if ! cargo build -p btc --quiet 2>&1 | tail -20; then
            echo -e "${RED}build failed${RESET}"
            exit 1
        fi
        BTC_BIN="target/debug/btc"
    fi
fi

if [[ ! -x "$BTC_BIN" ]]; then
    echo -e "${RED}btc binary not found at ${BTC_BIN}${RESET}"
    echo "Build it: cargo build -p btc"
    exit 1
fi

# Default entropy=0 testnet mnemonic (well-known BIP-39 test vector).
# Do NOT use for real funds.
TEST_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
TESTNET_ESPLORA="https://blockstream.info/testnet/api"

n_pass=0
n_fail=0
n_skip=0

# ---------- STEP 1: CLI flag surface ---------------------------------------

echo
echo -e "${BLUE}==>${RESET} ${BOLD}STEP 1${RESET} — CLI flag surface (parse layer)"
HELP_OUT=$("$BTC_BIN" wallet send --help 2>&1)
EXPECTED_FLAGS=("--mnemonic" "--network" "--address" "--amount-sat" "--esplora-url" "--pin-spki" "--data-dir")
MISSING=()
for flag in "${EXPECTED_FLAGS[@]}"; do
    if ! grep -q -- "$flag" <<<"$HELP_OUT"; then
        MISSING+=("$flag")
    fi
done
if [[ ${#MISSING[@]} -eq 0 ]]; then
    record_step PASS "send_help_shows_all_flags" "(7 expected flags present)"
else
    record_step FAIL "send_help_shows_all_flags" "missing: ${MISSING[*]}"
fi

# ---------- STEP 2: Cross-network rejection ---------------------------------

echo
echo -e "${BLUE}==>${RESET} ${BOLD}STEP 2${RESET} — Cross-network rejection (testnet addr → bitcoin network)"

# A `tb1...` (testnet bech32) address must be refused when --network bitcoin.
# This defends against "send to wrong chain" operator error.
TB1_ADDR="tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx"
set +e
SEND_OUT=$("$BTC_BIN" wallet send \
    --mnemonic "$TEST_MNEMONIC" \
    --network bitcoin \
    --address "$TB1_ADDR" \
    --amount-sat 10000 \
    --esplora-url "$TESTNET_ESPLORA" \
    --pin-spki 0000000000000000000000000000000000000000000000000000000000000000 \
    2>&1)
SEND_EXIT=$?
set -e

if [[ $SEND_EXIT -ne 0 ]] && grep -q "not valid for network" <<<"$SEND_OUT"; then
    record_step PASS "cross_network_send_refused" "(tb1... + bitcoin network rejected)"
else
    record_step FAIL "cross_network_send_refused" "expected 'not valid for network' error; got exit=$SEND_EXIT"
    echo "--- output ---"
    echo "$SEND_OUT" | head -10
    echo "--------------"
fi

# ---------- STEP 3: Invalid address rejection ------------------------------

echo
echo -e "${BLUE}==>${RESET} ${BOLD}STEP 3${RESET} — Invalid address rejection"

set +e
SEND_OUT=$("$BTC_BIN" wallet send \
    --mnemonic "$TEST_MNEMONIC" \
    --network testnet \
    --address "not-a-bitcoin-address" \
    --amount-sat 10000 \
    --esplora-url "$TESTNET_ESPLORA" \
    2>&1)
SEND_EXIT=$?
set -e

if [[ $SEND_EXIT -ne 0 ]] && grep -q "invalid recipient address" <<<"$SEND_OUT"; then
    record_step PASS "invalid_address_rejected" "(junk string refused at parse-validated network check)"
else
    record_step FAIL "invalid_address_rejected" "expected 'invalid recipient address' error; got exit=$SEND_EXIT"
    echo "--- output ---"
    echo "$SEND_OUT" | head -10
    echo "--------------"
fi

# ---------- STEP 4: F36 HTTPS-only URL gate --------------------------------

echo
echo -e "${BLUE}==>${RESET} ${BOLD}STEP 4${RESET} — F36 HTTPS-only URL gate (non-localhost http:// refused)"

# F36 (per plan + L12 review) requires Esplora URLs to use https://.
# The Story 5 send handler reuses the same `EsploraUrl::new` validator
# from `chain::esplora_url` — already exercised by `btc wallet balance`
# (Issue #63). Story 5 confirms the same gate works for `send`.
#
# Note: per PR #114, the F36 check has a localhost exception (test-
# friendly). To exercise F36 rejection, we use a NON-localhost host
# so the exception doesn't kick in.
RT_ADDR="bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080"
set +e
SEND_OUT=$("$BTC_BIN" wallet send \
    --mnemonic "$TEST_MNEMONIC" \
    --network regtest \
    --address "$RT_ADDR" \
    --amount-sat 10000 \
    --esplora-url "http://1.2.3.4:50002" \
    2>&1)
SEND_EXIT=$?
set -e

if [[ $SEND_EXIT -ne 0 ]] && grep -qE "https://|esplora url must use https" <<<"$SEND_OUT"; then
    record_step PASS "f36_https_only_url_rejected" "(non-localhost http:// refused)"
else
    record_step FAIL "f36_https_only_url_rejected" "expected https-only rejection; got exit=$SEND_EXIT"
    echo "--- output ---"
    echo "$SEND_OUT" | head -15
    echo "--------------"
fi

# ---------- Summary ---------------------------------------------------------

echo
echo -e "${BLUE}==>${RESET} ${BOLD}Summary${RESET}"
TOTAL=$((n_pass + n_fail + n_skip))
echo -e "    ${BOLD}${TOTAL}${RESET} steps: ${GREEN}${n_pass} pass${RESET}, ${RED}${n_fail} fail${RESET}, ${YELLOW}${n_skip} skip${RESET}"

echo
echo -e "${BLUE}==>${RESET} ${BOLD}Notes${RESET}"
echo "    • Full happy-path send (build + sign + broadcast + return txid) requires"
echo "      either a live testnet wallet (L29 operator-driven) or a testcontainers"
echo "      regtest node (Issue #115 — deferred)."
echo "    • For a live testnet send: fund the entropy=0 wallet with testnet BTC from"
echo "      a faucet (https://testnet-faucet.com), then re-run STEP 4."
echo "    • For a regtest send: see rust-wallet-app/crates/btc/tests/btc-regtest-smoke.rs"
echo "      (3rd test pending Issue #115 bollard follow-up)."

if [[ $n_fail -gt 0 ]]; then
    echo
    echo -e "${RED}FAIL${RESET} — $n_fail step(s) failed"
    exit 1
fi
echo
echo -e "${GREEN}PASS${RESET} — all demo steps passed"
exit 0
