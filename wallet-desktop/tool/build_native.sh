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
#   TARGET_ARCH=darwin-arm64 wallet-desktop/tool/build_native.sh  # override
#
# Output (host Linux):
#   wallet-desktop/native/linux-x64/librust_wallet_core.so
#
# Output (host macOS Intel):
#   wallet-desktop/native/darwin-x64/librust_wallet_core.dylib
#
# Output (host macOS Apple Silicon):
#   wallet-desktop/native/darwin-arm64/librust_wallet_core.dylib
#
# Output (host Windows):
#   wallet-desktop/native/windows-x64/rust_wallet_core.dll
#
# Exit codes:
#   0  — build + copy succeeded
#   1  — cargo build failed (stderr from cargo)
#   2  — unsupported host architecture (no mapped native dir)
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
        x86_64)  echo "darwin-x64" ;;
        arm64)   echo "darwin-arm64" ;;
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

# --- map arch → native lib filename (cdylib output naming per platform) ---
case "$host_arch" in
  linux-x64)    lib_basename="libbitcoin_wallet_core.so" ;;
  darwin-x64)   lib_basename="libbitcoin_wallet_core.dylib" ;;
  darwin-arm64) lib_basename="libbitcoin_wallet_core.dylib" ;;
  windows-x64)  lib_basename="bitcoin_wallet_core.dll" ;;
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

src_lib="$RUST_WORKSPACE_DIR/target/release/$lib_basename"
dst_lib="$native_dir/librust_wallet_core.${lib_basename##*.}"

if [[ ! -f "$src_lib" ]]; then
  echo "build_native.sh: expected cargo output not found: $src_lib" >&2
  exit 1
fi

cp "$src_lib" "$dst_lib" || {
  echo "build_native.sh: cp $src_lib $dst_lib failed" >&2
  exit 3
}

echo "build_native.sh: copied"
echo "  $src_lib"
echo "  -> $dst_lib"
