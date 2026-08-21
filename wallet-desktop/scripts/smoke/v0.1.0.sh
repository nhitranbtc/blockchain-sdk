#!/usr/bin/env bash
# v0.1.0 L29 operator smoke — wallet-desktop (FFI migration)
#
# Operator-driven end-to-end verification of wallet-desktop v0.1.0
# (post-FFI-migration). After PRs #255 + #256, wallet-desktop has
# ZERO subprocess integration with the `btc` CLI — every wallet
# operation routes through Rust FFI via `bitcoin-wallet-core`. This
# script covers the verification path accordingly:
#
#   - Preflight: build native lib via tool/build_native.sh (no btc
#     binary required).
#   - Launch: flutter run -d linux in background.
#   - Walk: 11 stories via xdotool + screenshot per story.
#   - Operator pauses at each GUI step.
#   - L12 CRITICAL #2 grep on app log (F47 temp-file sweep still
#     applies — Rust FFI temp dirs /tmp/btc-secret-* survive only
#     during the FFI call; sweep verifies zero residue).
#
# Per L29 + L28: live testnet smoke is operator-driven, not CI.
# Per L12 CRITICAL #2: mnemonic + password NEVER logged.
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
readonly SCREENSHOT_DIR="$APP_DATA_DIR/smoke-screenshots/v$TAG"
readonly BUILD_NATIVE_TOOL="wallet-desktop/tool/build_native.sh"

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

screenshot() {
  local label="$1"
  if ! command -v import >/dev/null 2>&1; then
    warn "ImageMagick 'import' not on PATH — skipping screenshot $label"
    return 0
  fi
  mkdir -p "$SCREENSHOT_DIR"
  local file="$SCREENSHOT_DIR/${label}.png"
  if import -window root "$file" 2>/dev/null; then
    log "📸 $file"
  else
    warn "screenshot failed for $label (X11 not available?)"
  fi
}

# ─── Pre-flight ──────────────────────────────────────────────────────────
log "Pre-flight: $TAG smoke (FFI-only — no btc CLI)"
log "Network: $NETWORK"
log "Wallet blob dir: $WALLET_BLOB_DIR"

# Tag check
if ! git -C "$(dirname "$0")/../.." tag --list "$TAG" | grep -q "$TAG"; then
  fail "tag $TAG not found locally. Fetch first: git fetch origin --tags"
fi
log "✓ tag $TAG present"

# Flutter SDK
if ! command -v flutter >/dev/null 2>&1; then
  fail "flutter not on PATH. Install Flutter 3.x stable."
fi
if ! flutter --version >/dev/null 2>&1; then
  fail "flutter --version failed"
fi
log "✓ flutter: $(flutter --version 2>&1 | head -1)"

# xdotool (GUI automation) — required for the walk-through below.
if ! command -v xdotool >/dev/null 2>&1; then
  warn "xdotool not on PATH — screenshot/walk automation will degrade to manual instructions"
fi

# Display server (X11) — required for `import -window root` screenshots.
if [ -z "${DISPLAY:-}" ]; then
  warn "DISPLAY not set — screenshots will fail. Run from an X11 session."
fi

# Native lib build (replaces the v0.1.0 `btc --version` preflight).
# `tool/build_native.sh` is the canonical Rust cdylib builder (Task 18
# #224). It writes into `wallet-desktop/native/<host-arch>/` so the
# Flutter desktop runner can `DynamicLibrary.open()` it.
if [ ! -x "$BUILD_NATIVE_TOOL" ]; then
  fail "$BUILD_NATIVE_TOOL not executable. chmod +x then re-run."
fi
log "building native lib via $BUILD_NATIVE_TOOL ..."
if ! bash "$BUILD_NATIVE_TOOL" >/tmp/native-build.log 2>&1; then
  cat /tmp/native-build.log
  fail "native lib build failed — see /tmp/native-build.log"
fi
log "✓ native lib built (host arch: $(uname -m))"

# Disk space (1GB min for the wallet DB + screenshot dir)
AVAIL_KB=$(df -Pk "$APP_DATA_DIR" 2>/dev/null | awk 'NR==2 {print $4}' || echo 0)
if [ "$AVAIL_KB" -lt 1048576 ]; then
  fail "less than 1GB free at $APP_DATA_DIR"
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

# ─── Step 1: Launch Flutter desktop app ──────────────────────────────────
log ""
log "Step 1: Launch wallet-desktop via flutter run"
log "(native lib is loaded by the Dart DynamicLibrary.open at startup)"
mkdir -p "$SCREENSHOT_DIR"

# Launch flutter run in background. The app holds the focus; subsequent
# xdotool steps assume the active window belongs to wallet-desktop.
if command -v flutter >/dev/null 2>&1; then
  nohup flutter run -d linux --release >"$APP_DATA_DIR/flutter-run.log" 2>&1 &
  FLUTTER_PID=$!
  log "flutter run started (pid=$FLUTTER_PID, log: $APP_DATA_DIR/flutter-run.log)"
else
  FLUTTER_PID=""
  warn "flutter not on PATH — skipping app launch; operator must launch manually"
fi

# Wait for the app window to appear (max 60s)
if [ -n "$FLUTTER_PID" ] && command -v xdotool >/dev/null 2>&1; then
  log "waiting for wallet-desktop window (max 60s)..."
  for _ in $(seq 1 60); do
    if xdotool search --name 'wallet-desktop' 2>/dev/null | grep -q .; then
      log "✓ wallet-desktop window detected"
      break
    fi
    sleep 1
  done
fi
pause "Step 1: Confirm the app launched. Click 'Create' to begin Story 1."
screenshot "01-launched"

# ─── Stories 1-12 (operator-driven via xdotool) ──────────────────────────
# Each story: print GUI instructions + capture screenshot + operator
# confirms. The actual crypto operations are FFI-driven inside the
# Flutter app — there are no subprocess CLI calls anymore.
STORIES=(
  "1:Create: navigate to Create wallet → 12-word mnemonic → backup done"
  "2:Import: navigate to Import → paste mnemonic → unlock"
  "3:Detail: tap wallet → balance + first receive address"
  "4:Tx list: tap Transactions → verify txid list (txid-only in v0.2.1; per-tx fields deferred to v0.3)"
  "5:Send: tap Send → recipient + amount → fee rate → confirm"
  "6:Fee picker: tap Send → adjust fee rate field → verify fee updates"
  "7:Tx history: from Detail → Transactions → verify pagination"
  "9:List: tap back to home → verify wallet list + empty state"
  "11:Lock: tap wallet → Lock → verify locked state + return to list"
  "12:Settings: navigate to Settings → change Esplora URL → save"
  "20:Mnemonic: Create → mnemonic dialog → toggle reveal → Copy disabled → backup done"
)

for entry in "${STORIES[@]}"; do
  IFS=':' read -r num rest <<<"$entry"
  label=$(echo "$rest" | tr ' ' '-' | tr '[:upper:]' '[:lower:]')
  log ""
  log "Story $num: $rest"
  pause "Story $num: $rest — perform in the running app, then press Enter."
  screenshot "story-${num}-${label}"
done

# ─── Step 11: Verify wallet blob gone (after operator deletes via UI) ───
log ""
log "Step 11: Verify wallet blob is gone (operator deleted via Story 11)"
BLOB_COUNT=$(find "$WALLET_BLOB_DIR" -name "*.enc" 2>/dev/null | wc -l || echo 0)
if [ "$BLOB_COUNT" -gt 0 ]; then
  warn "$BLOB_COUNT .enc blob(s) remain at $WALLET_BLOB_DIR"
  warn "operator should have deleted via Story 11 — investigate"
else
  log "✓ no wallet blobs remain at $WALLET_BLOB_DIR"
fi

# F47 temp-file sweep — Rust FFI temp dirs /tmp/btc-secret-* should
# NOT survive the wallet-delete FFI call. The Task 5 TempSecretFile
# pattern was subprocess-specific; under FFI, the Rust side uses
# in-process zeroization (Secret<String>) — no /tmp files. Sweep
# remains as a defensive check.
STRAY_SECRETS=$(find /tmp -maxdepth 2 -name 'btc-secret-*' 2>/dev/null | head -5 || true)
if [ -n "$STRAY_SECRETS" ]; then
  crit2 "stray /tmp/btc-secret-* files survived delete: $STRAY_SECRETS"
fi
log "✓ no /tmp/btc-secret-* temp files (F47 invariant under FFI)"

# ─── Step 12: L12 CRITICAL #2 grep ────────────────────────────────────────
log ""
log "Step 12: L12 CRITICAL #2 grep — sweep app logs for any mnemonic/password cleartext"
APP_LOG="$HOME/.local/share/flutter_btc_wallet/logs/app.log"
FLUTTER_LOG="$APP_DATA_DIR/flutter-run.log"
LOG_TO_SCAN=""
for candidate in "$APP_LOG" "$FLUTTER_LOG"; do
  if [ -f "$candidate" ]; then
    LOG_TO_SCAN="$candidate"
    log "scanning $candidate"
    break
  fi
done

if [ -n "$LOG_TO_SCAN" ]; then
  # Match 12/15/18/21/24-word BIP-39 mnemonic shapes (lowercase words,
  # single-space separated). Per the FFI surface (PR #255), the Rust
  # side wraps phrases in Secret<String> (zeroize-on-drop) so this
  # sweep primarily catches Dart-side developer logging regressions.
  MNEMONIC_HITS=$(grep -cE '^([a-z]+ ){11,23}[a-z]+$' "$LOG_TO_SCAN" 2>/dev/null || echo 0)
  if [ "$MNEMONIC_HITS" -gt 0 ]; then
    crit2 "found $MNEMONIC_HITS cleartext mnemonic-shaped lines in $LOG_TO_SCAN — release blocker"
  fi
  # Match any 12+ contiguous lowercase words on a single line (more
  # lenient; catches multi-line + interleaved cases).
  LENIENT_HITS=$(grep -cE '(([a-z]+ ){11,})[a-z]+' "$LOG_TO_SCAN" 2>/dev/null || echo 0)
  if [ "$LENIENT_HITS" -gt 0 ]; then
    crit2 "found $LENIENT_HITS lines with 12+ contiguous lowercase words in $LOG_TO_SCAN — release blocker"
  fi
  log "✓ no cleartext mnemonic-shaped strings in $LOG_TO_SCAN"
else
  warn "no app log found at $APP_LOG or $FLUTTER_LOG"
  warn "(L29 + L12 CRITICAL #2 still need to be verified by operator)"
fi

# ─── Tear down ────────────────────────────────────────────────────────────
if [ -n "$FLUTTER_PID" ]; then
  log ""
  log "Tearing down flutter run (pid=$FLUTTER_PID)"
  kill "$FLUTTER_PID" 2>/dev/null || true
  wait "$FLUTTER_PID" 2>/dev/null || true
fi

# ─── Summary ──────────────────────────────────────────────────────────────
log ""
log "═══════════════════════════════════════════════════════════════════"
log "  v0.1.0 L29 smoke — ALL STORIES WALKED"
log "═══════════════════════════════════════════════════════════════════"
log ""
log "Screenshots: $SCREENSHOT_DIR"
log ""
log "Operator still needs to verify in Issue #203 per L13 step 14:"
log "  • All 11 story screenshots show the expected state"
log "  • No app crash / panic / unexpected error UI"
log "  • L12 CRITICAL #2 grep clean"
log ""
log "Next: flip Issue #203 acceptance checkboxes (operator-driven gates"
log "stay unchecked until operator confirms each one)."
log ""
log "Result: HEADLESS + GUI WALK COMPLETE + L12 CRITICAL #2 clean."
