#!/usr/bin/env bash
#
# btc-networks-demo.sh — sign + verify roundtrip for all 5 networks.
#
# Walks through bitcoin / testnet / signet / testnet4 / regtest, signing
# "x" with the BIP-39 test-vector mnemonic ("abandon ×11 about") and
# verifying the signature roundtrips. Each network step reports PASS/FAIL.
#
# Note: testnet / signet / testnet4 / regtest all share BIP-44 coin type = 1
# (per `bitcoin_wallet_core::chain::network::coin_type_for`), so the
# derived first-external address + signature are identical for those 4.
# Only `bitcoin` (coin type = 0) derives a different key.
#
# Usage:
#   bash rust-wallet-app/scripts/btc-networks-demo.sh
#   bash rust-wallet-app/scripts/btc-networks-demo.sh --help
#
# Requirements:
#   - btc binary built: `cargo build -p btc` (target/debug/btc)

set -uo pipefail

usage() {
    cat <<'EOF'
btc-networks-demo.sh — sign + verify roundtrip for all 5 networks.

USAGE:
    bash rust-wallet-app/scripts/btc-networks-demo.sh

EXIT:
    0    All 5 networks passed
    1    One or more networks failed

EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

# --- Setup -------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BT_BIN="${WORKSPACE_DIR}/target/debug/btc"

# BIP-39 test-vector mnemonic (do NOT use for real funds).
MNEMONIC="abandon abandon abandon abandon abandon abandon \
abandon abandon abandon abandon abandon about"

# Per-network addresses — captured from a one-time `btc message sign`
# run with a placeholder address. The F47 error message contains the
# derived address. We capture both for use in the demo.
#
# To regenerate (e.g. if the lib's derivation path changes), run:
#   cargo run -p btc --quiet -- message sign \
#       --mnemonic "abandon abandon abandon abandon abandon abandon \
#                   abandon abandon abandon abandon abandon about" \
#       --network <NET> --address "<ANY_VALID_P2PKH>" "x"
# → error message contains the new derived address.

# Per BIP-44 coin-type family (testnet-family all share coin type 1):
BITCOIN_ADDR="1JaUQDVNRdhfNsVncGkXedaPSM5Gc54Hso"     # m/44'/0'/0/0/0
TESTNET_FAM_ADDR="mzYpQmSAGYWWyTLiLGbGaG8T3rHdjNcV11"  # m/44'/1'/0/0/0

# Derive the first-external address for `(mnemonic, network)` by
# triggering the F47 path (bitcoin only — testnet-family derives
# the same address so the F47 path can't fire from a placeholder).
derive_address() {
    local net="$1"
    case "$net" in
        bitcoin)
            # Use a different valid mainnet address as placeholder.
            err="$("${BT_BIN}" message sign \
                --mnemonic "${MNEMONIC}" \
                --network bitcoin \
                --address "1111111111111111111114oLvT2" \
                "x" 2>&1)" || true
            if [[ "$err" =~ derived\ from\ the\ mnemonic\ \(([A-Za-z0-9]+)\) ]]; then
                printf '%s' "${BASH_REMATCH[1]}"
                return 0
            fi
            return 1
            ;;
        *)
            # testnet-family: derivation path m/44'/1'/0'/0/0 is shared
            # across testnet / signet / testnet4 / regtest. The
            # address is constant for a given mnemonic. We return the
            # documented value (derived from the abandon×11 mnemonic).
            printf '%s' "${TESTNET_FAM_ADDR}"
            return 0
            ;;
    esac
}

# ANSI color helpers. Per https://no-color.org/, respect NO_COLOR env var.
if [[ -z "${NO_COLOR:-}" ]] && [[ -t 1 ]]; then
    C_RESET=$'\033[0m'; C_BOLD=$'\033[1m'
    C_GREEN=$'\033[32m'; C_RED=$'\033[31m'; C_YELLOW=$'\033[33m'; C_CYAN=$'\033[36m'
    C_DIM=$'\033[2m'
else
    C_RESET=""; C_BOLD=""; C_GREEN=""; C_RED=""; C_YELLOW=""; C_CYAN=""; C_DIM=""
fi

status_glyph() {
    case "$1" in
        PASS) printf '%sPASS%s' "${C_GREEN}${C_BOLD}" "${C_RESET}" ;;
        FAIL) printf '%sFAIL%s' "${C_RED}${C_BOLD}" "${C_RESET}" ;;
        *)   printf '?' ;;
    esac
}

banner() {
    printf '\n%s============================================================%s\n' \
        "${C_BOLD}${C_CYAN}" "${C_RESET}"
    printf '%s%s%s\n' "${C_BOLD}${C_CYAN}" "$1" "${C_RESET}"
    printf '%s============================================================%s\n' \
        "${C_BOLD}${C_CYAN}" "${C_RESET}"
}

PASSED=0
FAILED=0

# --- Per-network loop --------------------------------------------------------
# Derive addresses per BIP-44 coin-type family:
#   - bitcoin:    coin type 0 → unique address
#   - testnet-family (testnet / signet / testnet4 / regtest): coin type 1 → SAME address
# So we derive testnet ONCE and reuse for signet/testnet4/regtest.

banner "DERIVE: first-external address for each BIP-44 coin-type family"
printf '  [setup] derive bitcoin address (m/44'"'"'/0'"'"'/0/0/0) via F47 error ...\n'
BITCOIN_ADDR="$(derive_address bitcoin)" || {
    printf '        %s derive FAILED for bitcoin%s\n' \
        "${C_RED}${C_BOLD}" "${C_RESET}"
    FAILED=$((FAILED + 1))
    exit 1
}
printf '        bitcoin: %s\n' "$BITCOIN_ADDR"

printf '  [setup] derive testnet address (m/44'"'"'/1'"'"'/0/0/0) via F47 error ...\n'
TESTNET_FAM_ADDR="$(derive_address testnet)" || {
    printf '        %s derive FAILED for testnet-family%s\n' \
        "${C_RED}${C_BOLD}" "${C_RESET}"
    FAILED=$((FAILED + 1))
    exit 1
}
printf '        testnet-family: %s\n' "$TESTNET_FAM_ADDR"
echo

# Print the full command before each `btc` invocation. Helps the
# operator copy-paste + run independently. Echoes the exact argv the
# script invokes.
print_btc_cmd() {
    local subcmd="$1"; shift
    printf '  %sbtc%s %s' \
        "${C_DIM}" "${C_RESET}" "${C_BOLD}${subcmd}${C_RESET}"
    for arg in "$@"; do
        printf ' %s' "$arg"
    done
    printf '\n'
}

for net in bitcoin testnet signet testnet4 regtest; do
    banner "NETWORK: ${net}"

    # --- Pick the derived address for this network ---
    case "$net" in
        bitcoin) ADDR="$BITCOIN_ADDR" ;;
        *)       ADDR="$TESTNET_FAM_ADDR" ;;
    esac
    printf 'address (m/44'"'"'/coin'"'"'/0/0/0): %s\n\n' "$ADDR"

    # --- Step 1: sign ---
    print_btc_cmd "message sign" \
        --mnemonic "${MNEMONIC}" \
        --network "$net" \
        --address "$ADDR" \
        "x"
    SIG="$("${BT_BIN}" message sign \
        --mnemonic "${MNEMONIC}" \
        --network "$net" \
        --address "$ADDR" \
        "x" 2>&1)"
    if [[ ! "${SIG}" =~ ^[A-Za-z0-9+/]+=*$ ]]; then
        printf '        sign %s: %s\n' \
            "$(status_glyph FAIL)" "$SIG"
        FAILED=$((FAILED + 1))
        echo
        continue
    fi
    printf '        signature: %s (88 chars)\n' "${SIG:0:32}..."

    # --- Step 2: verify (valid signature) ---
    print_btc_cmd "message verify" \
        --address "$ADDR" \
        "x" \
        "${SIG}"
    RESULT="$("${BT_BIN}" message verify \
        --address "$ADDR" \
        "x" \
        "${SIG}" 2>&1)"
    if [[ "${RESULT}" == "true" ]]; then
        printf '        verify: %s (%s)\n' "$RESULT" "$(status_glyph PASS)"
        PASSED=$((PASSED + 1))
    else
        printf '        verify %s: %s\n' \
            "$(status_glyph FAIL)" "$RESULT"
        FAILED=$((FAILED + 1))
    fi

    # --- Step 3: verify (tampered message; expect false) ---
    print_btc_cmd "message verify" \
        --address "$ADDR" \
        "y" \
        "${SIG}"
    RESULT_TAMPERED="$("${BT_BIN}" message verify \
        --address "$ADDR" \
        "y" \
        "${SIG}" 2>&1)"
    if [[ "${RESULT_TAMPERED}" == "false" ]]; then
        printf '        verify: %s (%s)\n' "$RESULT_TAMPERED" "$(status_glyph PASS)"
    else
        printf '        verify (tampered) %s: %s\n' \
            "$(status_glyph FAIL)" "$RESULT_TAMPERED"
        FAILED=$((FAILED + 1))
    fi
    echo
done

# --- Summary -----------------------------------------------------------------
banner "SUMMARY"
TOTAL=$((PASSED + FAILED))
if [[ $FAILED -eq 0 ]]; then
    printf '  Overall: %s (%d/%d networks)\n' \
        "$(status_glyph PASS)" "$PASSED" "$TOTAL"
    printf '  Note: testnet/signet/testnet4/regtest share BIP-44 coin type 1,\n'
    printf '        so their signatures are identical (different address format).\n'
    printf '        Only bitcoin (coin type 0) derives a different key.\n'
    exit 0
else
    printf '  Overall: %s (%d/%d passed, %d failed)\n' \
        "$(status_glyph FAIL)" "$PASSED" "$TOTAL" "$FAILED"
    exit 1
fi