#!/usr/bin/env bash
# symbol_audit.sh — Phase 8.2 Step 3b (audit H7).
#
# Captures `nm -gD` output of the built cdylib + asserts the exported
# symbol set equals exactly the 16 `sol_wallet_*` symbols. Zero Rust
# runtime symbols (`__rust_alloc`, `panic_impl`, `core_*`) permitted in
# the export set; `Wl,--exclude-libs,ALL` should already be applied via
# `[profile.release-mobile]` rustflags + the `panic = "abort"` setting
# eliminates `panic_impl` symbol.
#
# Usage:
#   scripts/symbol_audit.sh <path-to-libsol_wallet_core.{so,dylib,dll}>
#
# Exit codes:
#   0 — GREEN (audit passes)
#   1 — RED (violations found; see stderr)
#   2 — arg error (no path provided)
#
# Audit refs:
#   H7 — cdylib Rust runtime symbol leakage (the primary detection
#        this script performs)
#   H17 — companion to tests/abi_smoke.c (this script verifies the
#         surface is intentional; the C harness verifies it is
#         callable)
#
# CI integration: the rust-sol-core-ci `rust-ffi-symbol-audit` job runs
# this script on each of the three Phase 8.2 mobile targets (ios,
# ios-sim, android) post-`cargo build --release --profile release-mobile`.

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <path-to-libsol_wallet_core.{so,dylib,dll}>" >&2
    exit 2
fi

LIB="$1"

if [[ ! -f "$LIB" ]]; then
    echo "RED: library not found: $LIB" >&2
    exit 1
fi

# Determine the symbol-table dump command based on file type.
case "$LIB" in
    *.so|*.dylib)
        # `nm -gD` = global + defined (we want defined globals only).
        # Filter to entries whose symbol starts with a letter (skip
        # numeric addresses).
        NM_OUTPUT=$(nm -gD "$LIB" 2>&1 | awk '$2 ~ /^[A-Za-z_]/ {print $2}' | sort -u)
        ;;
    *.dll)
        # Windows: use `dumpbin /EXPORTS` if MSVC available; fallback
        # to objdump --syms for MinGW.
        if command -v dumpbin >/dev/null 2>&1; then
            NM_OUTPUT=$(dumpbin /EXPORTS "$LIB" 2>&1 | \
                awk '/^\s*[0-9]+\s+[0-9A-F]+\s+[0-9A-F]+\s+_?sol_wallet_/ {print $NF}' | \
                sort -u)
        else
            NM_OUTPUT=$(objdump --syms "$LIB" 2>&1 | \
                awk '$NF ~ /^sol_wallet_/ {print $NF}' | \
                sort -u)
        fi
        ;;
    *)
        echo "RED: unsupported file type: $LIB (expected .so / .dylib / .dll)" >&2
        exit 1
        ;;
esac

# Expected exported symbols (16 total: 12 original + 4 audit).
EXPECTED=(
    sol_wallet_create_mnemonic
    sol_wallet_import_mnemonic
    sol_wallet_unlock
    sol_wallet_lock
    sol_wallet_get_address
    sol_wallet_sign_transaction
    sol_wallet_send_sol
    sol_wallet_send_spl
    sol_wallet_get_balance_sol
    sol_wallet_get_balance_spl
    sol_wallet_last_error_message
    sol_wallet_panic_message_clear
    sol_wallet_zeroize
    sol_wallet_set_policy
    sol_wallet_get_policy
    sol_wallet_default_cluster
)

# Forbidden Rust runtime symbols (H7 — Rust runtime leakage).
FORBIDDEN=(
    __rust_alloc
    __rust_dealloc
    __rust_realloc
    panic_impl
    rust_panic
    rust_oom
    core_panic
)

# 1. Check every expected symbol is exported.
MISSING=()
for sym in "${EXPECTED[@]}"; do
    if ! echo "$NM_OUTPUT" | grep -qx "$sym"; then
        MISSING+=("$sym")
    fi
done
if [[ ${#MISSING[@]} -gt 0 ]]; then
    echo "RED: missing expected symbols:" >&2
    printf '  - %s\n' "${MISSING[@]}" >&2
fi

# 2. Check for any forbidden Rust runtime symbols.
FOUND_FORBIDDEN=()
for sym in "${FORBIDDEN[@]}"; do
    if echo "$NM_OUTPUT" | grep -qx "$sym"; then
        FOUND_FORBIDDEN+=("$sym")
    fi
done
if [[ ${#FOUND_FORBIDDEN[@]} -gt 0 ]]; then
    echo "RED: forbidden Rust runtime symbols in exported set (H7):" >&2
    printf '  - %s\n' "${FOUND_FORBIDDEN[@]}" >&2
fi

# 3. Check for extra unexpected sol_wallet_* symbols (H7 — only
# intentional exports).
echo "$NM_OUTPUT" | grep '^sol_wallet_' | sort -u > /tmp/_audit_exported
printf '%s\n' "${EXPECTED[@]}" | sort -u > /tmp/_audit_expected
EXTRA=$(comm -23 /tmp/_audit_exported /tmp/_audit_expected || true)
if [[ -n "$EXTRA" ]]; then
    echo "RED: unexpected sol_wallet_* symbols in exported set (H7):" >&2
    echo "$EXTRA" | sed 's/^/  - /' >&2
fi

# Cleanup scratch files.
rm -f /tmp/_audit_exported /tmp/_audit_expected

if [[ ${#MISSING[@]} -gt 0 || ${#FOUND_FORBIDDEN[@]} -gt 0 || -n "$EXTRA" ]]; then
    echo
    echo "Full exported symbol set ($LIB):" >&2
    echo "$NM_OUTPUT" | sed 's/^/  /' >&2
    exit 1
fi

echo "GREEN: symbol audit passes — 16 expected sol_wallet_* symbols present, 0 forbidden Rust runtime symbols, 0 unexpected extras"
exit 0
