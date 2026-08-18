#!/usr/bin/env bash
# Build a runnable `btc` shim from fake_btc.sh for cross-platform /
# operator-driven integration testing (Task 24, issue #172).
#
# Copies test/integration/fixtures/fake_btc.sh to build/fake_btc/btc
# with chmod +x so the operator can `PATH=$PWD/build/fake_btc:$PATH
# btc wallet list` to drive any external tool that expects the real
# `btc` binary on PATH.
#
# Usage: ./scripts/build_fake_btc.sh
# Output: build/fake_btc/btc (executable)
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p build/fake_btc
cp test/integration/fixtures/fake_btc.sh build/fake_btc/btc
chmod +x build/fake_btc/btc
echo "Fake btc built at $(pwd)/build/fake_btc/btc"