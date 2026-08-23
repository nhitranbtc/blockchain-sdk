#!/usr/bin/env bash
#
# tests/spin-up.sh — acceptance test for #286 path B (custom regtest Esplora
# image, sub-issue #288 sub-task 286a).
#
# Three assertions cover the full canonical btc CLI path against the
# locally-built regtest Esplora image (e2e-esplora-regtest:local):
#
#   1. GET  /regtest/api/blocks/tip/height   → real int
#   2. btc wallet balance  --esplora-url ... → integer sats (no fallback)
#   3. btc wallet sync     --esplora-url ... → n_utxos + total_sat (no error URL)
#
# RED-state rationale: this test was written BEFORE Dockerfile.esplora-regtest
# and the compose build: directive land (per L13 step 9 TDD red-first).
# Baseline run on the pre-existing compose (which references
# blockstream/esplora:latest) fails at step 1 because that image exits on cold
# start per issue #286. GREEN-state after Dockerfile.esplora-regtest, shim.py,
# and the compose service swap all ship — all three assertions pass.
#
# L29 manual-smoke gate: this script also serves as the operator-driven
# pre-merge gate per L29. CI must never run it (requires docker + compose + btc
# binary on PATH + a fresh regtest datadir). For operator confirmation, the
# `L29 manual smoke` boxes in #288 stay `[ ]` until the operator runs this and
# reports back.
#
# Usage:
#   bash tests/spin-up.sh
#
# Overrides (env vars):
#   BTC_ESPLORA_URL   default http://localhost:50001/regtest/api
#   BTC_TEST_MNEMONIC default "abandon x11 about" (BIP-39 test vector 1)
#   BTC_BIN           default $SCRIPT_DIR/../../../target/debug/btc

set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$SCRIPT_DIR"

ESPLORA_URL="${BTC_ESPLORA_URL:-http://localhost:50001/regtest/api}"
MNEMONIC="${BTC_TEST_MNEMONIC:-abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about}"
BTC_BIN="${BTC_BIN:-${SCRIPT_DIR}/../../../target/debug/btc}"

fail() { echo "FAIL: $*" >&2; exit 1; }
ok()   { echo "  OK: $*"; }

[[ -x "$BTC_BIN" ]] || fail "btc binary not found at $BTC_BIN (build with 'cargo build -p btc' first)"

echo "=== 286a spin-up ==="
echo "compose dir:  $SCRIPT_DIR"
echo "esplora url:  $ESPLORA_URL"
echo "btc binary:   $BTC_BIN"
echo

echo "[1/3] Bringing up regtest stack (waits for healthchecks)..."
docker compose up -d --wait \
    || fail "docker compose up --wait failed (image missing or healthcheck timeout; this is the RED-state failure on broken blockstream/esplora:latest)"
ok "compose is up"

echo
echo "[2/3] GET \${ESPLORA_URL}/blocks/tip/height"
tip=$(curl -sS --max-time 5 "${ESPLORA_URL}/blocks/tip/height" 2>&1) \
    || fail "curl tip-height request failed"
[[ "$tip" =~ ^[0-9]+$ ]] \
    || fail "tip height not a non-negative integer (got: '$tip')"
ok "tip height = $tip"

echo
echo "[3/3] canonical btc CLI path: wallet balance + wallet sync"
bal_out=$("$BTC_BIN" wallet balance \
        --mnemonic "$MNEMONIC" \
        --network regtest \
        --esplora-url "$ESPLORA_URL" 2>&1) \
    || fail "btc wallet balance failed: $bal_out"
[[ "$bal_out" =~ ^[0-9]+$ ]] \
    || fail "balance not integer (got: '$bal_out')"
ok "balance = $bal_out sats"

sync_out=$("$BTC_BIN" wallet sync \
        --mnemonic "$MNEMONIC" \
        --network regtest \
        --esplora-url "$ESPLORA_URL" 2>&1) \
    || fail "btc wallet sync failed: $sync_out"
[[ "$sync_out" =~ ^n_utxos=[0-9]+\ total_sat=[0-9]+$ ]] \
    || fail "sync output malformed (got: '$sync_out'; expected 'n_utxos=N total_sat=S')"
ok "sync = $sync_out"

echo
docker compose down >/dev/null 2>&1 || true
echo "PASS — 286a canonical path is GREEN"
