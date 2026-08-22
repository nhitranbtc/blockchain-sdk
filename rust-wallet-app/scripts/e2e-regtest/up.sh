#!/usr/bin/env bash
#
# up.sh — start the regtest E2E stack (bitcoind + blockstream/esplora).
#
# After this returns 0, point btc-send-regtest-e2e.sh at the stack:
#   BTC_RPC_URL=http://localhost:18443
#   BTC_ESPLORA_URL=http://localhost:50001/regtest/api
#   BTC_DOCKER_CONTAINER=e2e-bitcoind-regtest
#
# Usage:
#   bash scripts/e2e-regtest/up.sh

set -uo pipefail

cd "$(dirname "$0")"

echo "Starting regtest stack (bitcoind + esplora)..."
docker compose up -d

echo
echo "Waiting for bitcoind healthcheck..."
for i in {1..30}; do
    if docker inspect --format='{{.State.Health.Status}}' e2e-bitcoind-regtest 2>/dev/null | grep -q healthy; then
        echo "  bitcoind: healthy"
        break
    fi
    sleep 2
done

echo "Waiting for esplora healthcheck (may take 30-60s on first run)..."
for i in {1..90}; do
    if docker inspect --format='{{.State.Health.Status}}' e2e-esplora-regtest 2>/dev/null | grep -q healthy; then
        echo "  esplora: healthy"
        break
    fi
    sleep 2
done

echo
echo "Stack up. Verify:"
echo "  curl -s http://localhost:50001/regtest/api/blocks/tip/height"
echo "  docker exec e2e-bitcoind-regtest bitcoin-cli -regtest -rpcuser=foo -rpcpassword=bar getblockchaininfo | head -3"
echo
echo "Run the E2E:"
echo "  set -a; source /tmp/btc-regtest.env; set +a"
echo "  export BTC_DOCKER_CONTAINER=e2e-bitcoind-regtest"
echo "  bash rust-wallet-app/scripts/btc-send-regtest-e2e.sh"
echo
echo "Tear down:"
echo "  bash scripts/e2e-regtest/down.sh"