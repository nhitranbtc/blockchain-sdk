#!/usr/bin/env bash
#
# polygon-send-amoy-e2e.sh — L29 operator-driven Amoy E2E for the polygon
# CLI integration-test binary `amoy_smoke` (Issue #464 / Phase 4 T7 of #416).
# Mirrors `eth-send-sepolia-e2e.sh` for the Polygon (PoS) chain.
#
# Default invocation (no env): print usage + exit 0 without network calls.
#
# Required env under POLYGON_AMOY=1:
#   POLYGON_AMOY_PK_FILE       mode-0600 file with Amoy-funded private key (hex)
#   POLYGON_AMOY_RECIPIENT     recipient address (any valid 0x...)
#
# Optional env:
#   POLYGON_AMOY_TIMEOUT_SECS  balance-poll timeout seconds (default 300, currently advisory)
#   POLYGON_AMOY_TEST_TARGETS  space-separated subset to run
#                              default: wallet_import faucet_url balance_after_funding
#                                       wallet_send fee_no_cache
#   SKIP_BUILD                 1 to skip `cargo build --tests -p polygon`
#
# Exit codes:
#   0  all enabled targets PASS
#   1  at least one target FAIL
#   2  missing required env or invalid input
#   3  build failed

set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
CRATE_DIR="$REPO_ROOT/rust-wallet-app/crates/polygon"

usage() {
    cat <<'EOF'
polygon-send-amoy-e2e.sh — L29 live-Amoy E2E for the polygon CLI amoy_smoke test binary.

Default (no env): print this usage, exit 0. No network calls.

Opt in:
  POLYGON_AMOY=1 \
    POLYGON_AMOY_PK_FILE=$HOME/.amoy-test-pk.hex \
    POLYGON_AMOY_RECIPIENT=0x0000000000000000000000000000000000000042 \
    bash rust-wallet-app/scripts/polygon-send-amoy-e2e.sh

Targets (one #[test] fn each under crates/polygon/tests/amoy_smoke.rs):
  wallet_import          Stories 1 + 9  — wallet import + list shows it
  faucet_url             Story 30       — polygon faucet prints canonical URL
  balance_after_funding  Story 3        — wallet balance > 0 POL (assumes funded PK)
  wallet_send            Story 5        — wallet send 0.01 POL broadcasts + receipt success
  fee_no_cache           Story 8        — polygon fee returns fresh estimate per call

Faucet (manual claim required if you use a fresh PK instead of pre-funded one):
  https://faucet.polygon.technology/
EOF
}

# --- Default: usage + exit (CI-safe) ---
if [[ "${POLYGON_AMOY:-0}" != "1" ]]; then
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
if [[ -z "${POLYGON_AMOY_PK_FILE:-}" ]]; then err_missing+=("POLYGON_AMOY_PK_FILE"); fi
if [[ -z "${POLYGON_AMOY_RECIPIENT:-}" ]]; then err_missing+=("POLYGON_AMOY_RECIPIENT"); fi
if [[ -n "${POLYGON_AMOY_PK_FILE:-}" && ! -f "${POLYGON_AMOY_PK_FILE}" ]]; then
    echo -e "${RED}POLYGON_AMOY_PK_FILE=${POLYGON_AMOY_PK_FILE} does not exist${RESET}" >&2
    exit 2
fi
if [[ -n "${POLYGON_AMOY_PK_FILE:-}" ]]; then
    pk_mode=$(stat -c '%a' "${POLYGON_AMOY_PK_FILE}" 2>/dev/null || stat -f '%Lp' "${POLYGON_AMOY_PK_FILE}" 2>/dev/null)
    if [[ "$pk_mode" != "600" ]]; then
        echo -e "${RED}POLYGON_AMOY_PK_FILE=${POLYGON_AMOY_PK_FILE} has mode $pk_mode (expected 600).${RESET}" >&2
        echo -e "${RED}Refusing to read; run: chmod 600 ${POLYGON_AMOY_PK_FILE}${RESET}" >&2
        exit 2
    fi
fi
if (( ${#err_missing[@]} > 0 )); then
    echo -e "${RED}Missing required env under POLYGON_AMOY=1:${RESET}" >&2
    for v in "${err_missing[@]}"; do echo "  - $v" >&2; done
    exit 2
fi

# --- Build the polygon test binary (skip if SKIP_BUILD=1) ---
if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
    echo -e "${BLUE}Building polygon tests...${RESET}" >&2
    ( cd "$CRATE_DIR" && cargo build --tests ) >&2 || {
        echo -e "${RED}polygon test build failed${RESET}" >&2
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
    local log="/tmp/polygon-amoy-e2e-${target}.log"

    echo -e "${BOLD}=== Target: $target ===${RESET}"
    if ( cd "$CRATE_DIR" && \
         RUN_POLYGON_AMOY=1 \
         POLYGON_AMOY_PK_FILE="$POLYGON_AMOY_PK_FILE" \
         POLYGON_AMOY_RECIPIENT="$POLYGON_AMOY_RECIPIENT" \
         cargo test --test amoy_smoke "$target" -- --ignored --nocapture ) >"$log" 2>&1; then
        record_step PASS "$target" "see $log"
    else
        record_step FAIL "$target" "see $log"
    fi
}

# --- Dispatch ---
: "${POLYGON_AMOY_TEST_TARGETS:=wallet_import faucet_url balance_after_funding wallet_send fee_no_cache}"

echo -e "${BOLD}polygon-send-amoy-e2e.sh${RESET} — L29 live Amoy E2E (Issue #464, Task 7)"
echo "Targets:        $POLYGON_AMOY_TEST_TARGETS"
echo "Crate:          $CRATE_DIR"
echo "PK file:        $POLYGON_AMOY_PK_FILE (mode 0600)"
echo "Recipient:      $POLYGON_AMOY_RECIPIENT"
echo

for target in $POLYGON_AMOY_TEST_TARGETS; do
    run_target "$target" || true
done

echo
echo -e "${BOLD}Summary:${RESET} pass=$n_pass fail=$n_fail skip=$n_skip"
if (( n_fail > 0 )); then
    exit 1
fi
exit 0
