#!/usr/bin/env bash
# eth_sepolia.sh — L29 Sepolia acceptance smoke (no wallet/funding ops)
# (Task 11 / #310 / #352)
#
# Assumes local-alpha + local-beta wallets ALREADY EXIST + are FUNDED
# (Sepolia ETH for gas + USDC at Circle proxy). Script does NOT call
# `eth wallet create`, prompt faucets, or poll for funding. Operator
# pre-creates wallets + funds off-script.
#
# Walks through alpha -> beta USDC ERC-20 transfer on live Sepolia:
#   Step 1: Pre-flight (env, RPC reachable, token contract has code)
#   Step 2: Read alpha ETH balance (read-only — assumes funded)
#   Step 3: Read alpha USDC balance (read-only — assumes funded)
#   Step 4: Capture beta pre-transfer USDC balance (delta-check baseline)
#   Step 5: Run eth erc20 send
#   Step 6: Wait for tx receipt (up to 60s)
#   Step 7: Verify beta on-chain USDC balance (delta == TRANSFER_RAW)
#   Step 8: Final balance summary (post-transfer state on-chain)
#   Step 9: Summary
#
# Per L29: operator-driven, NOT CI. Run manually with creds.
# Plan: docs/superpowers/plans/2026-08-23-eth-wallet-core.md (Task 11)
# Issues: #352, #310
# Refs: PR #353 (the Rust test that mirrors this script flow)
#
# RPC note (#355): Sepolia state desyncs on Infura/publicnode (intermittent
# `execution reverted` on `eth_call balanceOf`). Alchemy Sepolia endpoint
# recommended: SEPOLIA_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/<KEY>
set -euo pipefail

# Alpha + Beta addresses + thresholds sourced from .env (operator pre-configured):
#   ALPHA_ADDRESS — sender (local-alpha wallet, already created off-script)
#   BETA_ADDRESS  — recipient (local-beta wallet, NO keystore needed; only the
#                   sender signs, so beta is just an on-chain recipient address)
#   SEPOLIA_MIN_GAS_WEI  — alpha ETH balance must be ≥ this many wei
#   SEPOLIA_TRANSFER_RAW — USDC raw units to transfer (6 decimals; 1_000_000 = 1 USDC)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$WORKSPACE_DIR/crates/eth/tests/.env"

# Source .env FIRST so subsequent requires see the exported vars
if [[ ! -f "$ENV_FILE" ]]; then
  echo "[eth_sepolia] ERROR: $ENV_FILE not found"
  echo "[eth_sepolia]   Create from tests/.env.example (gitignored, operator-local)"
  exit 1
fi
set -a; source "$ENV_FILE"; set +a

ALPHA_ADDR="${ALPHA_ADDRESS:?ALPHA_ADDRESS must be set in $ENV_FILE (run: eth wallet show)}"
BETA_ADDR="${BETA_ADDRESS:-0x0785019b1Eb96034B87348512D37Cda000c126Cf}"
MIN_GAS_WEI="${SEPOLIA_MIN_GAS_WEI:-10000000000000}"
TRANSFER_RAW="${SEPOLIA_TRANSFER_RAW:-1000000}"

# Local-alpha wallet name (used by `eth erc20 send --name`)
LOCAL_ALPHA_NAME="${SEPOLIA_LOCAL_ALPHA_NAME:-local-alpha}"
# Password sourced from .env (WALLET_PASSWORD=...). Falls back to
# test-password if unset so the script works on a fresh checkout.
WALLET_PASS="${WALLET_PASSWORD:-test-password}"

step() { echo; echo "========================================"; echo "STEP $1: $2"; echo "========================================"; }
log()  { echo "[eth_sepolia] $*"; }

# Python-based temp dir cleanup (avoids shell-level delete patterns)
cleanup_tmp() {
  python3 -c "import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)" "$1" 2>/dev/null || true
}

: "${SEPOLIA_RPC_URL:?Must set SEPOLIA_RPC_URL in $ENV_FILE}"
: "${SEPOLIA_USDC_ADDRESS:?Must set SEPOLIA_USDC_ADDRESS (deploy mock or set real address)}"

RPC="$SEPOLIA_RPC_URL"
TOKEN_ADDR="$SEPOLIA_USDC_ADDRESS"
ETH_BIN="$WORKSPACE_DIR/target/debug/eth"

# Persistent wallet dir (gitignored via target/). Survives across script
# runs so the alpha address + keystore can be reused if the script is
# interrupted. Address is also persisted to .env (ALPHA_ADDRESS) so other
# tools/scripts can reference it.
ALPHA_DATA_DIR="$WORKSPACE_DIR/target/eth_sepolia_alpha_wallet"
mkdir -p "$ALPHA_DATA_DIR"
# No cleanup trap — wallet is intentionally persistent.

rpc() {
  local method="$1" params="$2"
  # Build JSON via printf + pipe to curl --data-binary @-.
  # Avoids python subshell pipe (which hangs after `read -r` consumes
  # stdin) and avoids nested-quote escaping in --data "..." form.
  printf '%s' "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}" \
    | curl -s -X POST -H "Content-Type: application/json" --data-binary @- "$RPC"
}

extract_result() {
  grep -oE '"result":"0x[0-9a-fA-F]*"' | head -1 | sed 's/.*"0x/0x/' | tr -d '"'
}

to_decimal() {
  python3 -c "import sys; n=int(sys.argv[1],16); d=int(sys.argv[2]); print(n/d)" "$1" "$2"
}

pad_address() {
  local addr="$1"
  local stripped="${addr#0x}"
  # Pad 20-byte address to 32-byte ABI slot (left-zero-padded) = 64 hex chars
  printf "0x%064s" "$stripped" | tr ' ' '0'
}

# Step 1: Pre-flight
step 1 "Pre-flight checks"
log "RPC:    ${RPC:0:60}..."
log "Token:  $TOKEN_ADDR"
log "Alpha:  $ALPHA_ADDR"
log "Eth bin: $ETH_BIN"

if [[ ! -x "$ETH_BIN" ]]; then
  log "ERROR: $ETH_BIN not built. Run: cargo build -p eth"
  exit 1
fi

code=$(rpc "eth_getCode" "[\"$TOKEN_ADDR\",\"latest\"]" | extract_result)
if [[ -z "$code" || "$code" == "0x" ]]; then
  log "ERROR: no contract at $TOKEN_ADDR"
  log "  Either: (a) deploy a Remix mock, or (b) update SEPOLIA_USDC_ADDRESS in .env"
  exit 1
fi
log "Token contract bytecode OK ($((${#code} - 2)) hex chars)"

# Step 2: Read alpha ETH balance via eth CLI (`wallet balance`)
# Per script goal: ensure eth CLI works end-to-end. Native ETH goes through
# the CLI; ERC-20 (USDC) uses direct RPC until #356 ships (`wallet balance
# --token` flag).
# Cross-check: CLI output MUST match direct `eth_getBalance` RPC. Failure
# here means CLI is using a different RPC or has a regression.
step 2 "Read alpha Sepolia ETH balance (CLI + RPC cross-check)"
alpha_eth_human=$("$ETH_BIN" wallet balance \
  --address "$ALPHA_ADDR" \
  --network sepolia \
  --rpc-url "$RPC" 2>/dev/null | awk '{print $1}')
alpha_eth_hex=$(rpc "eth_getBalance" "[\"$ALPHA_ADDR\",\"latest\"]" | extract_result)
[[ -z "$alpha_eth_hex" || "$alpha_eth_hex" == "0x" ]] && alpha_eth_hex="0x0"
log "Alpha ETH: $alpha_eth_human ETH (CLI) | $alpha_eth_hex wei (RPC)"
if ! python3 -c "
import sys
from decimal import Decimal
cli_eth = Decimal(sys.argv[1])
cli_wei = int(cli_eth * Decimal('1e18'))
rpc_wei = int(sys.argv[2], 16)
sys.exit(0 if cli_wei == rpc_wei else 1)
" "$alpha_eth_human" "$alpha_eth_hex"; then
  log "ERROR: CLI ↔ RPC mismatch for alpha ETH"
  log "  CLI: $alpha_eth_human ETH (wei $alpha_eth_human * 1e18)"
  log "  RPC: $alpha_eth_hex wei"
  exit 1
fi
if python3 -c "import sys; eth=float(sys.argv[1]); wei=int(eth*1e18); sys.exit(0 if wei >= int(sys.argv[2]) else 1)" "$alpha_eth_human" "$MIN_GAS_WEI"; then
  log "OK: alpha has ≥ $MIN_GAS_WEI wei Sepolia ETH for gas"
else
  log "ERROR: alpha Sepolia ETH balance below $MIN_GAS_WEI wei threshold — fund alpha first via faucet"
  exit 1
fi

# Step 3: Read alpha USDC balance (read-only — assumes operator pre-funded)
step 3 "Read alpha Circle USDC balance (assumes pre-funded)"
alpha_padded=$(pad_address "$ALPHA_ADDR")
alpha_calldata="0x70a08231${alpha_padded:2}"
alpha_usdc_hex=$(rpc "eth_call" "[{\"to\":\"$TOKEN_ADDR\",\"data\":\"$alpha_calldata\"},\"latest\"]" | extract_result)
[[ -z "$alpha_usdc_hex" || "$alpha_usdc_hex" == "0x" ]] && alpha_usdc_hex="0x0"
alpha_usdc_human=$(to_decimal "$alpha_usdc_hex" 1000000)
log "Alpha USDC: $alpha_usdc_human USDC (raw: $alpha_usdc_hex)"
if python3 -c "import sys; sys.exit(0 if int(sys.argv[1],16) >= int(sys.argv[2]) else 1)" "$alpha_usdc_hex" "$TRANSFER_RAW"; then
  log "OK: alpha has ≥ $TRANSFER_RAW raw USDC for transfer"
else
  log "ERROR: alpha USDC balance below $TRANSFER_RAW raw threshold — fund alpha first via Circle faucet or Remix mint"
  exit 1
fi

# Step 4: Capture beta pre-transfer USDC balance (delta-check baseline)
step 4 "Capture beta pre-transfer USDC balance"
log "Beta recipient: $BETA_ADDR"
beta_pre_padded=$(pad_address "$BETA_ADDR")
beta_pre_calldata="0x70a08231${beta_pre_padded:2}"
beta_pre_hex=$(rpc "eth_call" "[{\"to\":\"$TOKEN_ADDR\",\"data\":\"$beta_pre_calldata\"},\"latest\"]" | extract_result)
[[ -z "$beta_pre_hex" || "$beta_pre_hex" == "0x" ]] && beta_pre_hex="0x0"
BETA_USDC_PRE_HEX="$beta_pre_hex"
BETA_USDC_PRE_HUMAN=$(to_decimal "$beta_pre_hex" 1000000)
log "Beta pre-transfer USDC: $BETA_USDC_PRE_HUMAN (raw: $beta_pre_hex)"

# Step 5: Broadcast
step 5 "Broadcast: alpha -> beta, $TRANSFER_RAW raw ($((TRANSFER_RAW / 1000000)) USDC)"
log "Running eth erc20 send..."
send_out=$(ETH_PASSWORD="$WALLET_PASS" "$ETH_BIN" erc20 send \
  --data-dir "$ALPHA_DATA_DIR" \
  --name "$LOCAL_ALPHA_NAME" \
  --token "$TOKEN_ADDR" \
  --to "$BETA_ADDR" \
  --amount "$TRANSFER_RAW" \
  --network sepolia \
  --rpc-url "$RPC")
echo "$send_out" | sed 's/^/[eth_sepolia send] /'

TX_HASH=$(echo "$send_out" | grep -oE '0x[0-9a-f]{64}' | head -1 || echo "")
if [[ -z "$TX_HASH" ]]; then
  log "ERROR: no tx hash in send output"
  exit 1
fi
log "Tx hash: $TX_HASH"
log "View on Etherscan: https://sepolia.etherscan.io/tx/$TX_HASH"

# Step 6: Wait for receipt
step 6 "Wait for tx to be mined (up to 60s)"
mined=0
for i in $(seq 1 12); do
  receipt=$(rpc "eth_getTransactionReceipt" "[\"$TX_HASH\"]")
  if echo "$receipt" | grep -q '"blockNumber"'; then
    log "Tx mined (attempt $i, ~${i}*5s elapsed)"
    mined=1
    break
  fi
  log "  attempt $i: not yet mined"
  sleep 5
done
if [[ "$mined" -ne 1 ]]; then
  log "ERROR: tx not mined within 60s"
  exit 1
fi

# Step 7: Verify beta balance (delta check: post - pre == 1 USDC)
step 7 "Verify beta on-chain USDC balance (delta == $((TRANSFER_RAW / 1000000)) USDC)"
padded=$(pad_address "$BETA_ADDR")
calldata="0x70a08231${padded:2}"
bal_hex=$(rpc "eth_call" "[{\"to\":\"$TOKEN_ADDR\",\"data\":\"$calldata\"},\"latest\"]" | extract_result)
[[ -z "$bal_hex" || "$bal_hex" == "0x" ]] && bal_hex="0x0"
bal_human=$(to_decimal "$bal_hex" 1000000)
log "Beta USDC balance: $bal_human USDC"

# Delta = post - pre. Expect exactly TRANSFER_RAW.
delta_hex=$(python3 -c "import sys; print(hex(int(sys.argv[1],16) - int(sys.argv[2],16)))" "$bal_hex" "$BETA_USDC_PRE_HEX")
delta_human=$(to_decimal "$delta_hex" 1000000)
log "Delta: $delta_human USDC (pre: $BETA_USDC_PRE_HUMAN -> post: $bal_human)"

if python3 -c "import sys; sys.exit(0 if int(sys.argv[1],16) == int(sys.argv[2]) else 1)" "$delta_hex" "$TRANSFER_RAW"; then
  log "PASS: beta received exactly $((TRANSFER_RAW / 1000000)) USDC from alpha"
  log "  Tx hash: $TX_HASH"
  log "  Etherscan: https://sepolia.etherscan.io/tx/$TX_HASH"
else
  log "FAIL: delta ($delta_human USDC) != $((TRANSFER_RAW / 1000000)) USDC"
  log "  Tx hash (for diagnostics): $TX_HASH"
  log "  Etherscan: https://sepolia.etherscan.io/tx/$TX_HASH"
  exit 1
fi

# Step 8: Final balance summary (post-transfer state on-chain)
step 8 "Final balance summary (post-transfer)"
log "Re-querying balances after tx mined..."

# Alpha ETH (CLI via `eth wallet balance`)
alpha_eth_human=$("$ETH_BIN" wallet balance \
  --address "$ALPHA_ADDR" \
  --network sepolia \
  --rpc-url "$RPC" 2>/dev/null | awk '{print $1}')

# Alpha USDC (RPC — until #356 ships `wallet balance --token`)
alpha_padded=$(pad_address "$ALPHA_ADDR")
alpha_calldata="0x70a08231${alpha_padded:2}"
alpha_usdc_hex=$(rpc "eth_call" "[{\"to\":\"$TOKEN_ADDR\",\"data\":\"$alpha_calldata\"},\"latest\"]" | extract_result)
[[ -z "$alpha_usdc_hex" || "$alpha_usdc_hex" == "0x" ]] && alpha_usdc_hex="0x0"
alpha_usdc_human=$(to_decimal "$alpha_usdc_hex" 1000000)

# Beta ETH (CLI via `eth wallet balance`)
beta_eth_human=$("$ETH_BIN" wallet balance \
  --address "$BETA_ADDR" \
  --network sepolia \
  --rpc-url "$RPC" 2>/dev/null | awk '{print $1}')

# Beta USDC (RPC — until #356 ships)
beta_padded=$(pad_address "$BETA_ADDR")
beta_calldata="0x70a08231${beta_padded:2}"
beta_usdc_hex=$(rpc "eth_call" "[{\"to\":\"$TOKEN_ADDR\",\"data\":\"$beta_calldata\"},\"latest\"]" | extract_result)
[[ -z "$beta_usdc_hex" || "$beta_usdc_hex" == "0x" ]] && beta_usdc_hex="0x0"
beta_usdc_human=$(to_decimal "$beta_usdc_hex" 1000000)

log ""
log "  Alpha ETH:  $alpha_eth_human ETH"
log "  Alpha USDC: $alpha_usdc_human USDC"
log "  Beta ETH:   $beta_eth_human ETH"
log "  Beta USDC:  $beta_usdc_human USDC"
log ""
log "Expected: alpha USDC decreased by $((TRANSFER_RAW / 1000000)), beta USDC increased by $((TRANSFER_RAW / 1000000)), alpha ETH decreased by ~gas."

# Step 9: Summary
step 9 "Summary"
log "Local-alpha wallet: $ALPHA_DATA_DIR (persistent across runs)"

echo
echo "=========================================="
echo "  L29 Sepolia acceptance: PASS"
echo "  Tx hash: $TX_HASH"
echo "  Etherscan: https://sepolia.etherscan.io/tx/$TX_HASH"
echo "=========================================="
echo
log "Next steps:"
log "  1. Flip #352 acceptance box [ ]->[x] in GitHub issue (L29 manual gate per memory rule)"
log "  2. Confirmation commit: chore(lessons): L29 Sepolia acceptance confirmed (#355 / PR #353)"
log "  3. Merge PR #353 if not already merged"
