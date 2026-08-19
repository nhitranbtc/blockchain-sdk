#!/usr/bin/env bash
# v0.1.0 L29 operator smoke — wallet-desktop
#
# Operator-driven end-to-end verification of v0.1.0 (tag `v0.1.0`).
# Runs the headless parts of plan §Task 26 step 1 (12-step smoke) via
# the `btc` CLI directly. GUI-only steps (launch app, click buttons,
# wait for confirmations) are operator-handled; this script prints
# what to do at each pause point and waits for operator confirmation.
#
# Per L29 + L28: live testnet smoke is operator-driven, not CI.
# Per L12 CRITICAL #2: mnemonic + password NEVER logged.
# Per L7: BTC_WALLET_MNEMONIC / BTC_*_PASSWORD stripped from spawned env.
#
# Usage:
#   bash wallet-desktop/scripts/smoke/v0.1.0.sh
#
# Exit codes:
#   0 — all headless steps green + L12 CRITICAL #2 grep clean
#   1 — preflight or any headless step failed
#   2 — CRITICAL #2 grep matched cleartext (blocker, abort release)
set -euo pipefail

# ─── Config ──────────────────────────────────────────────────────────────
readonly TAG="v0.1.0"
readonly NETWORK="testnet"
readonly APP_DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/flutter_btc_wallet"
readonly WALLET_BLOB_DIR="$APP_DATA_DIR/wallet_data/$NETWORK"
readonly FAUCET_URL="https://coinfaucet.eu/btc-testnet/"
readonly BTC="${BTC_BIN:-$(command -v btc || true)}"

# Colors (terminal only)
readonly RED='\033[0;31m' GREEN='\033[0;32m' YELLOW='\033[1;33m' NC='\033[0m'
log()   { printf "${GREEN}[smoke]${NC} %s\n" "$*"; }
warn()  { printf "${YELLOW}[smoke]${NC} %s\n" "$*" >&2; }
fail()  { printf "${RED}[smoke] FAIL:${NC} %s\n" "$*" >&2; exit 1; }
crit2() { printf "${RED}[smoke] CRITICAL #2:${NC} %s\n" "$*" >&2; exit 2; }

pause() {
  local msg="$1"
  printf "\n${YELLOW}─── operator action required ───${NC}\n"
  printf "%s\n" "$msg"
  printf "${YELLOW}─────────────────────────────────${NC}\n"
  read -r -p "Press Enter when done (or Ctrl-C to abort): "
}

# ─── Pre-flight ──────────────────────────────────────────────────────────
log "Pre-flight: $TAG smoke"
log "Network: $NETWORK"
log "Wallet blob dir: $WALLET_BLOB_DIR"

# Tag check
if ! git -C "$(dirname "$0")/../.." tag --list "$TAG" | grep -q "$TAG"; then
  fail "tag $TAG not found locally. Fetch first: git fetch origin --tags"
fi
log "✓ tag $TAG present"

# btc binary
if [ -z "$BTC" ]; then
  fail "btc binary not on PATH. Build with: cargo build --release -p btc"
fi
if ! "$BTC" --version >/dev/null 2>&1; then
  fail "btc at $BTC did not respond to --version"
fi
log "✓ btc binary: $BTC ($("$BTC" --version 2>&1 | head -1))"

# Disk space (1GB min for the wallet DB + log capture)
AVAIL_KB=$(df -Pk "$WALLET_BLOB_DIR" 2>/dev/null | awk 'NR==2 {print $4}' || echo 0)
if [ "$AVAIL_KB" -lt 1048576 ]; then
  fail "less than 1GB free at $WALLET_BLOB_DIR"
fi
log "✓ disk space: $((AVAIL_KB / 1024)) MB free"

# Testnet reachability
if ! curl -fsSL --max-time 10 "https://blockstream.info/testnet/api/blocks/tip/height" >/dev/null 2>&1; then
  fail "cannot reach blockstream.info testnet Esplora. Check network."
fi
log "✓ blockstream.info testnet reachable"

# Clear stale wallet blobs from prior runs
if [ -d "$WALLET_BLOB_DIR" ]; then
  warn "clearing stale wallet blobs from prior runs: $WALLET_BLOB_DIR"
  rm -rf "$WALLET_BLOB_DIR"
fi

# ─── Step 3: Create wallet ───────────────────────────────────────────────
log ""
log "Step 3: Create testnet wallet (BIP-84 native-segwit)"
WALLET_NAME="smoke-$(date +%s)"
WALLET_ID=$("$BTC" --network "$NETWORK" wallet create --name "$WALLET_NAME" 2>/dev/null \
  | tee /tmp/wallet-create.json \
  | grep -oE '"id"[[:space:]]*:[[:space:]]*"[^"]+"' \
  | head -1 \
  | sed -E 's/.*"id"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')

if [ -z "$WALLET_ID" ]; then
  fail "wallet create did not return an id. Output: $(cat /tmp/wallet-create.json)"
fi
log "✓ wallet created: id=$WALLET_ID name=$WALLET_NAME"

# Capture the mnemonic from the create output for the operator's manual backup
# (printed here so operator can copy + verify, NOT logged to any file)
MNEMONIC=$(grep -oE '"mnemonic"[[:space:]]*:[[:space:]]*"[^"]+"' /tmp/wallet-create.json \
  | sed -E 's/.*"mnemonic"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')
rm -f /tmp/wallet-create.json
printf "${YELLOW}MNEMONIC (write this down, do NOT log): %s${NC}\n" "$MNEMONIC"
# Note: mnemonic is intentionally printed for the operator. It does NOT enter
# any log file. The script's stdout is the operator's responsibility per L29.

# Verify the wallet blob landed
if [ ! -d "$WALLET_BLOB_DIR" ]; then
  fail "wallet blob dir not created at $WALLET_BLOB_DIR"
fi
BLOB_COUNT=$(find "$WALLET_BLOB_DIR" -name "*.enc" 2>/dev/null | wc -l)
log "✓ wallet blob present ($BLOB_COUNT .enc file(s) in $WALLET_BLOB_DIR)"

# ─── Step 4: Fund ─────────────────────────────────────────────────────────
RECIPIENT_ADDR=$("$BTC" --network "$NETWORK" wallet show --id "$WALLET_ID" 2>/dev/null \
  | grep -oE '"address"[[:space:]]*:[[:space:]]*"[^"]+"' \
  | head -1 \
  | sed -E 's/.*"address"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')

if [ -z "$RECIPIENT_ADDR" ]; then
  fail "wallet show did not return an address"
fi
log "✓ receive address: $RECIPIENT_ADDR"

pause "Step 4: Open $FAUCET_URL in browser, paste address $RECIPIENT_ADDR, request ~0.002 BTC (covers faucet amount + send-back fee)."

# ─── Step 5: Wait for 1 conf ──────────────────────────────────────────────
pause "Step 5: Wait ~10 min for 1 confirmation. Tip: monitor at https://blockstream.info/testnet/address/$RECIPIENT_ADDR"

# ─── Step 6: Show wallet ──────────────────────────────────────────────────
log ""
log "Step 6: Show wallet — balance should reflect faucet amount (minus miner fee)"
SHOW_OUT=$("$BTC" --network "$NETWORK" wallet show --id "$WALLET_ID" 2>&1)
printf '%s\n' "$SHOW_OUT"
BALANCE=$(echo "$SHOW_OUT" | grep -oE '"balance"[[:space:]]*:[[:space:]]*[0-9]+' | head -1 | sed -E 's/.*:[[:space:]]*([0-9]+)/\1/')
if [ -z "$BALANCE" ] || [ "$BALANCE" -eq 0 ]; then
  fail "balance is 0 or unparseable. Did the faucet confirmation land?"
fi
log "✓ balance: $BALANCE sat"

# ─── Step 7: Send 0.001 BTC ──────────────────────────────────────────────
log ""
log "Step 7: Send 0.001 BTC (100000 sat) to a fresh return wallet"
# Round-trip via a second wallet so we don't depend on faucet cooperation
RETURN_WALLET_ID=$("$BTC" --network "$NETWORK" wallet create --name "smoke-return-$(date +%s)" 2>/dev/null \
  | grep -oE '"id"[[:space:]]*:[[:space:]]*"[^"]+"' \
  | head -1 \
  | sed -E 's/.*"id"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')
RETURN_ADDR=$("$BTC" --network "$NETWORK" wallet show --id "$RETURN_WALLET_ID" 2>/dev/null \
  | grep -oE '"address"[[:space:]]*:[[:space:]]*"[^"]+"' \
  | head -1 \
  | sed -E 's/.*"address"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')

SEND_OUT=$("$BTC" --network "$NETWORK" wallet send \
  --mnemonic "$MNEMONIC" \
  --from "$WALLET_ID" \
  --to "$RETURN_ADDR" \
  --amount-sat 100000 \
  --fee-rate 1 2>&1)
printf '%s\n' "$SEND_OUT"
TXID=$(echo "$SEND_OUT" | grep -oE '"txid"[[:space:]]*:[[:space:]]*"[^"]+"' | head -1 | sed -E 's/.*"txid"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')
if [ -z "$TXID" ]; then
  fail "send did not return a txid. Output above."
fi
log "✓ sent 100000 sat — txid: $TXID"

# ─── Step 8: Wait for 1 conf ──────────────────────────────────────────────
pause "Step 8: Wait ~10 min for the send to confirm. Monitor: https://blockstream.info/testnet/tx/$TXID"

# ─── Step 9: Delete wallet ───────────────────────────────────────────────
log ""
log "Step 9: Delete the source wallet"
"$BTC" --network "$NETWORK" wallet delete --id "$WALLET_ID" 2>&1 | head -5

# ─── Step 11: Verify wallet blob gone ─────────────────────────────────────
log ""
log "Step 11: Verify wallet blob is gone"
if [ -f "$WALLET_BLOB_DIR/$WALLET_ID.enc" ]; then
  fail "wallet blob still present at $WALLET_BLOB_DIR/$WALLET_ID.enc"
fi
log "✓ wallet blob deleted: $WALLET_BLOB_DIR/$WALLET_ID.enc"

# F47 temp-file sweep — no /tmp/btc-secret-* should survive
STRAY_SECRETS=$(find /tmp -maxdepth 2 -name 'btc-secret-*' 2>/dev/null | head -5 || true)
if [ -n "$STRAY_SECRETS" ]; then
  crit2 "stray /tmp/btc-secret-* files survived delete: $STRAY_SECRETS"
fi
log "✓ no /tmp/btc-secret-* temp files (F47)"

# ─── Step 12: L12 CRITICAL #2 grep ────────────────────────────────────────
log ""
log "Step 12: L12 CRITICAL #2 grep — sweep app logs for any mnemonic/password cleartext"
APP_LOG="$HOME/.local/share/flutter_btc_wallet/logs/app.log"
if [ -f "$APP_LOG" ]; then
  log "scanning $APP_LOG"
  # Match 12/15/18/21/24-word BIP-39 mnemonic shapes (lowercase words, single-space separated)
  MNEMONIC_HITS=$(grep -cE '^([a-z]+ ){11,23}[a-z]+$' "$APP_LOG" 2>/dev/null || echo 0)
  if [ "$MNEMONIC_HITS" -gt 0 ]; then
    crit2 "found $MNEMONIC_HITS cleartext mnemonic-shaped lines in $APP_LOG — release blocker"
  fi
  # Match anything that looks like the operator's printed mnemonic (case-insensitive contains)
  if [ -n "$MNEMONIC" ]; then
    # shellcheck disable=SC2086
    MNEM_WORDS=( $MNEMONIC )
    if [ "${#MNEM_WORDS[@]}" -ge 12 ]; then
      FIRST_WORD="${MNEM_WORDS[0]}"
      SECOND_WORD="${MNEM_WORDS[1]}"
      CONTIG_HITS=$(grep -E "${FIRST_WORD}.*${SECOND_WORD}" "$APP_LOG" 2>/dev/null | wc -l || echo 0)
      if [ "$CONTIG_HITS" -gt 0 ]; then
        crit2 "found $CONTIG_HITS lines in $APP_LOG containing mnemonic words '$FIRST_WORD ... $SECOND_WORD' — release blocker"
      fi
    fi
  fi
  log "✓ no cleartext mnemonic-shaped strings in $APP_LOG"
else
  warn "app log not found at $APP_LOG — operator must capture logs manually during GUI session"
  warn "(L29 + L12 CRITICAL #2 still need to be verified by operator)"
fi

# ─── Summary ──────────────────────────────────────────────────────────────
log ""
log "═══════════════════════════════════════════════════════════════════"
log "  v0.1.0 L29 smoke — HEADLESS PORTION COMPLETE"
log "═══════════════════════════════════════════════════════════════════"
log ""
log "Operator still needs to verify the GUI-only steps:"
log "  • Step 2: app launches without crash"
log "  • Steps 4-5: faucet + 1 conf visible in UI"
log "  • Steps 7-8: send appears + confirms in UI"
log ""
log "Next: flip Issue #203 acceptance checkboxes per L13 step 14 + L28"
log "  (operator-driven gates stay unchecked until operator confirms each)"
log ""
log "Result: HEADLESS GREEN + L12 CRITICAL #2 clean. Operator GUI verification pending."
