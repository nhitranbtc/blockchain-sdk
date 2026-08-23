#!/usr/bin/env bash
#
# btc-check-balance-regtest.sh — One-shot balance-check workflow on local
# regtest. Companion to btc-send-regtest-e2e.sh but smaller scope:
#
#   1. Stack up (bitcoind + esplora)
#   2. Build btc
#   3. (Optional) Create a regtest wallet
#   4. List wallets
#   5. Derive the first receive address from --mnemonic
#   6. Fund the address via bitcoind RPC (workaround for missing Esplora)
#   7. Mine 1 confirmation block
#   8. Print the address balance (via bitcoind getreceivedbyaddress + getbalance)
#
# Workaround note: the btc CLI's `wallet sync` / `wallet balance`
# commands require an Esplora HTTP API. In this regtest env the
# blockstream/esplora image is unreliable, so this script:
#   - Derives the address from the btc error path's request URL (grep)
#   - Funds via direct bitcoind RPC
#   - Reads the balance via direct bitcoind RPC
# Swap in a working Esplora image and this script collapses to a single
# `btc wallet balance` call.
#
# Usage:
#   bash scripts/btc-check-balance-regtest.sh \
#       --mnemonic "<12-or-24-word phrase>"
#
# Exit codes:
#   0  balance printed successfully
#   1  setup / build / runtime failure
#   2  missing required arg
#   3  bitcoind unreachable

set -uo pipefail

# --- Args ---
MNEMONIC=""
CREATE_WALLET=0
DATA_DIR="${BTC_DATA_DIR:-/tmp/btc-data}"
RPC_URL="http://localhost:18443"
RPC_USER="foo"
RPC_PASS="bar"
CONTAINER="e2e-bitcoind-regtest"
WALLET_NAME="default"
FUND_BTC="1.0"

usage() {
    cat <<'EOF'
btc-check-balance-regtest.sh — local regtest balance-check workflow.

Usage:
  bash scripts/btc-check-balance-regtest.sh --mnemonic "<phrase>"
  bash scripts/btc-check-balance-regtest.sh --mnemonic "<phrase>" --create-wallet
  bash scripts/btc-check-balance-regtest.sh --help

Required:
  --mnemonic <phrase>    BIP-39 phrase to derive the first receive address

Optional:
  --create-wallet        Also create + persist the wallet under --data-dir
  --data-dir <path>      btc data dir (default: /tmp/btc-data)
  --container <name>     bitcoind Docker container (default: e2e-bitcoind-regtest)
  --rpc-url <url>        bitcoind RPC URL (default: http://localhost:18443)
  --rpc-user <u> --rpc-pass <p>  RPC creds (default: foo/bar)
  --fund-btc <n>         Amount to fund (default: 1.0)
  --wallet <name>        bitcoind wallet name (default: default)

Exit 0 prints the final balance (sats + BTC).
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --mnemonic)   MNEMONIC="$2"; shift 2 ;;
        --create-wallet) CREATE_WALLET=1; shift ;;
        --data-dir)   DATA_DIR="$2"; shift 2 ;;
        --container)  CONTAINER="$2"; shift 2 ;;
        --rpc-url)    RPC_URL="$2"; shift 2 ;;
        --rpc-user)   RPC_USER="$2"; shift 2 ;;
        --rpc-pass)   RPC_PASS="$2"; shift 2 ;;
        --fund-btc)   FUND_BTC="$2"; shift 2 ;;
        --wallet)     WALLET_NAME="$2"; shift 2 ;;
        -h|--help)    usage; exit 0 ;;
        *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
    esac
done

if [[ -z "$MNEMONIC" && CREATE_WALLET -eq 0 ]]; then
    echo "Missing required --mnemonic (or pass --create-wallet to generate one)" >&2
    usage
    exit 2
fi

# --- Color helpers ---
if [[ -t 1 ]]; then
    GREEN='\033[0;32m'; BLUE='\033[0;34m'; BOLD='\033[1m'; RESET='\033[0m'
else
    GREEN=''; BLUE=''; BOLD=''; RESET=''
fi
log_step() { printf '  %s %s\n' "$(date '+%H:%M:%S')" "$1"; }

# --- Step 1: stack up ---
echo -e "${BOLD}=== Step 1: stack up ===${RESET}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
log_step "running e2e-regtest/up.sh"
bash "$SCRIPT_DIR/e2e-regtest/up.sh" >/dev/null 2>&1 || {
    log_step "stack already up (or up.sh failed; continuing)"
}

# --- Step 2: build btc ---
echo -e "${BOLD}=== Step 2: build btc ===${RESET}"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
log_step "cargo build -p btc"
( cd "$REPO_ROOT" && cargo build --quiet -p btc ) 2>&1 | tail -3
BTC_BIN="$REPO_ROOT/target/debug/btc"
[[ -x "$BTC_BIN" ]] || { echo "build failed: $BTC_BIN not found" >&2; exit 1; }

# --- Step 3: create wallet (optional) ---
if (( CREATE_WALLET )); then
    echo -e "${BOLD}=== Step 3: create wallet ===${RESET}"
    log_step "btc wallet create --words 12 --network regtest"
    CREATE_OUT=$(echo "demo-pw" | "$BTC_BIN" --data-dir "$DATA_DIR" wallet create \
        --words 12 --network regtest --password "demo-pw" 2>&1 | tee /tmp/btc-check-balance-create.log)
    # Extract the mnemonic from the create output so the next step (sync)
    # doesn't need a separate --mnemonic arg.
    if [[ -z "$MNEMONIC" ]]; then
        MNEMONIC=$(echo "$CREATE_OUT" | grep -oE '[a-z]+( [a-z]+){11,23}' | head -1)
        if [[ -z "$MNEMONIC" ]]; then
            MNEMONIC=$(echo "$CREATE_OUT" | awk '/^Mnemonic/{getline; print}' | tr -s ' ')
        fi
        if [[ -n "$MNEMONIC" ]]; then
            log_step "captured mnemonic from create output: $MNEMONIC"
        else
            echo "Could not extract mnemonic from create output" >&2
            exit 1
        fi
    fi
fi

# --- Step 4: list wallets ---
echo -e "${BOLD}=== Step 4: list wallets ===${RESET}"
log_step "btc wallet list --network regtest"
"$BTC_BIN" --data-dir "$DATA_DIR" wallet list --network regtest 2>&1 | head -10

# --- Step 5: derive first receive address ---
echo -e "${BOLD}=== Step 5: derive first receive address ===${RESET}"
log_step "btc wallet sync (Esplora fallback via error path)"
SYNC_OUT=$("$BTC_BIN" --data-dir "$DATA_DIR" wallet sync \
    --mnemonic "$MNEMONIC" \
    --network regtest \
    --esplora-url http://localhost:50001/regtest/api 2>&1 || true)
RECIPIENT=$(echo "$SYNC_OUT" | grep -oE 'bcrt1q[a-z0-9]{38,}|tb1q[a-z0-9]{38,}|2[A-Za-z0-9]{33,}|m[A-Za-z0-9]{33,}|n[A-Za-z0-9]{33,}' | head -1)
if [[ -z "$RECIPIENT" ]]; then
    echo "Could not derive address from sync output" >&2
    echo "$SYNC_OUT"
    exit 1
fi
log_step "address: $RECIPIENT"
echo "  regtest native-segwit, BIP-84 m/84'/1'/0'/0/0"

# --- Step 6: fund ---
echo -e "${BOLD}=== Step 6: fund ===${RESET}"
log_step "bitcoin-cli sendtoaddress $RECIPIENT $FUND_BTC"
FUND_TXID=$(docker exec "$CONTAINER" bitcoin-cli -regtest \
    -rpcuser="$RPC_USER" -rpcpassword="$RPC_PASS" -rpcwallet="$WALLET_NAME" \
    sendtoaddress "$RECIPIENT" "$FUND_BTC" 2>&1)
echo "  funding tx: $FUND_TXID"

# --- Step 7: mine confirmation ---
echo -e "${BOLD}=== Step 7: mine 1 confirmation block ===${RESET}"
MINER_ADDR=$(docker exec "$CONTAINER" bitcoin-cli -regtest \
    -rpcuser="$RPC_USER" -rpcpassword="$RPC_PASS" -rpcwallet="$WALLET_NAME" \
    getnewaddress)
docker exec "$CONTAINER" bitcoin-cli -regtest \
    -rpcuser="$RPC_USER" -rpcpassword="$RPC_PASS" -rpcwallet="$WALLET_NAME" \
    generatetoaddress 1 "$MINER_ADDR" >/dev/null
log_step "mined 1 block (height: $(docker exec "$CONTAINER" bitcoin-cli -regtest -rpcuser="$RPC_USER" -rpcpassword="$RPC_PASS" getblockcount))"

# --- Step 8: return balance ---
echo -e "${BOLD}=== Step 8: balance ===${RESET}"
# Find the recipient's vout (sendtoaddress puts change at vout 0 in
# bitcoind 25+, recipient at vout 1 — but order varies; iterate to match).
BAL_BTC=""
RECIPIENT_VOUT=""
for vout in 0 1 2 3; do
    UTXO_JSON=$(docker exec "$CONTAINER" bitcoin-cli -regtest \
        -rpcuser="$RPC_USER" -rpcpassword="$RPC_PASS" \
        gettxout "$FUND_TXID" "$vout" 2>&1)
    if [[ "$UTXO_JSON" != "{"* ]]; then
        continue  # no UTXO at this vout (likely spent)
    fi
    # Use grep -oE for JSON field extraction (multi-line JSON safe).
    VOUT_ADDR=$(echo "$UTXO_JSON" | grep -oE '"address":\s*"[^"]+"' | head -1 | sed -E 's/.*"([^"]+)"$/\1/')
    if [[ "$VOUT_ADDR" == "$RECIPIENT" ]]; then
        BAL_BTC=$(echo "$UTXO_JSON" | grep -oE '"value":\s*[0-9.]+' | head -1 | sed -E 's/.*"value":\s*//')
        CONFIRMATIONS=$(echo "$UTXO_JSON" | grep -oE '"confirmations":\s*[0-9]+' | head -1 | sed -E 's/.*"confirmations":\s*//')
        RECIPIENT_VOUT="$vout"
        break
    fi
done
# L12 / security: validate balance is numeric before feeding awk, then
# pass as a -v DATA variable (not interpolated into the program text).
if [[ -z "$BAL_BTC" ]] || [[ ! "$BAL_BTC" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    echo "Could not find recipient UTXO in $FUND_TXID for addr $RECIPIENT" >&2
    echo "Last UTXO JSON: $UTXO_JSON" >&2
    exit 1
fi
BAL_SATS=$(awk -v btc="$BAL_BTC" 'BEGIN { printf "%.0f", btc * 100000000 }')
log_step "address:  $RECIPIENT"
log_step "utxo:     $FUND_TXID:$RECIPIENT_VOUT"
log_step "balance:  ${BAL_BTC} BTC (${BAL_SATS} sats), confirmations=$CONFIRMATIONS"

# also show the bitcoind default-wallet balance (mining reward)
MINING_BAL_BTC=$(docker exec "$CONTAINER" bitcoin-cli -regtest \
    -rpcuser="$RPC_USER" -rpcpassword="$RPC_PASS" -rpcwallet="$WALLET_NAME" \
    getbalance)
log_step "miner wallet ($WALLET_NAME) balance: ${MINING_BAL_BTC} BTC"

echo
echo -e "${GREEN}${BOLD}Result:${RESET}"
echo "  recipient: $RECIPIENT"
echo "  balance:   $BAL_BTC BTC ($BAL_SATS sats)"
echo "  confirmations: $CONFIRMATIONS"
echo "  funding tx: $FUND_TXID (recipient at vout $RECIPIENT_VOUT)"

exit 0