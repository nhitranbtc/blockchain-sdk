#!/usr/bin/env bash
#
# eth-send-sepolia-e2e.sh — L29 operator-driven Sepolia E2E for the 3
# sample tests in this spike (Issue #299 — ref samples for #298).
#
# Default invocation (no env): print usage + exit 0 without network calls.
#
# Required env under ETH_E2E_TESTNET=1:
#   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
#   ETH_E2E_MNEMONIC        BIP-39 sender phrase (12+ words)
#   ETH_E2E_MNEMONIC_FILE   alt: path to mnemonic, mode 0o600 enforced
#   ETH_E2E_TOKEN_ADDRESS  required only for e2e_sepolia_erc20_balance
#
# Optional env:
#   ETH_E2E_RECIPIENT       override recipient on send_native (default = m/44'/60'/0'/0/1)
#   ETH_E2E_TEST_TARGETS    space-separated subset to run
#                           default: balance send_native erc20_balance
#
# Exit codes:
#   0  all enabled targets PASS (or SKIP via #[ignore] on env)
#   1  at least one target FAIL
#   2  missing required env or invalid input
#   3  build failed

set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
SPIKE_DIR="$REPO_ROOT/rust-wallet-app/spikes/alloy-v1"

usage() {
    cat <<'EOF'
eth-send-sepolia-e2e.sh — L29 live-Sepolia E2E for spike samples.

Default (no env): print this usage, exit 0. No network calls.

Opt in:
  ETH_E2E_TESTNET=1 \
    ETH_E2E_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
    ETH_E2E_MNEMONIC_FILE=$HOME/.sepolia-test-mnem.txt \
    ETH_E2E_TOKEN_ADDRESS=0x1c7D4B196Cb0F7BB1D82a98fE3bfD0BfE4aEb287 \
    bash rust-wallet-app/scripts/eth-send-sepolia-e2e.sh

Targets (sub-tests):
  balance        Story 3  — read-only ETH balance check
  send_native    Story 5  — sign + broadcast 0.001 ETH transfer
  erc20_balance  Story 22 — ERC-20 balanceOf via sol! macro

Faucets:
  https://sepoliafaucet.com/
  https://www.alchemy.com/faucets/ethereum-sepolia
EOF
}

# --- Default: usage + exit (CI-safe) ---
if [[ "${ETH_E2E_TESTNET:-0}" != "1" ]]; then
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
if [[ -z "${ETH_E2E_RPC_URL:-}" ]]; then err_missing+=("ETH_E2E_RPC_URL"); fi
if [[ -z "${ETH_E2E_MNEMONIC:-}" && -z "${ETH_E2E_MNEMONIC_FILE:-}" ]]; then
    err_missing+=("ETH_E2E_MNEMONIC (or ETH_E2E_MNEMONIC_FILE)")
fi
if [[ -n "${ETH_E2E_MNEMONIC:-}" && -n "${ETH_E2E_MNEMONIC_FILE:-}" ]]; then
    echo -e "${RED}ETH_E2E_MNEMONIC and ETH_E2E_MNEMONIC_FILE are mutually exclusive${RESET}" >&2
    exit 2
fi
if [[ -n "${ETH_E2E_TEST_TARGETS:-}" ]] && [[ "${ETH_E2E_TEST_TARGETS:-}" == *"erc20_balance"* ]]; then
    if [[ -z "${ETH_E2E_TOKEN_ADDRESS:-}" ]]; then
        err_missing+=("ETH_E2E_TOKEN_ADDRESS (required when erc20_balance is in ETH_E2E_TEST_TARGETS)")
    fi
fi
if (( ${#err_missing[@]} > 0 )); then
    echo -e "${RED}Missing required env under ETH_E2E_TESTNET=1:${RESET}" >&2
    for v in "${err_missing[@]}"; do echo "  - $v" >&2; done
    exit 2
fi

# --- Resolve mnemonic (env or file with mode 0o600 enforcement) ---
resolve_mnemonic() {
    if [[ -n "${ETH_E2E_MNEMONIC:-}" ]]; then
        printf '%s' "$ETH_E2E_MNEMONIC"
        return 0
    fi
    local f="$ETH_E2E_MNEMONIC_FILE"
    if [[ ! -f "$f" ]]; then
        echo -e "${RED}ETH_E2E_MNEMONIC_FILE=$f does not exist${RESET}" >&2
        return 1
    fi
    local mode
    mode=$(stat -c '%a' "$f" 2>/dev/null || stat -f '%Lp' "$f" 2>/dev/null)
    if [[ "$mode" != "600" ]]; then
        echo -e "${RED}ETH_E2E_MNEMONIC_FILE=$f has mode $mode (expected 600).${RESET}" >&2
        echo -e "${RED}Refusing to read; run: chmod 600 $f${RESET}" >&2
        return 1
    fi
    <"$f" tr -d '\n'
}

if ! MNEMONIC_PHRASE=$(resolve_mnemonic); then
    exit 2
fi

# Export into the test process environment (sub-shells of cargo test inherit).
export ETH_E2E_RPC_URL
export ETH_E2E_MNEMONIC="$MNEMONIC_PHRASE"
export RUN_ETH_E2E=1
if [[ -n "${ETH_E2E_RECIPIENT:-}" ]]; then export ETH_E2E_RECIPIENT; fi
if [[ -n "${ETH_E2E_TOKEN_ADDRESS:-}" ]]; then export ETH_E2E_TOKEN_ADDRESS; fi

# --- Build the spike (skip if SKIP_BUILD=1) ---
if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
    echo -e "${BLUE}Building spike tests...${RESET}" >&2
    ( cd "$SPIKE_DIR" && cargo build --tests ) >&2 || {
        echo -e "${RED}spike build failed${RESET}" >&2
        exit 3
    }
fi

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

# --- Run one target ---
run_target() {
    local target="$1"
    local test_bin=""
    case "$target" in
        balance)       test_bin="e2e_sepolia_balance" ;;
        send_native)   test_bin="e2e_sepolia_send_native" ;;
        erc20_balance) test_bin="e2e_sepolia_erc20_balance" ;;
        *)
            echo -e "${RED}Unknown target: $target${RESET}" >&2
            return 1
            ;;
    esac

    echo -e "${BOLD}=== Target: $target ($test_bin) ===${RESET}"
    local log="/tmp/eth-e2e-${target}.log"
    if ( cd "$SPIKE_DIR" && cargo test --test "$test_bin" -- --ignored --nocapture ) >"$log" 2>&1; then
        record_step PASS "$target" "see $log"
    else
        record_step FAIL "$target" "see $log"
    fi
}

# --- Dispatch ---
: "${ETH_E2E_TEST_TARGETS:=balance send_native erc20_balance}"

echo -e "${BOLD}eth-send-sepolia-e2e.sh${RESET} — L29 live Sepolia E2E"
echo "Targets:        $ETH_E2E_TEST_TARGETS"
echo "RPC:            $ETH_E2E_RPC_URL"
echo "Mnemonic src:   $([[ -n "${ETH_E2E_MNEMONIC_FILE:-}" ]] && echo "file:$ETH_E2E_MNEMONIC_FILE" || echo '<env>')"
[[ -n "${ETH_E2E_TOKEN_ADDRESS:-}" ]] && echo "Token:          $ETH_E2E_TOKEN_ADDRESS"
[[ -n "${ETH_E2E_RECIPIENT:-}" ]] && echo "Recipient:      $ETH_E2E_RECIPIENT"
echo

for target in $ETH_E2E_TEST_TARGETS; do
    run_target "$target" || true
done

echo
echo -e "${BOLD}Summary:${RESET} pass=$n_pass fail=$n_fail skip=$n_skip"
if (( n_fail > 0 )); then
    exit 1
fi
exit 0
