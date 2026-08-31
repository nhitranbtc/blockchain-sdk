# Polygon CLI Scenario Runbook (Issue #438 / T7 prep)

**Date:** 2026-08-29
**Owner:** T7 / Phase 4 / Issue #426 (sub-task of #416 Q1 Option A)
**Status:** Anvil leg live in CI; Amoy-fork leg operator-driven per L29
**Sibling docs:**
- Issue body — `gh issue view 438`
- Plan — `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 4 (T7)
- Parent — Issue #426 Task 7

---

## 1. Goal

Provide pre-flight evidence that the `polygon` CLI wallet subcommands
behave end-to-end (`create → list → balance → send`) before T7 (live
Amoy smoke) and T8 (mainnet + ETH regression). Two execution paths,
one CI gate, one operator runbook.

## 2. Two execution paths

| Path | Tooling | Where it runs | Gate |
|------|---------|---------------|------|
| Anvil (chain_id 80002 — matches Amoy) | `cargo test -p polygon --test polygon_wallet_scenario` | CI: `.github/workflows/rust-evm-core-ci.yml` `polygon-anvil-e2e` job | MANDATORY — gates every PR to `rust-evm-core` |
| Amoy-fork (real RPC) | `scripts/polygon-wallet-scenario.sh --env=amoy-fork` | Operator workstation only | MANUAL — pre-release only |

The split exists because:
- CI must be hermetic, deterministic, fast (<60s wall-clock per #438).
  The `cargo test` path spawns Anvil in-process via `alloy-node-bindings`
  and needs no external `anvil` binary on PATH beyond what the
  Foundry toolchain action provides.
- The Amoy-fork path requires a pre-funded signer (faucet drip at
  <https://faucet.polygon.technology>) + a real signer pubkey in
  `$POLYGON_AMOY_PK`. These cannot be modeled in a cargo test without
  leaking operator secrets.

## 3. Running the Anvil leg locally

```bash
cd rust-wallet-app
cargo test -p polygon --test polygon_wallet_scenario -- --nocapture
```

Expected: 2 tests pass in ~36s wall-clock (well under the 60s budget).
The integration test verifies:

- `polygon wallet create --name <n>` exits 0, prints address.
- `polygon wallet list --network amoy` returns the new wallet name.
- `polygon wallet balance --address <addr>` returns numeric wei.
- `polygon wallet send --name <n> --to <recipient> --amount 0.001` returns
  `tx_hash: 0x<64-hex>` after receipt confirms status `1`.
- Negative case: `polygon wallet send ... --to 0xnotavalidaddress` exits
  non-zero with `--to`-mentioning error.

## 4. Running the Amoy-fork leg (operator)

Pre-flight:
1. Build the CLI: `cargo build --release -p polygon` → `target/release/polygon`.
2. Fund a signer at <https://faucet.polygon.technology> (5,000 POL /
   24h / address).
3. Export the signer private key as hex without `0x` prefix:
   `export POLYGON_AMOY_PK=<64-hex-chars>`.
4. (Optional) Override the default Amoy RPC:
   `export POLYGON_AMOY_RPC=https://polygon-amoy-bor-rpc.publicnode.com` (per Issue #474; was `polygon-amoy.drpc.org` before 2025-Q3 keyless-tier tightening; any working RPC accepted — `POLYGON_RPC_URL` env override honored by the smoke harness).

Run:
```bash
scripts/polygon-wallet-scenario.sh --env=amoy-fork
```

The script:
1. Creates a wallet via `polygon wallet create` (using the funded
   signer via the `--mnemonic` flag — see `polygon wallet import
   --help` for the alternative mnemonic path).
2. Lists wallets; expects the new name.
3. Queries balance; expects non-zero POL (post-faucet drip).
4. Sends 0.01 POL to a fixed recipient; expects `tx_hash: 0x…` and
   receipt success after `polygon wallet send --wait`.
5. Negative case: same send with bad `--to`; expects non-zero exit.

Expected wall-clock: < 30s on a broadband connection. The Amoy RPC
public endpoint rate-limits aggressive operators — back off and retry
on 429 responses.

## 5. Bugs surfaced and fixed by this scenario (Issue #438)

These were latent bugs that the integration test caught because it
exercises every `polygon wallet` subcommand end-to-end:

- **`WalletAction::Create` + `Import` were stubs in `main.rs:169-170`.**
  PR #451 added handler bodies but did not wire the `run()` dispatch
  arm. The CLI returned `Error::Rpc("wallet create: deferred past T6b …")`
  for every invocation.
- **`WalletAction::{Balance,Sync}.address` field typed `String` but
  `value_parser = parse_address` returns `Address`.** Clap downcast
  panicked on every `wallet balance` / `wallet sync` invocation.
- **`SendArgs.to` same type mismatch.** `wallet send` downcast panicked.
- **`wallet_list` `Path::extension()` filter rejected `.meta.json`
  files (returns `"json"`, not `"meta.json"`).** Function silently
  returned empty list even when wallets existed.
- **`WalletManager::scan_disk_into` allowlist dropped polygon subdirs**
  (`mainnet | sepolia | anvil` only). In-memory cache was empty after
  every CLI restart, so `unlock_signer` returned `NotFound { wallet_id }`
  on every subsequent operation. Critical-tier bug — would have
  blocked every T6+ operator session.

## 6. CI gate

Job: `polygon-anvil-e2e` in `.github/workflows/rust-evm-core-ci.yml`.
The new step runs after the existing spike verification:

```yaml
- name: Polygon CLI wallet scenario (Issue #438 / T7 prep)
  working-directory: rust-wallet-app
  run: cargo test -p polygon --test polygon_wallet_scenario
```

Scope: `-p polygon` only (L55 step 11 verify gate — never `--workspace`).
No env var required (Anvil is spawned in-process; no `anvil` binary
needed beyond Foundry toolchain which the job already installs).

## 7. Acceptance criteria (from #438)

- [x] Scenario driver exercises all 4 `wallet` subcommands end-to-end.
- [x] Script supports `--env=anvil` and `--env=amoy-fork` flags.
- [x] Anvil path runs in CI (< 60s wall-clock; measured ~36s for 2 tests).
- [x] Forked Amoy path documented + runnable locally.
- [x] Exit code 0 only when every subcommand returns expected output.
- [x] Negative case asserted (`wallet send --to 0xinvalid`).
- [x] CI workflow entry gates on Anvil leg only.
- [x] README/operator-doc section (this file) updated.

## 8. Out of scope

- T8 mainnet run + ETH regression (separate issue).
- `tx`, `erc20`, `fee`, `sign` subcommand scenarios (separate issues
  per #438 body).
- Ledger / Trezor hardware integration (v0.3+).

## 9. References

- Plan: `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 4 (T7)
- User-stories: `docs/wallets/2026-08-27-polygon-wallet-user-stories.md`
- Interface design: `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
- Prior PRs: #433 (scaffold), #434 (T6b dispatch), #435 (T6c handlers),
  #436–#452 (T6c1–T6c5 follow-ups)