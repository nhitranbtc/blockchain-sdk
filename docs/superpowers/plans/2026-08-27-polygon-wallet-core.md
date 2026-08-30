# evm-wallet-core + polygon-wallet-core (v0.1) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver `rust-wallet-app/crates/evm-wallet-core/` (refactor of `eth-wallet-core` with `Network` enum supporting both Ethereum + Polygon) + thin `rust-wallet-app/crates/polygon-wallet-core/` wrapper + `polygon` CLI binary. Mirrors `bitcoin-wallet-core/` (v0.1) + `eth-wallet-core/` (v0.2) + in-flight `tron-wallet-core/` (v0.1) structure. Resolves the 8 open questions from `docs/wallets/2026-08-27-polygon-rust-sdks-deep-dive.md` and issue #416.

**Drift note (2025-Q4, Issue #474):** Q4 RPC defaults drifted. Originally `polygon-rpc.com` (mainnet) + `polygon-amoy.drpc.org` (Amoy). Per #458 / #474 evidence, `polygon-rpc.com` tightened keyless-tier access (HTTP 401 on `estimate_eip1559_fees` + `get_block_number`). `polygon-amoy.drpc.org` showed similar rate-limit signal in PR #473 smoke. Defaults switched to `https://polygon-bor-rpc.publicnode.com` (mainnet) + `https://polygon-amoy-bor-rpc.publicnode.com` (Amoy). ETH mainnet default also drifted from `cloudflare-eth.com` to `https://ethereum-rpc.publicnode.com` for consistency. The `POLYGON_RPC_URL` / `ETH_RPC_URL` env overrides (L29 / L61) remain as defense-in-depth for operators needing paid-tier (Alchemy/Infura) or alternate vendors. This drift note supersedes Q4 + the URL references throughout §1 / §4 / §T2 / §T8 / §Q4 references below; the latter kept for historical fidelity.

**Architecture:** Five phases (Phase 0.0 network-selection pre-step + Phases 0–4).
- **Phase 0** = refactor `eth-wallet-core` → `evm-wallet-core` (extract `Network` enum + chain config) + canonical mnemonic test (mirrors ETH derivation on both chains).
- **Phase 1** = add `polygon-wallet-core` thin wrapper (Network::Polygon config + POL display) — ~200 lines, no signing/RPC code.
- **Phase 2** = RPC integration (add Polygon mainnet 137 + Amoy 80002 RPC defaults to existing `evm-wallet-core::provider`).
- **Phase 3** = Polygon-specific config (POL token display, native USDC vs USDC.e footgun flag, gas-estimation cadence assertion).
- **Phase 4** = `polygon` CLI binary + Amoy testnet smoke + mainnet smoke + release cut.

**Tech Stack:** Rust 1.94 stable, `alloy = "=1.8.3"` (Q1, reused from eth-wallet-core v0.2), `alloy-chains` (NEW direct dep for `Chain::Polygon` enum), `bip32` ^0.5 + `bip39` 2.2 (workspace deps, reused), `reqwest` 0.12 + `rustls` 0.23 (raw SPKI pin per Q7), `alloy-sol-types` (ERC-20 ABI, reused from eth), `alloy-node-bindings` (Anvil regtest), `clap` (CLI). **Zero new direct deps beyond `alloy-chains`** — all EVM primitives (signing, RPC, ABI, gas estimation) inherited from `evm-wallet-core`.

## Global Constraints (verbatim from #416 resolutions)

- **Q1 — EVM-reuse strategy: Option A.** Refactor `eth-wallet-core` → `evm-wallet-core` + thin `eth` + `polygon` wrappers. Single source of truth for EVM primitives. Future EVM chains (Base, Arbitrum, Optimism) = new wrapper, not new core.
- **Q2 — Scope: PoS only.** Polygon zkEVM deferred to v0.2.
- **Q3 — Token registry: static bundled JSON.** `polygon-wallet-core/tokens/mainnet.json` + `amoy.json` (mirrors eth shape).
- **Q4 — RPC provider default:** `polygon-rpc.com` primary (mainnet), `polygon-amoy.drpc.org` fallback (Amoy). No Alchemy key bundled.
- **Q5 — Gas pricing: EIP-1559 only.** Type 2 transactions. London hardfork active since 2022-01-18 (block 23,850,000 on Polygon). Re-estimate `max_fee_per_gas` immediately before broadcast (2-second blocks).
- **Q6 — Hardware wallet: deferred to v0.2.** `alloy-signer-ledger` / `alloy-signer-trezor` available when needed.
- **Q7 — Signature replay protection:** `chain_id` always in EIP-712 domain separator + `with_chain_id(137|80002)` in `TransactionRequest`. Cross-chain replay rejected.
- **Q8 — POL display:** "POL" default, "MATIC" alias for legacy wallet UX (post-MATIC rebrand 2024-09-04).

## File Structure (decomposition)

```text
rust-wallet-app/crates/evm-wallet-core/        # REFACTORED from eth-wallet-core (Q1 Option A)
├── Cargo.toml
├── src/
│   ├── lib.rs                  # pub mod + Error enum + WalletManager
│   ├── wallet.rs               # create / import / list / delete / show
│   ├── mnemonic.rs             # bip39 generate + zeroize wrap
│   ├── derivation.rs           # bip32 derive m/44'/60'/0'/0/{idx} (reuses ETH SLIP-44 coin type 60)
│   ├── provider.rs             # alloy::Provider + SPKI pin verifier (no change from ETH)
│   ├── erc20.rs                # sol! transfer + decimals query
│   ├── tokens.rs               # bundled token registry loader
│   ├── error.rs                # ~20-variant Error enum
│   ├── network.rs              # NEW: Network { Ethereum, Polygon } enum + chain_id + RPC URL + gas token display
│   └── config.rs               # WalletConfig { network: Network, rpc_url, derivation_path, gas_token_label }
├── tokens/
│   ├── mainnet_eth.json        # moved from eth-wallet-core/tokens/mainnet.json
│   └── sepolia.json            # moved from eth-wallet-core/tokens/sepolia.json
└── tests/
    ├── mnemonic.rs
    ├── erc20_calldata.rs
    ├── erc20_anvil.rs
    ├── spki_pin.rs
    └── network_chain_id.rs     # NEW: verifies get_chain_id() returns 1 (ETH) or 137/80002 (Polygon)

rust-wallet-app/crates/eth-wallet-core/        # THIN WRAPPER (Q1)
├── Cargo.toml                  # evm-wallet-core (path dep) only
├── src/lib.rs                  # re-exports + Network::Ethereum config
└── (no tests — inherits from evm-wallet-core)

rust-wallet-app/crates/polygon-wallet-core/    # NEW THIN WRAPPER (Q1)
├── Cargo.toml                  # evm-wallet-core (path dep) + alloy-chains (NEW direct dep)
├── src/lib.rs                  # re-exports + Network::Polygon config
├── src/network.rs              # Network::Polygon { mainnet_rpc: "polygon-rpc.com", amoy_rpc: "polygon-amoy.drpc.org", gas_token_label: "POL", legacy_label: "MATIC", chain_id_mainnet: 137, chain_id_amoy: 80002 }
└── tokens/
    ├── mainnet.json            # USDC (0x3c499c...3359), USDT (0xc2132D...e8F), DAI (0x8f3Cf7...63)
    └── amoy.json               # USDC (0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582)

rust-wallet-app/crates/polygon/                # CLI binary
├── Cargo.toml
└── src/main.rs                  # clap subcommands: create, import, list, show, send, erc20 send, balance, fee, faucet, config

rust-wallet-app/spikes/polygon-v1/             # verification harness (V1–V10, one per Q)
├── Cargo.toml                   # workspace member; deps = alloy + reqwest + tokio + bitcoin-wallet-core (SPKI pin reuse)
├── README.md                    # V1-V10 acceptance + run instructions (mirrors TRON spike README shape)
├── ROADMAP.md                   # spike purpose + Phase 0.0 network selection + use_case reference
├── RESULT.md                    # PASS evidence log (filled post-smoke run)
├── src/
│   ├── lib.rs                    # re-exports + spike config
│   ├── config.rs                 # POLYGON_MAINNET_RPC_URL, POLYGON_AMOY_RPC_URL, chain-ids, gas-token labels
│   ├── address.rs                # EIP-55 checksum helper (wraps alloy_primitives::Address::to_checksum_buffer)
│   ├── provider.rs               # alloy Provider helpers for polygon-rpc.com + polygon-amoy.drpc.org
│   ├── tokens.rs                 # bundled token registry loader (mirror eth-wallet-core::tokens)
│   ├── spki.rs                   # SPKI pin wrapper (reuses bitcoin-wallet-core::chain::spki — Q7)
│   └── erc20.rs                  # ERC-20 ABI helpers (wraps alloy_sol_types::sol! for transfer/balanceOf/decimals)
├── tests/
│   ├── env.example               # RUN_POLYGON_AMOY=1, RUN_POLYGON_MAINNET=1, RUN_POLYGON_ANVIL=1
│   ├── use_case_alpha_sends_beta_100_usdc.rs  # end-to-end smoke (V8 + V9 combined; 100 USDC native on Polygon)
│   ├── v1_evm_reuse.rs           # cargo build -p evm-wallet-core -p eth-wallet-core -p polygon-wallet-core clean
│   ├── v2_chain_id.rs            # get_chain_id() returns 137 (mainnet) + 80002 (amoy)
│   ├── v3_derivation.rs          # m/44'/60'/0'/0/0 → same address on ETH + Polygon
│   ├── v4_eip1559_estimates.rs   # estimate_eip1559_fees() — re-estimate cadence proof
│   ├── v5_rpc_connectivity.rs    # provider.get_block_number() against polygon-rpc.com
│   ├── v6_token_registry.rs      # mainnet.json + amoy.json load + decimals() verify
│   ├── v7_amoy_faucet.rs         # request Amoy POL, verify balance update
│   ├── v8_native_pol_transfer.rs # send 0.01 POL on Amoy, verify balance change
│   ├── v9_erc20_transfer.rs      # deploy MockERC20 to Anvil (Polygon-fork), transfer, verify
│   └── v10_eip712_replay.rs      # sign EIP-712 with chain_id 137, verify replay on 1 fails
└── tokens/
    ├── mainnet.json              # USDC (0x3c499c...3359), USDT (0xc2132D...e8F), DAI (0x8f3Cf7...63) — 3 entries
    └── amoy.json                 # USDC Amoy (0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582) — 1 entry

**Spike build dependencies (mirrors TRON plan §"Spike build dependency", with EVM deltas):**
- `protoc` **NOT needed** — EVM has no protobuf transactions (explicit delta vs TRON spike; drop `build.rs` + `proto/` dirs entirely)
- `alloy-node-bindings` (dev-dep) for `AnvilInstance::new().spawn()` Polygon-fork mode (`--fork-url https://polygon-rpc.com --fork-block-number 60000000`)
- `tokio` (workspace) for `#[tokio::test]` async tests
- `bitcoin-wallet-core` (path dep) for SPKI pin verifier reuse (Q7)
- `alloy` features added on top of workspace: `transport-http`, `provider`, `network`, `sol-types` (cargo feature unification is additive across members)

**Spike live-testnet gating (per L29, mirrors TRON plan §"Spike live-testnet gating"):**
- V1/V3/V9/V10 always run (offline — Anvil Polygon-fork + deterministic derivation + EIP-712 fixture)
- V2/V4/V6/V7/V8 gated behind `RUN_POLYGON_AMOY=1` (live Amoy RPC `https://polygon-amoy.drpc.org`)
- V5 gated behind `RUN_POLYGON_MAINNET=1` (live mainnet RPC `https://polygon-rpc.com`, operator-driven only)
- Without env vars, gated tests print `[SKIP — RUN_POLYGON_AMOY=1 required]` and exit 0

**Async test policy:** every test touching async code (RPC via `alloy_provider::Provider`, HTTP via `reqwest`, tokio primitives) MUST be `async fn` + `#[tokio::test]` per eth #333 + plan §"Async test policy". Sync `#[test]` is forbidden for any code path that touches `alloy_provider::Provider`, `reqwest` transport, or any tokio primitive. Mirrors eth-wallet-core v0.3 test policy.
```

**Spike live-testnet gating (per L29):** V2/V4/V5/V6/V7/V8 require live RPC access. These gated behind `RUN_POLYGON_AMOY=1` (Amoy) and `RUN_POLYGON_MAINNET=1` (mainnet — operator-driven only). V1/V3/V9/V10 are offline (Anvil) and always run.

## Phase 0.0 — Network selection + local-dev testnet (NEW, mirrors TRON §"Phase 0.0")

Per L13 spec + L29, lock in network decisions before any production code lands.

### 0.0.a — Polygon networks targeted

| Network | Chain ID | Native gas | Use | Faucet | Endpoint |
|---|---|---|---|---|---|
| Mainnet | **137** (`0x89`) | **POL** | Production POL + ERC-20 | none | `https://polygon-rpc.com` |
| Amoy (testnet) | **80002** (`0x13882`) | **POL** | **Primary testnet** for v0.1 | `https://faucet.polygon.technology/` (5,000 POL / 24h / address) | `https://polygon-amoy.drpc.org` (or `https://rpc-amoy.polygon.technology/`) |
| Mumbai (deprecated) | 80001 | POL | Replaced by Amoy 2024-01 | n/a | offline |

**Drift correction (2026-08-27):** prior docs conflated Mumbai + Amoy chain-ids; corrected to **Amoy = 80002**. Verified via live `POST /jsonrpc {"method":"eth_chainId"}` against `polygon-amoy.drpc.org` returns `"0x13882"` (= 80002 decimal). Mainnet chain-id `0x89` (= 137 decimal) verified live 2026-08-27.

**Mumbai rejected** for v0.1 (Goerli-rooted, deprecated 2024-Q2 after Goerli's deprecation; both operated concurrently during transition). Amoy is the only current Polygon PoS testnet.

### 0.0.b — Local testnet (in-process chain)

| Option | Pros | Cons | Decision |
|---|---|---|---|
| **Anvil (Foundry)** in **Polygon-fork mode** | Already in `[dev-dependencies]` of `eth-wallet-core`; pure Rust; `AnvilInstance::new().spawn()` returns running node + 10 prefunded accounts | Requires Foundry install for the binary; `anvil --fork-url https://polygon-rpc.com --fork-block-number 60000000` preserves Polygon state at the forked block | **Pick for v0.1** — preserves Polygon mainnet state for testing against real USDC/USDT/DAI contracts. |
| Polygon local node (`polygon-cli` / `bor` standalone) | Real consensus client | ~1.5 GB Docker image, slow startup, Java dep for full node | Defer to v0.3+ if Anvil Polygon-fork inadequate |
| testcontainers + Docker Polygon image | Reproducible | Same heavy setup | Reject — Anvil Polygon-fork simpler |

**Decision: Anvil Polygon-fork mode** for v0.1 unit tests + integration tests. Mirror the `alloy-node-bindings::AnvilInstance` pattern from `eth-wallet-core` (Story 26 / `tests/erc20_anvil.rs`).

### 0.0.c — Use case validation (cross-reference ROADMAP)

End-to-end use case: "alpha → beta 100 USDC on Polygon mainnet" (mirrors TRON's `use_case_alpha_sends_beta_100_usdt`). Status: pending — depends on Circle-issued native USDC contract `0x3c499c...3359` (no separate USDC.e address). Tracked as the spike V9 acceptance test.

## Phase 0 — Refactor `eth-wallet-core` → `evm-wallet-core` (1 task)

### Task 1 (#416 T1): Refactor + canonical mnemonic test

**Files:**
- Modify: `rust-wallet-app/Cargo.toml` (add `evm-wallet-core` to workspace `members`)
- Create: `rust-wallet-app/crates/evm-wallet-core/Cargo.toml` (path dep on `alloy`)
- Create: `rust-wallet-app/crates/evm-wallet-core/src/{lib,mnemonic,derivation,network,error,config,provider,erc20,tokens}.rs` (move from eth-wallet-core with Network enum addition)
- Modify: `rust-wallet-app/crates/eth-wallet-core/Cargo.toml` (replace direct alloy deps with `evm-wallet-core` path dep)
- Modify: `rust-wallet-app/crates/eth-wallet-core/src/lib.rs` (re-export from `evm-wallet-core` + `Network::Ethereum` config)
- Create: `rust-wallet-app/crates/evm-wallet-core/tests/network_chain_id.rs` (V1 + V2 mirror)

**Interfaces:**
- `evm_wallet_core::network::Network` enum: `Ethereum { chain_id: 1, rpc_url: "https://cloudflare-eth.com", gas_token: "ETH" }`, `Polygon { chain_id_mainnet: 137, chain_id_amoy: 80002, rpc_url_mainnet: "https://polygon-rpc.com", rpc_url_amoy: "https://polygon-amoy.drpc.org", gas_token: "POL", legacy_gas_token: "MATIC" }`
- `evm_wallet_core::network::Network::chain_id() -> u64` (returns 1 for Ethereum, 137 for Polygon mainnet, 80002 for Amoy)
- `evm_wallet_core::network::Network::rpc_url() -> &str`
- `evm_wallet_core::network::Network::gas_token_label() -> &str` (returns "ETH" or "POL")
- All existing `eth_wallet_core::*` functions (move verbatim — signatures unchanged)

**Steps:**
- [ ] Step 1: Create `evm-wallet-core` crate skeleton (copy `eth-wallet-core/` structure, add `network.rs` with Network enum)
- [ ] Step 2: Move `eth-wallet-core/src/{lib,mnemonic,derivation,provider,erc20,tokens,error,config}.rs` → `evm-wallet-core/src/` (verbatim, no signature changes)
- [ ] Step 3: Add `Network::Ethereum` variant referencing existing eth-wallet-core token registry (rename `tokens/mainnet.json` → `tokens/mainnet_eth.json`)
- [ ] Step 4: Add `Network::Polygon` variant — empty config, populated in Phase 1
- [ ] Step 5: Replace `eth-wallet-core/src/lib.rs` with thin re-export: `pub use evm_wallet_core::*;` + `Network::Ethereum::default()` accessor
- [ ] Step 6: Verify `cargo build -p evm-wallet-core -p eth-wallet-core` clean (per L55 scope rule — per-crate test, not workspace)
- [ ] Step 7: Write `tests/network_chain_id.rs` — `Network::Ethereum.chain_id() == 1`, `Network::Polygon.chain_id() == 137` (mainnet default)
- [ ] Step 8: Run `cargo test -p evm-wallet-core --test mnemonic` — verifies all-`abandon` mnemonic → `0x9858EfFD232B4033E47d90003D41EC34EcaEda94` (ETH derivation preserved through refactor)
- [ ] Step 9: Verify gate (cargo fmt + clippy --all-targets -- -D warnings + test per L55)
- [ ] Step 10: Commit `refactor(evm): extract evm-wallet-core from eth-wallet-core for #416 (Task 1)`

## Phase 1 — `polygon-wallet-core` thin wrapper (1 task)

### Task 2 (#416 T2): Polygon wrapper + derivation cross-check

**Files:**
- Create: `rust-wallet-app/crates/polygon-wallet-core/Cargo.toml` (path deps: `evm-wallet-core`, `alloy-chains`)
- Create: `rust-wallet-app/crates/polygon-wallet-core/src/lib.rs` (re-export from evm-wallet-core + Polygon config)
- Create: `rust-wallet-app/crates/polygon-wallet-core/src/network.rs` (Polygon-specific RPC URLs + gas token label)
- Create: `rust-wallet-app/crates/polygon-wallet-core/tokens/mainnet.json` (USDC + USDT + DAI per deep-dive §"Top ERC-20 stablecoins")
- Create: `rust-wallet-app/crates/polygon-wallet-core/tokens/amoy.json` (USDC Amoy)
- Create: `rust-wallet-app/crates/polygon-wallet-core/tests/derivation_cross_check.rs` (V3 mirror — ETH + Polygon derivation produces same address)

**Interfaces:**
- `polygon_wallet_core::network::POLYGON_MAINNET_RPC_URL: &str = "https://polygon-rpc.com"` (Q4)
- `polygon_wallet_core::network::POLYGON_AMOY_RPC_URL: &str = "https://polygon-amoy.drpc.org"`
- `polygon_wallet_core::network::CHAIN_ID_POLYGON_MAINNET: u64 = 137` (Q4 verified via `eth_chainId`)
- `polygon_wallet_core::network::CHAIN_ID_POLYGON_AMOY: u64 = 80002` (Q4 verified via `eth_chainId`)
- `polygon_wallet_core::network::GAS_TOKEN_LABEL: &str = "POL"` (Q8)
- `polygon_wallet_core::network::LEGACY_GAS_TOKEN_LABEL: &str = "MATIC"` (Q8 alias)

**Steps:**
- [ ] Step 1: Create `polygon-wallet-core` skeleton with `evm-wallet-core` (path) + `alloy-chains` (NEW direct dep) as dependencies
- [ ] Step 2: Author `tokens/mainnet.json` — USDC `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359` (6 dec), USDT `0xc2132D05D31c914a87C6611C10748AEb04B58e8F` (6 dec), DAI `0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063` (18 dec) — verified 2026-08-27 via Circle + StableRegistry sources (per deep-dive §"Top ERC-20 stablecoins")
- [ ] Step 3: Author `tokens/amoy.json` — USDC Amoy `0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582` (6 dec)
- [ ] Step 4: Implement `network.rs` constants (RPC URLs, chain-ids, gas-token labels) — verified against Polygon docs + Circle sources
- [ ] Step 5: Write `tests/derivation_cross_check.rs` — verifies `m/44'/60'/0'/0/0` produces the **same address on ETH + Polygon** (cross-chain identity per deep-dive §"Crate-by-crate notes → alloy-signer-local")
- [ ] Step 6: Verify gate (cargo fmt + clippy --all-targets -- -D warnings + test per L55)
- [ ] Step 7: Commit `feat(polygon): scaffold polygon-wallet-core thin wrapper for #416 (Task 2)`

## Phase 2 — RPC integration (1 task — extends existing evm-wallet-core)

### Task 3 (#416 T3): Polygon RPC connectivity + gas estimation cadence

**Files:**
- Modify: `rust-wallet-app/crates/evm-wallet-core/src/provider.rs` (add `provider::new_http_polygon_mainnet()` + `new_http_polygon_amoy()` convenience constructors)
- Create: `rust-wallet-app/crates/evm-wallet-core/tests/polygon_rpc.rs` (V2 + V5 mirror — get_chain_id + estimate_eip1559_fees)

**Interfaces:**
- `evm_wallet_core::provider::new_http_polygon_mainnet() -> Result<RootProvider, Error>` (returns provider against `https://polygon-rpc.com`)
- `evm_wallet_core::provider::new_http_polygon_amoy() -> Result<RootProvider, Error>` (returns provider against `https://polygon-amoy.drpc.org`)
- Reuse existing `new_http_pinned(url, spki)` for SPKI pin against `pinned://<spki>@polygon-rpc.com` (Q7)

**Steps:**
- [ ] Step 1: Add `new_http_polygon_mainnet()` constructor — `Provider::new_http("https://polygon-bor-rpc.publicnode.com".parse()?)` (Issue #474 drift from `polygon-rpc.com`)
- [ ] Step 2: Add `new_http_polygon_amoy()` constructor — `Provider::new_http("https://polygon-amoy-bor-rpc.publicnode.com".parse()?)` (Issue #474 drift from `polygon-amoy.drpc.org`)
- [ ] Step 3: Test `tests/polygon_rpc.rs` — `provider.get_chain_id() == 137` (mainnet) and `== 80002` (Amoy). Live test gated behind `RUN_POLYGON_AMOY=1` / `RUN_POLYGON_MAINNET=1`.
- [ ] Step 4: Test `provider.estimate_eip1559_fees()` returns valid `(max_fee_per_gas, max_priority_fee_per_gas)` tuple (V4 — re-estimate cadence proof)
- [ ] Step 5: Verify gate (cargo fmt + clippy --all-targets -- -D warnings + test per L55)
- [ ] Step 6: Commit `feat(evm): add Polygon RPC constructors + chain-id tests for #416 (Task 3)`

## Phase 3 — Polygon-specific config + POL display + USDC footgun (2 tasks)

### Task 4 (#416 T4): Token registry + decimals resolution

**Files:**
- Create: `polygon-wallet-core/tokens/{mainnet,amoy}.json` (Task 2 already created; this task verifies decimals resolution)
- Create: `rust-wallet-app/crates/polygon-wallet-core/tests/token_registry_decimals.rs` (V6 mirror — `decimals()` selector verifies 6 for USDC, 18 for DAI)

**Steps:**
- [ ] Step 1: `tokens.rs::load(Network::Polygon, "mainnet")` returns 3 tokens (USDC, USDT, DAI)
- [ ] Step 2: `tokens.rs::load(Network::Polygon, "amoy")` returns 1 token (USDC)
- [ ] Step 3: Test USDC mainnet `decimals()` selector returns `6` via `provider.call(&req)` (mirrors eth Story 22 pattern)
- [ ] Step 4: Test DAI mainnet `decimals()` selector returns `18`
- [ ] Step 5: Verify gate
- [ ] Step 6: Commit

### Task 5 (#416 T5): Native USDC vs USDC.e footgun + POL display

**Files:**
- Create: `rust-wallet-app/crates/polygon-wallet-core/src/disambig.rs` (warning emitter + footgun flag)
- Modify: `rust-wallet-app/crates/polygon-wallet-core/src/lib.rs` (export disambig module)

**Interfaces:**
- `polygon_wallet_core::disambig::assert_native_usdc(address: Address) -> Result<(), Error>` — fails if address matches bridged `USDC.e` (verify via known `USDC.e` address list)
- `polygon_wallet_core::disambig::gas_token_label(use_legacy: bool) -> &'static str` — returns "POL" or "MATIC" (Q8)

**Steps:**
- [ ] Step 1: Implement `assert_native_usdc()` — checks address against bridged USDC.e address list (Polygon mainnet `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174` historical; deferred if address unknown)
- [ ] Step 2: Implement `gas_token_label()` — returns "POL" (default) or "MATIC" (when `--legacy-token-symbol` flag set, per user-stories Story 31)
- [ ] Step 3: Test `disambig.rs` — `assert_native_usdc(0x3c499c...3359)` succeeds (native USDC); rejects the bridged USDC.e address
- [ ] Step 4: Verify gate
- [ ] Step 5: Commit

## Phase 4 — `polygon` CLI + smoke + release cut (4 tasks)

### Task 6 (#416 T6): `polygon` CLI scaffold + wallet commands

**Files:**
- Create: `rust-wallet-app/crates/polygon/Cargo.toml` (path deps: `polygon-wallet-core`, `clap`, `tokio`)
- Create: `rust-wallet-app/crates/polygon/src/main.rs` (clap subcommands per user-stories §"Story → alloy sub-crate map")

**Steps:**
- [ ] Step 1: Add `polygon` to umbrella `members`
- [ ] Step 2: Implement clap subcommands + flags covering all 31 user-stories + 3 cross-cutting (full mapping per audit doc §"User-stories → Plan traceability matrix"):
  - `wallet create --name w --network amoy|mainnet|anvil` — Story 1
  - `wallet import --name w --mnemonic "..." --network ...` (also `--private-key`) — Story 2
  - `wallet list [--network ...]` — Story 9
  - `wallet show --name w --network ... [--addresses | --export]` — Stories 9, 19
  - `wallet balance --address 0x... --network ... [--unit pol|wei|matic(deprecated)]` — Stories 3, 31
  - `wallet sync --address 0x... --network ...` — Story 4
  - `wallet send --name w --password p --to 0x... --amount 0.01 --network ... [--batch <file> | --drain | --nonce <N> | --gas-limit <N> | --fee fastest|half_hour|hour|economy | --max-fee-gwei <N> | --priority-fee-gwei <N> | --dry-run | --wait]` — Stories 5, 6, 13, 14, 15, 16
  - `wallet send speed-up --tx-hash 0x... --max-fee-gwei <N> --priority-fee-gwei <N> --network ...` — Story 17
  - `tx list --address 0x... --network ... [--since-block <N>] [--limit <N>]` — Story 7
  - `tx get --tx-hash 0x... --network ...` — Story 7
  - `erc20 send --name w --password p --token USDC|USDT|DAI --to 0x... --amount 1.5 --network ... [--token-address 0x...]` — Story 21
  - `erc20 balance --address 0x... --token USDC|USDT|DAI --network ... [--all]` — Story 22
  - `erc20 list --network ... [--json]` — Story 23
  - `erc20 register --address 0x... --network ... [--list | --remove --symbol FOOBAR]` — Story 24
  - `erc20 approve --name w --password p --token USDC --spender 0x... --amount 100 --network ... [--amount unlimited|max]` — Story 25
  - `fee --network ... [--json]` — Story 8
  - `config show [--json]` — Story 11
  - `faucet --address 0x... --network amoy [--faucet-token <TOKEN>] [--auto]` — Story 30
  - `sign-message --name w --password p --message "..." --address <addr> [--verify <addr>]` — Story 18
  - `sign-typed --name w --password p --typed-data '<JSON>' --chain-id 137|80002 [--typed-data-file <path>] [--verify <addr>]` — Story 27 (EIP-712 with chain_id validation per Q7 + C1)
  - Cross-cutting: `--json` flag on every command (Story -json); `std::process::ExitCode` stable exit codes (Story -exit); no daemons (Story -nodaemon); `zeroize::Zeroizing<Mnemonic>` for mnemonic (Story -zeroize); `alloy_primitives::Address::to_checksum_buffer(None)` EIP-55 display (Story -eip55)
- [ ] Step 3: Default network = Amoy; mainnet opt-in via `--network mainnet`
- [ ] Step 4: `--rpc-url` flag overrides default RPC URL
- [ ] Step 5: `--legacy-token-symbol` flag enables MATIC alias display
- [ ] Step 6: Verify gate
- [ ] Step 7: Commit `feat(polygon-cli): scaffold polygon CLI binary for #416 (Task 6)`

### Task 7 (#416 T7): Amoy testnet smoke (operator-driven per L29)

**Files:**
- Create: `rust-wallet-app/scripts/polygon-send-amoy-e2e.sh` (mirror `eth-send-sepolia-e2e.sh`)
- Create: `rust-wallet-app/crates/polygon/tests/amoy_smoke.rs` (`#[ignore]` + `RUN_POLYGON_AMOY=1` gated)

**Steps:**
- [ ] Step 1: `polygon wallet create --name w --network amoy` succeeds
- [ ] Step 2: `polygon faucet --address <addr> --network amoy` prints Amoy faucet URL (Story 30)
- [ ] Step 3: After manual faucet claim, `polygon wallet balance --network amoy` returns > 0 POL
- [ ] Step 4: `polygon wallet send --to 0xAbC... --amount 0.01 --network amoy` broadcasts, `txID` returned, receipt shows status success
- [ ] Step 5: `polygon fee --network amoy` returns current gas tiers (Story 8 — must re-fetch per call, NOT cache)
- [ ] Step 6: Verify gate
- [ ] Step 7: Commit

### Task 8 (#416 T8): Mainnet smoke + Ethereum regression smoke (Option A acceptance test)

**Files:**
- Create: `rust-wallet-app/crates/polygon/tests/mainnet_smoke.rs`
- Create: `rust-wallet-app/crates/eth-wallet-core/tests/regression_post_refactor.rs`

**Steps:**
- [ ] Step 1: `polygon wallet balance --address 0xAbC... --network mainnet` returns real Polygon mainnet balance (no synthetic data — `RUN_POLYGON_MAINNET=1`)
- [ ] Step 2: `eth wallet balance --address 0xAbC... --network mainnet` still works post-refactor (regression test — confirms eth-wallet-core functionality preserved)
- [ ] Step 3: Same mnemonic + derivation path produces same address on ETH + Polygon (cross-chain identity verified)
- [ ] Step 4: SPKI pin against `pinned://<spki>@polygon-bor-rpc.publicnode.com` succeeds (Q7 + Q7 = SPKI pin reuse from `bitcoin-wallet-core`; Issue #474 URL drift)
- [ ] Step 5: Verify gate
- [ ] Step 6: Commit

### Task 9 (#416 T9): Release cut + L21/L24 cascade

**Files:**
- Modify: `rust-wallet-app/crates/polygon-wallet-core/Cargo.toml` (bump to `0.1.0`)
- Modify: `rust-wallet-app/crates/evm-wallet-core/Cargo.toml` (bump to `0.1.0`)
- Modify: `rust-wallet-app/crates/eth-wallet-core/Cargo.toml` (bump to `0.3.0` — preserves v0.3.0 release per #311 + adds refactor marker)
- Create: `docs/CHANGELOG.md` (v0.1.0 entry per L24)
- Modify: `.superpowers/sdd/2026-08-27-polygon-wallet-core/` (estimate-report + ai-cost-report per L21)

**Steps:**
- [ ] Step 1: L24 CHANGELOG entry — "v0.1.0 — Initial release. evm-wallet-core extracted from eth-wallet-core (Q1 Option A refactor); polygon-wallet-core thin wrapper added (chain-id 137 mainnet + 80002 Amoy); POL native gas token display with MATIC legacy alias; EIP-1559 gas estimation with 2-second-block re-estimate cadence; native Circle USDC support (NOT bridged USDC.e); EIP-712 chain_id replay protection; SPKI pin reuse from bitcoin-wallet-core."
- [ ] Step 2: L21 update estimate-report + ai-cost-report with Polygon work in-flight count + completion percentage
- [ ] Step 3: Tag `evm-wallet-core-v0.1.0` + `polygon-wallet-core-v0.1.0` + `polygon-cli-v0.1.0`
- [ ] Step 4: Open PR per L25 — single PR bundles Phase 0–4 work, references issue #416
- [ ] Step 5: L8 — flip issue #416 checkboxes `[ ]`→`[x]` before squash-merge
- [ ] Step 6: Final commit + tag push

## Spike closure (V1–V10 acceptance)

After Phase 0–4 ship, the `rust-wallet-app/spikes/polygon-v1/` spike produces PASS evidence for V1–V10 (one per Q + 2 cross-cutting acceptance tests).

| V# | Q | What it verifies | Maps to Phase |
|---|---|---|---|
| V1 | Q1 EVM-reuse | `cargo build -p evm-wallet-core -p eth-wallet-core -p polygon-wallet-core` clean | Phase 0 Task 1 |
| V2 | Q4 RPC connectivity | `provider.get_chain_id()` returns 137 (mainnet) + 80002 (Amoy) | Phase 2 Task 3 |
| V3 | Q1 derivation | `m/44'/60'/0'/0/0` produces same address on ETH + Polygon | Phase 1 Task 2 |
| V4 | Q5 EIP-1559 cadence | `estimate_eip1559_fees()` re-estimated twice 3s apart shows different values (proves 2-second-block volatility) | Phase 2 Task 3 |
| V5 | Q4 RPC connectivity | `provider.get_block_number()` against `polygon-rpc.com` returns sane value | Phase 2 Task 3 |
| V6 | Q3 token registry | `tokens/mainnet.json` 3 entries load + USDC decimals = 6 + DAI decimals = 18 verified | Phase 3 Task 4 |
| V7 | Q4 Amoy faucet | Request Amoy POL via `https://faucet.polygon.technology/`, verify receipt via `provider.get_balance()` | Phase 4 Task 7 |
| V8 | Q5 native POL transfer | Send 0.01 POL on Amoy, verify recipient `get_balance()` reflects change | Phase 4 Task 7 |
| V9 | Q3 ERC-20 stablecoin transfer | Deploy MockERC20 to Anvil (Polygon-fork), transfer 100 tokens, verify `balanceOf` | Phase 4 Task 7 |
| V10 | Q7 signature replay protection | Sign EIP-712 typed message on chain-id 137, verify replay attempt on chain-id 1 (Ethereum) fails with `InvalidSignature` | Phase 3 Task 5 |

### Per-Vn run protocol

```bash
# Offline Vns (always run — Anvil + offline RPC mock)
cargo test -p polygon-spike-v1 --test v1_evm_reuse
cargo test -p polygon-spike-v1 --test v3_derivation
cargo test -p polygon-spike-v1 --test v9_erc20_transfer
cargo test -p polygon-spike-v1 --test v10_eip712_replay

# Gated Vns (require RUN_POLYGON_AMOY=1 or RUN_POLYGON_MAINNET=1 — live RPC access)
RUN_POLYGON_AMOY=1 cargo test -p polygon-spike-v1 --test v2_chain_id
RUN_POLYGON_AMOY=1 cargo test -p polygon-spike-v1 --test v4_eip1559_estimates
RUN_POLYGON_AMOY=1 cargo test -p polygon-spike-v1 --test v7_amoy_faucet
RUN_POLYGON_AMOY=1 cargo test -p polygon-spike-v1 --test v8_native_pol_transfer
RUN_POLYGON_MAINNET=1 cargo test -p polygon-spike-v1 --test v5_rpc_connectivity

# All Vns at once
cargo test -p polygon-spike-v1 --test '*'                                # offline only
RUN_POLYGON_AMOY=1 RUN_POLYGON_MAINNET=1 cargo test -p polygon-spike-v1 --test '*'  # full
```

### PASS evidence requirements

Each Vn must produce:
- **Command output:** the `cargo test` stdout/stderr showing test pass.
- **SHA:** the git SHA of the commit that added/ran the test (per L13 review trail).
- **Recorded in:** `rust-wallet-app/spikes/polygon-v1/RESULT.md` — one section per Vn.

When all 10 Vns pass, issue #416 acceptance criterion "Open questions resolved before code" flips `[x]` — the deep-dive resolves Q1-Q4 with citations + Q5-Q8 resolved by the spike's PASS evidence.

## Out of scope (deferred per issue #416 body)

- Polygon zkEVM (chain-id 1101) — different chain-id + RPC + token registry. Add via `Network::PolygonZkEvm` enum variant at v0.2.
- Mumbai testnet — deprecated 2024-Q2 (Goerli-rooted). Use Amoy.
- Bridged `USDC.e` on Polygon — footgun, only native Circle USDC supported.
- Hardware wallet (Ledger/Trezor) via `alloy-signer-ledger` / `alloy-signer-trezor` — deferred to v0.2 (Q6).
- Smart-contract deployment via wallet — sign-only + broadcast external path is enough.
- L2 DEX integration (QuickSwap swaps, Uniswap-v3-Polygon swaps) — wallet is sign-only + broadcast.
- Polygon staking delegation (POL staking via Stake 2.0 equivalent) — wallet is for transfer only.
- Flashbots / MEV protection on Polygon private RPCs — defer.
- EIP-712 typed-data signing is in v0.1 scope per Q7, but full domain-separation + complex nested types land in v0.3.
- ENS resolution (`alice.eth`) — defer to v1.x.
- EIP-4337 account abstraction — defer to v1.x.

## Dependencies

- **Issue body:** #416 (deep-dive ✓ at `docs/wallets/2026-08-27-polygon-rust-sdks-deep-dive.md`, user-stories ✓ at `docs/wallets/2026-08-27-polygon-wallet-user-stories.md`, plan = this doc, spike = next)
- **Prior plans (templates):**
  - `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` (v0.1 — pattern source)
  - `docs/superpowers/plans/2026-08-23-eth-wallet-core.md` (v0.2 — refactor target + async test pattern)
  - `docs/superpowers/plans/2026-08-27-tron-wallet-core.md` (v0.1 — sibling non-EVM chain template + spike mapping)
- **eth-wallet-core source (refactor input):** `rust-wallet-app/crates/eth-wallet-core/`
- **Workspace deps to add (Phase 1 Task 2 Step 1):** `alloy-chains` (NEW direct dep for `polygon-wallet-core` only — for `Chain::Polygon` enum). All other deps (alloy, bip32, bip39, reqwest, rustls) reused.
- **Workspace `members` array (Phase 0 — plan doc only, not actual file):** add `"spikes/polygon-v1"` to `rust-wallet-app/Cargo.toml` `members` (mirrors `"spikes/tron-v1"` entry). Per L25 this is documentation of a future workspace edit, NOT an action — user instruction was to enrich the plan only (per session 2026-08-27 "we only edit plan with polygon-v1 structure in plan, don't execute").
- **Bitcoin SPKI pin reuse:** `bitcoin-wallet-core/src/chain/spki.rs` (F20 / Q7)
- **Alloy version:** `=1.8.3` (matches eth-wallet-core v0.2 — Q1 MSRV parity)
- **Tokio test policy:** every test that touches async code MUST be `async fn` + `#[tokio::test]` per eth #333.

## References

- Issue #416: https://github.com/nhitranbtc/blockchain-sdk/issues/416
- Polygon deep-dive: `docs/wallets/2026-08-27-polygon-rust-sdks-deep-dive.md`
- Polygon user-stories: `docs/wallets/2026-08-27-polygon-wallet-user-stories.md`
- ETH deep-dive (companion): `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md`
- ETH user-stories (template): `docs/wallets/2026-08-23-eth-wallet-user-stories.md`
- ETH plan (template + refactor target): `docs/superpowers/plans/2026-08-23-eth-wallet-core.md`
- TRON plan (sibling template + Phase 0.0 pattern): `docs/superpowers/plans/2026-08-27-tron-wallet-core.md`
- ETH spike (precedent): `rust-wallet-app/spikes/alloy-v1/`
- TRON spike (precedent): `rust-wallet-app/spikes/tron-v1/`
- Bitcoin SPKI pin source: `bitcoin-wallet-core/src/chain/spki.rs`
- Polygon docs: https://docs.polygon.technology/
- Circle USDC contract addresses (canonical): https://developers.circle.com/stablecoins/usdc-contract-addresses
- L13 pipeline spec: `tasks/lessons.md` L13 (apply literally)