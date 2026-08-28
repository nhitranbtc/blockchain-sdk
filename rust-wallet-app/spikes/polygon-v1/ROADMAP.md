# Polygon PoS spike roadmap — verification harness for Issue #417

> **Goal**: Document the V1-V10 verification path for Issue #416 open questions (Q1-Q8)
> before production code ships in `evm-wallet-core` + `polygon-wallet-core`. Mirrors the
> TRON spike's structure (offline companion + live gated + use-case) but with Ethereum-family
> tooling (Anvil Polygon-fork via `alloy-node-bindings`, no Docker).

---

## Phase 0.0: Network selection

The spike's `config::Network` enum carries the same shape that `evm-wallet-core` will adopt
(Phase 0 of the plan):

```text
enum Network { Ethereum, Polygon, PolygonAmoy }
```

This is the Phase 0.0 pre-step per plan §Phase 0.0 — before any refactor of `eth-wallet-core`
to `evm-wallet-core`, the spike establishes that the same `alloy`-backed primitives work for
both Ethereum + Polygon. Q1 (EVM-reuse) is the empirical proof of this design.

---

## Test inventory

### Offline (always run in CI)

| Test | Module | What it proves |
|------|--------|----------------|
| `v1_compile` | `tests/v1_compile.rs` | `cargo build -p polygon-v1-spike` + workspace co-build pass |
| `v3_derivation` | `tests/v3_derivation.rs` | Cross-chain EVM address derivation identity (ETH = Polygon for same mnemonic) |
| `v9_erc20_transfer` | `tests/v9_erc20_transfer.rs` | Anvil Polygon-fork: deploy mock USDC → `transfer(beta, N)` → `balanceOf` |
| `v10_eip712_replay` | `tests/v10_eip712_replay.rs` | EIP-712 domain-separator chain-id binds; cross-chain replay MUST fail |
| `use_case_alpha_sends_beta_100_usdc_offline` | `tests/use_case_alpha_sends_beta_100_usdc.rs` | End-to-end on Anvil Polygon-fork: alpha → beta 100 USDC |

### Live (operator-driven per L29, gated)

| Test | Gate | What it proves |
|------|------|----------------|
| V2 (chain-id) | `RUN_POLYGON_AMOY=1` | Live `eth_chainId` → `0x13882` (80002) on Amoy |
| V4 (EIP-1559 + baseFee cadence) | `RUN_POLYGON_AMOY=1` | Live `eth_gasPrice` + `eth_maxPriorityFeePerGas`; 2-second block cadence |
| V6 (token registry on-chain decimals) | `RUN_POLYGON_AMOY=1` | Live `decimals()` verify on `tokens/amoy.json` USDC entry |
| V7 (Amoy faucet) | `RUN_POLYGON_AMOY=1` | Faucet reachability + fund pattern |
| V8 (native POL transfer) | `RUN_POLYGON_AMOY=1` | POL transfer via `eth_sendRawTransaction` + receipt poll |
| V5 (mainnet RPC) | `RUN_POLYGON_MAINNET=1` | Mainnet `eth_blockNumber` + finality (~256 blocks ≈ 8 min) |
| Use-case live | `RUN_POLYGON_AMOY=1` + privkey | Full e2e broadcast on Amoy |

---

## Cross-spike pattern

Reuses TRON spike's structure verbatim:
1. Offline companion tests (always run in CI) — pure crypto + wire format
2. Live gated tests (L29 operator-driven) — `#[ignore]` + env var
3. Use-case test demonstrating V1-V10 stack end-to-end

Delta from TRON: local chain = Anvil Polygon-fork (in-process via `alloy-node-bindings`),
not Docker testcontainers. Ethereum-family chains reuse `alloy-node-bindings::Anvil` for
Polygon state forks — see `alloy-v1/tests/v6_erc20_anvil.rs:31` for the pattern.

---

## Verify gate

```bash
cargo fmt --all -- --check
cargo clippy -p polygon-v1-spike --all-targets -- -D warnings
cargo test -p polygon-v1-spike --tests
```

All three gates must pass before any commit per L13 step 11.