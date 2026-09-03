# `polygon` examples — operator-driven tools

Each file in this directory is an example binary, not a test. They are
**operator-driven** (per L29 — no CI gate). Build with
`cargo build -p polygon --examples` and run with
`cargo run -p polygon --example <name> -- <args>`.

---

## `amoy_faucet_and_verify` — operator-driven Amoy faucet + verify

**Issue:** [#519](https://github.com/nhitranbtc/blockchain-sdk/issues/519)
(Phase 8 / P8-T-manualfaucet, landed 2026-09-02).
**Branch:** `task/polygon-full-scenario/8-amoy-gap-fill-manualfaucet`
(child of `task/polygon-full-scenario/8-amoy-gap-fill`).
**Plan:** [`docs/superpowers/engineering/2026-09-02-polygon-amoy-test-plan.md`](../../../docs/superpowers/engineering/2026-09-02-polygon-amoy-test-plan.md) §P8-T-manualfaucet.

End-to-end validation that the `polygon` CLI works against the Amoy faucet.
Two paths share the same Phase 2/3 polling + reporting pipeline:

- **Path A (create wallet):** spawn `polygon wallet create` subprocess →
  parse EIP-55 from stdout → print address + 2 faucet URLs → block on
  `stdin.read_line` until the operator confirms they funded the address.
- **Path B (re-check existing wallet):** operator passes `--address <eip55>`
  → skip wallet creation + skip the funding pause → go straight to polling.

### Usage

**Path A (create + fund):**

```bash
export POLYGON_WALLET_PASSWORD='<your-password>'  # security: never pass on argv
cargo run -p polygon --example amoy_faucet_and_verify -- \
    --name test --network amoy --timeout 60
```

The binary forwards `POLYGON_WALLET_PASSWORD` to the spawned `polygon`
subprocess as the `POLYGON_PASSWORD` env var. The polygon CLI reads
`POLYGON_PASSWORD` (per its own existing warning; passing via argv leaks
through `/proc/<pid>/cmdline` + shell history).

**Path B (re-check an already-funded wallet):**

```bash
cargo run -p polygon --example amoy_faucet_and_verify -- \
    --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 \
    --network amoy --timeout 30 --poll-interval 5
```

Defaults: `--timeout 300` (5 min), `--poll-interval 5` (5 s).

### CLI args

```text
amoy_faucet_and_verify [--name <wallet-name>] [--address <eip55>]
                       --network amoy
                       [--timeout 300] [--poll-interval 5]
```

- Either `--name` OR `--address` must be supplied.
- `--password` was removed (security review feedback); use `POLYGON_WALLET_PASSWORD`.
- `--network` accepts only `amoy` (mainnet has no canonical faucet).

### Configuration

All settings (RPC + 2 faucets + explorer + USDC token + test-harness vars)
are loaded from `${CARGO_MANIFEST_DIR}/tokens/amoy.json`. Shell env vars
override individual fields. Resolution priority:

1. Shell-exported env var (highest — paid-tier override path)
2. `tokens/amoy.json` — committed Amoy config (single source of truth)

**No `DEFAULT_*` fallback**: if `tokens/amoy.json` is missing or malformed,
the binary exits 2 with a clear error pointing at the file. Math constants
(`WEI_PER_POL`, `USDC_UNIT`, `BALANCE_OF_SELECTOR`) are not config and stay
in the binary.

`tokens/amoy.json` schema:

```json
{
  "chain_id": 80002,
  "rpc_url": "https://polygon-amoy-bor-rpc.publicnode.com",
  "faucet_pol_url": "https://faucet.polygon.technology",
  "faucet_circle_url": "https://faucet.circle.com/",
  "explorer_url": "https://amoy.polygonscan.com",
  "tokens": [
    {"symbol": "USDC", "address": "0x8B0180f2101c8260d49339abfEe87927412494B4", "decimals": 6}
  ],
  "test_harness": {
    "amoy_funded_pk_hex": "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    "run_polygon_amoy": "1"
  }
}
```

| Env var                | `tokens/amoy.json` value                         |
|------------------------|--------------------------------------------------|
| `POLYGON_RPC_URL`      | `https://polygon-amoy-bor-rpc.publicnode.com`    |
| `POLYGON_FAUCET_URL`   | `https://faucet.polygon.technology`              |
| `CIRCLE_FAUCET_URL`    | `https://faucet.circle.com/`                     |
| `AMOY_POLYGONSCAN_URL` | `https://amoy.polygonscan.com`                   |
| `POLYGON_USDC_ADDRESS` | `0x8B0180f2101c8260d49339abfEe87927412494B4`     |

### What it checks

**Phase 2 — polling** probes both:

- Native POL balance via `eth_getBalance(addr, "latest")`
- ERC-20 USDC balance via `eth_call balanceOf(holder)` against the
  configured USDC contract (`0x8B0180...94B4` Mock USDC(PoS))

Each cycle logs a dot. The loop breaks on the **first non-zero on either
token** and proceeds to Phase 3. Either token ending the loop is a success
(polling stops, Phase 3 reports both balances).

**Phase 3 — report** writes the full balance report (raw wei + raw USDC
plus human-readable POL: 18-decimal / USDC: 6-decimal) + Amoy Polygonscan
link + a compact balance summary table.

**Phase 3b — parity check** spawns `polygon wallet balance --address <X>
--network amoy` subprocess and compares its stdout against the alloy
`eth_getBalance` readback. Output includes a grep-able
`parity: true|false (polygon_cli_vs_eth_getBalance = ...)` line. Surfaces
[#522](https://github.com/nhitranbtc/blockchain-sdk/issues/522) (CLI
formatter off by 10^3) without failing the run.

To override via shell (e.g. paid-tier RPC):

```bash
export POLYGON_RPC_URL=https://polygon-amoy.g.alchemy.com/v2/<KEY>
cargo run -p polygon --example amoy_faucet_and_verify -- --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 ...
```

### Audit log

Each run appends a timestamped block to
`.local/tmp/amoy_faucet_and_verify_report.md` (directory gitignored per
root `.gitignore:19 .local/`). Per-run block:

```text
=== amoy_faucet_and_verify run <unix-epoch-seconds> address=0xB954...369 exit=0 disposition=FUNDED ===
[phase 3/3] final report
  address:        0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369
  balance_pol:    369999998677000 wei
  balance_usdc:   10000000 (raw, 6-decimal)
  ...
=== end run <unix-epoch-seconds> ===

+-------------------+----------------------+--------------------------------+
| Source            | Value (display)      | Notes                          |
+-------------------+----------------------+--------------------------------+
| polygon CLI       | 0.369999998677000    POL | ✗ MISMATCH (1000× off — #522)  |
| eth_getBalance    | 369999998677000 wei  | raw oracle (raw = displayed/1000) |
+-------------------+----------------------+--------------------------------+
parity: false (polygon_cli_vs_eth_getBalance = false)
USDC parity skipped — `polygon erc20 balance` deferred to T6d-2.1 (#523)
```

Log writes are best-effort: a filesystem failure prints `warn: ...` to
stderr but does not fail the run. The same block also goes to stdout
(visible to the operator running the binary).

### Exit codes

| Code | Meaning |
|------|---------|
| `0`  | At least one of {native POL, USDC} went non-zero within the timeout (Phase 3 reports) |
| `1`  | Timed out — no non-zero POL or USDC observed (Phase 3 reports last-seen zero + hint) |
| `2`  | Phase 1 subprocess failed, OR missing/malformed `tokens/amoy.json` |

### Out of scope

- Auto-detect funding via webhook (operator-driven pause is the current
  contract per L29)
- ERC-20 send (this script only reads balances; for transferring USDC, use
  `polygon erc20 send --token USDC --token-address <addr> ...`)
- Multi-network support (Amoy only; mainnet funding has different faucets)
- `polygon erc20 balance` parity (deferred to T6d-2.1 per
  [#523](https://github.com/nhitranbtc/blockchain-sdk/issues/523) — current
  CLI stubbed)

### Worked example: re-checking the funded wallet

The wallet `0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369` (created by an
earlier P8-T-manualfaucet run, funded with ~0.00037 POL + 10 USDC.e via the
Polygon + Circle faucets) is used as the canonical `--address` example:

```bash
cargo run -p polygon --example amoy_faucet_and_verify -- \
    --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 \
    --network amoy --timeout 30 --poll-interval 5
```

Expected output (compressed):

```text
=== amoy_faucet_and_verify run 1788360252 address=0xB954...369 exit=0 disposition=FUNDED ===
  ✓ POL non-zero: 369999998677000 wei
  ✓ USDC non-zero: 10000000 (raw, 6-decimal)
Balance summary:
+---------+----------------------+---------------------------------------------+
| POL   | | 0.000370 POL | eth_getBalance → 369999998677000 wei |
| USDC  | | 10.000000 USDC | eth_call balanceOf(0x8B0180...94B4) → 10000000 raw |
+---------+----------------------+---------------------------------------------+
parity: false (polygon_cli_vs_eth_getBalance = false)   # #522 known
success — wallet funded.
```
