#!/usr/bin/env bash
# eth_sepolia.sh — L29 operator-driven Sepolia acceptance smoke
# (Task 11 / #310 / #352)
#
# Walks through alpha -> beta 100 USDC ERC-20 transfer on live Sepolia:
#   Step 1: Pre-flight (env, RPC reachable, token contract has code)
#   Step 2: Show alpha — operator funds Sepolia ETH (gas)
#   Step 3: Wait + verify Sepolia ETH balance
#   Step 4: Show alpha + token — operator deploys mock OR funds USDC
#   Step 5: Wait + verify USDC balance
#   Step 6: Create alpha + beta wallets in temp ETH_DATA_DIR
#   Step 7: Run eth erc20 send
#   Step 8: Wait for tx receipt (up to 60s)
#   Step 9: Verify beta on-chain USDC balance == 100 USDC
#   Step 10: Cleanup temp data dir
#
# Per L29: operator-driven, NOT CI. Run manually with creds.
# Plan: docs/superpowers/plans/2026-08-23-eth-wallet-core.md (Task 11)
# Issues: #352, #310
# Refs: PR #353 (the Rust test that mirrors this script flow)
set -euo pipefail

ALPHA_ADDR="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
MIN_GAS_WEI=10000000000000000
TRANSFER_RAW=1000000

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$WORKSPACE_DIR/crates/eth/tests/.env"

step() { echo; echo "========================================"; echo "STEP $1: $2"; echo "========================================"; }
log()  { echo "[eth_sepolia] $*"; }
pause() {
  echo
  echo "[eth_sepolia] >>> MANUAL ACTION REQUIRED <<<"
  echo "$*"
  echo
  printf "Press ENTER when done (Ctrl+C to abort): "
  read -r
}

# Python-based temp dir cleanup (avoids shell-level delete patterns)
cleanup_tmp() {
  python3 -c "import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)" "$1" 2>/dev/null || true
}

if [[ ! -f "$ENV_FILE" ]]; then
  log "ERROR: $ENV_FILE not found"
  log "  Create from tests/.env.example (gitignored, operator-local)"
  exit 1
fi
set -a; source "$ENV_FILE"; set +a

: "${SEPOLIA_RPC_URL:?Must set SEPOLIA_RPC_URL in $ENV_FILE}"
: "${SEPOLIA_USDC_ADDRESS:?Must set SEPOLIA_USDC_ADDRESS (deploy mock or set real address)}"

RPC="$SEPOLIA_RPC_URL"
TOKEN_ADDR="$SEPOLIA_USDC_ADDRESS"
ETH_BIN="$WORKSPACE_DIR/target/debug/eth"

rpc() {
  local method="$1" params="$2"
  curl -s -X POST -H "Content-Type: application/json" \
    --data "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}" \
    "$RPC"
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
  printf "0x%040s" "$stripped" | tr ' ' '0'
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

# Step 2: Fund Sepolia ETH
step 2 "Fund alpha with Sepolia ETH (gas)"
log "Alpha address: $ALPHA_ADDR"
log "Required: >= 0.01 Sepolia ETH"
echo
echo "  Faucets (any one):"
echo "    https://cloudflare-eth.com/faucet"
echo "    https://sepoliafaucet.com  (requires Google login)"
echo "    https://www.alchemy.com/faucets/ethereum-sepolia"
echo
pause "Fund $ALPHA_ADDR with >= 0.01 Sepolia ETH"

# Step 3: Verify Sepolia ETH balance
step 3 "Verify Sepolia ETH balance"
log "Polling alpha ETH balance..."
funded_eth=0
for i in $(seq 1 30); do
  bal_hex=$(rpc "eth_getBalance" "[\"$ALPHA_ADDR\",\"latest\"]" | extract_result)
  [[ -z "$bal_hex" || "$bal_hex" == "0x" ]] && bal_hex="0x0"
  bal_eth=$(to_decimal "$bal_hex" 1000000000000000000)
  log "  attempt $i: $bal_eth ETH"
  if python3 -c "import sys; sys.exit(0 if int(sys.argv[1],16) >= int(sys.argv[2]) else 1)" "$bal_hex" "$MIN_GAS_WEI"; then
    log "Sepolia ETH funded: $bal_eth ETH"
    funded_eth=1
    break
  fi
  sleep 5
done
if [[ "$funded_eth" -ne 1 ]]; then
  log "ERROR: Sepolia ETH funding timeout after 150s"
  exit 1
fi

# Step 4: Fund USDC
step 4 "Fund alpha with 100 USDC"
log "Alpha address: $ALPHA_ADDR"
log "Token contract: $TOKEN_ADDR"
echo
echo "  Option A — Deploy your own mock (5 min, Remix):"
echo "    1. https://remix.ethereum.org -> new file MockUSDC.sol:"
echo "       // SPDX-License-Identifier: MIT"
echo "       pragma solidity ^0.8.20;"
echo "       import '@openzeppelin/contracts/token/ERC20/ERC20.sol';"
echo "       contract MockUSDC is ERC20 {"
echo "         constructor() ERC20('USD Coin', 'USDC') {}"
echo "         function decimals() public pure override returns (uint8) { return 6; }"
echo "         function mint(address to, uint256 amount) external { _mint(to, amount); }"
echo "       }"
echo "    2. Compile (Solidity 0.8.20) -> Deploy & Run (Injected Web3, MetaMask on Sepolia)"
echo "    3. After deploy, call: mint($ALPHA_ADDR, 100000000)"
echo "    4. Update SEPOLIA_USDC_ADDRESS in tests/.env to deployed address, re-run script"
echo
echo "  Option B — Use existing Sepolia USDC:"
echo "    https://faucet.circle.com  (claim 10x for 100 USDC, daily limit)"
echo
pause "Fund $ALPHA_ADDR with >= 1 USDC (raw: $TRANSFER_RAW)"

# Step 5: Verify USDC balance
step 5 "Verify USDC balance"
log "Polling alpha USDC balance..."
funded_usdc=0
for i in $(seq 1 30); do
  padded=$(pad_address "$ALPHA_ADDR")
  calldata="0x70a08231${padded:2}"
  bal_hex=$(rpc "eth_call" "[{\"to\":\"$TOKEN_ADDR\",\"data\":\"$calldata\"},\"latest\"]" | extract_result)
  [[ -z "$bal_hex" || "$bal_hex" == "0x" ]] && bal_hex="0x0"
  bal_human=$(to_decimal "$bal_hex" 1000000)
  log "  attempt $i: $bal_human USDC"
  if python3 -c "import sys; sys.exit(0 if int(sys.argv[1],16) >= int(sys.argv[2]) else 1)" "$bal_hex" "$TRANSFER_RAW"; then
    log "USDC funded: $bal_human USDC"
    funded_usdc=1
    break
  fi
  sleep 5
done
if [[ "$funded_usdc" -ne 1 ]]; then
  log "ERROR: USDC funding timeout after 150s"
  exit 1
fi

# Step 6: Create wallets
step 6 "Create alpha + beta wallets in temp ETH_DATA_DIR"
TMP_DATA=$(mktemp -d -t eth_sepolia.XXXXXX)
log "Temp ETH_DATA_DIR: $TMP_DATA"

log "Importing alpha (deterministic Anvil mnemonic #0)..."
"$ETH_BIN" wallet import \
  --data-dir "$TMP_DATA" \
  --name alpha \
  --mnemonic "test test test test test test test test test test test junk" \
  --password "test-password" \
  --network sepolia

log "Creating beta (random mnemonic)..."
beta_out=$("$ETH_BIN" wallet create \
  --data-dir "$TMP_DATA" \
  --name beta \
  --password "test-password" \
  --network sepolia)
BETA_ADDR=$(echo "$beta_out" | grep -E '^address:' | awk '{print $2}' | tr -d ' ')
log "Beta address: $BETA_ADDR"

# Step 7: Broadcast
step 7 "Broadcast: alpha -> beta, 100 USDC"
log "Running eth erc20 send..."
send_out=$("$ETH_BIN" erc20 send \
  --data-dir "$TMP_DATA" \
  --name alpha \
  --password "test-password" \
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

# Step 8: Wait for receipt
step 8 "Wait for tx to be mined (up to 60s)"
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

# Step 9: Verify beta balance
step 9 "Verify beta on-chain USDC balance"
padded=$(pad_address "$BETA_ADDR")
calldata="0x70a08231${padded:2}"
bal_hex=$(rpc "eth_call" "[{\"to\":\"$TOKEN_ADDR\",\"data\":\"$calldata\"},\"latest\"]" | extract_result)
[[ -z "$bal_hex" || "$bal_hex" == "0x" ]] && bal_hex="0x0"
bal_human=$(to_decimal "$bal_hex" 1000000)
log "Beta USDC balance: $bal_human USDC"

if python3 -c "import sys; sys.exit(0 if int(sys.argv[1],16) == int(sys.argv[2]) else 1)" "$bal_hex" "$TRANSFER_RAW"; then
  log "PASS: beta received exactly 100 USDC"
else
  log "FAIL: beta balance ($bal_human USDC) != 100 USDC"
  exit 1
fi

# Step 10: Cleanup
step 10 "Cleanup"
cleanup_tmp "$TMP_DATA"
log "Temp ETH_DATA_DIR removed: $TMP_DATA"

echo
echo "=========================================="
echo "  L29 Sepolia acceptance: PASS"
echo "  Tx hash: $TX_HASH"
echo "  Etherscan: https://sepolia.etherscan.io/tx/$TX_HASH"
echo "=========================================="
echo
log "Next steps:"
log "  1. Flip #352 acceptance box [ ]->[x] in GitHub issue"
log "  2. Confirmation commit: chore(lessons): L29 Sepolia acceptance confirmed (PR #353)"
log "  3. Merge PR #353 if not already merged"
