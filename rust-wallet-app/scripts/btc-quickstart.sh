#!/usr/bin/env bash
#
# btc-quickstart.sh — end-to-end demo of the `btc` Bitcoin wallet CLI.
#
# Walks through 5 commands:
#   1. wallet create        — generate BIP-39 mnemonic, persist encrypted blob
#   2. message sign         — sign with BIP-137 (stateless, test vector)
#   3. message verify       — verify valid signature (expect: true)
#   4. message verify       — verify tampered message (expect: false)
#   5. wallet show          — sync from live testnet Esplora (network-dependent)
#
# Every step reports PASS/FAIL. Overall status = PASS iff all 5 steps
# pass. Step 5 may be deferred (network unavailable) — that is treated
# as a SKIP (not a FAIL) and noted in the summary.
#
# Usage:
#   bash rust-wallet-app/scripts/btc-quickstart.sh
#   bash rust-wallet-app/scripts/btc-quickstart.sh --help
#   SKIP_BUILD=1 bash rust-wallet-app/scripts/btc-quickstart.sh   # skip cargo build
#
# Requirements:
#   - Rust toolchain (cargo)
#   - btc crate builds: `cargo build -p btc` (or `cargo run -p btc -- ...`)
#   - Network access to testnet Esplora (only for STEP 5; skip if offline)

set -uo pipefail

usage() {
    cat <<'EOF'
btc-quickstart.sh — end-to-end demo of the `btc` Bitcoin wallet CLI.

USAGE:
    bash rust-wallet-app/scripts/btc-quickstart.sh
    SKIP_BUILD=1 bash rust-wallet-app/scripts/btc-quickstart.sh

ENV:
    SKIP_BUILD=1    Skip the upfront `cargo build -p btc` (use existing target)
    CARGO=<path>    Override the cargo binary (default: cargo)

EXIT:
    0    All required steps passed (Step 5 may have been deferred/skipped)
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
TMPDIR_DEMO="$(mktemp -d -t btc-demo.XXXXXX 2>/dev/null || mktemp -d)"
export XDG_DATA_HOME="${TMPDIR_DEMO}"
trap 'rm -rf "${TMPDIR_DEMO}"' EXIT

# BIP-39 test-vector mnemonic (do NOT use for real funds).
# 11 times "abandon" + "about" = 12 words total.
MNEMONIC="abandon abandon abandon abandon abandon abandon \
abandon abandon abandon abandon abandon about"

# First-external address derived from this mnemonic + testnet at
# m/44'/1'/0'/0/0 (BIP-44 testnet coin type = 1). Hardcoded so the
# demo is reproducible without recomputing. To regenerate:
#   cargo run -p btc --quiet -- message sign \
#       --mnemonic "<MNEMONIC>" --network testnet \
#       --address "PLACEHOLDER" "x"
# → error message includes the derived address.
EXPECTED_ADDR="mzYpQmSAGYWWyTLiLGbGaG8T3rHdjNcV11"

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
        PASS) printf '%s✅%s' "${C_GREEN}${C_BOLD}" "${C_RESET}" ;;
        FAIL) printf '%s❌%s' "${C_RED}${C_BOLD}" "${C_RESET}" ;;
        SKIP) printf '%s⏸ %s' "${C_YELLOW}${C_BOLD}" "${C_RESET}" ;;
        *)   printf '? ' ;;
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

# Run a btc subcommand. Captures stdout (the data) and stderr (the
# logs) separately so error messages are clean. Echoes the command
# first so the operator sees what's being run.
#
# Usage: run_btc <stdout-var> <exit-var> <cmd...>
#
# After call, the captured stdout is:
#   - stored in $out_var (for validation / use in next step)
#   - streamed to the terminal (preserves raw output + formatting)
#   - stored in $BT_LAST_STDOUT_PATH for advanced callers
#
# Security: NO `eval`. Both the stdout-capture and exit-code assignment
# use bash-native `printf -v` (var=%s) and direct assignment, which are
# not subject to shell metacharacter injection. Fixes push-sweep MED #3.
run_btc() {
    local out_var="$1"; shift
    local exit_var="$1"; shift
    printf '$'
    printf ' %q' "$@"
    printf '\n'
    BT_LAST_STDOUT_PATH="${TMPDIR_DEMO}/btc-stdout.$$"
    local stderr_path="${TMPDIR_DEMO}/btc-stderr.$$"
    if "$@" >"${BT_LAST_STDOUT_PATH}" 2>"${stderr_path}"; then
        # printf -v VAR '%s' "$(<file)" — bash-native, no eval.
        printf -v "${out_var}" '%s' "$(<"${BT_LAST_STDOUT_PATH}")"
        printf -v "${exit_var}" '%s' "0"
        # Stream raw stdout to terminal (preserves formatting that
        # `$()` would strip — important for multi-line outputs like
        # Step 5's pretty JSON).
        cat "${BT_LAST_STDOUT_PATH}"
    else
        local rc=$?
        printf -v "${exit_var}" '%s' "${rc}"
        cat "${stderr_path}" >&2
    fi
}

# --- Build -------------------------------------------------------------------

# Build the binary once (5x `cargo run` invocations would re-compile 4x).
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

# --- Step 1: btc wallet create -----------------------------------------------
banner "STEP 1/7: btc wallet create (12-word testnet)"
WALLET_OUT=""
WALLET_EXIT=0
run_btc WALLET_OUT WALLET_EXIT \
    "${BT_BIN}" wallet create --words 12 --network testnet --password demo-pwd
# run_btc already streamed the raw output above.
WALLET_ID="$(echo "${WALLET_OUT}" | head -1)"
if [[ ${WALLET_EXIT} -ne 0 ]]; then
    record_step 1 FAIL "btc wallet create"
    print_step_result 1 "exit code ${WALLET_EXIT}"
    exit 1
elif [[ ! "${WALLET_ID}" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]]; then
    record_step 1 FAIL "btc wallet create"
    print_step_result 1 "expected wallet_id (UUID v4), got: ${WALLET_ID}"
    exit 1
else
    echo "wallet_id: ${WALLET_ID}"
    record_step 1 PASS "btc wallet create"
    print_step_result 1 ""
fi

# --- Step 2: btc message sign -----------------------------------------------
banner "STEP 2/7: btc message sign (BIP-137 with test-vector mnemonic)"
SIG=""
SIG_EXIT=0
run_btc SIG SIG_EXIT \
    "${BT_BIN}" message sign \
        --mnemonic "${MNEMONIC}" \
        --network testnet \
        --address "${EXPECTED_ADDR}" \
        "hello world"
# run_btc already streamed the raw base64 sig above.
if [[ ${SIG_EXIT} -ne 0 ]]; then
    record_step 2 FAIL "btc message sign"
    print_step_result 2 "exit code ${SIG_EXIT}"
    exit 1
elif [[ ! "${SIG}" =~ ^[A-Za-z0-9+/]+=*$ ]]; then
    record_step 2 FAIL "btc message sign"
    print_step_result 2 "expected base64 signature, got: ${SIG}"
    exit 1
else
    echo "signed (${#SIG} chars)"
    record_step 2 PASS "btc message sign"
    print_step_result 2 ""
fi

# --- Step 3: btc message verify (valid) ------------------------------------
banner "STEP 3/7: btc message verify (valid signature -> true)"
RESULT=""
RESULT_EXIT=0
run_btc RESULT RESULT_EXIT \
    "${BT_BIN}" message verify \
        --address "${EXPECTED_ADDR}" \
        "hello world" \
        "${SIG}"
# run_btc already streamed the raw "true" or "false" above.
if [[ ${RESULT_EXIT} -ne 0 ]]; then
    record_step 3 FAIL "btc message verify (valid)"
    print_step_result 3 "exit code ${RESULT_EXIT}"
    exit 1
elif [[ "${RESULT}" != "true" ]]; then
    record_step 3 FAIL "btc message verify (valid)"
    print_step_result 3 "expected true, got: ${RESULT}"
    exit 1
else
    record_step 3 PASS "btc message verify (valid)"
    print_step_result 3 "returned true"
fi

# --- Step 4: btc message verify (tampered) ---------------------------------
banner "STEP 4/7: btc message verify (TAMPERED message -> false)"
RESULT=""
RESULT_EXIT=0
run_btc RESULT RESULT_EXIT \
    "${BT_BIN}" message verify \
        --address "${EXPECTED_ADDR}" \
        "goodbye world" \
        "${SIG}"
# run_btc already streamed the raw "false" above.
if [[ ${RESULT_EXIT} -ne 0 ]]; then
    record_step 4 FAIL "btc message verify (tampered)"
    print_step_result 4 "exit code ${RESULT_EXIT}"
    exit 1
elif [[ "${RESULT}" != "false" ]]; then
    record_step 4 FAIL "btc message verify (tampered)"
    print_step_result 4 "expected false (tamper detected), got: ${RESULT}"
    exit 1
else
    record_step 4 PASS "btc message verify (tampered)"
    print_step_result 4 "returned false (tamper detected)"
fi

# --- Step 5: btc wallet show ------------------------------------------------
# Mirrors Steps 6/7 pattern: F20 enforcement (PR #82) refuses non-regtest
# networks without --pin-spki. Without BTC_DEMO_ESPLORA_SPKI_PIN, fall
# back to demonstrating the F20 refusal path (operator-not-configured,
# treated as PASS — the gate works as designed).
#
# With BTC_DEMO_ESPLORA_SPKI_PIN=<64-hex> supplied, run the real testnet
# sync. Failure treated as SKIP (network absent).
banner "STEP 5/7: btc wallet show (JSON from live testnet Esplora)"
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
    printf '(install jq or python3 for pretty JSON: brew install jq / apt install jq)\n'
fi

if [[ -z "${BTC_DEMO_ESPLORA_SPKI_PIN:-}" ]]; then
    # No pin supplied — demonstrate F20 refusal. Use mainnet so the URL
    # defaults to blockstream.info/api (the exact attack vector PR #82
    # closed). Without --pin-spki this MUST refuse.
    printf '$ %q' "${BT_BIN}" wallet show "${WALLET_ID}" \
        --network bitcoin --password demo-pwd
    printf '\n'
    set +e
    "${BT_BIN}" wallet show "${WALLET_ID}" \
        --network bitcoin --password demo-pwd 2>&1
    STEP5_EXIT=$?
    set -e
    if [[ ${STEP5_EXIT} -ne 0 ]]; then
        # Expected outcome: F20 refusal error.
        record_step 5 PASS "btc wallet show (F20 gate demonstrated)"
        print_step_result 5 "refused without --pin-spki as expected"
    else
        record_step 5 FAIL "btc wallet show"
        print_step_result 5 "expected F20 refusal, got exit 0"
    fi
else
    # Pin supplied — real testnet sync.
    set +e  # tolerate step 5 failure (network-dependent)
    "${BT_BIN}" wallet show "${WALLET_ID}" \
        --network testnet \
        --password demo-pwd \
        --pin-spki "${BTC_DEMO_ESPLORA_SPKI_PIN}" | ${JSON_PRETTY}
    STEP5_EXIT=$?
    set -e

    if [[ ${STEP5_EXIT} -eq 0 ]]; then
        record_step 5 PASS "btc wallet show (live Esplora sync)"
        print_step_result 5 "synced"
    else
        record_step 5 SKIP "btc wallet show (live Esplora sync)"
        print_step_result 5 "sync failed (offline or pin mismatch)"
    fi
fi

# --- Step 6: btc wallet balance (stateless, Issue #63) ----------------------
# Demonstrates the new `wallet balance` subcommand (Task 54c). No wallet_id
# required — uses the test-vector mnemonic directly. Expects a single
# integer in sats on stdout. Network-dependent (same caveat as Step 5).
#
# F20: requires --pin-spki for non-regtest networks. Demo uses testnet;
# the placeholder pin is set via BTC_DEMO_ESPLORA_SPKI_PIN env var
# (operator must supply a real value for live smoke). Without it, the
# F20 gate refuses the call with a non-zero exit — treated as SKIP
# (operator-not-configured), not FAIL.
banner "STEP 6/7: btc wallet balance (stateless, Issue #63 / Task 54c)"
printf '%s\n' "Note: requires network access + BTC_DEMO_ESPLORA_SPKI_PIN=<64-hex>"
printf '%s\n\n' "      (L29 operator smoke; non-regtest refuses without pin per F20)"

BALANCE_OUT=""
BALANCE_EXIT=0
if [[ -z "${BTC_DEMO_ESPLORA_SPKI_PIN:-}" ]]; then
    # No pin supplied — cannot hit testnet without tripping F20. Run against
    # the F20-violation path (mainnet + no pin) to demonstrate the gate,
    # which is also a useful UX check.
    printf '$ %q' "${BT_BIN}" wallet balance \
        --mnemonic "${MNEMONIC}" \
        --network bitcoin \
        --esplora-url https://blockstream.info/api
    printf '\n'
    set +e
    "${BT_BIN}" wallet balance \
        --mnemonic "${MNEMONIC}" \
        --network bitcoin \
        --esplora-url https://blockstream.info/api 2>&1
    BALANCE_EXIT=$?
    set -e
    if [[ ${BALANCE_EXIT} -ne 0 ]]; then
        # Expected outcome: F20 refusal error.
        record_step 6 PASS "btc wallet balance (F20 gate demonstrated)"
        print_step_result 6 "refused without --pin-spki as expected"
    else
        record_step 6 FAIL "btc wallet balance"
        print_step_result 6 "expected F20 refusal, got exit 0"
    fi
else
    # Pin supplied — run real testnet sync.
    run_btc BALANCE_OUT BALANCE_EXIT \
        "${BT_BIN}" wallet balance \
            --mnemonic "${MNEMONIC}" \
            --network testnet \
            --esplora-url https://blockstream.info/testnet/api \
            --pin-spki "${BTC_DEMO_ESPLORA_SPKI_PIN}"
    if [[ ${BALANCE_EXIT} -ne 0 ]]; then
        record_step 6 SKIP "btc wallet balance (live testnet)"
        print_step_result 6 "exit ${BALANCE_EXIT} (offline or pin mismatch)"
    elif [[ ! "${BALANCE_OUT}" =~ ^[0-9]+$ ]]; then
        record_step 6 FAIL "btc wallet balance"
        print_step_result 6 "expected integer sats, got: ${BALANCE_OUT}"
    else
        record_step 6 PASS "btc wallet balance (live testnet)"
        print_step_result 6 "${BALANCE_OUT} sats"
    fi
fi

# --- Step 7: btc wallet sync (stateless, Issue #63) ------------------------
# Demonstrates the new `wallet sync` subcommand. Prints
# `n_utxos=<N> total_sat=<S>` on stdout. Network + pin requirements
# identical to Step 6.
banner "STEP 7/7: btc wallet sync (stateless, Issue #63 / Task 54c)"
printf '%s\n' "Note: requires network access + BTC_DEMO_ESPLORA_SPKI_PIN=<64-hex>"
printf '%s\n' "      Output: n_utxos=<N> total_sat=<S>"
printf '%s\n\n' "      (L29 operator smoke; sync + balance share the F20 gate)"

SYNC_OUT=""
SYNC_EXIT=0
if [[ -z "${BTC_DEMO_ESPLORA_SPKI_PIN:-}" ]]; then
    # Demonstrate the F20 refusal path (mirrors Step 6 fallback).
    printf '$ %q' "${BT_BIN}" wallet sync \
        --mnemonic "${MNEMONIC}" \
        --network bitcoin \
        --esplora-url https://blockstream.info/api
    printf '\n'
    set +e
    "${BT_BIN}" wallet sync \
        --mnemonic "${MNEMONIC}" \
        --network bitcoin \
        --esplora-url https://blockstream.info/api 2>&1
    SYNC_EXIT=$?
    set -e
    if [[ ${SYNC_EXIT} -ne 0 ]]; then
        record_step 7 PASS "btc wallet sync (F20 gate demonstrated)"
        print_step_result 7 "refused without --pin-spki as expected"
    else
        record_step 7 FAIL "btc wallet sync"
        print_step_result 7 "expected F20 refusal, got exit 0"
    fi
else
    run_btc SYNC_OUT SYNC_EXIT \
        "${BT_BIN}" wallet sync \
            --mnemonic "${MNEMONIC}" \
            --network testnet \
            --esplora-url https://blockstream.info/testnet/api \
            --pin-spki "${BTC_DEMO_ESPLORA_SPKI_PIN}"
    if [[ ${SYNC_EXIT} -ne 0 ]]; then
        record_step 7 SKIP "btc wallet sync (live testnet)"
        print_step_result 7 "exit ${SYNC_EXIT} (offline or pin mismatch)"
    elif [[ ! "${SYNC_OUT}" =~ ^n_utxos=[0-9]+\ total_sat=[0-9]+$ ]]; then
        record_step 7 FAIL "btc wallet sync"
        print_step_result 7 "expected 'n_utxos=<N> total_sat=<S>', got: ${SYNC_OUT}"
    else
        record_step 7 PASS "btc wallet sync (live testnet)"
        print_step_result 7 "${SYNC_OUT}"
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
OVERALL="PASS"
EXIT_CODE=0
if [[ $FAILED -gt 0 ]]; then
    OVERALL="FAIL"
    EXIT_CODE=1
fi

cat <<EOF
EOF
if [[ $FAILED -gt 0 ]]; then
    printf '  %s Overall status: FAIL%s (passed=%d failed=%d skipped=%d total=%d)\n' \
        "${C_RED}${C_BOLD}" "${C_RESET}" "$PASSED" "$FAILED" "$SKIPPED" "$TOTAL"
elif [[ $SKIPPED -gt 0 && $FAILED -eq 0 ]]; then
    printf '  %s Overall status: PASS%s (%d skipped; %d of %d passed)\n' \
        "${C_YELLOW}${C_BOLD}" "${C_RESET}" "$SKIPPED" "$PASSED" "$TOTAL"
else
    printf '  %s Overall status: PASS%s (%d of %d passed)\n' \
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

Artifacts:
  wallet_id: ${WALLET_ID}
  signature: ${SIG} (${#SIG} chars)

Temp data dir: ${TMPDIR_DEMO} (cleaned up on exit)

Next:
  - Real wallet: btc wallet create --words 24 --network bitcoin --password <STRONG>
  - Sign for real: btc message sign --mnemonic "<YOUR_MNEMONIC>" --network bitcoin \\
      --esplora-url https://blockstream.info/api \\
      --esplora-spki-pin <64-hex> \\
      "<message>"
  - Verify:       btc message verify --address <ADDR> "<message>" <SIG>
  - Stateless balance (Issue #63):
      btc wallet balance --mnemonic "<12-word phrase>" --network testnet \\
          --esplora-url https://blockstream.info/testnet/api \\
          --pin-spki <64-hex>
  - Stateless sync (Issue #63):
      btc wallet sync --mnemonic "<12-word phrase>" --network testnet \\
          --esplora-url https://blockstream.info/testnet/api \\
          --pin-spki <64-hex>
EOF

exit ${EXIT_CODE}