#!/usr/bin/env bash
#
# start.sh — entrypoint for the regtest Esplora image (Dockerfile.esplora-regtest).
#
# Sequence:
#   1. Launch electrs as a background process.
#   2. Poll the Electrum TCP port until electrs accepts a connection (handles
#      cold-start indexing, which takes 60-90 s on a fresh RocksDB).
#   3. exec the Python shim — kernel forwards SIGTERM via dumb-init (PID 1)
#      so docker stop cleans up both processes.
#
# Env (defaults match Dockerfile.esplora-regtest ENV block):
#   ELECTRS_NETWORK        regtest
#   ELECTRS_RPC_ADDR       127.0.0.1:50002 (loopback; compose override sets this)
#   ELECTRS_DB_DIR         /var/lib/electrs/db
#   BITCOIND_RPC_HOST      bitcoind        (docker compose service name)
#   BITCOIND_RPC_PORT      18443
#   BITCOIND_RPC_USER      foo
#   BITCOIND_RPC_PASS      bar
#   BITCOIND_P2P_PORT      18444
#
# Why this wrapper script (vs a raw `electrs` entrypoint): electrs alone can't
# serve the Esplora HTTP shape that EsploraClient speaks. The shim is the HTTP
# front end; electrs is the backend. Both must run. dumb-init handles PID 1.

set -euo pipefail

ELECTRS_NETWORK="${ELECTRS_NETWORK:-regtest}"
ELECTRS_RPC_ADDR="${ELECTRS_RPC_ADDR:-0.0.0.0:50001}"
ELECTRS_DB_DIR="${ELECTRS_DB_DIR:-/var/lib/electrs/db}"
BITCOIND_RPC_HOST="${BITCOIND_RPC_HOST:-bitcoind}"
BITCOIND_RPC_PORT="${BITCOIND_RPC_PORT:-18443}"
BITCOIND_RPC_USER="${BITCOIND_RPC_USER:-foo}"
BITCOIND_RPC_PASS="${BITCOIND_RPC_PASS:-bar}"
BITCOIND_P2P_PORT="${BITCOIND_P2P_PORT:-18444}"

BIND_HOST="${ELECTRS_RPC_ADDR%:*}"
BIND_PORT="${ELECTRS_RPC_ADDR#*:}"
# If electrs is bound to 0.0.0.0 we still poll via loopback.
POLL_HOST=$([ "$BIND_HOST" = "0.0.0.0" ] && echo "127.0.0.1" || echo "$BIND_HOST")

echo "[start.sh] launching electrs: bitcoind=${BITCOIND_RPC_HOST}:${BITCOIND_RPC_PORT} p2p=${BITCOIND_P2P_PORT} network=${ELECTRS_NETWORK}"

# --http-disable keeps electrs off its built-in HTTP (we serve Esplora via shim).
# --daemon-p2p-addr lets electrs follow the regtest chain (mempool awareness).
electrs \
    --network "${ELECTRS_NETWORK}" \
    --electrum-rpc-addr "${ELECTRS_RPC_ADDR}" \
    --db-dir "${ELECTRS_DB_DIR}" \
    --daemon-rpc-addr "${BITCOIND_RPC_HOST}:${BITCOIND_RPC_PORT}" \
    --daemon-rpc-user "${BITCOIND_RPC_USER}" \
    --daemon-rpc-pass "${BITCOIND_RPC_PASS}" \
    --daemon-p2p-addr "${BITCOIND_RPC_HOST}:${BITCOIND_P2P_PORT}" \
    --http-disable \
    &
ELECTRS_PID=$!

# Forward SIGTERM/SIGINT to electrs on container shutdown. The `exec` later
# replaces this shell so SIGTERM goes to the shim (PID 1 via dumb-init).
# `exit 143` (128 + SIGTERM) propagates the signal's exit code so docker sees
# a clean stop instead of falling through to `exec shim` with a dead electrs.
trap 'kill -TERM "$ELECTRS_PID" 2>/dev/null || true; exit 143' TERM INT

echo "[start.sh] waiting for electrs at ${POLL_HOST}:${BIND_PORT} (up to 360 s — cold RocksDB index on first run)"
ready=0
# Active Electrum-protocol probe: opens TCP, sends `server.ping`, reads one
# line. Readiness means electrs is serving RPCs, not just bound. Per L12
# round-2 finding, the passive connect check returned 0 the moment electrs
# opened its listener (before the RocksDB indexer was serving RPCs).
for _ in $(seq 1 60); do
    if timeout 3 bash -c '
        exec 3<>/dev/tcp/"$1"/"$2" || exit 1
        printf "{\"id\":1,\"method\":\"server.ping\",\"params\":[]}\n" >&3
        if read -t 1 -r line <&3; then
            case "$line" in
                *jsonrpc*) r=0 ;;
                *) r=1 ;;
            esac
        else
            r=1
        fi
        exec 3<&- 2>/dev/null || true
        exec 3>&- 2>/dev/null || true
        exit $r
    ' bash "$POLL_HOST" "$BIND_PORT" >/dev/null 2>&1; then
        ready=1
        break
    fi
    sleep 6
done
if [[ "$ready" -ne 1 ]]; then
    echo "[start.sh] electrs never responded to server.ping at ${POLL_HOST}:${BIND_PORT} after 360 s" >&2
    kill -TERM "$ELECTRS_PID" 2>/dev/null
    exit 1
fi

echo "[start.sh] electrs ready — starting shim"
exec /opt/shim-venv/bin/python /usr/local/bin/shim.py
