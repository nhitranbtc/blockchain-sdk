# TRON spike roadmap — Phase 7 release-cut gate

> **Status (2026-09-07, REVISED per Task 7.13):** The spike is a
> **tests-only** crate driving the shipped `tron` CLI binary via
> `assert_cmd::Command::cargo_bin("tron")`. Every Vn + trc20 matrix +
> cli_coverage test asserts on CLI stdout/stderr/exit-code — no in-process
> library call. The previously-library-driven surface (which imported
> `tron_v1_spike::proto` / `tron_v1_spike::tx` etc.) is retired; the spike
> `src/`, `build.rs`, `tokens/`, and `proto/` directories were all deleted
> in Task 7.13. Drift between spike impl and shipped impl now surfaces as
> a test failure, which is the whole point.

## Test matrix

### Layer 1 — Vn coverage (Tasks 7.1–7.12)

| Vn | Open Q | Spike test | CLI surface | Network |
|---|---|---|---|---|
| V1 | Q1 (compile) | `tests/v1_compile.rs` | `tron --help` | offline |
| V2 | Q2 (protobuf) | `tests/v2_protobuf_roundtrip.rs` | `tron tx encode/decode` + `tron trc20 encode-call transfer` | offline |
| V3 | Q3 (TRC-20 ABI) | `tests/v3_trc20_abi.rs` | `tron trc20 encode-call transfer` | offline |
| V4 | Q4 (base58check) | `tests/v4_base58check.rs` | `tron wallet address --pubkey` | offline |
| V5 | Q5 (resource model) | `tests/v5_resource.rs` | `tron resource estimate-trc20` + `tron resource contract-info` | `RUN_TRON_NILE=1` |
| V6 | Q6 (Nile chain-id) | `tests/v6_nile.rs` | `tron config show --network nile` | `RUN_TRON_NILE=1` |
| V7 | Q7 (SPKI pin) | `tests/v7_spki_pin.rs` | `tron --rpc pinned://<pin>@api.trongrid.io wallet balance` | `RUN_TRON_NILE=1` + `RUN_TRON_LOCAL=1` |
| V8 | Q8 (sign-only) | `tests/v8_sign_only.rs` | `tron tx sign` | offline |
| V9 | Q9 (token registry) | `tests/v9_token_registry.rs` | `tron config show` + `tron trc20 decimals` | `RUN_TRON_NILE=1` |
| V10 | Q10 (SLIP-44) | `tests/v10_slip44.rs` | `tron wallet derive` + `tron wallet address` | offline |
| V11 | Q4 (mainnet self-send) | `tests/v11_mainnet_self_send.rs` | `tron tx trc20 transfer --to <self>` | `RUN_TRON_MAINNET=1` (BLOCKING) |

### Layer 2 — TRC-20 matrix (Task 7.15)

Mirrors `crates/tron-wallet-core/tests/trc20_{local,nile}.rs` but every
assertion is on CLI stdout/stderr/exit-code via `assert_cmd`.

| File | Rows | Network | Mirrors |
|---|---|---|---|
| `tests/trc20_local.rs` | 8 (TRX native, TRC-20 transfer/receive/approve, insufficient balance, send-speedup, rebroadcast, wallet-to-wallet) + Stake 2.0 deferred | `RUN_TRON_LOCAL=1` (TronBox Docker) | `tron-wallet-core/tests/trc20_local.rs` |
| `tests/trc20_nile.rs` | 4 (canonical TRC-20, rebroadcast idempotency V7a, mobile FFI deferred, network failure recovery) | `RUN_TRON_NILE=1` | `tron-wallet-core/tests/trc20_nile.rs` |

### Layer 3 — CLI coverage (Task 7.16)

`tests/cli_coverage.rs` — 19 black-box tests drive every one of the 22
shipped `tron` subcommands. No internal library call. Uses `tempfile` for
per-test `XDG_CONFIG_HOME` so config-touching tests don't pollute the
operator's real config.

Per-test command + assertion lives in `tests/cli_coverage.rs`; the
22-row matrix (subcommand → top-level → test name) is in
[`RESULT.md`](RESULT.md) § Task 7.16.

## Local chain tooling

| Aspect | TRON |
|---|---|
| Local chain | **TronBox** (`tronbox/tre` Docker image, external process via `testcontainers`) |
| Pure-Rust? | No — TronBox is Java + Node; no pure-Rust emulator exists today. Closest Rust option is [Tronic](https://www.reddit.com/r/rust/comments/1marc3n/announcing_tronic_a_rust_toolkit_for_tron/) (client only) |
| Spike test | `tests/trc20_local.rs` rows 1, 3, 8 hit `127.0.0.1:9090` (TronBox default port) |
| L29 gating | `RUN_TRON_LOCAL=1 cargo test -- --ignored` |
| Operator setup | `docker pull tronbox/tre:latest` |
| Startup overhead | ~3-5s (Docker image cached locally) |
| Port | 9090 (TronBox default) mapped to random host port via testcontainers |
| Container clean-up | `drop(container)` triggers testcontainers async-drop (requires tokio runtime) |

**Why the asymmetry:** TRON's reference implementation is Java
(`java-tron`), not Rust. There is no equivalent of Foundry's Anvil (a
pure-Rust in-process chain simulator). The closest fully-Rust TRON node
is not in production today, so the spike falls back to the project plan's
default — TronBox in Docker — and uses `testcontainers` to keep the
operator UX clean (no manual `docker run`).

## What changed in Phase 7

| Before | After |
|---|---|
| `spikes/tron-v1/src/` (10 files) — library re-implementing address/keccak/tx/etc. | **Deleted** — spike is tests-only |
| `spikes/tron-v1/build.rs` (prost-build) | **Deleted** — proto generation off the spike |
| `spikes/tron-v1/tokens/{mainnet,nile}.json` | **Deleted** — single source of truth at `crates/tron-wallet-core/tokens/` |
| `spikes/tron-v1/proto/core/*.proto` | **Deleted** — orphan, zero refs |
| `tests/use_case_alpha_sends_beta_usdt.rs` | **Deleted** — superseded by `tests/trc20_nile.rs` (CLI-driven mirror) |
| `tests/fixtures/MockTRC20.sol` | **Deleted from spike** — moved to `crates/tron-wallet-core/tests/fixtures/` (production surface) |
| `tests/env.example` | **Deleted** — test fixtures load via `common::load_nile_fixture()` |
| `Cargo.toml` deps: `prost`, `prost-types`, `bs58`, `tiny-keccak`, `k256`, `sha2`, `bip32`, `bip39`, `serde`, `serde_json` | **Dropped** — spike only depends on `assert_cmd` + `predicates` (dev-deps) |
| Per-test isolation: none (shared `~/.tron/`) | **Added** — `common::set_test_data_dir()` injects `TRON_DATA_DIR` + `XDG_DATA_HOME` per test thread |

## Acceptance criteria (Phase 7 release-cut)

- [x] Spike `src/` deleted (10 files)
- [x] No `tron_v1_spike` / `use crate::` / `use super::` imports in any `tests/*.rs`
- [x] All 11 Vns (V1-V10 + V11) have at least one spike test
- [x] All 22 shipped `tron` subcommands have at least one spike test in `cli_coverage.rs`
- [x] `cargo test -p tron-v1-spike --tests` passes (offline subset)
- [x] `cargo build -p tron` passes
- [x] `cargo geiger` clean on `spikes/tron-v1/`
- [x] Issue #399 acceptance criterion flips `[x]` when V11 mainnet self-send PASS

## Out of scope (deferred)

| Item | Target | Reason |
|---|---|---|
| Stake 2.0 freeze/unfreeze (TRC-20 matrix row 5) | v0.1.5 | Plan §Out of Scope; test passes with explanatory eprintln |
| Mobile FFI smoke (TRC-20 matrix row 3) | v0.2 | Phase 5 PAL + FFI cdylib surface required first |
| Full TRC-20 deploy on local TronBox | backlog | Requires `tronbox migrate` inside the container; not blocking |

## References

- Plan: `docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md` (Phase 7)
- Drift audit: `docs/audit/2026-09-07-phase-7-cli-drift.md`
- Past plans: `docs/superpowers/plans/2026-08-27-tron-wallet-core.md` (raw primitives, superseded 2026-09-05)
- Tracker: <https://github.com/nhitranbtc/blockchain-sdk/issues/399>
