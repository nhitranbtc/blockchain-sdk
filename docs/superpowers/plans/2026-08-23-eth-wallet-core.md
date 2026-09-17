# eth-wallet-core (v0.2) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Ethereum (ETH + ERC-20 stablecoin) wallet support across `rust-wallet-app/crates/eth-wallet-core/` (thin shim) + `rust-wallet-app/crates/evm-wallet-core/` (real impl, `Network::Ethereum(EthereumChain)` + `Network::Polygon(PolygonChain)` per Q1 Option A from PR #290/Issue #416) + an `eth` CLI in the umbrella. Phase 0 refactor shipped: `eth-wallet-core` is `pub use evm_wallet_core::*;` so the historical crate name + import path stays valid for `eth` CLI + downstream consumers. Resolves the 9 open questions from `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` and PR #290.

**Architecture:** Five phases (Phase 0 = refactor landed; Phase 1-4 = original roadmap).

- **Phase 0 (shipped)** = refactor: all ETH code moved into `evm-wallet-core/src/` with `Network::Ethereum(EthereumChain)` variant. `eth-wallet-core` reduced to 1-line shim re-exporting `evm_wallet_core::*`. `polygon-wallet-core` mirrors this shape.
- **Phase 1** = core wallet ops (create / import / list / delete / show / sign).
- **Phase 2** = RPC integration (provider + raw reqwest SPKI pin).
- **Phase 3** = ERC-20 stablecoin transfer (USDT + USDC) + token registry.
- **Phase 4** = `eth` CLI + Sepolia/mainnet smoke + release cut.

**Tech Stack:** Rust 1.94 stable, `alloy = "=1.8.3"` (Q1, 2.x line deferred per #474), `bip32` ^0.5 + `bip39` 2.2 (workspace deps, reused from Bitcoin), `reqwest` 0.12 + `rustls` 0.23 (raw SPKI pin per Q2, deferred per #330), `alloy-sol-types` (ERC-20 ABI), `alloy-node-bindings` (Anvil regtest per Q8, dev-dep), `clap` (CLI), `rpassword` (TTY password prompt per #351), `dotenvy` (`.env` loader for ETH_NETWORK/ETH_RPC_URL/ETH_PASSWORD per #341), `tracing` + `tracing-subscriber` (logging), `tempfile` dev-dep (hermetic fixtures).

## Global Constraints (verbatim from #293 resolutions)

- **Q1 — MSRV**: Pin `alloy = "=1.8.3"` (latest 1.x; MSRV 1.91 < workspace toolchain 1.94). 2.x line (2.4.1, MSRV 1.94.1) deferred to BTC-side ecosystem settle.
- **Q2 — TLS pinning transport**: `alloy-transport-http` does NOT expose a custom `ServerCertVerifier` hook. Workaround = raw `reqwest` + `rustls` with custom verifier for pinned endpoints (mirrors Bitcoin F20 / Task 7). alloy-provider stays for non-pinned endpoints (localhost Anvil during dev).
- **Q3 — derivation path default**: `m/44'/60'/0'/0/0` (Ledger/SLIP-44). Configurable via `WalletConfig` (matches Bitcoin F11).
- **Q4 — fillers vs explicit**: Use `Provider::new_http(url)` (no auto-fillers). Explicit nonce + gas estimation in `WalletManager`, parallel to Bitcoin.
- **Q5 — decimals**: Cache (one `eth_call decimals()` per token at startup, persist in token registry). NOT hard-coded.
- **Q6 — registry location**: (c) bundled repo `rust-wallet-app/crates/evm-wallet-core/tokens/{anvil,mainnet_eth,sepolia}.json` (shipped); (b) `~/.config/<app>/tokens.json` for v1.x.
- **Q7 — zeroize**: Mirror Bitcoin Task 30. `Mnemonic` → `Zeroizing<Mnemonic>`, `XPrv` → wrap, `PrivateKeySigner`'s internal key — extract into zeroize-owned buffer; defer full audit to eth/ crate implementation.
- **Q8 — Anvil**: Add `alloy-node-bindings` as `[dev-dependencies]` for regtest-style smoke (mirrors Bitcoin Docker regtest).
- **Q9 — stablecoin source of truth**: Versioned registry + `--update-tokens` CLI command that fetches Circle's published list at runtime. Tether publishes no equivalent page; risk = stale USDT data. Circle source: `developers.circle.com/stablecoins/usdc-contract-addresses`.

## F47 zeroize gap (F47 from Bitcoin plan applies)

Mnemonic + password never logged. With FFI integration deferred to v0.3 (per #292), F47 stays in-process: mnemonic wraps in `Secret<String>` (Rust) or `Zeroizing<Mnemonic>` (Dart heap cleared on drop). Do NOT log in error paths.

## File Structure (current — Phase 0 refactor landed)

```text
rust-wallet-app/crates/eth-wallet-core/        # thin shim (Phase 0)
├── Cargo.toml                                 # deps: evm-wallet-core; dev: tempfile
└── src/
    └── lib.rs                                 # `pub use evm_wallet_core::*;` (1-line re-export)

rust-wallet-app/crates/evm-wallet-core/        # real impl (Q1 Option A)
├── Cargo.toml
├── src/
│   ├── lib.rs                                 # pub mod + Error re-export + Network enum
│   ├── wallet.rs                              # WalletManager + create / import / list / delete / show (Phase 1)
│   ├── mnemonic.rs                            # bip39 generate + Zeroizing wrap (Q7)
│   ├── crypto.rs                              # zeroize + key handling (Q7)
│   ├── signer.rs                              # PrivateKeySigner facade
│   ├── derivation.rs                          # (via wallet.rs) bip32 derive m/44'/60'/0'/0/{idx} (Q3)
│   ├── network.rs                             # Network::{Ethereum(EthereumChain), Polygon(PolygonChain)} + parse_cli_eth
│   ├── provider.rs                            # alloy::Provider + new_http() (Q4; SPKI pin deferred #330)
│   ├── erc20.rs                               # sol! transfer + decimals query (Q5)
│   ├── tokens.rs                              # bundled token registry loader (Q6, Q9)
│   ├── error.rs                               # Error enum + exit_code() (stable 0..=5 per #297 M11)
│   └── redact.rs                              # secret-safe formatting helpers
├── tests/                                     # 14 integration tests
│   ├── common/                                # shared Anvil fixture helpers
│   ├── mnemonic.rs                            # V2 mirror — deterministic ETH address
│   ├── network_chain_id.rs                    # chain-id resolution per Network variant
│   ├── wallet_manager.rs                      # create / import / list / delete / show
│   ├── wallet_balance.rs                      # native ETH balance RPC read
│   ├── wallet_send_native.rs                  # sign + broadcast native ETH (PR-B)
│   ├── wallet_send_speedup.rs                 # EIP-1559 replace-by-fee (Issue #381)
│   ├── wallet_sync.rs                         # wallet-sync chain-state rebuild
│   ├── erc20_balance.rs                       # ERC-20 balanceOf via eth_call
│   ├── erc20_send.rs                          # ERC-20 transfer via `sol!` (V5/V6 mirror)
│   ├── erc20_anvil.rs                         # Anvil MockERC20 deploy + transfer + balanceOf (V6)
│   ├── erc20_approve.rs                       # ERC-20 approve + allowance (Issue #343)
│   ├── fee.rs                                 # gas price / base fee RPC read
│   ├── tx_list_get.rs                         # tx-history scan + tx-by-hash fetch
│   ├── polygon_rpc.rs                         # Polygon-specific RPC smoke
│   └── spki_pin_localnet.rs                   # SPKI pin verifier integration smoke
└── tokens/
    ├── anvil.json                             # MockERC20 for Anvil regtest
    ├── mainnet_eth.json                       # USDC, USDT mainnet (Q6, Q9)
    └── sepolia.json                           # USDC Sepolia + USDT placeholder

rust-wallet-app/crates/eth/                    # CLI binary
├── Cargo.toml                                 # clap + eth-wallet-core + alloy-* + tracing + rpassword + dotenvy + serde_json
├── src/
│   ├── main.rs                                # clap subcommands + run() dispatch + tokio block_on + password resolution (Issue #351)
│   └── handlers.rs                            # per-subcommand fns + open_manager/open_provider/map_wallet_err
└── tests/
    ├── cli_localnet.rs                        # Anvil-backed end-to-end CLI smoke
    ├── cli_sepolia.rs                         # `#[ignore]` Sepolia testnet smoke (L29)
    ├── password.rs                             # TTY prompt + ETH_PASSWORD + argv precedence (Issue #351)
    └── wallet_send_speedup.rs                 # EIP-1559 replace-by-fee CLI smoke
```

## Phase 0 — Refactor into evm-wallet-core shim (1 task — shipped)

### Task 1 (#295 + #416): Refactor ETH impl into `evm-wallet-core`; `eth-wallet-core` becomes shim

**Files (current state):**

- `rust-wallet-app/crates/eth-wallet-core/Cargo.toml` — deps: `evm-wallet-core = { path = "../evm-wallet-core" }`; dev: `tempfile = "3"`
- `rust-wallet-app/crates/eth-wallet-core/src/lib.rs` — 1-line shim: `pub use evm_wallet_core::*;`
- `rust-wallet-app/crates/evm-wallet-core/src/{lib,wallet,mnemonic,crypto,signer,network,provider,erc20,tokens,error,redact}.rs` — all ETH impl
- `rust-wallet-app/crates/evm-wallet-core/tokens/{anvil,mainnet_eth,sepolia}.json` — bundled registry (Q6, Q9)
- `rust-wallet-app/crates/evm-wallet-core/tests/mnemonic.rs` — V2 mirror (all-`abandon` mnemonic → `0x9858EfFD232B4033E47d90003D41EC34EcaEda94`)

**Public surface (`eth_wallet_core::*` re-exports `evm_wallet_core::*`):**

- `eth_wallet_core::mnemonic::generate_12_word() -> Zeroizing<Mnemonic>` (Q7)
- `eth_wallet_core::mnemonic::derive_address(phrase: &Mnemonic, index: u32) -> Address` (Q3 default path `m/44'/60'/0'/0/0`)
- `eth_wallet_core::wallet::WalletManager` (create/import/list/delete/show + `open()` / `open_at()` per #297)
- `eth_wallet_core::network::Network::{Ethereum(EthereumChain), Polygon(PolygonChain)}` + `Network::parse_cli_eth(&str)`
- `eth_wallet_core::provider::new_http(url) -> RootProvider<Ethereum>` (SPKI pin deferred #330)
- `eth_wallet_core::erc20::{transfer_calldata, decimals, MockERC20}` (V5 + V6)
- `eth_wallet_core::tokens::{Token, load_chain, query_decimals}` (Q5, Q6, Q9)
- `eth_wallet_core::error::{Error, Result}` + `Error::exit_code() -> i32` (stable 0..=5 per #297 M11)

**Steps (historical — all done):**

- [x] Step 1: Add `eth-wallet-core` + `evm-wallet-core` to umbrella `members`
- [x] Step 2: Add `alloy = "=1.8.3"` to workspace deps with minimal features
- [x] Step 3: Implement `evm_wallet_core::mnemonic::{generate_12_word, derive_address}` using `MnemonicBuilder::english().phrase(m).index(0).expect("valid account index").build().expect("build signer")` (full V2 verified chain with both `Result`-returning calls handled)
- [x] Step 4: Write `evm-wallet-core/tests/mnemonic.rs` mirroring V2 — all-`abandon` mnemonic → `0x9858EfFD232B4033E47d90003D41EC34EcaEda94`
- [x] Step 5: Reduce `eth-wallet-core/src/lib.rs` to `pub use evm_wallet_core::*;`
- [x] Step 6: Verify gate (cargo fmt + clippy --all-targets + test)
- [x] Step 7: Commit `refactor(eth): move ETH impl into evm-wallet-core — eth-wallet-core is now thin shim (Task 1 / #416)`

## Phase 1 — Core wallet ops (3 tasks)

### Task 2 (#301): WalletManager + create/import/list/delete — shipped (evm-wallet-core/src/wallet.rs)

- [x] Step 1: `WalletManager` holds `RwLock<HashMap<WalletId, Zeroizing<Mnemonic>>>` (Q7 Zeroizing wrap)
- [x] Step 2: `create_wallet(words, password) -> WalletCreated`
- [x] Step 3: `import_wallet(phrase, password) -> WalletId` (Q7 — wraps in Zeroizing; also supports `--private-key` per CLI `WalletAction::Import`)
- [x] Step 4: `list_wallets() -> Vec<WalletInfo>` + `delete_wallet(id)` + `show_wallet(id)`
- [x] Step 5: Tests in `evm-wallet-core/tests/wallet_manager.rs`; persistence uses JSON-on-disk under XDG data dir (`WalletManager::open()` / `open_at(p)`)
- [x] Step 6: Commit

### Task 3 (#302): Sign-only path (no broadcast) — shipped

- [x] Step 1: `sign_native_eth_tx(signer, tx) -> SignedTx` (used by `wallet send-native` PR-B; current impl via `alloy_signer_local::PrivateKeySigner::from_slice(secret)` per handlers.rs)
- [x] Step 2: `sign_erc20_transfer(signer, token, to, amount) -> SignedTx` (ERC-20 calldata via `sol!`; used by `wallet send-erc20` PR-B)
- [x] Step 3: Tests using V2 signer (deterministic) — `evm-wallet-core/tests/wallet_send_native.rs`
- [x] Step 4: Commit

### Task 4 (#303): Error enum + serde — shipped (in evm-wallet-core/src/error.rs)

- [x] Step 1: Error enum (more than 17 variants; covers `Rpc`, `InvalidInput`, `InvalidMnemonic`, `InvalidPrivateKey`, `WalletNotFound`, `WalletNotFoundByName`, `WalletExists`, `WalletCorrupt`, `DecryptionFailed`, `FeeTooLow`, etc.)
- [x] Step 2: thiserror impl + `Result<T, Error>` alias + `Error::exit_code() -> i32` (stable 0..=5 per #297 M11)
- [x] Step 3: Commit

## Phase 2 — RPC integration (2 tasks — partially shipped)

### Task 5 (#304): Provider + raw reqwest SPKI pin (Q2 + Q4) — DEFERRED per #330

- [ ] Step 1: `provider::new_http_pinned(rpc_url, pinned_spki_sha256) -> Result<Provider, Error>` — raw reqwest + custom `ServerCertVerifier` (V7 pattern) + `alloy::Provider::new_http_with_client(...)` or hand-rolled JSON-RPC for pinned endpoints. Deferred per #330; current `new_http()` uses default rustls TLS + system CAs.
- [ ] Step 2: V7 production test — hit `https://ethereum.reth.rs/rpc` with pinned SPKI; with wrong SPKI → connection rejected. Local smoke lives in `evm-wallet-core/tests/spki_pin_localnet.rs`.
- [ ] Step 3: Commit (pending #330 resolution)

### Task 6 (#305): Provider for non-pinned endpoints (Q4) — shipped

- [x] Step 1: `evm_wallet_core::provider::new_http(url) -> RootProvider<Ethereum>` using `alloy::Provider::new_http(...)` — for localhost Anvil + untrusted-dev endpoints. Re-exported as `eth_wallet_core::new_http`.
- [x] Step 2: Commit

## Phase 3 — ERC-20 stablecoin (3 tasks — shipped in evm-wallet-core)

### Task 7 (#306): ERC-20 calldata + selector (V5) — shipped

- [x] Step 1: `evm_wallet_core::erc20::transfer_calldata(to, value) -> Bytes` using `alloy_sol_types::sol! transfer(address,uint256)`. First 4 bytes = `0xa9059cbb`.
- [x] Step 2: V5 mirror test in `evm-wallet-core/tests/erc20_send.rs`
- [x] Step 3: Commit

### Task 8 (#307): Token registry + decimals cache (Q5, Q6, Q9) — shipped

- [x] Step 1: `Token` struct (address, symbol, decimals, chain_id) in `evm-wallet-core/src/tokens.rs`
- [x] Step 2: Bundled `evm-wallet-core/tokens/{anvil,mainnet_eth,sepolia}.json`
- [x] Step 3: `tokens::load_chain(chain_id) -> Vec<Token>`
- [x] Step 4: `tokens::query_decimals(provider, token_addr) -> u8` (one `eth_call decimals()` per token, cache in-memory)
- [x] Step 5: Commit

### Task 9 (#308): Anvil regtest for ERC-20 (V6, Q8) — shipped

- [x] Step 1: `alloy-node-bindings` in `[dev-dependencies]` of `evm-wallet-core/Cargo.toml`
- [x] Step 2: Anvil + MockERC20 deploy/transfer/balanceOf test in `evm-wallet-core/tests/erc20_anvil.rs` + `erc20_balance.rs` + `erc20_approve.rs` (Issue #343)
- [x] Step 3: Commit

## Phase 4 — CLI + verification (3 tasks — shipped, partial scope)

### Task 10 (#309): `eth` CLI scaffold — shipped (PR-A scope)

- [x] Step 1: `rust-wallet-app/crates/eth/` binary crate (`src/{main.rs, handlers.rs}`)
- [x] Step 2: clap subcommands split PR-A (shipped) + PR-B (deferred per #337):
  - PR-A: `wallet create|import|list|show|delete|balance`, `tx get|list`, `config show`, `version`
  - PR-B (deferred): `wallet send-native|send-erc20`, `wallet sync`, `fee`, `sign-message|sign-typed`, `erc20 {balance|send|list|register|approve|deploy}` (most backends), `wallet speedup` (shipped separately per Issue #381)
  - Q9 `--update-tokens` deferred with PR-B
- [x] Step 3: Tests in `eth/tests/{cli_localnet, cli_sepolia, password, wallet_send_speedup}.rs` (Anvil-backed + `#[ignore]` Sepolia)
- [x] Step 4: Commit

### Task 11 (#310): Sepolia smoke script (operator-driven per L29) — shipped

- [x] Step 1: `eth/tests/cli_sepolia.rs` (Sepolia testnet integration smoke)
- [x] Step 2: `#[ignore]`-gated integration test, run via L29 operator protocol (requires `ETH_RPC_URL` + funded Sepolia wallet)
- [x] Step 3: Commit
- Follow-up: #298 (per-user-story e2e — post-v0.2 expansion of this smoke)

### Task 12 (#311): Release cut — shipped

- [x] Step 1: L24 — CHANGELOG `[v0.2.0]` entry + User Stories table checkbox flip
- [x] Step 2: L21 — update estimate-report + ai-cost-report
- [x] Step 3: Tag `v0.2.0` + push (`eth-wallet-core` `Cargo.toml:3` version = `0.2.0`)

## Out of scope (deferred to v0.3+)

- Per-user-story testnet e2e ([#298](https://github.com/nhitranbtc/blockchain-sdk/issues/298), 10 RPC-touching stories → `tests/e2e_sepolia/` + `tests/e2e_mainnet/`, L29 operator-driven). Post-v0.2.
- EIP-712 typed-data signing (deferred per PR #290/#291/#292 reconcile; `sign-typed` CLI stub returns `Error::Rpc("...deferred")`)
- L2 chains (Optimism/Arbitrum/Base) — v1.x (Polygon mirrors ETH shape in `polygon-wallet-core` per `2026-08-27-polygon-wallet-core.md`)
- ENS name resolution — v1.x
- Hardware wallets (Ledger/Trezor) — v1.x via `alloy-signer-ledger` / `alloy-signer-trezor`
- EIP-4337 account abstraction — v1.x
- FFI integration to `wallet-desktop` — v0.3 (parallel to FFI for Bitcoin wallet)
- SPKI-pinned RPC transport (raw `reqwest` + `rustls` custom verifier per Q2) — deferred per #330
- `--update-tokens` runtime fetcher per Q9 — deferred with PR-B

## References

- Issue #293 (resolution record): <https://github.com/nhitranbtc/blockchain-sdk/issues/293>
- Issue #416 (Phase 0 refactor → evm-wallet-core, Q1 Option A)
- Issue #297 (Error enum + exit codes)
- Issue #330 (SPKI pin deferral)
- Issue #337 (CLI PR-A / PR-B split)
- Issue #341 (.env loading via dotenvy)
- Issue #343 (ERC-20 e2e + MockERC20)
- Issue #351 (TTY password prompt primary path)
- Issue #358 (`wallet balance --all` + `--token` override)
- Issue #379 (clap `value_parser` for `--token` Address)
- Issue #381 (`wallet speedup` EIP-1559 replace-by-fee)
- Issue #474 (Q4 RPC defaults drift)
- Deep-dive doc: `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md`
- Sibling plan (Polygon mirrors this shape): `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md`
- Spike (verification evidence): `rust-wallet-app/spikes/alloy-v1/`
- Bitcoin precedent (pattern source): `docs/superpowers/plans/2026-08-19-flutter-ffi-bitcoin-wallet-core.md`
- Bitcoin plan: `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`
