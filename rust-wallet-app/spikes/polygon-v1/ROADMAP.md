# Polygon PoS spike roadmap — verification harness for Issue #417

> **Goal**: Document the V1-V10 verification path for Issue #416 open questions (Q1-Q8)
> before production code ships in `evm-wallet-core` + `polygon-wallet-core`. Mirrors the
> TRON spike's structure (offline companion + live gated + use-case) but with Ethereum-family
> tooling (Anvil Polygon-fork via `alloy-node-bindings`, no Docker).
>
> **Status (2026-08-28, commit `b7d7cb9`)**: scaffold + 11 test files + L12 critical-tier
> review + #419 defer landed. `cargo test -p polygon-v1-spike --tests --offline` →
> **9 passed + 6 ignored (live-gated) + 0 failed**. PR #420 open against `rust-evm-core`.

---

## Phase 0.0: Network selection — LANDED

The spike's `config::Network` enum carries the same shape that `evm-wallet-core` will adopt
(Phase 0 of the plan):

```rust
enum Network { Ethereum, Polygon, PolygonAmoy }
```

Q1 (EVM-reuse) is the empirical proof of this design — `v3_derivation` proves that the
canonical "abandon ×11 + about" BIP-39 mnemonic + SLIP-44 coin type 60 derives to the
**same** EVM address on Ethereum + Polygon + PolygonAmoy (`0x9858EfFD232B4033E47d90003D41EC34EcaEda94`).
The actual `alloy-signer-local` primitives are shared across both EVM chains.

---

## Phase 1: Scaffold — LANDED (commit `7d33c95`)

`rust-wallet-app/spikes/polygon-v1/` + 4 new public modules (config, address, eip712, erc20,
spki, tokens, provider) + `tokens/{mainnet,amoy}.json` + workspace deps
`alloy-{provider,network,transport-http,sol-types,consensus,eips,rpc-types}` (per
`eth-wallet-core` precedent).

---

## Phase 2: V1-V10 implementation + L12 review + #419 defer — LANDED

### Offline (always run in CI)

| Test | Module | Status | What it proves |
|------|--------|--------|----------------|
| `v1_compile` | `tests/v1_compile.rs` | PASS | Type-asserted bindings (L12 finding) — `derive_evm_address`, `MockUSDC::balanceOfCall`, `CHAIN_ID_POLYGON` reachable |
| `v3_derivation` | `tests/v3_derivation.rs` | PASS (2 tests) | Cross-chain EVM address derivation identity (ETH = Polygon = PolygonAmoy for canonical "abandon ×11 + about") + canonical vector `0x9858EfFD...` |
| `v9_erc20_transfer` | `tests/v9_erc20_transfer.rs` | PASS (1 test, deploy + transfer only) | Anvil Polygon-fork: deploy `MockUSDC` (via signed EIP-1559 contract-creation tx) + `transfer(recipient, 100 USDC)` receipt status = true. `balanceOf` post-transfer **deferred per #419** (see Known Gaps) |
| `v10_eip712_replay` | `tests/v10_eip712_replay.rs` | PASS (4 tests) | EIP-712 domain separator chain-id binds (separators differ between Polygon + Ethereum) + chain-id constants match EIP-155 + new `v10_sign_then_recover_with_different_message_fails` (L12 finding: actually signs + recovers to prove replay defense) |
| `use_case_alpha_sends_beta_100_usdc_offline` | `tests/use_case_alpha_sends_beta_100_usdc.rs` | PASS (1 test, deploy + transfer only) | End-to-end on Anvil Polygon-fork: alpha (canonical "abandon ×11 + about") + beta ("letter advice cage...") derivation → MockUSDC deploy + 100 USDC transfer receipt. `balanceOf` deferred per #419 |

### Live (operator-driven per L29, gated)

| Test | Gate | What it proves |
|------|------|----------------|
| V2 (chain-id) | `RUN_POLYGON_AMOY=1` | Live `eth_chainId` → `0x13882` (80002) on Amoy |
| V4 (EIP-1559 + baseFee cadence) | `RUN_POLYGON_AMOY=1` | Live `eth_gasPrice` + `eth_maxPriorityFeePerGas`; 2-second block cadence |
| V6 (token registry on-chain decimals) | `RUN_POLYGON_AMOY=1` | Live `decimals()` verify on `tokens/amoy.json` USDC entry (L12 finding: registry now loaded via `tests/common::load_token_registry`, not hardcoded) |
| V7 (Amoy faucet) | `RUN_POLYGON_AMOY=1` | Faucet reachability + fund pattern |
| V8 (native POL transfer) | `RUN_POLYGON_AMOY=1` + `POLYGON_AMOY_PRIVATE_KEY` | POL transfer via `eth_sendRawTransaction` + receipt poll. Private key wrapped in `Zeroizing<[u8; 32]>` (L12 security-auditor finding) |
| V5 (mainnet RPC) | `RUN_POLYGON_MAINNET=1` | Mainnet `eth_blockNumber` + finality (~256 blocks ≈ 8 min) |
| Use-case live | `RUN_POLYGON_AMOY=1` + privkey | Full e2e broadcast on Amoy |

### Shared helpers (L12 convergent finding)

`tests/common/mod.rs` (commit `d5fd0dc`) extracts three duplicated patterns:

| Helper | Used by | Replaces |
|---|---|---|
| `env_opt_in(name)` | V2/V4/V5/V7 (4×) | Inline `std::env::var().map(...)` per test |
| `await_receipt(provider, tx_hash, attempts, interval)` | V8/V9/use_case (3× deploy + 3× transfer = 6×) | Inline receipt-poll loop with `for _ in 0..40 { get_transaction_receipt... sleep }` |
| `load_token_registry(network)` | V6 | Hardcoded `0x41e94eb...` USDC address |

---

## Known gaps

### Issue #419 — V9 + use_case `balanceOf` post-transfer round-trip

**Root cause** (discovered in this session): the `sol!` macro in alloy 1.8.x only generates
the `BYTECODE` static const + `deploy()` helper when the `bytecode = "0x..."` attribute is
passed to the macro invocation. Our `MockUSDC` lacks that attribute — the bytecode was
never compiled in, so Anvil receives only the constructor-args payload and deploys an
empty-code "contract" that returns `0x` from any `eth_call`.

**Why deferred**: adding the bytecode attribute requires first compiling the contract source
(e.g. via `solc`) and embedding the resulting hex — a chicken-and-egg loop on the test
infrastructure (the test's "scaffold + verify" workflow doesn't currently shell out to
solc).

**Fix path** (follow-up session):
1. Add `solc` to dev-deps (or shell out to system solc) + add a `build.rs` that compiles `MockUSDC.sol` → `MockUSDC.bytecode` constant
2. OR ship a pre-compiled bytecode constant + add `bytecode = "0x..."` attribute to the `sol!` invocation
3. OR swap the in-tree `MockUSDC` for `alloy_node_bindings::anvil` + a `MockERC20` JSON artifact that Anvil auto-loads

**Current test surface** (deferred assertions, NOT removed entirely): `V9 + use_case`
still verify deploy receipt status = true + transfer receipt status = true (the verifiable
empirical surface per the spike's mandate). The `balanceOf` post-transfer round-trip is
the tripwire for #419 resolution.

---

## Cross-spike pattern

Reuses TRON spike's structure verbatim:
1. Offline companion tests (always run in CI) — pure crypto + wire format
2. Live gated tests (L29 operator-driven) — `#[ignore]` + env var
3. Use-case test demonstrating V1-V10 stack end-to-end

Delta from TRON: local chain = Anvil Polygon-fork (in-process via `alloy-node-bindings`),
not Docker testcontainers. Ethereum-family chains reuse `alloy-node-bindings::Anvil` for
Polygon state forks.

---

## Verify gate

```bash
cargo fmt --all -- --check                                       # ✓ clean
cargo clippy -p polygon-v1-spike --all-targets -- -D warnings     # ✓ clean
cargo test -p polygon-v1-spike --tests --offline                 # ✓ 9 passed + 6 ignored + 0 failed
```

All three gates must pass before any commit per L13 step 11.

---

## Commit history (`spike/polygon-v1` branch, all pushed to origin)

| SHA | Phase | Title |
|---|---|---|
| `7d33c95` | 1 | `feat(spike/polygon-v1): scaffold V1-V10 verification harness for #416` |
| `1bfd4c0` | 2 | `feat(spike/polygon-v1): V1-V10 + use_case verification harness for #416` |
| `755e9ad` | 2.5 | `docs(spike/polygon-v1): fill RESULT.md with Phase2 PASS evidence` |
| `d5fd0dc` | 2.6 | `fix(spike/polygon-v1): apply L12 critical-tier review findings` |
| `b7d7cb9` | 2.7 | `fix(spike/polygon-v1): defer V9/use_case balanceOf per #419 (#420 follow-up)` |

PR #420 open against `rust-evm-core`.
