#!/usr/bin/env bash
# scripts/polygon-wallet-scenario.sh — Issue #438 / Phase 4 T7 prep
#
# Operator-driven scenario driver for the `polygon` CLI wallet subcommands.
# Runs the same 4-command scenario the CI integration test exercises
# (polygon/tests/polygon_wallet_scenario.rs), but against either:
#   --env=anvil       — local Anvil (CI gating; fast, deterministic)
#   --env=amoy-fork   — forked Polygon Amoy testnet (operator smoke)
#
# The Anvil leg duplicates the cargo test only because the Amoy-fork
# leg needs a pre-funded signer + faucet drip that a cargo integration
# test cannot model cleanly. CI uses the cargo test (not this script)
# because the script needs shell + jq + curl + anvil binary on PATH.
#
# Per L29: live network smoke is operator-driven, not CI.
# Per #438 acceptance: script exits 0 only when every subcommand
# returns expected output.

set -euo pipefail

ENV_FLAG="${1:-}"
DATA_DIR="$(mktemp -d -t polygon-scenario-XXXXXX)"
PASSWORD="scenario-pw-ignore-leak"
WALLET_NAME="scenario-$(date +%s)"
AMOY_RPC="${POLYGON_AMOY_RPC:-https://polygon-amoy.drpc.org}"

cleanup() { rm -rf "$DATA_DIR"; }
trap cleanup EXIT

require() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "error: required tool '$1' not on PATH" >&2
        exit 3
    }
}

start_anvil() {
    if ! command -v anvil >/dev/null 2>&1; then
        echo "error: 'anvil' (foundry) not on PATH; install foundry.sh or use --env=amoy-fork" >&2
        exit 3
    fi
    # Polygon Amoy chain_id (80002) so the wallet created with
    # `--network amoy` matches the RPC chain (handlers verify chain_id
    # at send time per L13 critical-tier review).
    anvil --chain-id 80002 --balance 1000 --silent &
    ANVIL_PID=$!
    sleep 1
    ANVIL_RPC="http://127.0.0.1:8545"
}

assert_exit_0() {
    local label="$1"
    if [[ "$STATUS" -ne 0 ]]; then
        echo "FAIL: $label exited $STATUS" >&2
        echo "stderr: $STDERR" >&2
        exit 1
    fi
}

run_polygon() {
    local subcmd="$1"
    shift
    local out err
    set +e
    out=$(POLYGON_PASSWORD="$PASSWORD" \
          POLYGON_NETWORK=amoy \
          "$POLYGON_BIN" "$subcmd" "$@" \
              --data-dir "$DATA_DIR" \
              --network amoy \
              --rpc-url "$RPC_URL" 2>/tmp/poly.stderr)
    STATUS=$?
    set -e
    STDOUT="$out"
    STDERR="$(cat /tmp/poly.stderr)"
}

case "$ENV_FLAG" in
    --env=anvil)
        require curl
        start_anvil
        RPC_URL="$ANVIL_RPC"
        ;;
    --env=amoy-fork)
        RPC_URL="$AMOY_RPC"
        # Real Amoy signer funded via faucet — operator must set
        # $POLYGON_AMOY_PK (hex, no 0x prefix) before invoking.
        if [[ -z "${POLYGON_AMOY_PK:-}" ]]; then
            echo "error: set POLYGON_AMOY_PK (hex, no 0x) before running --env=amoy-fork" >&2
            echo "  (fund via https://faucet.polygon.technology first)" >&2
            exit 3
        fi
        ;;
    *)
        echo "usage: $0 --env=anvil | --env=amoy-fork" >&2
        exit 2
        ;;
esac

# Locate the polygon binary. cargo-built binary in target/ takes
# precedence; falls back to PATH.
POLYGON_BIN="$(command -v polygon || true)"
if [[ -z "$POLYGON_BIN" ]] && [[ -x "$(dirname "$0")/../target/debug/polygon" ]]; then
    POLYGON_BIN="$(dirname "$0")/../target/debug/polygon"
fi
if [[ -z "$POLYGON_BIN" ]]; then
    echo "error: 'polygon' binary not found (cargo build -p polygon first)" >&2
    exit 3
fi

echo "==> polygon wallet create --name $WALLET_NAME"
run_polygon wallet create --name "$WALLET_NAME"
assert_exit_0 "wallet create"
echo "$STDOUT"

# Extract address from stdout: "wallet created: name=X id=Y address=0xZ"
ADDR="$(echo "$STDOUT" | sed -nE 's/.*address=(0x[0-9a-fA-F]+).*/\1/p')"
if [[ -z "$ADDR" ]]; then
    echo "FAIL: could not parse address from wallet create stdout" >&2
    exit 1
fi

echo "==> polygon wallet list"
run_polygon wallet list
assert_exit_0 "wallet list"
echo "$STDOUT"
[[ "$STDOUT" == *"$WALLET_NAME"* ]] || {
    echo "FAIL: wallet list missing $WALLET_NAME" >&2
    exit 1
}

if [[ "$ENV_FLAG" == "--env=anvil" ]]; then
    echo "==> anvil_setBalance $ADDR (10 POL)"
    BAL_HEX="0x$(printf '%x' $((10 * 10**18)))"
    cast rpc anvil_setBalance "$ADDR" "$BAL_HEX" --rpc-url "$RPC_URL" >/dev/null \
        || curl -s -X POST -H 'content-type: application/json' \
              --data "{\"jsonrpc\":\"2.0\",\"method\":\"anvil_setBalance\",\"params\":[\"$ADDR\",\"$BAL_HEX\"],\"id\":1}" \
              "$RPC_URL" >/dev/null
fi

echo "==> polygon wallet balance --address $ADDR"
run_polygon wallet balance --address "$ADDR" --unit wei
assert_exit_0 "wallet balance"
echo "$STDOUT"

RECIPIENT="0x0000000000000000000000000000000000000042"
echo "==> polygon wallet send --name $WALLET_NAME --to $RECIPIENT --amount 0.001"
run_polygon wallet send --name "$WALLET_NAME" --to "$RECIPIENT" \
    --amount 0.001 --unit pol
assert_exit_0 "wallet send"
echo "$STDOUT"
[[ "$STDOUT" == *"tx_hash: 0x"* ]] || {
    echo "FAIL: wallet send stdout missing tx_hash line" >&2
    exit 1
}

echo "==> negative: polygon wallet send --to 0xnotavalidaddress (expect non-zero)"
run_polygon wallet send --name "$WALLET_NAME" --to 0xnotavalidaddress \
    --amount 0.001 --unit pol
[[ "$STATUS" -ne 0 ]] || {
    echo "FAIL: invalid --to should exit non-zero (got $STATUS)" >&2
    exit 1
}
echo "(exit $STATUS — expected)"

if [[ -n "${ANVIL_PID:-}" ]]; then
    kill "$ANVIL_PID" 2>/dev/null || true
fi

echo "==> scenario PASS"