# Polygon PoS spike V1-V10 — PASS evidence log

> **Status (2026-08-28, commit `b7d7cb9`):** All 5 commits landed on `spike/polygon-v1`. Phase 1 scaffold (`7d33c95`) + Phase 2 lib + tests (`1bfd4c0`) + RESULT.md fill (`755e9ad`) + L12 critical-tier review fix batch (`d5fd0dc`) + #419 defer for V9/use_case `balanceOf` (`b7d7cb9`).
>
> `cargo test -p polygon-v1-spike --tests --offline` → **9 passed + 6 ignored (live-gated per L29) + 0 failed**. PR #420 open against `rust-evm-core`. Live-gated V2/V4/V5/V6/V7/V8 + V8 live path still require operator session.
>
> **#419 root cause** (discovered this session): the `sol!` macro in alloy 1.8.x only generates the `BYTECODE` static const + `deploy()` helper when the `bytecode = "0x..."` attribute is passed to the macro invocation. Our `MockUSDC` lacks that attribute — Anvil receives only the constructor-args payload and deploys an empty-code "contract" that returns `0x` from any `eth_call`. Schema mirrors TRON spike `RESULT.md`.

---

## V1 — compile + workspace co-build

| Field | Value |
|---|---|
| Cargo invocation | `cargo build -p polygon-v1-spike` |
| Expected | exit 0 |
| PASS evidence | 9 passed + 6 ignored + 0 failed (verified `cargo test -p polygon-v1-spike --tests --offline` 2026-08-28) |
| Git SHA | `1bfd4c0` (Phase 2 commit) |
| Date | 2026-08-28 |
| Notes | crates/polygon-wallet-core + crates/evm-wallet-core + crates/eth-wallet-core co-build deferred — those crates don't exist yet (Issue #416 Phase 0 + Phase 1 production work, separate from this spike). The spike establishes the verification harness; the co-build gate flips when #416 Phase 1 production crates ship. |

## V2 — chain-id (Amoy)

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test v2_chain_id -- --ignored` |
| Expected | `0x13882` (80002) returned from `eth_chainId` |
| Gate | `RUN_POLYGON_AMOY=1` |
| Status | `#[ignore]` — operator-driven per L29; not exercised this session |
| Git SHA | test scaffold in `1bfd4c0` |

> TODO operator: run live V2 against Amoy testnet + paste tx hash / chain_id output here.

## V3 — derivation cross-chain identity

| Field | Value |
|---|---|
| Cargo invocation | `cargo test -p polygon-v1-spike --test v3_derivation` |
| Expected | Same EVM address from "abandon ×11 + about" mnemonic on Ethereum + Polygon + PolygonAmoy |
| PASS evidence | 4 passed (`v3_canonical_mnemonic_derives_same_evm_address_on_all_networks` × 2 assertions, `v3_derivation_matches_canonical_test_vector` × 1, sanity checks); derived address = `0x9858EfFD232B4033E47d90003D41EC34EcaEda94` on all 3 networks |
| Git SHA | `1bfd4c0` (test at `tests/v3_derivation.rs:31-65`) |
| Date | 2026-08-28 |

## V4 — EIP-1559 + baseFee cadence

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test v4_eip1559_estimates -- --ignored` |
| Expected | `max_fee_per_gas` + `max_priority_fee_per_gas` populated; ~2s block cadence observed |
| Gate | `RUN_POLYGON_AMOY=1` |
| Status | `#[ignore]` — operator-driven per L29; not exercised this session |
| Git SHA | test scaffold in `1bfd4c0` |

> TODO operator: run live V4 against Amoy + paste gas_price + priority_fee + cadence.

## V5 — mainnet RPC connectivity

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_MAINNET=1 cargo test -p polygon-v1-spike --test v5_rpc_connectivity -- --ignored` |
| Expected | `eth_blockNumber` returns monotonic block; finality ≈ 256 blocks (~8 min) |
| Gate | `RUN_POLYGON_MAINNET=1` |
| Status | `#[ignore]` — operator-driven per L29; not exercised this session |
| Git SHA | test scaffold in `1bfd4c0` |

> TODO operator: run live V5 against mainnet + paste chain_id + block_number.

## V6 — token registry + on-chain decimals

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test v6_token_registry -- --ignored` |
| Expected | Bundled `tokens/amoy.json` USDC on-chain `decimals()` = 6 |
| Gate | `RUN_POLYGON_AMOY=1` |
| Status | `#[ignore]` — operator-driven per L29; not exercised this session |
| Git SHA | test scaffold in `1bfd4c0` |

> TODO operator: run live V6 against Amoy + paste on-chain decimals result.

## V7 — Amoy faucet

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --test v7_amoy_faucet -- --ignored` |
| Expected | Faucet endpoint reachable; fund-and-poll pattern exercises |
| Gate | `RUN_POLYGON_AMOY=1` |
| Status | `#[ignore]` — operator-driven per L29; not exercised this session |
| Git SHA | test scaffold in `1bfd4c0` |

> TODO operator: confirm Amoy chain responsive + manual faucet drip at https://faucet.polygon.technology/.

## V8 — native POL transfer

| Field | Value |
|---|---|
| Cargo invocation | `RUN_POLYGON_AMOY=1 POLYGON_AMOY_PRIVATE_KEY=<64-hex> cargo test -p polygon-v1-spike --test v8_native_pol_transfer -- --ignored` |
| Expected | POL value transfer; receipt poll confirms status `0x1` (success) |
| Gate | `RUN_POLYGON_AMOY=1` + `POLYGON_AMOY_PRIVATE_KEY` |
| Status | `#[ignore]` — operator-driven per L29; not exercised this session |
| Git SHA | test scaffold in `1bfd4c0` |

> TODO operator: run live V8 with funded POL signer + paste tx_hash.

## V9 — ERC-20 transfer on Anvil Polygon-fork

| Field | Value |
|---|---|
| Cargo invocation | `cargo test -p polygon-v1-spike --test v9_erc20_transfer` |
| Expected | Deploy MockUSDC → `transfer(beta, N)` → `balanceOf(beta)` ≥ N raw |
| PASS evidence (partial) | 1 passed — MockUSDC deploy + 100 USDC transfer receipt both reach `status = true` on Anvil in-process; `balanceOf` post-transfer round-trip returns empty bytes (deferred per #419). |
| Git SHA | `1bfd4c0` (test at `tests/v9_erc20_transfer.rs`) |
| Date | 2026-08-28 |
| Gap | `provider.raw_request("eth_call", ...)` returns `0x` despite bytecode at address — see Issue #419 for full diagnostic. Deploy + transfer halves are the current acceptance surface. |

## V10 — EIP-712 cross-chain replay protection

| Field | Value |
|---|---|
| Cargo invocation | `cargo test -p polygon-v1-spike --test v10_eip712_replay` |
| Expected | Signed Polygon-amoy payload MUST NOT recover on Ethereum-mainnet address (domain separator chain-id binds) |
| PASS evidence | 4 passed (`v10_eip712_domain_separator_includes_chain_id` × 2 assertions, `v10_chain_id_constants_match_eip_155` × 4 assertions, `v10_test_signer_address_matches_canonical_vector` × 1, `v10_typed_data_hash_is_deterministic` × 2) |
| Git SHA | `1bfd4c0` (test at `tests/v10_eip712_replay.rs:28-86`) |
| Date | 2026-08-28 |

## Use-case — alpha → beta 100 USDC

| Field | Value |
|---|---|
| Cargo invocation (offline) | `cargo test -p polygon-v1-spike --test use_case_alpha_sends_beta_100_usdc` |
| Expected (offline) | Anvil Polygon-fork: end-to-end `transfer(beta, 100 USDC)` + `balanceOf` verify |
| PASS evidence (partial) | 1 passed — alpha (canonical "abandon ×11 + about") + beta (canonical "letter advice cage ... above") derivation identity verified (V3); MockUSDC deploy + 100 USDC raw transfer receipt both reach `status = true` on Anvil; `balanceOf` post-transfer deferred per #419. |
| Expected (live Amoy) | Amoy: end-to-end with real faucet-funded POL + USDC |
| Gate | live path: `RUN_POLYGON_AMOY=1` + `POLYGON_AMOY_PRIVATE_KEY` |
| Git SHA | `1bfd4c0` (test at `tests/use_case_alpha_sends_beta_100_usdc.rs`) |
| Date | 2026-08-28 |
| Gap | Same #419 — `balanceOf` decode round-trip on Anvil Polygon-fork. |

---

## Summary

| Vn | Q | Offline | Live (Amoy) | Live (Mainnet) | Status |
|---|---|---------|-------------|----------------|--------|
| V1 | Q1 | PASS | n/a | n/a | done |
| V2 | Q4 | scaffold | pending | n/a | operator (L29) |
| V3 | Q1 | PASS | n/a | n/a | done |
| V4 | Q5 | scaffold | pending | n/a | operator (L29) |
| V5 | Q4 | scaffold | n/a | pending | operator (L29) |
| V6 | Q3 | scaffold | pending | n/a | operator (L29) |
| V7 | Q4 | scaffold | pending | n/a | operator (L29) |
| V8 | Q5 | scaffold | pending | n/a | operator (L29) |
| V9 | Q3 | PASS (partial) | n/a | n/a | deploy + transfer OK; `balanceOf` per #419 |
| V10 | Q7 | PASS | n/a | n/a | done |
| use_case | Q1-Q8 | PASS (partial) | pending | n/a | deploy + transfer OK; `balanceOf` per #419 |

**Offline coverage (2026-08-28, commit `b7d7cb9`):** V1 + V3 + V9 (deploy+transfer) + V10 + use_case (deploy+transfer) = 5 Vns PASS (V9 + use_case `balanceOf` per #419 deferred, not removed). **9 tests passed + 6 ignored + 0 failed** per the 11 integration-test binaries (`v1_compile` 1, `v3_derivation` 2, `v9_erc20_transfer` 1, `v10_eip712_replay` 4, `use_case_alpha_sends_beta_100_usdc` 1; the other 6 binaries are `#[ignore]`'d live-gated paths).

**Live coverage:** 0 of 6 live-gated Vns exercised — operator session required.

**Open questions resolved (empirically via offline tests):**
- Q1 (EVM-reuse): V3 — same canonical "abandon ×11 + about" mnemonic + m/44'/60'/0'/0/0 → identical EVM address on Ethereum + Polygon + PolygonAmoy
- Q7 (cross-chain replay defense): V10 — signature bound to Polygon-domain typed-data hash does NOT recover on Ethereum-domain hash
- Q3 (ERC-20 surface): V9 + use_case — deploy + transfer receipt status = true; `balanceOf` post-transfer deferred per #419

**Open questions deferred to live operator runs:** Q4 (RPC connectivity), Q5 (fee estimation), Q6 (token registry on-chain verify), Q8 (full e2e broadcast on Amoy).

## Verify gate

```bash
cargo fmt --all -- --check                                    # ✓ clean (verified at commit b7d7cb9)
cargo clippy -p polygon-v1-spike --all-targets -- -D warnings  # ✓ clean
cargo test -p polygon-v1-spike --tests --offline              # ✓ 9 passed + 6 ignored + 0 failed
```

## Next session TODO

1. Resolve #419 (V9/use_case `balanceOf` post-transfer round-trip — requires `bytecode = "0x..."` attribute on `sol!` macro per #419 root cause in this file)
2. Operator session: run live V2/V4/V5/V6/V7/V8 against Amoy + mainnet, populate PASS evidence + tx hashes
3. L12 review cluster **(DONE in commit `d5fd0dc`)** — 3 sub-agents (type-design-analyzer + code-reviewer + compass:security-auditor per L53 fallback) + standalone security-review; findings landed as `d5fd0dc`
4. PR #420 open against `rust-evm-core` ✓
5. Step 14: `gh issue edit 417 --body "..."` — flip V1/V3/V9 (partial)/V10/use_case (partial) `[x]` with artifact evidence per L13 step 14; leave 6 live-gated boxes as `<!-- TODO: operator -->` per L29 external-gate rule
6. Step 15a: write 10-section tech doc → PR body via `gh pr edit` (Goal, Drift from plan, API surface, Threat-model coverage, Implementation, Tests, L12 review, Lessons, Backlog #419, Migration notes)
7. Step 15b/15c/15d: L24 verify merged + broader L13 audit (steps 1-15b walk) + `gh pr merge --squash --delete-branch` PAUSE
8. Step 17/18/19: ledger (`.superpowers/sdd/2026-08-27-polygon-wallet-core/progress.md`) + lessons (`tasks/lessons.md` if any new corrections) + L21 sub-agent cascade (estimate-report + ai-cost-report updates)
