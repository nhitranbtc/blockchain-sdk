# eth-wallet-core (v0.2) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver `rust-wallet-app/crates/eth-wallet-core/` — an Ethereum (ETH + ERC-20 stablecoin) wallet library built on alloy v1.8.x, plus an `eth` CLI in the umbrella. Mirrors `bitcoin-wallet-core/` (v0.1) structure. Resolves the 9 open questions from `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` and PR #290.

**Architecture:** Four phases.
- **Phase 0** = scaffold crate + canonical test (MnemonicBuilder → address).
- **Phase 1** = core wallet ops (create / import / list / delete / show / sign).
- **Phase 2** = RPC integration (provider + raw reqwest SPKI pin).
- **Phase 3** = ERC-20 stablecoin transfer (USDT + USDC) + token registry.
- **Phase 4** = `eth` CLI + Sepolia/mainnet smoke + release cut.

**Tech Stack:** Rust 1.94 stable, `alloy = "=1.8.3"` (Q1), `bip32` ^0.5 + `bip39` 2.2 (workspace deps, reused from Bitcoin), `reqwest` 0.12 + `rustls` 0.23 (raw SPKI pin per Q2), `alloy-sol-types` (ERC-20 ABI), `alloy-node-bindings` (Anvil regtest per Q8), `clap` (CLI).

## Global Constraints (verbatim from #293 resolutions)

- **Q1 — MSRV**: Pin `alloy = "=1.8.3"` (latest 1.x; MSRV 1.91 < workspace toolchain 1.94). 2.x line (2.4.1, MSRV 1.94.1) deferred to BTC-side ecosystem settle.
- **Q2 — TLS pinning transport**: `alloy-transport-http` does NOT expose a custom `ServerCertVerifier` hook. Workaround = raw `reqwest` + `rustls` with custom verifier for pinned endpoints (mirrors Bitcoin F20 / Task 7). alloy-provider stays for non-pinned endpoints (localhost Anvil during dev).
- **Q3 — derivation path default**: `m/44'/60'/0'/0/0` (Ledger/SLIP-44). Configurable via `WalletConfig` (matches Bitcoin F11).
- **Q4 — fillers vs explicit**: Use `Provider::new_http(url)` (no auto-fillers). Explicit nonce + gas estimation in `WalletManager`, parallel to Bitcoin.
- **Q5 — decimals**: Cache (one `eth_call decimals()` per token at startup, persist in token registry). NOT hard-coded.
- **Q6 — registry location**: (c) bundled repo `rust-wallet-app/crates/eth-wallet-core/tokens/mainnet.json` for v0.2; (b) `~/.config/<app>/tokens.json` for v1.x.
- **Q7 — zeroize**: Mirror Bitcoin Task 30. `Mnemonic` → `Zeroizing<Mnemonic>`, `XPrv` → wrap, `PrivateKeySigner`'s internal key — extract into zeroize-owned buffer; defer full audit to eth/ crate implementation.
- **Q8 — Anvil**: Add `alloy-node-bindings` as `[dev-dependencies]` for regtest-style smoke (mirrors Bitcoin Docker regtest).
- **Q9 — stablecoin source of truth**: Versioned registry + `--update-tokens` CLI command that fetches Circle's published list at runtime. Tether publishes no equivalent page; risk = stale USDT data. Circle source: `developers.circle.com/stablecoins/usdc-contract-addresses`.

## F47 zeroize gap (F47 from Bitcoin plan applies)

Mnemonic + password never logged. With FFI integration deferred to v0.3 (per #292), F47 stays in-process: mnemonic wraps in `Secret<String>` (Rust) or `Zeroizing<Mnemonic>` (Dart heap cleared on drop). Do NOT log in error paths.

## File Structure (decomposition)

```
rust-wallet-app/crates/eth-wallet-core/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # pub mod + Error enum + WalletManager
│   ├── wallet.rs              # create / import / list / delete / show
│   ├── mnemonic.rs            # bip39 generate + zeroize wrap (Q7)
│   ├── derivation.rs          # bip32 derive m/44'/60'/0'/0/{idx} (Q3)
│   ├── provider.rs            # alloy::Provider + SPKI pin verifier (Q2, Q4)
│   ├── erc20.rs               # sol! transfer + decimals query (Q5)
│   ├── tokens.rs              # bundled token registry loader (Q6, Q9)
│   ├── error.rs               # 17-variant Error enum (mirrors bitcoin Error)
│   └── config.rs              # WalletConfig (network, rpc_url, derivation_path)
├── tests/
│   ├── mnemonic.rs            # V2 mirror — deterministic ETH address
│   ├── erc20_calldata.rs      # V5 — transferCall → 0xa9059cbb
│   ├── erc20_anvil.rs         # V6 — Anvil MockERC20 transfer + balanceOf
│   └── spki_pin.rs            # V7 — production verifier with webpki
└── tokens/
    ├── mainnet.json           # USDC, USDT mainnet (Q6, Q9)
    └── sepolia.json           # USDC Sepolia + USDT placeholder

rust-wallet-app/crates/eth/    # CLI binary
├── Cargo.toml
└── src/main.rs                # clap subcommands: create, import, list, send, recv, balance
```

## Phase 0 — Scaffold + canonical mnemonic test (1 task)

### Task 1: Crate scaffold + V2 mirror test

**Files:**
- Create: `rust-wallet-app/crates/eth-wallet-core/Cargo.toml`
- Create: `rust-wallet-app/crates/eth-wallet-core/src/lib.rs`
- Create: `rust-wallet-app/crates/eth-wallet-core/src/mnemonic.rs`
- Create: `rust-wallet-app/crates/eth-wallet-core/tests/mnemonic.rs`

**Interfaces:**
- `eth_wallet_core::mnemonic::generate_12_word() -> Zeroizing<Mnemonic>` (Q7)
- `eth_wallet_core::mnemonic::derive_address(phrase: &Mnemonic, index: u32) -> Address` (Q3 default path)

**Steps:**
- [ ] Step 1: Add `eth-wallet-core` to umbrella `members`
- [ ] Step 2: Add `alloy = "=1.8.3"` to workspace deps with minimal features
- [ ] Step 3: Implement `mnemonic::generate_12_word()` + `derive_address()` using `MnemonicBuilder::english().phrase().index(0).build()` pattern (V2 verified path)
- [ ] Step 4: Write `tests/mnemonic.rs` mirroring V2 — all-`abandon` mnemonic → `0x9858EfFD232B4033E47d90003D41EC34EcaEda94`
- [ ] Step 5: Verify gate (cargo fmt + clippy --all-targets + test)
- [ ] Step 6: Commit `feat(eth): scaffold eth-wallet-core crate — MnemonicBuilder + derivation path (Task 1)`

## Phase 1 — Core wallet ops (3 tasks)

### Task 2: WalletManager + create/import/list/delete
- [ ] Step 1: Implement `WalletManager` holding `RwLock<HashMap<WalletId, Zeroizing<Mnemonic>>>`
- [ ] Step 2: Implement `create_wallet(words, password) -> WalletCreated`
- [ ] Step 3: Implement `import_wallet(phrase, password) -> WalletId` (Q7 — wraps in Zeroizing)
- [ ] Step 4: Implement `list_wallets() -> Vec<WalletInfo>` + `delete_wallet(id)` + `show_wallet(id)`
- [ ] Step 5: Tests for each op + persistence (SQLite or sled; mirror Bitcoin `WalletStore`)
- [ ] Step 6: Commit

### Task 3: Sign-only path (no broadcast)
- [ ] Step 1: Implement `sign_native_eth_tx(signer, tx) -> SignedTx`
- [ ] Step 2: Implement `sign_erc20_transfer(signer, token, to, amount) -> SignedTx` (ERC-20 calldata via `sol!`)
- [ ] Step 3: Tests using V2 signer (deterministic)
- [ ] Step 4: Commit

### Task 4: Error enum + serde
- [ ] Step 1: 17-variant Error enum mirroring Bitcoin Error schema (Q1 derives from deep-dive)
- [ ] Step 2: thiserror impl + `Result<T, Error>` alias
- [ ] Step 3: Commit

## Phase 2 — RPC integration (2 tasks)

### Task 5: Provider + raw reqwest SPKI pin (Q2 + Q4)
- [ ] Step 1: Implement `provider::new_http_pinned(rpc_url, pinned_spki_sha256) -> Result<Provider, Error>` — uses raw reqwest + custom `ServerCertVerifier` (V7 pattern) + `alloy::Provider::new_http_with_client(...)` or hand-rolled JSON-RPC for pinned endpoints
- [ ] Step 2: Tests: V7 production test — hit `https://ethereum.reth.rs/rpc` with pinned SPKI (capture from `openssl s_client`) → returns block_number; with wrong SPKI → connection rejected
- [ ] Step 3: Commit

### Task 6: Provider for non-pinned endpoints (Q4)
- [ ] Step 1: Implement `provider::new_http(rpc_url) -> Provider` using `alloy::ProviderBuilder::new().connect_http(...)` — for localhost Anvil + untrusted-dev endpoints
- [ ] Step 2: Commit

## Phase 3 — ERC-20 stablecoin (3 tasks)

### Task 7: ERC-20 calldata + selector (V5)
- [ ] Step 1: Implement `erc20::transfer_calldata(to: Address, value: U256) -> Bytes` using `alloy_sol_types::sol! transfer(address,uint256)`. First 4 bytes must be `0xa9059cbb`.
- [ ] Step 2: Test: V5 mirror — `transferCall { to, value }.abi_encode()` produces calldata with prefix `0xa9059cbb`
- [ ] Step 3: Commit

### Task 8: Token registry + decimals cache (Q5, Q6, Q9)
- [ ] Step 1: Define `Token` struct (address, symbol, decimals, chain_id) in `tokens.rs`
- [ ] Step 2: Bundle `tokens/mainnet.json` (USDC, USDT) + `tokens/sepolia.json` (USDC Sepolia)
- [ ] Step 3: Implement `tokens::load_chain(chain_id) -> Vec<Token>`
- [ ] Step 4: Implement `tokens::query_decimals(provider, token_addr) -> u8` — one `eth_call decimals()` per token at startup, cache in-memory
- [ ] Step 5: Commit

### Task 9: Anvil regtest for ERC-20 (V6, Q8)
- [ ] Step 1: Add `alloy-node-bindings` as `[dev-dependencies]`
- [ ] Step 2: Test: spin up Anvil, deploy `MockERC20` (5-line sol! + `ContractInstance::deploy`), call `transfer(...)`, assert recipient `balanceOf` reflects change
- [ ] Step 3: Commit

## Phase 4 — CLI + verification (3 tasks)

### Task 10: `eth` CLI scaffold
- [ ] Step 1: Create `rust-wallet-app/crates/eth/` binary crate
- [ ] Step 2: clap subcommands: `create`, `import`, `list`, `show`, `delete`, `send-native`, `send-erc20`, `balance`, `update-tokens` (Q9)
- [ ] Step 3: Tests + smoke run against Anvil regtest
- [ ] Step 4: Commit

### Task 11: Sepolia smoke script (operator-driven per L29)
- [ ] Step 1: `rust-wallet-app/scripts/eth-send-sepolia-e2e.sh` (mirror Bitcoin pattern)
- [ ] Step 2: `#[ignore]` integration test against Sepolia testnet RPC
- [ ] Step 3: Commit

### Task 12: Release cut
- [ ] Step 1: L24 — CHANGELOG `[v0.2.0]` entry + User Stories table checkbox flip
- [ ] Step 2: L21 — update estimate-report + ai-cost-report
- [ ] Step 3: Tag `v0.2.0` + push

## Out of scope (deferred to v0.3+)

- EIP-712 typed-data signing (already deferred per PR #290/#291/#292 reconcile)
- L2 chains (Optimism/Arbitrum/Base/Polygon) — v1.x
- ENS name resolution — v1.x
- Hardware wallets (Ledger/Trezor) — v1.x via `alloy-signer-ledger` / `alloy-signer-trezor`
- EIP-4337 account abstraction — v1.x
- FFI integration to `wallet-desktop` — v0.3 (parallel to FFI for Bitcoin wallet)

## References

- Issue #293 (resolution record): https://github.com/nhitranbtc/blockchain-sdk/issues/293
- Deep-dive doc: `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md`
- Spike (verification evidence): `rust-wallet-app/spikes/alloy-v1/`
- Bitcoin precedent (pattern source): `docs/superpowers/plans/2026-08-19-flutter-ffi-bitcoin-wallet-core.md`
- Bitcoin plan: `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`