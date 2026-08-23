# regtest E2E stack

Spins up a local Bitcoin regtest network + Esplora-compatible HTTP API for end-to-end testing of `btc wallet send` without depending on testnet or mainnet.

## What you get

- `e2e-bitcoind-regtest` (port 18443) — `bitcoin/bitcoin:25` in regtest mode
- `e2e-esplora-regtest` (port 50001) — `blockstream/esplora:latest` with regtest backend, Esplora HTTP API at `/regtest/api/`

## Usage

```bash
# 1. Start the stack
bash scripts/e2e-regtest/up.sh

# 2. Configure operator env (mode 0600; .env wildcard in .gitignore covers it)
cp rust-wallet-app/scripts/btc-send-regtest-e2e.env.example /tmp/btc-regtest.env
chmod 600 /tmp/btc-regtest.env
# edit /tmp/btc-regtest.env: set BTC_E2E_MNEMONIC (or BTC_E2E_MNEMONIC_FILE)

# 3. Run the E2E
set -a; source /tmp/btc-regtest.env; set +a
export BTC_DOCKER_CONTAINER=e2e-bitcoind-regtest
bash rust-wallet-app/scripts/btc-send-regtest-e2e.sh

# 4. Tear down
bash scripts/e2e-regtest/down.sh           # keep volumes
bash scripts/e2e-regtest/down.sh --volumes  # wipe state
```

## Healthchecks

Both services have Docker healthchecks:
- `bitcoind`: `bitcoin-cli getblockchaininfo` every 3s
- `esplora`: `wget /regtest/api/blocks/tip/height` every 5s after 30s start_period (initial electrs sync can take 30-60s on first run)

`up.sh` polls the healthchecks and prints a status line per service.

## Endpoints (once up)

```bash
# Esplora HTTP
curl -s http://localhost:50001/regtest/api/blocks/tip/height
curl -s http://localhost:50001/regtest/api/address/tb1q...

# bitcoind RPC
docker exec e2e-bitcoind-regtest bitcoin-cli -regtest \
    -rpcuser=foo -rpcpassword=bar getblockchaininfo
```

## Files

- `docker-compose.yml` — bitcoind + esplora services
- `up.sh` — start stack + wait for healthchecks
- `down.sh` — stop stack (with optional `--volumes` wipe)

## Out of scope (this directory)

- The `btc-send-regtest-e2e.sh` script itself — lives at `rust-wallet-app/scripts/`
- Testnet E2E (separate concern; see `rust-wallet-app/scripts/btc-send-testnet-e2e.sh`)
- The Option B refactor (testnet path via `hyper-util`) — Issue #281

## Refs

- Issue #284 (regtest E2E spike sub-task of #283)
- PR #285 (the script this stack drives)
- Issue #281 (Esplora reqwest use_preconfigured_tls downcast — full fix for testnet path)