#!/usr/bin/env bash
# wallet-desktop/tool/build_native.sh
#
# Task 18 / Issue #224 — native lib build helper.
#
# Builds the `bitcoin-wallet-core` cdylib (Rust FFI surface consumed
# by wallet-desktop via `dart:ffi`) and copies the resulting shared
# library into `wallet-desktop/native/<host-arch>/` so the Flutter
# desktop runner can `DynamicLibrary.open()` it.
#
# Replaces the inline `Build bitcoin-wallet-core cdylib` +
# `Copy cdylib to Flutter native dir` steps that previously lived in
# `.github/workflows/wallet-desktop-ci.yml`. Both local development
# and CI now invoke this single script.
#
# Usage:
#   wallet-desktop/tool/build_native.sh                  # build for host arch
#   TARGET_ARCH=macos-arm64 wallet-desktop/tool/build_native.sh  # override
#
# Output (host Linux):
#   wallet-desktop/native/linux-x64/librust_wallet_core.so
#
# Output (host macOS Intel):
#   wallet-desktop/native/macos-x64/librust_wallet_core.dylib
#
# Output (host macOS Apple Silicon):
#   wallet-desktop/native/macos-arm64/librust_wallet_core.dylib
#
# Output (host Windows):
#   wallet-desktop/native/windows-x64/rust_wallet_core.dll
#
# Exit codes:
#   0  — build + copy succeeded
#   1  — cargo build failed (stderr from cargo)
#   2  — unsupported host architecture (no mapped native dir)
#   3  — cp failed (filesystem error)
#   4  — post-copy assertion failed (destination missing or empty)
#   5  — destination filename / arch key not found in Dart loader's _HostOs enum
#   3  — cp failed (filesystem error)

set -euo pipefail

# --- locate repo root (script lives at wallet-desktop/tool/build_native.sh) ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WALLET_DESKTOP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$WALLET_DESKTOP_DIR/.." && pwd)"
RUST_WORKSPACE_DIR="$REPO_ROOT/rust-wallet-app"

# --- pick target arch (allow override via TARGET_ARCH env var) ---
detect_host_arch() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)   echo "linux-x64" ;;
    Darwin)
      case "$arch" in
        x86_64)  echo "macos-x64" ;;
        arm64)   echo "macos-arm64" ;;
        *)       return 1 ;;
      esac
      ;;
    MINGW*|CYGWIN*|MSYS*)
      echo "windows-x64"
      ;;
    *)
      return 1
      ;;
  esac
}

if [[ -n "${TARGET_ARCH:-}" ]]; then
  host_arch="$TARGET_ARCH"
else
  host_arch="$(detect_host_arch)" || {
    echo "build_native.sh: unsupported host arch: $(uname -s)/$(uname -m)" >&2
    exit 2
  }
fi

echo "build_native.sh: target arch = $host_arch"

# --- map arch → (cargo output filename, dart-side destination filename) ---
# Single source of truth: the Dart loader in
# `wallet-desktop/lib/core/ffi/native_lib.dart` reads from
# `native/<host_arch>/<dst_filename>`. Both must match this table.
# Drift between cargo output and Dart loader = silent FFI load
# failure (Dart falls through to system path which also fails).
# Discovered via security-guidance review of PR #248 (2026-08-21).
case "$host_arch" in
  linux-x64)    cargo_basename="libbitcoin_wallet_core.so";  dst_filename="librust_wallet_core.so" ;;
  macos-x64)    cargo_basename="libbitcoin_wallet_core.dylib"; dst_filename="librust_wallet_core.dylib" ;;
  macos-arm64)  cargo_basename="libbitcoin_wallet_core.dylib"; dst_filename="librust_wallet_core.dylib" ;;
  windows-x64)  cargo_basename="bitcoin_wallet_core.dll";     dst_filename="rust_wallet_core.dll" ;;
  *)
    echo "build_native.sh: unknown TARGET_ARCH '$host_arch'" >&2
    exit 2
    ;;
esac

# --- run cargo build ---
echo "build_native.sh: cargo build --release -p bitcoin-wallet-core"
(
  cd "$RUST_WORKSPACE_DIR"
  cargo build --release -p bitcoin-wallet-core
) || {
  echo "build_native.sh: cargo build failed" >&2
  exit 1
}

# --- copy shared lib into native dir ---
native_dir="$WALLET_DESKTOP_DIR/native/$host_arch"
mkdir -p "$native_dir"

src_lib="$RUST_WORKSPACE_DIR/target/release/$cargo_basename"
dst_lib="$native_dir/$dst_filename"

if [[ ! -f "$src_lib" ]]; then
  echo "build_native.sh: expected cargo output not found: $src_lib" >&2
  exit 1
fi

# Remove any stale artifact at the destination. Without `rm -f`, an
# older build's library would remain at the loader's expected path
# even when the new cargo build produced something different (e.g.
# a debug-vs-release mix-up). `rm -f` is safe — the destination will
# be overwritten by `cp` immediately below.
rm -f "$dst_lib"

cp "$src_lib" "$dst_lib" || {
  echo "build_native.sh: cp $src_lib $dst_lib failed" >&2
  exit 3
}

# Post-copy assertion: the file the Dart loader would open must
# exist at the exact loader-expected path AND be non-empty. A future
# key drift (script renames macos-x64 back to darwin-x64, Windows
# drops its prefix, etc.) fails the build loudly here instead of
# silently leaving a stale library for the loader to find.
if [[ ! -s "$dst_lib" ]]; then
  echo "build_native.sh: post-copy assertion failed: $dst_lib missing or empty" >&2
  exit 4
fi

# Cross-check the produced artifact is the one the Dart loader
# expects by reading the loader's own enum. This catches silent drift
# even if both sides "look right" individually.
loader_dart="wallet-desktop/lib/core/ffi/native_lib.dart"
if [[ -f "$WALLET_DESKTOP_DIR/../$loader_dart" ]]; then
  expected_subdir="native/$host_arch"
  expected_lib="$dst_filename"
  if ! grep -q "subdir: '$expected_subdir'" "$WALLET_DESKTOP_DIR/../$loader_dart"; then
    echo "build_native.sh: arch key '$host_arch' not found in $loader_dart (_HostOs enum)" >&2
    echo "  Expected subdir: '$expected_subdir'" >&2
    echo "  Add the new arch to the _HostOs enum in native_lib.dart" >&2
    exit 5
  fi
  if ! grep -q "libName: '$expected_lib'" "$WALLET_DESKTOP_DIR/../$loader_dart"; then
    echo "build_native.sh: lib name '$expected_lib' not found in $loader_dart (_HostOs enum)" >&2
    exit 5
  fi
fi

echo "build_native.sh: copied"
echo "  $src_lib"
echo "  -> $dst_lib"
