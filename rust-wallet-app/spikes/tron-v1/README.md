# TRON spike — CLI-driven verification (Phase 7)

Black-box test harness for the shipped `tron` CLI binary. Lives in
`rust-wallet-app/spikes/tron-v1/` as a **tests-only** crate: no `src/`,
no `build.rs`, no production deps — every assertion drives the CLI via
[`assert_cmd::Command::cargo_bin("tron")`](tests/common/mod.rs).

Per plan §Phase 7 (Task 7.13): the spike is the verification layer for the
shipped `crates/tron/` CLI and `crates/tron-wallet-core/` library. Drift
between spike impl and shipped impl now surfaces as a test failure — the
whole point of routing assertions through the CLI binary.

## Test layout

| Test file | Layer | Status | Network |
|---|---|---|---|
| `tests/v1_compile.rs` | V1 — `tron --help` enumerates subcommands | offline | — |
| `tests/v2_protobuf_roundtrip.rs` | V2 — `tron tx encode/decode` + `tron trc20 encode-call transfer` | offline | — |
| `tests/v3_trc20_abi.rs` | V3 — `tron trc20 encode-call transfer` selector + calldata shape | offline | — |
| `tests/v4_base58check.rs` | V4 — `tron wallet address --pubkey` (kobe-tron KAT) | offline | — |
| `tests/v5_resource.rs` | V5 — `tron resource estimate-trc20` + `tron resource contract-info` | gated | `RUN_TRON_NILE=1` |
| `tests/v6_nile.rs` | V6 — `tron config show --network nile` chain-id + address | gated | `RUN_TRON_NILE=1` |
| `tests/v7_spki_pin.rs` | V7 — `tron --rpc pinned://<pin>@api.trongrid.io wallet balance` | gated | `RUN_TRON_NILE=1` + `RUN_TRON_LOCAL=1` |
| `tests/v8_sign_only.rs` | V8 — `tron tx sign --no-broadcast` (no `--no-broadcast` enforces RPC) | offline | — |
| `tests/v9_token_registry.rs` | V9 — `tron config show` + `tron trc20 decimals` | gated | `RUN_TRON_NILE=1` |
| `tests/v10_slip44.rs` | V10 — `tron wallet derive` + `tron wallet address` (kobe-tron KAT) | offline | — |
| `tests/v11_mainnet_self_send.rs` | V11 — `tron tx trc20 transfer --to <self>` pre-check + mainnet self-send | gated + BLOCKING | `RUN_TRON_MAINNET=1` |
| `tests/trc20_local.rs` | Task 7.15 — 8-row TRC-20 matrix mirroring `tron-wallet-core/tests/trc20_local.rs` | gated | `RUN_TRON_LOCAL=1` |
| `tests/trc20_nile.rs` | Task 7.15 — 4-row TRC-20 matrix + V7a rebroadcast, mirroring `tron-wallet-core/tests/trc20_nile.rs` | gated | `RUN_TRON_NILE=1` |
| `tests/cli_coverage.rs` | Task 7.16 — 19 black-box tests driving every shipped `tron` subcommand | mixed | per-test |
| `tests/common/mod.rs` | Shared helpers — `tron()`, `set_test_data_dir()`, `live_spki_pin()` | n/a | — |

All gated tests use `#[ignore]` + loud-RED panic when env var is missing
(per Plan §Conventions → Gated live tests: never silent `return`).

## Run

### All offline tests (CI-friendly, no network)

```bash
cargo test -p tron-v1-spike --tests
```

### Live tests (operator-driven per L29)

```bash
# Nile-gated: V5, V6, V7, V9, trc20_nile, partial cli_co
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --tests -- --ignored --nocapture

# Local TronBox-gated: V7 localhost case, trc20_local, partial cli_co
RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike --tests -- --ignored --nocapture

# Mainnet BLOCKING gate: V11 (real value — operator only)
RUN_TRON_MAINNET=1 TRON_MAINNET_OPERATOR_WALLET=<self-t-addr> \
  cargo test -p tron-v1-spike --test v11_mainnet_self_send -- --ignored --nocapture
```

### Per-test replay

See [`RESULT.md`](RESULT.md) § Phase 7 PASS evidence scaffold for the
exact `tron` subcommand + assertion per Vn.

## Per-test isolation

`tests/common/mod.rs` exposes a thread-local `TEST_DATA_DIR` (`OnceCell<PathBuf>`).
Every `tron()` invocation injects `TRON_DATA_DIR` + `XDG_DATA_HOME` on the
spawned subprocess, so parallel tests never stomp each other's wallet/config
state. Tests register their dir via `set_test_data_dir(path)` in setup.

## What this spike is NOT

- **Not a library.** The crate has no `lib.rs`; `spikes/tron-v1/src/`
  (10 files), `build.rs`, `tokens/`, and `proto/` were deleted in Task 7.13
  per plan. Re-add only with explicit operator approval + the spike-stays-tests-only
  invariant intact.
- **Not a protobuf compiler.** `.proto` generation lives in
  `crates/tron/build.rs` (if needed). The spike only spawns the shipped CLI.
- **Not the production surface.** Production code lives in
  `crates/tron-wallet-core/` (library) + `crates/tron/` (CLI). Tokens in
  `crates/tron-wallet-core/tokens/` (single source of truth).

## References

- Plan: `docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`
- Audit: `docs/audit/2026-09-07-phase-7-cli-drift.md`
- Tracker: <https://github.com/nhitranbtc/blockchain-sdk/issues/399>
