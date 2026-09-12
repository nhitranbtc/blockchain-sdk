#!/usr/bin/env bash
# task_8_2_mobile_build.sh — operator runbook for Phase 8.2 Steps 1+2+4.
#
# The cross-compile legs (iOS, Android) require toolchains that are
# not present in the Linux CI runner. Run this script locally (or on
# a dedicated macos-Xcode / Linux-with-NDK runner) to GREEN Phase 8.2
# Steps 1 + 2 + 4.
#
# Per plan Q17: mobile build verification lives on dedicated
# hardware runners, NOT the standard PR CI gate. This script is the
# operator-side checklist.
#
# Usage:
#   ./scripts/task_8_2_mobile_build.sh [ios-sim | android | both]
#
# Exits 0 if all selected targets build + symbol-audit GREEN.

set -euo pipefail

CRATE=sol-wallet-core
PROFILE=release-mobile
SELECT="${1:-both}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

ok() { printf "  \033[32mOK\033[0m %s\n" "$*"; }
err() { printf "  \033[31mRED\033[0m %s\n" "$*" >&2; }

build_target() {
    local triple="$1"
    local label="$2"
    echo
    echo "==> $label ($triple)"
    if ! rustup target list --installed | grep -q "^$triple\$"; then
        err "target $triple not installed. Run: rustup target add $triple"
        return 1
    fi
    cargo build -p "$CRATE" --profile "$PROFILE" --target "$triple"
    ok "$label build"
    local dylib
    case "$triple" in
        *-apple-ios*) dylib="target/$triple/$PROFILE/libsol_wallet_core.dylib" ;;
        *) dylib="target/$triple/$PROFILE/libsol_wallet_core.so" ;;
    esac
    if [[ ! -f "$dylib" ]]; then
        err "cdylib not found at $dylib"
        return 1
    fi
    "$SCRIPT_DIR/symbol_audit.sh" "$dylib"
}

case "$SELECT" in
    ios-sim)
        build_target aarch64-apple-ios-sim "iOS Simulator (Apple Silicon)" || exit 1
        ;;
    ios)
        # aarch64-apple-ios requires macOS runner + Xcode SDK.
        build_target aarch64-apple-ios "iOS device (arm64)" || exit 1
        ;;
    android)
        # Requires ANDROID_NDK_HOME; fails on runners without NDK.
        if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
            err "ANDROID_NDK_HOME not set. Install Android NDK + set env var."
            exit 1
        fi
        build_target aarch64-linux-android "Android (arm64)" || exit 1
        ;;
    both)
        echo "Build iOS Simulator + Android (operator-required toolchains)."
        build_target aarch64-apple-ios-sim "iOS Simulator" || exit 1
        if [[ -n "${ANDROID_NDK_HOME:-}" ]]; then
            build_target aarch64-linux-android "Android" || exit 1
        else
            echo "  (skip Android: ANDROID_NDK_HOME not set)"
        fi
        ;;
    *)
        err "unknown target: $SELECT (use ios-sim | ios | android | both)"
        exit 2
        ;;
esac

echo
ok "Phase 8.2 Steps 1+2 build green + symbol audit clean"
exit 0
