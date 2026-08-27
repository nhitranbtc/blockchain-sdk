# TRON spike roadmap — use case: alpha → beta USDT-TRC20 on local testnet + Nile

> **Goal**: Document the end-to-end "send stablecoin on TRON" use case, mirror the
> Ethereum pattern (Anvil in-process), and track the gap between current spike
> coverage and a full production-grade TRC-20 transfer pipeline. Covers both the
> local-testnet (TronBox via testcontainers) and live-Nile paths now that the
> `use_case_alpha_sends_beta_usdt_live_nile` e2e is green (#409 closed).

---

## Local chain tooling: Ethereum vs TRON

| Aspect | Ethereum | TRON |
|--------|----------|------|
| Local chain | **Anvil** (Foundry, in-process via `alloy-node-bindings`) | **TronBox** (`tronbox/tre` Docker image, external process via `testcontainers`) |
| Pure-Rust? | **Yes** — `Anvil::new().spawn()` returns an `AnvilInstance` (no Docker, no Node) | **No** — TronBox is Java + Node; no pure-Rust emulator exists today (verified 2026-08-27). Closest Rust option is [Tronic](https://www.reddit.com/r/rust/comments/1marc3n/announcing_tronic_a_rust_toolkit_for_tron/) (client only) |
| Reference in repo | `alloy-v1/tests/v6_erc20_anvil.rs:31` — `use alloy_node_bindings::{Anvil, AnvilInstance}; let _anvil: AnvilInstance = Anvil::new().spawn();` | `tron-v1/tests/use_case_alpha_sends_beta_100_usdt.rs` — `use testcontainers::{runners::AsyncRunner, GenericImage}; let container = GenericImage::new("tronbox/tre", "latest").with_exposed_port(9090.tcp()).start().await;` |
| L29 gating | `RUN_V6_ANVIL=1 cargo test -- --ignored` | `RUN_TRON_LOCAL=1 cargo test -- --ignored` |
| Operator setup | `cargo install --git https://github.com/foundry-rs/foundry --bin anvil --locked` | `docker pull tronbox/tre:latest` |
| Startup overhead | ~1s (in-process) | ~3-5s (Docker image cached locally) |
| Port | random (anvil assigns) | 9090 (TronBox default) mapped to random host port |
| Container clean-up | Drop of `AnvilInstance` kills subprocess | `drop(container)` triggers testcontainers async-drop (requires tokio runtime) |

**Why the asymmetry**: TRON's reference implementation is Java (`java-tron`), not Rust. There is no equivalent of Foundry's Anvil (a pure-Rust in-process chain simulator). The closest fully-Rust TRON node is not in production today, so the spike falls back to the project's plan §C default — TronBox in Docker — and uses `testcontainers` to keep the operator UX clean (no manual `docker run`).

---

## Test inventory

### Offline (always run in CI)

| Test | Module | What it proves |
|------|--------|----------------|
| `use_case_alpha_sends_beta_100_usdt_offline` | `tests/use_case_alpha_sends_beta_100_usdt.rs` | alpha + beta wallet derivation from canonical BIP-39 mnemonics + 68-byte TRC-20 `transfer(address,uint256)` calldata (selector `0xa9059cbb` + 32-byte `to` + 32-byte `value`) + k256 ECDSA over deterministic prehash → 65-byte `r‖s‖v` with `v ∈ {0, 1}` (NOT Ethereum `v+27`). No network, no container, no protobuf (TRC-20 contract proto not vendored). |
| V1-V10 sub-tests | `tests/v{1..10}_*.rs` | Per-plan-Vn coverage (compile, protobuf, ABI, address, sign, registry, SPKI parser, SLIP-44). See the F1-F10 rows in [Features + implementation status](#features--implementation-status-2026-08-27) below for per-Vn status. |

### Live (operator-driven per L29, gated)

| Test | Gate | What it proves |
|------|------|----------------|
| `use_case_alpha_sends_beta_100_usdt_live_local_node` | `RUN_TRON_LOCAL=1` | Spawns `tronbox/tre:latest` via `testcontainers`, asserts the node boots and `/wallet/getnowblock` returns a non-empty `blockID` over HTTP. Wall-clock ~4s with image cached locally. |
| V5 (Nile resource model) | `RUN_TRON_NILE=1` | Live `triggerconstantcontract` against `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (community-test USDT on Nile, fixed 2026-08-27) `decimals()` → `6`, `energy_used ∈ [50k, 150k]` — confirms Stake 2.0 / DEM model. |
| V6 (Nile JSON-RPC) | `RUN_TRON_NILE=1` | Live `POST /jsonrpc eth_chainId` → `0xcd8690dc`; `walletsolidity/getnowblock` for TAPOS. |
| V7 (live TLS pin) | `RUN_TRON_NILE=1` + real Cloudflare SPKI pin | Live cert rejection against `api.trongrid.io` (rotates ~30 days). **Honest gap:** pin parsed and recorded on `JsonRpcClient` but not enforced on outbound HTTP — `#408` ship-gate follow-up. |
| V9-live (on-chain decimals) | `RUN_TRON_NILE=1` | Live `decimals()` call against `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` USDT-TRC20 → on-chain value matches registry's `6`. |
| **`use_case_alpha_sends_beta_usdt_live_nile`** | **`TRON_NILE_PRIVATE_KEY` + `TRON_NILE_RECIPIENT_ADDRESS` + `TRON_NILE_SPKI_PIN`** | **Full live e2e (#409 closed 2026-08-27)**: derive sender T-address from privkey → build TriggerSmartContract `transfer(beta, 1 USDT)` → sign 65-byte `r‖s‖v` (v ∈ {0, 1}) over `SHA-256(raw_data)` → broadcast via SPKI-pinned RPC → poll receipt endpoint with `id == tx_id` + `result == "SUCCESS"` gating → query `balanceOf(beta)` ≥ 1_000_000 raw. Last green tx: [`c69a2105…`](https://nile.tronscan.org/#/transaction/c69a2105cbd0beefc3b5e84fefcec1e41b3011995e21c122feb6a758f86be26f). |

---

## Features + implementation status (2026-08-27)

| # | Feature | Implementation | Offline test | Live test | Status |
|---|---|---|---|---|---|
| F1 | `cargo build -p tron-v1-spike` (workspace deps resolve: `prost` 0.14.4, `prost-types` 0.14.4, `bs58` 0.5, `tiny-keccak` 2.0.2) | V1 | V1_compile.rs | — | ✅ done |
| F2 | `prost-build` → vendored `core/Tron.proto` → Rust types; round-trip on `AccountId` + `Transaction::Raw` | V2 | V2_protobuf_roundtrip.rs | — | ✅ done (production crate will vendor `core/contract/*.proto` too) |
| F3 | TRC-20 ABI calldata: `transfer(address,uint256)` = 68 B, `balanceOf(address)` = 36 B; canonical selectors `0xa9059cbb`, `0x70a08231`, `0x313ce567` | V3 | V3_trc20_abi.rs | — | ✅ done |
| F4 | T-base58check address: keccak256 + `0x41` prefix + base58check; 34-char `T…` | V4 | V4_base58check.rs | — | ✅ done |
| F5 | Nile Stake 2.0 / DEM energy model sizing | V5 | V5_resource.rs | V5 (Nile `triggerconstantcontract`) | ✅ done |
| F6 | Nile JSON-RPC: chain-id `0xcd8690dc`, TAPOS via `walletsolidity/getnowblock` | V6 | V6_nile.rs | V6 (Nile `/jsonrpc eth_chainId`) | ✅ done |
| F7 | SPKI pin URL parser: `pinned://<64-hex>@host[:port]` | V7 | V7_spki_pin.rs | V7-live (TLS handshake) | ✅ partial — **parse only**, enforcement is `#408` ship-gate follow-up |
| F8 | k256 ECDSA sign over `SHA-256(raw_data)`; 65-byte `r‖s‖v` with `v ∈ {0, 1}` (NOT `v+27`) | V8 | V8_sign_only.rs | (used inside use-case live_nile) | ✅ done |
| F9 | Token registry load: `tokens/{mainnet,nile}.json` (5 + 1 entries); mainnet USDT `decimals=6` | V9 | V9_token_registry.rs | V9-live (Nile `decimals()`) | ✅ done |
| F10 | SLIP-44 coin type 195 = TRX; `m/44'/195'/0'/0/0` derivation from canonical "abandon ×11 + about" BIP-39 mnemonic | V10 | V10_slip44.rs | — | ✅ done |
| F11 | Full live broadcast + receipt poll + balance verify (alpha → beta 1 USDT-TRC20) | use_case | `use_case_alpha_sends_beta_usdt_offline` | **`use_case_alpha_sends_beta_usdt_live_nile`** | ✅ done (#409 closed 2026-08-27) |
| F12 | Local devnet spawn via testcontainers (`tronbox/tre:latest`) — readiness probe | use_case | — | `use_case_alpha_sends_beta_usdt_live_local_node` | ✅ partial — node spawn + `/wallet/getnowblock` only; TRC-20 contract deploy inside container is backlog |
| F13 | SPKI pin enforcement on outbound HTTP (custom reqwest `ClientBuilder` wiring `EsploraVerifier`) | — | — | — | ⏳ not done — `#408` ship-gate |
| F14 | Full production crate `tron-wallet-core` (vendors full proto tree, builds `tron` CLI, Nile + mainnet smoke) | — | — | — | ⏳ not done — production crate per plan `docs/superpowers/plans/2026-08-27-tron-wallet-core.md` |

**Counts:** 42 lib + integration tests pass, 0 fail (verified 2026-08-27).

**Live path status:**

- **V5/V6/V9-live:** PASS (read-only Nile calls; `RUN_TRON_NILE=1`)
- **V7-live:** TLS handshake PASS; pin enforcement deferred
- **`use_case_alpha_sends_beta_usdt_live_local_node`:** PASS (4-stage readiness probe, ~3-5s)
- **`use_case_alpha_sends_beta_usdt_live_nile`:** **PASS** (#409 closed; 4 on-chain txs broadcast during debugging — recipient holds 7 USDT after the last successful run)

**Verify gate:** `cargo fmt + clippy -p tron-v1-spike --all-targets -- -D warnings` clean.

---

## Gap to full TRC-20 transfer + balance verify

### Shipped (#409 closed 2026-08-27 — live Nile e2e now PASS)

| Step | Status | Evidence |
|---|---|---|
| 1. Contract deployment | ⏭️ skipped (live Nile USDT already deployed at `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`) | Nile faucet drips community-test USDT — see `tests/env.example` |
| 2. `tronbox migrate` inside container | ⏭️ skipped (n/a for live Nile) | n/a |
| 3. Pre-fund alpha with USDT | ✅ via Nile faucet | Faucet drips community-test USDT to claimed accounts |
| 4. End-to-end broadcast (sign + POST `/wallet/broadcasttransaction` + poll) | ✅ done | `use_case_alpha_sends_beta_usdt_live_nile` PASS; 4 on-chain txs; tronscan `c69a2105…` |
| 5. Balance verify (`balanceOf(beta)` ≥ 1_000_000 raw) | ✅ done | Test asserts ≥ 1 USDT; recipient holds 7 USDT after debugging runs |

### Remaining backlog (local devnet path only — F12 partial)

For the `use_case_alpha_sends_beta_usdt_live_local_node` path to exercise the full
TRC-20 flow against a fresh `tronbox/tre` devnet, the spike still needs:

1. **`tests/fixtures/MockTRC20.sol`** — minimal TRC-20 implementing `transfer`,
   `balanceOf`, `decimals`. Pure-Rust alternative: hand-roll the equivalent
   triggerconstantcontract envelope in test code (skips solc entirely).
2. **`tronbox migrate --network development` inside the container** — via
   `testcontainers::Image::exec` or shell-out. Currently the local node only
   proves `/wallet/getnowblock` reachability.
3. **End-to-end on local** — repeat steps 4-5 against the container-mapped
   host port (no SPKI pin, no faucet).

Tracked as backlog work — not blocking #399 ship-gate (live Nile path is the
acceptance surface for the production crate).

---

## Cross-spike pattern (for future spikes)

When porting this use-case pattern to another chain, the local-chain tooling row of
the comparison table above is the only delta. The test structure (offline companion +
live `#[ignore]` gated on env var + container drop + drift-notes update) is reusable:

1. Add dev-dep `testcontainers = "<major>"` (or `alloy-node-bindings = "<major>"` for
   Ethereum-family chains).
2. Write `tests/use_case_<scenario>.rs` with 2 `#[test]` fns:
   - `..._offline`: pure crypto + wire format; always runs.
   - `..._live_local_node`: `#[ignore]` + env-var gate; spawns container via
     testcontainers or `Anvil::new().spawn()`.
3. Document the gap between current coverage and full end-to-end (steps 4-5 above
   for TRC-20).
4. Add drift notes + per-Vn evidence in README + ROADMAP.
