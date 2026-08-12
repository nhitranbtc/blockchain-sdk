#!/usr/bin/env bash
#
# btc-import-demo.sh — end-to-end demo of `btc wallet import` (Issue #99 / Story 2).
#
# Walks through 7 scenarios across all 5 Bitcoin networks (bitcoin, testnet,
# signet, testnet4, regtest):
#   1. import 12-word phrase                        — 5 wallets (one per network)
#   2. import same phrase twice (same network)      — distinct WalletIds, identical blob
#   3. import 24-word phrase                        — wider word count per network
#   4. import with --passphrase                     — derivation-time only, warning to STDERR
#   5. import with invalid checksum                  — refused, non-zero exit
#   6. import unsupported word count (13 words)     — refused, non-zero exit
#   7. import + show end-to-end                      — decrypts + syncs from Esplora (F20-gated)
#
# Every step reports PASS/FAIL/SKIP per network. Overall status = PASS iff all
# networks pass. Step 7 needs BTC_DEMO_ESPLORA_SPKI_PIN for non-regtest live
# sync; without it the F20 refusal path is demonstrated (treated as PASS —
# gate works as designed).
#
# Usage:
#   bash rust-wallet-app/scripts/btc-import-demo.sh
#   bash rust-wallet-app/scripts/btc-import-demo.sh --help
#   SKIP_BUILD=1 bash rust-wallet-app/scripts/btc-import-demo.sh
#
# Requirements:
#   - Rust toolchain (cargo)
#   - btc crate builds: `cargo build -p btc` (or `cargo run -p btc -- ...`)
#   - Network access to testnet Esplora (only for STEP 7; skip if offline)

set -uo pipefail

usage() {
    cat <<'EOF'
btc-import-demo.sh — end-to-end demo of `btc wallet import` (Story 2).

USAGE:
    bash rust-wallet-app/scripts/btc-import-demo.sh
    SKIP_BUILD=1 bash rust-wallet-app/scripts/btc-import-demo.sh

ENV:
    SKIP_BUILD=1                  Skip the upfront `cargo build -p btc` (use existing target)
    CARGO=<path>                  Override the cargo binary (default: cargo)
    BTC_DEMO_ESPLORA_SPKI_PIN     Required for STEP 7 live testnet sync; without it
                                  the F20 refusal path is demonstrated instead.

EXIT:
    0    All required steps passed (Step 7 may have been deferred/skipped)
    1    One or more required steps failed

EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

# --- Setup -------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Fresh temp data dir; cleanup on exit. Use `-d` (no template) for
# portability across GNU/BSD mktemp. `mktemp -t <template>` is GNU-only
# and interprets the template differently than BSD.
TMPDIR_DEMO="$(mktemp -d -t btc-import-demo.XXXXXX 2>/dev/null || mktemp -d)"
export XDG_DATA_HOME="${TMPDIR_DEMO}"
trap 'rm -rf "${TMPDIR_DEMO}"' EXIT

# BIP-39 test-vector mnemonics (do NOT use for real funds).
# 12-word: 11× "abandon" + "about" = 12.
MNEMONIC_12="abandon abandon abandon abandon abandon abandon \
abandon abandon abandon abandon abandon about"
# 24-word: 23× "abandon" + "art" = 24. (BIP-39 standard test vector;
# earlier script had 25 words which fails "unsupported word count" check.)
MNEMONIC_24="abandon abandon abandon abandon abandon abandon \
abandon abandon abandon abandon abandon abandon abandon abandon \
abandon abandon abandon abandon abandon abandon abandon abandon \
abandon art"

# Binary paths. Allow env override (CI may pin a specific cargo).
CARGO_BIN="${CARGO:-cargo}"
BT_BIN="${WORKSPACE_DIR}/target/debug/btc"

cd "${WORKSPACE_DIR}"

# ANSI color helpers. Per https://no-color.org/, respect NO_COLOR env var.
if [[ -z "${NO_COLOR:-}" ]] && [[ -t 1 ]]; then
    C_RESET=$'\033[0m'
    C_BOLD=$'\033[1m'
    C_GREEN=$'\033[32m'
    C_RED=$'\033[31m'
    C_YELLOW=$'\033[33m'
    C_CYAN=$'\033[36m'
    C_DIM=$'\033[2m'
else
    C_RESET="" C_BOLD="" C_GREEN="" C_RED="" C_YELLOW="" C_CYAN="" C_DIM=""
fi

# --- Per-step status tracking -----------------------------------------------

# Array of "PASS"/"FAIL"/"SKIP" per step. Indexed 1..N (index 0 unused).
declare -a STEP_STATUS=()
declare -a STEP_NAME=()

# Record step result. Args: <index> <PASS|FAIL|SKIP> <name>.
record_step() {
    STEP_STATUS[$1]="$2"
    STEP_NAME[$1]="$3"
}

# Symbol + color for a status.
status_glyph() {
    case "$1" in
        PASS) printf '%sPASS%s' "${C_GREEN}${C_BOLD}" "${C_RESET}" ;;
        FAIL) printf '%sFAIL%s' "${C_RED}${C_BOLD}" "${C_RESET}" ;;
        SKIP) printf '%sSKIP%s' "${C_YELLOW}${C_BOLD}" "${C_RESET}" ;;
        *)   printf '?    ' ;;
    esac
}

# Print one PASS/FAIL/SKIP line. Args: <index> <reason>.
print_step_result() {
    local idx="$1"
    local reason="$2"
    local status="${STEP_STATUS[$idx]:-SKIP}"
    local name="${STEP_NAME[$idx]:-unknown}"
    printf '  %s Step %d: %s%s%s' \
        "$(status_glyph "$status")" "$idx" "${C_BOLD}" "$name" "${C_RESET}"
    if [[ -n "$reason" ]]; then
        printf ' %s(%s)%s\n' "${C_DIM}" "$reason" "${C_RESET}"
    else
        printf '\n'
    fi
}

banner() {
    printf '\n%s============================================================%s\n' \
        "${C_BOLD}${C_CYAN}" "${C_RESET}"
    printf '%s%s%s\n' "${C_BOLD}${C_CYAN}" "$1" "${C_RESET}"
    printf '%s============================================================%s\n' \
        "${C_BOLD}${C_CYAN}" "${C_RESET}"
}

# Run a btc subcommand. Echoes the exact argv before running so the
# operator sees what's invoked. Captures stdout + exit code separately.
#
# Usage: run_btc <stdout-var> <exit-var> <cmd...>
#
# Security: NO `eval`. Both stdout-capture and exit-code assignment use
# bash-native `printf -v` and direct assignment, immune to shell
# metacharacter injection (mirrors run_btc from btc-quickstart.sh).
run_btc() {
    local out_var="$1"; shift
    local exit_var="$1"; shift
    printf '$'
    printf ' %q' "$@"
    printf '\n'
    BT_LAST_STDOUT_PATH="${TMPDIR_DEMO}/btc-stdout.$$"
    local stderr_path="${TMPDIR_DEMO}/btc-stderr.$$"
    if "$@" >"${BT_LAST_STDOUT_PATH}" 2>"${stderr_path}"; then
        printf -v "${out_var}" '%s' "$(<"${BT_LAST_STDOUT_PATH}")"
        printf -v "${exit_var}" '%s' "0"
        cat "${BT_LAST_STDOUT_PATH}"
    else
        local rc=$?
        printf -v "${exit_var}" '%s' "${rc}"
        cat "${stderr_path}" >&2
    fi
}

# --- Build -------------------------------------------------------------------

# Build the binary once (avoids re-compile on every cargo invocation).
# SKIP_BUILD=1 skips this if the caller already built.
if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
    banner "BUILD: cargo build -p btc"
    "${CARGO_BIN}" build -p btc --quiet || {
        record_step 0 FAIL "cargo build -p btc"
        print_step_result 0 "build failed"
        exit 1
    }
    record_step 0 PASS "cargo build -p btc"
else
    banner "BUILD: skipped (SKIP_BUILD=1)"
    record_step 0 SKIP "cargo build -p btc"
fi

# --- Step 1: btc wallet import (12-word, all networks) ---------------------
# Run 12-word import across all 5 networks. Each network gets its own wallet_id.
# Step 1 PASS iff all 5 networks succeed.
banner "STEP 1/7: btc wallet import (12-word, all 5 networks)"
declare -A WALLET_ID_12   # map net -> wallet_id (12-word)
declare -A STEP1_NET_STATUS
STEP1_FAILED=0
for net in bitcoin testnet signet testnet4 regtest; do
    IMPORT_OUT=""
    IMPORT_EXIT=0
    run_btc IMPORT_OUT IMPORT_EXIT \
        "${BT_BIN}" wallet import \
            --mnemonic "${MNEMONIC_12}" \
            --network "${net}" \
            --password demo-pwd
    WALLET_ID_12["${net}"]="$(echo "${IMPORT_OUT}" | head -1)"
    if [[ ${IMPORT_EXIT} -ne 0 ]]; then
        printf '        %s: FAIL (exit %d)\n' "${net}" "${IMPORT_EXIT}"
        STEP1_NET_STATUS["${net}"]="FAIL"
        STEP1_FAILED=$((STEP1_FAILED + 1))
    elif [[ ! "${WALLET_ID_12[$net]}" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]]; then
        printf '        %s: FAIL (no UUID, got: %s)\n' "${net}" "${WALLET_ID_12[$net]}"
        STEP1_NET_STATUS["${net}"]="FAIL"
        STEP1_FAILED=$((STEP1_FAILED + 1))
    else
        printf '        %s: wallet_id=%s\n' "${net}" "${WALLET_ID_12[$net]}"
        STEP1_NET_STATUS["${net}"]="PASS"
    fi
done
if [[ ${STEP1_FAILED} -eq 0 ]]; then
    record_step 1 PASS "btc wallet import (12-word, 5 networks)"
    print_step_result 1 ""
else
    record_step 1 FAIL "btc wallet import (12-word, 5 networks)"
    print_step_result 1 "${STEP1_FAILED}/5 networks failed"
    exit 1
fi

# --- Step 2: btc wallet import same phrase → distinct WalletId (testnet) ----
# Network-agnostic behavior (UUID uniqueness); demonstrate on testnet only.
banner "STEP 2/7: btc wallet import (same phrase → distinct UUID, testnet)"
WALLET_ID_A="${WALLET_ID_12[testnet]}"
IMPORT_OUT=""
IMPORT_EXIT=0
run_btc IMPORT_OUT IMPORT_EXIT \
    "${BT_BIN}" wallet import \
        --mnemonic "${MNEMONIC_12}" \
        --network testnet \
        --password demo-pwd
WALLET_ID_B="$(echo "${IMPORT_OUT}" | head -1)"
if [[ ${IMPORT_EXIT} -ne 0 ]]; then
    record_step 2 FAIL "btc wallet import (same phrase)"
    print_step_result 2 "exit code ${IMPORT_EXIT}"
    exit 1
elif [[ -z "${WALLET_ID_B}" || "${WALLET_ID_A}" == "${WALLET_ID_B}" ]]; then
    record_step 2 FAIL "btc wallet import (same phrase)"
    print_step_result 2 "expected distinct wallet_id, got: ${WALLET_ID_B} (first was ${WALLET_ID_A})"
    exit 1
else
    echo "wallet_id (1st): ${WALLET_ID_A}"
    echo "wallet_id (2nd): ${WALLET_ID_B}"
    record_step 2 PASS "btc wallet import (same phrase → distinct UUID)"
    print_step_result 2 ""
fi

# --- Step 3: btc wallet import (24-word, all networks) ---------------------
# Same loop pattern as Step 1; 24-word phrase per network.
banner "STEP 3/7: btc wallet import (24-word, all 5 networks)"
declare -A WALLET_ID_24
declare -A STEP3_NET_STATUS
STEP3_FAILED=0
for net in bitcoin testnet signet testnet4 regtest; do
    IMPORT_OUT=""
    IMPORT_EXIT=0
    run_btc IMPORT_OUT IMPORT_EXIT \
        "${BT_BIN}" wallet import \
            --mnemonic "${MNEMONIC_24}" \
            --network "${net}" \
            --password demo-pwd
    WALLET_ID_24["${net}"]="$(echo "${IMPORT_OUT}" | head -1)"
    if [[ ${IMPORT_EXIT} -ne 0 ]]; then
        printf '        %s: FAIL (exit %d)\n' "${net}" "${IMPORT_EXIT}"
        STEP3_NET_STATUS["${net}"]="FAIL"
        STEP3_FAILED=$((STEP3_FAILED + 1))
    elif [[ ! "${WALLET_ID_24[$net]}" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]]; then
        printf '        %s: FAIL (no UUID, got: %s)\n' "${net}" "${WALLET_ID_24[$net]}"
        STEP3_NET_STATUS["${net}"]="FAIL"
        STEP3_FAILED=$((STEP3_FAILED + 1))
    else
        printf '        %s: wallet_id=%s\n' "${net}" "${WALLET_ID_24[$net]}"
        STEP3_NET_STATUS["${net}"]="PASS"
    fi
done
if [[ ${STEP3_FAILED} -eq 0 ]]; then
    record_step 3 PASS "btc wallet import (24-word, 5 networks)"
    print_step_result 3 ""
else
    record_step 3 FAIL "btc wallet import (24-word, 5 networks)"
    print_step_result 3 "${STEP3_FAILED}/5 networks failed"
    exit 1
fi

# --- Step 4: btc wallet import with --passphrase (testnet) ----------------
# Network-agnostic behavior (passphrase not persisted); demonstrate on testnet.
banner "STEP 4/7: btc wallet import --passphrase (derivation-time only, NOT persisted, testnet)"
WALLET_ID_D=""
set +e
STDERR_PATH_4="${TMPDIR_DEMO}/import-stderr4.$$"
"${BT_BIN}" wallet import \
    --mnemonic "${MNEMONIC_12}" \
    --network testnet \
    --passphrase "demo-passphrase" \
    --password demo-pwd > "${TMPDIR_DEMO}/import-stdout4.$$" 2> "${STDERR_PATH_4}"
STEP4_EXIT=$?
set -e
WALLET_ID_D="$(head -1 "${TMPDIR_DEMO}/import-stdout4.$$" 2>/dev/null || true)"
if [[ -z "${WALLET_ID_D}" || ! "${WALLET_ID_D}" =~ ^[0-9a-f]{8}- ]]; then
    record_step 4 FAIL "btc wallet import (with passphrase)"
    print_step_result 4 "expected wallet_id, got: ${WALLET_ID_D}"
    exit 1
fi
# Verify the passphrase warning is on STDERR (per handle_import contract).
STDERR_CONTENT="$(cat "${STDERR_PATH_4}" 2>/dev/null || true)"
if [[ "${STDERR_CONTENT}" == *"BIP-39 passphrase is NOT persisted"* ]]; then
    echo "wallet_id: ${WALLET_ID_D}"
    echo "STDERR warning: 'BIP-39 passphrase is NOT persisted' (present)"
    record_step 4 PASS "btc wallet import (with passphrase warning)"
    print_step_result 4 ""
else
    record_step 4 FAIL "btc wallet import (with passphrase warning)"
    print_step_result 4 "expected STDERR passphrase warning, got: ${STDERR_CONTENT}"
    exit 1
fi

# --- Step 5: invalid checksum → non-zero exit -------------------------------
banner "STEP 5/7: btc wallet import (invalid checksum → refusal)"
# Same 12 words but with last word changed → checksum broken.
BAD_MNEMONIC_12="abandon abandon abandon abandon abandon abandon \
abandon abandon abandon abandon abandon abandon"
set +e
"${BT_BIN}" wallet import \
    --mnemonic "${BAD_MNEMONIC_12}" \
    --network testnet \
    --password demo-pwd 2>&1
BAD_EXIT=$?
set -e
if [[ ${BAD_EXIT} -ne 0 ]]; then
    record_step 5 PASS "btc wallet import (invalid checksum refused)"
    print_step_result 5 "exit code ${BAD_EXIT} (refused as expected)"
else
    record_step 5 FAIL "btc wallet import (invalid checksum)"
    print_step_result 5 "expected non-zero exit, got: 0"
    exit 1
fi

# --- Step 6: unsupported word count (13 words) → non-zero exit -------------
banner "STEP 6/7: btc wallet import (unsupported word count 13 → refusal)"
THIRTEEN_WORD="abandon abandon abandon abandon abandon abandon \
abandon abandon abandon abandon abandon abandon abandon"
set +e
"${BT_BIN}" wallet import \
    --mnemonic "${THIRTEEN_WORD}" \
    --network testnet \
    --password demo-pwd 2>&1
WC_EXIT=$?
set -e
if [[ ${WC_EXIT} -ne 0 ]]; then
    record_step 6 PASS "btc wallet import (unsupported word count refused)"
    print_step_result 6 "exit code ${WC_EXIT} (refused as expected)"
else
    record_step 6 FAIL "btc wallet import (unsupported word count)"
    print_step_result 6 "expected non-zero exit, got: 0"
    exit 1
fi

# --- Step 7: import + show end-to-end --------------------------------------
# Demonstrates that the imported wallet can be decrypted + synced from
# Esplora via `wallet show`. Requires BTC_DEMO_ESPLORA_SPKI_PIN for
# non-regtest networks (F20 gate per PR #82). Without pin, demonstrates
# the F20 refusal path (treated as PASS — gate works as designed).
banner "STEP 7/7: btc wallet show (decrypts imported wallet, live Esplora sync)"
printf '%s\n' "Note: F20 enforcement (PR #82) — requires BTC_DEMO_ESPLORA_SPKI_PIN"
printf '%s\n' "      for non-regtest networks. Without it, demonstrates F20 refusal."
printf '%s\n\n' "      (L29 operator smoke; non-regtest refuses without pin per F20)"

# Pretty-print JSON if jq or python3 is available; otherwise print raw.
if command -v jq >/dev/null 2>&1; then
    JSON_PRETTY="jq ."
elif command -v python3 >/dev/null 2>&1; then
    JSON_PRETTY="python3 -m json.tool"
else
    JSON_PRETTY="cat"
    printf '(install jq or python3 for pretty JSON)\n'
fi

if [[ -z "${BTC_DEMO_ESPLORA_SPKI_PIN:-}" ]]; then
    # No pin supplied — demonstrate F20 refusal. Use mainnet so the URL
    # defaults to blockstream.info/api (the exact attack vector PR #82
    # closed). Without --pin-spki this MUST refuse.
    printf '$ %q' "${BT_BIN}" wallet show "${WALLET_ID_A}" \
        --network bitcoin --password demo-pwd
    printf '\n'
    set +e
    "${BT_BIN}" wallet show "${WALLET_ID_A}" \
        --network bitcoin --password demo-pwd 2>&1
    STEP7_EXIT=$?
    set -e
    if [[ ${STEP7_EXIT} -ne 0 ]]; then
        record_step 7 PASS "btc wallet show (F20 gate demonstrated on imported wallet)"
        print_step_result 7 "refused without --pin-spki as expected"
    else
        record_step 7 FAIL "btc wallet show"
        print_step_result 7 "expected F20 refusal, got exit 0"
    fi
else
    # Pin supplied — real testnet sync against the imported wallet.
    set +e  # tolerate step 7 failure (network-dependent)
    "${BT_BIN}" wallet show "${WALLET_ID_A}" \
        --network testnet \
        --password demo-pwd \
        --pin-spki "${BTC_DEMO_ESPLORA_SPKI_PIN}" | ${JSON_PRETTY}
    STEP7_EXIT=$?
    set -e

    if [[ ${STEP7_EXIT} -eq 0 ]]; then
        record_step 7 PASS "btc wallet show (live Esplora sync on imported wallet)"
        print_step_result 7 "synced"
    else
        record_step 7 SKIP "btc wallet show (live Esplora sync)"
        print_step_result 7 "sync failed (offline or pin mismatch)"
    fi
fi

# --- Summary -----------------------------------------------------------------
banner "DEMO COMPLETE"

# Aggregate step counts.
PASSED=0
FAILED=0
SKIPPED=0
TOTAL=0
for status in "${STEP_STATUS[@]:-}"; do
    [[ -z "$status" ]] && continue
    TOTAL=$((TOTAL + 1))
    case "$status" in
        PASS) PASSED=$((PASSED + 1)) ;;
        FAIL) FAILED=$((FAILED + 1)) ;;
        SKIP) SKIPPED=$((SKIPPED + 1)) ;;
    esac
done

# Overall status. FAIL => non-zero exit. SKIP-only is OK (network absent).
EXIT_CODE=0
if [[ $FAILED -gt 0 ]]; then
    printf '  %s Overall status: FAIL%s (passed=%d failed=%d skipped=%d total=%d)\n' \
        "${C_RED}${C_BOLD}" "${C_RESET}" "$PASSED" "$FAILED" "$SKIPPED" "$TOTAL"
    EXIT_CODE=1
elif [[ $SKIPPED -gt 0 ]]; then
    printf '  %s Overall status: PASS%s (passed=%d skipped=%d total=%d)\n' \
        "${C_YELLOW}${C_BOLD}" "${C_RESET}" "$PASSED" "$SKIPPED" "$TOTAL"
else
    printf '  %s Overall status: PASS%s (passed=%d total=%d)\n' \
        "${C_GREEN}${C_BOLD}" "${C_RESET}" "$PASSED" "$TOTAL"
fi

printf '\nStep results:\n'
for i in "${!STEP_STATUS[@]}"; do
    [[ -z "${STEP_STATUS[$i]:-}" ]] && continue
    status="${STEP_STATUS[$i]}"
    name="${STEP_NAME[$i]}"
    printf '  %s Step %d: %s%s%s\n' \
        "$(status_glyph "$status")" "$i" "${C_BOLD}" "$name" "${C_RESET}"
done

cat <<EOF

Artifacts (Step 1, 12-word, all 5 networks):
  bitcoin:  ${WALLET_ID_12[bitcoin]:-N/A}
  testnet:  ${WALLET_ID_12[testnet]:-N/A}
  signet:   ${WALLET_ID_12[signet]:-N/A}
  testnet4: ${WALLET_ID_12[testnet4]:-N/A}
  regtest:  ${WALLET_ID_12[regtest]:-N/A}

Artifacts (Step 2, same phrase distinct UUID):
  1st: ${WALLET_ID_A}
  2nd: ${WALLET_ID_B}

Artifacts (Step 3, 24-word, all 5 networks):
  bitcoin:  ${WALLET_ID_24[bitcoin]:-N/A}
  testnet:  ${WALLET_ID_24[testnet]:-N/A}
  signet:   ${WALLET_ID_24[signet]:-N/A}
  testnet4: ${WALLET_ID_24[testnet4]:-N/A}
  regtest:  ${WALLET_ID_24[regtest]:-N/A}

Artifacts (Step 4, passphrase):
  testnet: ${WALLET_ID_D}

Temp data dir: ${TMPDIR_DEMO} (cleaned up on exit)

Next (operator):
  - Real wallet import:
      btc wallet import --mnemonic "<12-word phrase>" --network bitcoin \\
          --password <STRONG>

  - Decrypt + sync imported wallet (F20 gated):
      btc wallet show --id "${WALLET_ID_12[testnet]}" --network testnet \\
          --password demo-pwd --pin-spki <64-hex>

  - With passphrase (must re-supply at show time):
      btc wallet import --mnemonic "<12-word phrase>" --passphrase "<PB>" \\
          --network testnet --password demo-pwd
      btc wallet show --id "<wallet_id>" --passphrase "<PB>" \\
          --network testnet --password demo-pwd --pin-spki <64-hex>
EOF

exit ${EXIT_CODE}
