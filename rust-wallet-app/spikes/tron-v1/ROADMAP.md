# TRON spike roadmap — use case: alpha → beta 100 USDT-TRC20 on local testnet

> **Goal**: Document the end-to-end "send stablecoin on local testnet" use case for TRON,
> mirror the Ethereum pattern (Anvil in-process), and track the gap between current
> spike coverage and a full production-grade TRC-20 transfer pipeline.

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
| V1-V10 sub-tests | `tests/v{1..10}_*.rs` | Per-plan-Vn coverage (compile, protobuf, ABI, address, sign, registry, SPKI parser, SLIP-44). See [RESULT.md](RESULT.md) for per-Vn evidence. |

### Live (operator-driven per L29, gated)

| Test | Gate | What it proves |
|------|------|----------------|
| `use_case_alpha_sends_beta_100_usdt_live_local_node` | `RUN_TRON_LOCAL=1` | Spawns `tronbox/tre:latest` via `testcontainers`, asserts the node boots and `/wallet/getnowblock` returns a non-empty `blockID` over HTTP. Wall-clock ~4s with image cached locally. |
| V5 (Nile resource model) | `RUN_TRON_NILE=1` | Live `triggerconstantcontract` against `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` `decimals()` — confirms Stake 2.0 / DEM model. |
| V6 (Nile JSON-RPC) | `RUN_TRON_NILE=1` | Live `POST /jsonrpc eth_chainId` → `0xcd8690dc`; `walletsolidity/getnowblock` for TAPOS. |
| V7 (live TLS pin) | `RUN_TRON_NILE=1` + real Cloudflare SPKI pin | Live cert rejection against `api.trongrid.io` (rotates ~30 days). |
| V9-live (on-chain decimals) | `RUN_TRON_NILE=1` | Live `decimals()` call against USDT-TRC20 → on-chain value matches registry's `6`. |

---

## Status (2026-08-27)

- **Offline**: 37 tests pass, 0 fail across 12 binaries (V1-V10 + use-case offline + internal abi tests).
- **Live use-case**: **PASS** locally via testcontainers. Observed `blockID = 0000000000000000c93baa76a4a508f798a96f59156d9eb17ecede8ec845df2f` (genesis) on spawned `tronbox/tre:latest` (SHA `f4332e11df12a9f360639a4546fd046593909630fda48af00b30410c144342f0`). Container auto-drops on test exit (`drop(container)` in async-drop path).
- **Commits**: `c16a7f0` (initial V1-V10 spike) + `439c2e0` (use-case test + RESULT.md update).
- **PR**: [#405](https://github.com/nhitranbtc/blockchain-sdk/pull/405) (base `docs/tron-wallet-core-399`).
- **Verify gate**: `cargo fmt + clippy -p tron-v1-spike --all-targets -- -D warnings` clean.

---

## Gap to full TRC-20 transfer + balance verify (backlog)

The current live use-case verifies **container spawn + node readiness**, not the full
broadcast + balance-verify flow. The remaining work requires:

1. **Contract deployment fixture**: ship `tests/fixtures/MockTRC20.sol` (a minimal TRC-20
   implementing `transfer(address,uint256)`, `balanceOf(address)`, `decimals()`).
2. **`tronbox migrate` inside container**: after spawn, exec
   `tronbox migrate --network development` against the live container so the contract
   is deployed at a known address.
3. **Pre-fund alpha with USDT**: call `MockTRC20.mint(alpha, 1_000_000 * 10^6)` (1M USDT)
   before the transfer (or seed alpha at node-genesis).
4. **End-to-end broadcast**: build TriggerSmartContract tx (full proto struct — requires
   vendoring `core/contract/*.proto` files into spike scope), sign, POST to
   `/wallet/broadcasttransaction`, poll for receipt.
5. **Balance verify**: query `balanceOf(beta)` via `/wallet/triggerconstantcontract`,
   assert `100_000_000` base units.

The wire format for steps 4-5 is already proven by V2 (protobuf roundtrip), V3 (TRC-20
ABI calldata), and V8 (sign-only with `v ∈ {0, 1}`). The new work is **infrastructure**
(solc, migrations, fixture), not crypto.

Tracked as a separate backlog issue (to be filed after #403 closes).

---

## Cross-spike pattern (for future spikes)

When porting this use-case pattern to another chain, the local-chain tooling row of
the comparison table above is the only delta. The test structure (offline companion +
live `#[ignore]` gated on env var + container drop + RESULT.md update) is reusable:

1. Add dev-dep `testcontainers = "<major>"` (or `alloy-node-bindings = "<major>"` for
   Ethereum-family chains).
2. Write `tests/use_case_<scenario>.rs` with 2 `#[test]` fns:
   - `..._offline`: pure crypto + wire format; always runs.
   - `..._live_local_node`: `#[ignore]` + env-var gate; spawns container via
     testcontainers or `Anvil::new().spawn()`.
3. Document the gap between current coverage and full end-to-end (steps 4-5 above
   for TRC-20).
4. Add RESULT.md section + drift notes.
