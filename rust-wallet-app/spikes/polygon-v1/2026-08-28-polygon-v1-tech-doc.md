# Polygon PoS spike V1-V10 — tech doc (PR #420)

> Status: 2026-08-28, commit `0a358ff` (latest). 6 commits on `spike/polygon-v1`,
> 9 passed + 6 ignored + 0 failed. PR #420 open against `rust-evm-core`. Local
> gates clean (`cargo fmt --all -- --check`, `cargo clippy -- -D warnings`).

## 1. Goal

Issue #417 spike resolves Q1-Q8 from Issue #416 before production code ships
in `evm-wallet-core` + `polygon-wallet-core`. V1-V10 + use-case verify that
the planned EVM-family tooling (`alloy-node-bindings::Anvil` for Polygon-fork,
`alloy_signer_local::MnemonicBuilder` for SLIP-44 coin-type-60 derivation,
alloy `sol!` macro for contract types) covers the design surface end-to-end.
Mirrors TRON spike structure: offline companion tests + live gated + use-case.

## 2. Drift from plan

| Plan said | Shipped | Drift |
|---|---|---|
| `evm-wallet-core` + `polygon-wallet-core` co-build V1 gate | Spike builds standalone in `spikes/polygon-v1/` | none — V1 documented as "gate flips when Phase 1 production crates ship" |
| `sol!` macro auto-generates `BYTECODE` + `deploy()` | alloy 1.8.x only generates when `bytecode = "0x..."` attribute present | **drift** — root cause for #419; deploy+transfer halves PASS, `balanceOf` post-transfer round-trip deferred |
| `Zeroizing` for private-key material in V8 | `Zeroizing<[u8; 32]>` wrapped + `.zeroize()` at end of test fn | none — added per L12 security-auditor finding |
| Token registry hardcoded for V6 | `tests/common::load_token_registry(Network)` reads bundled `tokens/amoy.json` | improvement — L12 type-design-analyzer HIGH finding fixed |
| Inline env-var check, inline receipt-poll loop | `tests/common::{env_opt_in, await_receipt}` helpers | improvement — L12 convergent finding |

## 3. API surface

`polygon_v1_spike::*` (public lib, 5 modules):

| Module | Public items |
|---|---|
| `config` | `Network` enum (`Ethereum`, `Polygon`, `PolygonAmoy`); `ChainConfig { network, chain_id, default_rpc_url }`; `ChainConfig::for_network(network)`; chain-id consts `ETHEREUM_MAINNET_CHAIN_ID=1`, `POLYGON_MAINNET_CHAIN_ID=137`, `POLYGON_AMOY_CHAIN_ID=80_002` |
| `address` | `build_signer(mnemonic, network) -> Result<PrivateKeySigner, DeriveError>` (m/44'/60'/0'/0/0); `derive_evm_address(mnemonic, network) -> Result<Address, DeriveError>`; `DeriveError(String)` |
| `erc20` | `sol!`-generated `MockUSDC` (`name`, `symbol`, `decimals`, `totalSupply`, `balanceOf`, `transfer`, `constructor(uint256 initialSupply)`); const `USDC_DECIMALS: u8 = 6`; `usdc_to_raw(units: u64) -> U256` |
| `eip712` | `domain_for_chain(chain_id, verifying_contract) -> Eip712Domain` (`name="PolygonV1Spike"`, `version="1"`); `build_test_signer() -> PrivateKeySigner`; `wrap_digest(message: [u8;32]) -> B256` (renamed from `typed_data_hash` per L12); chain-id consts mirror `config::*` |
| `tokens` | `TokenEntry { symbol, address, decimals }` (serde-deserialized); `include_str!("../../tokens/amoy.json")` + `mainnet.json` bundled |

Internal-only test helpers live in `tests/common/mod.rs` (3 functions, all
`#[allow(dead_code)]` for `#[ignore]`-gated tests).

## 4. Threat-model coverage

| Threat | Mitigation | Evidence |
|---|---|---|
| Private key leak in test memory | `Zeroizing<[u8;32]>` wrapper, `.zeroize()` at end of V8 | `tests/v8_native_pol_transfer.rs` |
| Signature replay across EVM chains | EIP-712 domain separator binds `chain_id`; `v10_sign_then_recover_with_different_message_fails` actually signs + recovers to prove replay defense | `tests/v10_eip712_replay.rs` (4 tests) |
| Wrong-network USDC transfer (ETH-USDC vs Polygon-USDC.e) | `Network` enum + chain-id consts surface this at compile time | `config.rs` |
| Empty-code contract deploy (the #419 root cause) | `status = true` assertion on deploy receipt (defense-in-depth — `contract_address` can populate even with empty bytecode) | V9 + use_case |
| Mnemonic entropy weakness (canonical "abandon ×11 + about") | Test-only mnemonic, never used in production paths; gated behind `#[ignore]` for live paths | V3 + use_case + V8 |
| RPC endpoint trust | Default URLs are public Polygon endpoints; tests accept operator override via env | V5 |
| Gas griefing (stale priority fee) | `max_fee_per_gas` + `max_priority_fee_per_gas` both explicit; not relying on auto-estimation | V9 + use_case |

Out-of-scope for spike: HSM-backed key storage, MPC, multi-sig — those
belong in production `polygon-wallet-core`, not in the verification harness.

## 5. Implementation

Five phases landed (mirrors plan):

**Phase 0.0** — `Network` enum + `ChainConfig` (`config.rs`). Same shape
`evm-wallet-core` will adopt; V3 proves cross-chain identity empirically.

**Phase 1** — Scaffold (commit `7d33c95`). 5 lib modules + `tokens/{mainnet,
amoy}.json` + 6 alloy workspace deps (`alloy-{provider,network,transport-
http,consensus,eips,rpc-types}` =1.8.3; `alloy-sol-types` from crates.io
because workspace doesn't pin it).

**Phase 2** — V1-V10 + use_case (commit `1bfd4c0`). Key choices:
- Local chain = `alloy-node-bindings::Anvil` (in-process Polygon-fork),
  NOT Docker testcontainers (delta from TRON spike).
- Derivation = `MnemonicBuilder::english().phrase(m).index(0).build()` →
  `alloy_signer_local::PrivateKeySigner`. SLIP-44 coin type 60 implicit in
  `alloy_signer_local`'s default EVM path.
- Sign + send = `Signer::sign_transaction_sync(&mut TxEip1559)` →
  `into_signed(sig).encoded_2718()` → `eth_sendRawTransaction`. Bypasses
  `sol!`-generated `deploy()` helper per #419 (see §9).
- Receipt poll = `tests/common::await_receipt(provider, tx_hash, 40, 100ms)`.

**L12 fix batch** (commit `d5fd0dc`) — applied 5 HIGH, 8 MEDIUM, 7 LOW
findings from 3 sub-agents. Highlights: `Zeroizing` for V8 privkey, type-
asserted `v1_compile`, `tests/common/` extraction, alpha-mnemonic signer in
use_case (not `random()`), `status = true` assertion on deploy receipt.

**#419 defer** (commit `b7d7cb9`) — V9 + use_case `balanceOf` round-trip
removed; deploy + transfer halves still PASS `status = true`.

**Docs refresh** (commits `755e9ad`, `0a358ff`) — RESULT.md + ROADMAP.md
landed with empirical PASS evidence + known-gaps section.

## 6. Tests

11 integration test binaries, all offline-runnable:

| Binary | Tests | Status | Proves |
|---|---|---|---|
| `v1_compile.rs` | 1 type-asserted | PASS | Type-asserted bindings reachable from `polygon_v1_spike::*` |
| `v2_chain_id.rs` | 1 `#[ignore]` | gate | Live `eth_chainId` → `0x13882` on Amoy |
| `v3_derivation.rs` | 2 | PASS | Cross-chain EVM address identity (ETH = Polygon = PolygonAmoy) + canonical vector `0x9858EfFD...` |
| `v4_eip1559_estimates.rs` | 1 `#[ignore]` | gate | Live `eth_gasPrice` + `eth_maxPriorityFeePerGas` |
| `v5_rpc_connectivity.rs` | 1 `#[ignore]` | gate | Mainnet `eth_blockNumber` + finality |
| `v6_token_registry.rs` | 1 `#[ignore]` | gate | On-chain `decimals()` matches `tokens/amoy.json` |
| `v7_amoy_faucet.rs` | 1 `#[ignore]` | gate | Faucet reachability |
| `v8_native_pol_transfer.rs` | 1 `#[ignore]` | gate | POL transfer via `eth_sendRawTransaction` |
| `v9_erc20_transfer.rs` | 1 | PASS (partial) | Anvil: deploy + transfer receipt `status = true`; `balanceOf` per #419 |
| `v10_eip712_replay.rs` | 4 | PASS | Domain separator chain-id binds + replay defense (sign-then-recover) |
| `use_case_alpha_sends_beta_100_usdc.rs` | 1 | PASS (partial) | Anvil: alpha→beta 100 USDC end-to-end; `balanceOf` per #419 |

**Totals:** 9 passed + 6 ignored + 0 failed.

**Verify gate:**
```bash
cargo fmt --all -- --check                                    # clean
cargo clippy -p polygon-v1-spike --all-targets -- -D warnings  # clean
cargo test -p polygon-v1-spike --tests --offline              # 9 passed + 6 ignored + 0 failed
```

## 7. L12 review (commit `d5fd0dc`)

Critical-tier L12 review (3 sub-agents parallel): `ecc:type-design-analyzer`,
`ecc:code-reviewer`, `compass:security-auditor` (L53 fallback from
`ecc:security-reviewer` unavailability). 20 findings landed: 5 HIGH, 8
MEDIUM, 7 LOW. 17 files touched, +487/-224 lines.

| Severity | Count | Examples |
|---|---|---|
| HIGH | 5 | `v10_sign_then_recover_with_different_message_fails` actually signs+recovers (not just domain-separator formula); `load_token_registry` reads bundled JSON; remove `transfer_selector()`/`balance_of_selector()` (sol! generates); rename `typed_data_hash` → `wrap_digest` (typed-data hash ≠ bare digest); `MockUSDC::balanceOfCall` type-asserted |
| MEDIUM | 8 | alpha-mnemonic signer in use_case (was `random()`); `status = true` on deploy receipt; `tests/common/` extraction; `env_opt_in` helper |
| LOW | 7 | unused-import `#[allow]` instead of `_placeholder`; `dead_code` `#[allow]` on common helpers |

L13 Q5 budget: 1/3 fix rounds used. 2 rounds remain if follow-up reviews
surface issues.

## 8. Lessons

**L13** — L12 critical-tier review catches real issues (HIGH: V10 was
testing a formula, not the actual replay attack surface). 3-sub-agent
parallel worth the cost.

**L29** — Live-gated tests (`#[ignore]` + env-var) belong in operator
session, not CI. RESULT.md documents the gate so operator knows what to run.

**Alloy 1.8.x quirk** — `sol!` macro requires `bytecode = "0x..."` attribute
to emit `BYTECODE` const + `deploy()` helper. Missing attribute → Anvil
deploys empty-code "contract" → `eth_call` returns `0x`. Documented in #419.

**Shared helpers** — `tests/common/mod.rs` (commit `d5fd0dc`) extracted 3
duplicated patterns. Worth the L12 convergent-finding cost.

**V8 Zeroizing** — Security-auditor flagged bare `[u8; 32]` for privkey.
Wrap in `Zeroizing<[u8;32]>`, zero at end of test fn. Single-line change,
real defense-in-depth.

## 9. Backlog #419

**Title:** V9 + use_case `balanceOf` post-transfer round-trip returns empty
bytes on Anvil Polygon-fork.

**Root cause** (discovered in commit `b7d7cb9`): `sol!` macro in alloy 1.8.x
only generates `BYTECODE` static const + `deploy()` helper when
`bytecode = "0x..."` attribute is passed to the macro invocation. Our
`MockUSDC` lacks that attribute — Anvil receives only the constructor-args
payload and deploys an empty-code "contract" that returns `0x` from any
`eth_call`.

**Why deferred:** adding the bytecode attribute requires first compiling
the contract source (e.g. via `solc`) and embedding the resulting hex — a
chicken-and-egg loop on the test infrastructure.

**Fix paths** (3, ordered by complexity):
1. Add `solc` to dev-deps (or shell out to system `solc`) + add `build.rs`
   that compiles `MockUSDC.sol` → `MockUSDC.bytecode` constant
2. Ship pre-compiled bytecode constant + add `bytecode = "0x..."` attribute
3. Swap in-tree `MockUSDC` for `alloy_node_bindings::anvil`-loaded JSON artifact

**Current state:** V9 + use_case assert deploy + transfer `status = true`
(the verifiable empirical surface per spike mandate). `balanceOf` is the
tripwire — once #419 fixed, the round-trip assertion lands.

## 10. Migration notes

**For `polygon-wallet-core` (Phase 1 production):**
- Adopt `Network` enum shape verbatim; drop `Ethereum` if polygon-only CLI
- Replace `derive_evm_address` thin wrapper with `Bip32Signer` derivation
  once `evm-wallet-core` lands
- `eip712::domain_for_chain` generalizes — production needs EIP-712
  payloads for Permit2 / Seaport, not just our spike domain
- `tokens::{amoy,mainnet}.json` ship as-is; production adds a registry
  source (chainlist.org JSON, CoinGecko, etc.)
- `MockUSDC` stays as test-only; production uses real USDC.e / native USDC
  per `chain_id`

**For `evm-wallet-core` (Phase 0 refactor):**
- Network enum + ChainConfig already shaped correctly; no rewrite needed
- `alloy-signer-local` MnemonicBuilder is fine for now; production may
  want `bip32` crate for HD wallet paths beyond m/44'/60'/0'/0/0
- Anvil in-process is fine for unit tests; production CI may need
  testcontainers for hermetic runs

**For downstream crates (CLI, SDK bindings):**
- `polygon-wallet-core` is ~200 LoC wrapper — no signing/RPC duplication
- EIP-1559 + EIP-712 surface already proven by V4 + V10; ship both
- Live V2/V4/V5/V6/V7/V8 + use_case live path must run before merge to
  mainnet branch — operator session pending
