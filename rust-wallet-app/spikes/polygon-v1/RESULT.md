# Polygon PoS spike V1-V10 — PASS evidence log

> **Status**: scaffold landed; PASS evidence populated post-smoke (Phase 3).
> Schema mirrors TRON spike `RESULT.md`. Filled per-Vn with cargo invocation + git SHA
> after each test passes.

---

## V1 — compile + workspace co-build

| Field | Value |
|---|---|
| Cargo invocation | `cargo build -p polygon-v1-spike` |
| Expected | exit 0 |
| Git SHA | _(filled post-smoke)_ |
| Date | _(filled post-smoke)_ |
| PASS evidence | _(filled post-smoke)_ |

## V2 — chain-id (Amoy)

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test v2_chain_id` |
| Expected | `0x13882` (80002) returned from `eth_chainId` |
| Gate | `RUN_POLYGON_AMOY=1` |
| Git SHA | _(filled post-smoke)_ |
| Date | _(filled post-smoke)_ |

## V3 — derivation cross-chain identity

| Field | Value |
|---|---|
| Cargo invocation | `cargo test -p polygon-v1-spike --test v3_derivation` |
| Expected | Same EVM address from "abandon ×11 + about" mnemonic on Ethereum + Polygon |
| Git SHA | _(filled post-smoke)_ |

## V4 — EIP-1559 + baseFee cadence

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test v4_eip1559_estimates` |
| Expected | `max_fee_per_gas` + `max_priority_fee_per_gas` populated; ~2s block cadence observed |
| Gate | `RUN_POLYGON_AMOY=1` |
| Git SHA | _(filled post-smoke)_ |

## V5 — mainnet RPC connectivity

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_MAINNET=1 cargo test -p polygon-v1-spike --test v5_rpc_connectivity` |
| Expected | `eth_blockNumber` returns monotonic block; finality ≈ 256 blocks (~8 min) |
| Gate | `RUN_POLYGON_MAINNET=1` |
| Git SHA | _(filled post-smoke)_ |

## V6 — token registry + on-chain decimals

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test v6_token_registry` |
| Expected | Bundled `tokens/amoy.json` USDC on-chain `decimals()` = 6 |
| Gate | `RUN_POLYGON_AMOY=1` |
| Git SHA | _(filled post-smoke)_ |

## V7 — Amoy faucet

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test v7_amoy_faucet` |
| Expected | Faucet endpoint reachable; fund-and-poll pattern exercises |
| Gate | `RUN_POLYGON_AMOY=1` |
| Git SHA | _(filled post-smoke)_ |

## V8 — native POL transfer

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test v8_native_pol_transfer` |
| Expected | POL value transfer; receipt poll confirms status `0x1` (success) |
| Gate | `RUN_POLYGON_AMOY=1` |
| Git SHA | _(filled post-smoke)_ |

## V9 — ERC-20 transfer on Anvil Polygon-fork

| Field | Value |
|---|---|
| Cargo invocation | `cargo test -p polygon-v1-spike --test v9_erc20_transfer` |
| Expected | Deploy mock USDC → `transfer(beta, N)` → `balanceOf(beta)` ≥ N raw |
| Git SHA | _(filled post-smoke)_ |

## V10 — EIP-712 cross-chain replay protection

| Field | Value |
|---|---|
| Cargo invocation | `cargo test -p polygon-v1-spike --test v10_eip712_replay` |
| Expected | Signed Polygon-amoy payload MUST NOT recover on Ethereum-mainnet address (domain separator chain-id binds) |
| Git SHA | _(filled post-smoke)_ |

## Use-case — alpha → beta 100 USDC

| Field | Value |
|---|---|
| Cargo invocation (offline) | `cargo test -p polygon-v1-spike --test use_case_alpha_sends_beta_100_usdc` |
| Cargo invocation (live Amoy) | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test use_case_alpha_sends_beta_100_usdc -- --ignored --nocapture` |
| Expected (offline) | Anvil Polygon-fork: end-to-end `transfer(beta, 100 USDC)` + `balanceOf` verify |
| Expected (live Amoy) | Amoy: end-to-end with real faucet-funded POL + USDC |
| Gate | live path: `RUN_POLYGON_AMOY=1` |
| Git SHA | _(filled post-smoke)_ |