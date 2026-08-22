#!/usr/bin/env bash
#
# down.sh — tear down the regtest E2E stack (bitcoind + esplora).
#
# Usage:
#   bash scripts/e2e-regtest/down.sh
#   bash scripts/e2e-regtest/down.sh --volumes   # also remove volumes

set -uo pipefail

cd "$(dirname "$0")"

if [[ "${1:-}" == "--volumes" ]]; then
    docker compose down -v
else
    docker compose down
fi