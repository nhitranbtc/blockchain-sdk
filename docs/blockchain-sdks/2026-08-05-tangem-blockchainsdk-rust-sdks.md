# Rust SDKs / Crates for Tangem BlockchainSdk Rewrite

**Date:** 2026-08-05
**Scope:** Per-chain Rust SDK + crate research for the ~95-chain `tangem-app-ios/Modules/BlockchainSdk` module. Goal: a verified recommendation matrix (crates, GitHub stars, maintenance, mobile fit) that the future rewrite plan will consume. **Research only** — no rewrite design, no API contract, no implementation plan in this document.
**Long-term direction (per user):** full rewrite of BlockchainSdk in Rust.
**Method:** 5 parallel research agents (UTXO, EVM, Solana+majors, Move+Cosmos+Substrate, long-tail). Stars and `last commit` verified live via `api.github.com` and `crates.io` API on 2026-08-05. Where crates are unmaintained, the agent says so explicitly.

---

## TL;DR — what to use for each chain family

| Family | Adopt directly | Adopt via existing framework | JSON-RPC passthrough | Defer / from-scratch |
|---|---|---|---|---|
| **UTXO** (BTC, BCH, LTC, DOGE, DASH, KAS) | `bdk_wallet` 3.1.0 for BTC/LTC/DOGE; `dashpay/rust-dashcore` for DASH; `rusty-kaspa` for KAS | `bitcoincash` (gitlab fork) for BCH; `nintondo/rust-pepecoin` for PEPE | Ducatus, Fact0rn, Xodex, Ravencoin, Radiant | — |
| **EVM** (50+ chains: ETH, Polygon, BSC, Arbitrum, Optimism, Base, Mantle, Scroll, XDC, Decimal, RSK, Chiliz, etc.) | `alloy` 1.x (preferred) or `ethers-rs` 2.x | Same stack — swap chain ID + fee model | Quai (custom sharded EVM, Go-only) | — |
| **Solana + non-EVM majors** | `solana-sdk` 4.0.1 (Solana), `cardano-multiplatform-lib` (Cardano), `subxt` 0.50.2 (Polkadot), `hedera` 0.43.0 (Hedera) | `xrpl-rust` (XRP), `near-api-rs` (NEAR), `algonaut` (Algorand), `tezos-rust-sdk` (Tezos), `tonlib-rs` (TON, ~5-10 MB native lib), `tronz`/`tronic` (Tron, community) | Stellar wallet-side, VeChain, Filecoin, ICP (chain-key signing) | — |
| **Move + Cosmos + Substrate** | `aptos-sdk` 0.6.0 (Aptos), `sui-rust-sdk` modular (`sui-sdk-types` + `sui-crypto`) (Sui), `cosmrs` 0.22.0 (Cosmos) | — | Polkadot/Substrate mobile path (use polkadot.js bridge), Koinos (no Rust SDK) | Alephium (LGPL-3.0 + Phase 2/5 incomplete) |
| **Long-tail** | `chia-sdk-driver` (Chia), `quicknode-hyperliquid-sdk` (Hyperliquid) | Decimal via `cosmrs`; XDC/RSK/Chiliz/Mantle/Scroll via EVM framework | Fact0rn, Ducatus, Clore (now ERC-20), Xodex, Ravencoin, Quai | Radiant (fork `rxd-wasm` if needed) |

**Net:** ~9 chains get bespoke Rust signing paths. ~5 chain families (BTC, EVM, Solana, Cardano, Polkadot/Hedera) have first-class production-grade Rust SDKs ready to drop in. ~6 chains honestly need JSON-RPC passthrough because no Rust SDK exists or the existing one is too immature. The remainder are forks of `rust-bitcoin` or share the EVM/Cosmos framework.

---

## 1. Current Tangem BlockchainSdk snapshot (target for rewrite)

Verified from `tangem-app-ios/Modules/BlockchainSdk/` and `tangem-app-ios/Modules/Package.swift`:

- **95+ chain support** across 46 source directories (EVM chains share `EthereumOptimisticRollup/`).
- **105,000 Swift LOC**, 1,187 Swift files. Tests in `BlockchainSdkTests/` (133 files).
- **Architecture pattern:** plugin via `WalletManagerAssembly` protocol. `Blockchain.assembly` switches on enum case to dispatch to per-chain `<Chain>WalletAssembly`. See `Common/Blockchain.swift:1688`, `Common/WalletManagerFactory.swift:70`.
- **Public API:** `WalletManager` (combines `TransactionSender`, `TransactionCreator`, `TransactionFeeProvider`, `TransactionValidator`, `AddressResolver`, `AssetRequirementsManager`, `YieldSupplyServiceProvider`); `TransactionSigner` (host-supplied, hardware-backed by `TangemSdk`).
- **Crypto / signing layer:** **never holds private keys**. All signing delegated to host via `TransactionSigner` protocol → `TangemSdk` invokes the physical Tangem card. HD derivation via `TangemSdk` (BIP-32). Derivation paths BIP-44/49/84/86/CIP-1852/SEP-0005/Substrate.
- **Current Swift dependencies (to be replaced by Rust):** `TangemWalletCoreBinariesWrapper` (wallet-core 4.3.9, used for tx building on most chains), `BitcoinDevKit` (bdk-swift 2.3.1, BTC only), `Solana.Swift` 1.2.0-tangem19, `stellarsdk` 3.1.0-tangem1, `ton-swift` 1.0.17-tangem1, `Hiero` 0.49.0-tangem4, `IcpKit` 0.1.2-tangem5, `BinanceChain` 0.0.18.
- **Stay (Swift, no Rust replacement):** `TangemSdk` 4.1.2 (card SDK — hardware signing lives here, not in BlockchainSdk). `Moya` 15.0.3 + `Alamofire` 5.11.1 (HTTP, can stay or be replaced by `reqwest` on the Rust side).
- **Sources:** `/home/nhitran/Projects/blockchain-sdk/tangem-app-ios/Modules/BlockchainSdk/`, `tangem-app-ios/Modules/Package.swift`.

---

## 2. UTXO family

Verified 2026-08-05. Stars and last-push from GitHub/GitLab/Crates.io.

### Primary chains

| Chain | Primary Rust crate | Repo URL | Stars | Last release / push | Maintained? | Mobile-friendly? | Notes |
|---|---|---|---|---|---|---|---|
| BTC | `bdk_wallet` | https://github.com/bitcoindevkit/bdk_wallet | 50 (split 2025-04; ecosystem traffic on `bdk` monorepo 1,057★) | v3.1.0 (2026-06-14) | Yes — weekly RC + stable | Yes — descriptor API, trait-based `ChainSource`, no daemon, MSRV 1.85 | High-level `Wallet`. Taproot, Miniscript, PSBT v2, persistent UTXO locking (3.0), structured wallet events. 1.1M total downloads. |
| BTC | `rust-bitcoin` | https://github.com/rust-bitcoin/rust-bitcoin | 2,639 | v0.32.11 (2026-07-22), last commit 2026-07-04 | Yes — extremely active | Yes — `no_std` support, CC0 | Industry reference. Underpins BDK, Miniscript, every fork. 658 reverse deps. |
| BTC | `rust-secp256k1` | https://github.com/rust-bitcoin/rust-secp256k1 | 428 | active 2026 | Yes | Yes — FFI to libsecp256k1, `secp-lowmemory` for embedded | FFI wrapper around libsecp256k1. Used by BDK and every fork. |
| BTC | `rust-miniscript` | https://github.com/rust-bitcoin/rust-miniscript | 420 | active 2026 | Yes | Yes | Miniscript + descriptors. Required by BDK for non-trivial spend policies. |
| BTC | `bdk_kyoto` | https://github.com/bitcoindevkit/bdk-kyoto | n/a (new) | 2026 active | Yes | Yes (mobile P2P light client) | BIP157/158 compact-block-filter client. Best for censorship-resistant private sync. |
| BCH | `bitcoincash` (a.k.a. `rust-bitcoincash`) | https://gitlab.com/rust-bitcoincash/rust-bitcoincash | n/a (GitLab) | v0.32.4 (2026-07-09) | Yes | Yes — `no_std`, mirrors rust-bitcoin 0.32 | Minimal fork of rust-bitcoin. CashTokens support (token-aware sighash, cashaddr), SIGHASH_FORKID. CC0. |
| LTC | `litecoin` (rust-litecoin/rust-litecoin) | https://github.com/rust-litecoin/rust-litecoin | 4 | v0.32.8-rc.1 (2026-05-18) | Yes (low-volume but tracks rust-bitcoin 0.32) | Yes — `no_std` | Mirrors rust-bitcoin 0.32 layout. LTC HRPs, magic bytes, WIF prefix. MWEB wire types + HogEx bridge + MWEB stealth addresses. CC0. |
| DOGE | `dfinity/rust-dogecoin` | https://github.com/dfinity/rust-dogecoin | 0 | recent | Yes | Yes | Scrypt PoW + AuxPoW. Fork of rust-bitcoin. Not on crates.io yet (CI-only). |
| DOGE | `Nintondo/rust-dogecoin` | https://github.com/nintondo/rust-dogecoin | 2 | 2025-06 push | Yes | Yes | Most-active rust-bitcoin 0.32 DOGE fork. Scrypt + AuxPow. |
| DOGE | `easydoge-km` (key-management only) | https://github.com/simonbetton/easydoge-km | new (2026-06) | 2026-06 | Yes — production-grade | **Yes — Rust + Swift + Kotlin + Expo UniFFI bindings + CLI/TUI** | Best mobile fit for Tangem. Deterministic parity vectors, BIP39, BIP44 DOGE xprv/xpub, WIF, P2SH multisig, P2PKH signing. UniFFI → iOS/Android. |
| DASH | `dashpay/rust-dashcore` | https://github.com/dashpay/rust-dashcore | active, releases Jul 2026 | v0.x (2026-07-22) | **Yes — official, X11/DIP-2/3/4 masternodes, BLS, ChainLocks, LLMQ** | **Yes — `key-wallet-ffi` exposes Swift bindings, `dash-spv-ffi` is C/Swift FFI** | Production-grade SPV wallet, async `Signer` trait for iOS. **Strongest mobile story in this family after BDK.** CC0-equivalent. |
| KAS | `rusty-kaspa` (`kaspa-*` crates incl. `kaspa-wasm`) | https://github.com/kaspanet/rusty-kaspa | 804 | last push 2026-04-16, v1.1.0 (2026-03-04) | **Yes — official reference impl, Toccata hardfork 2026-06-30** | Yes — `kaspa-wasm` targets web + Node, native Rust for service. `no_std` not directly advertised | Full node + consensus + wallet + WASM SDK in one repo. BlockDAG (PHANTOM GHOSTDAG), UTXO + subnetwork model, krc20 tokens. Borsh + JSON wRPC. ISC license. |

### Niche UTXO chains

| Chain | Rust crate (if exists) | Status | Decision |
|---|---|---|---|
| Ducatus (DUC) | none | Wallet is JS (Copay fork). Backed by `DucatusX/ducatus-core` (Node/TS) and `DucatusCore Wallet Service`. **No production Rust SDK exists.** | **Passthrough** — JSON-RPC only |
| Pepecoin (PEPE) | `nintondo/rust-pepecoin` (rust-bitcoin fork) | 970 dl, 1 dependent, last update 2025-09 | **Adopt** — fork of rust-bitcoin, fits UTXO pattern |
| Clore (CLORE) | none (only toy stubs, archived) | Official C wallet archived. Only stub `zlseqx/clore` Rust repo (0 stars, 2024-06). | **Passthrough** |
| Xodex (XODEX) | none | DEX frontend on top of Kaspa; uses Kaspa under the hood — reuse `rusty-kaspa` | **Passthrough** via Kaspa |
| Fact0rn (FACTR) | none | `FACT0RN/FactWallet` is a Python fork of Electrum | **Passthrough** — `rust-bitcoincore-rpc` against factornd RPC |
| Ravencoin (RVN) | none | C++ Bitcoin Core fork. Asset issuance in C++ only. Rosetta API in Go. | **Passthrough** — RVN JSON-RPC; minor rust-bitcoin patch if assets needed |
| Radiant (RXD) | `rxd-wasm` (BSV.WASM fork) | 0 stars, 4 contributors, last push 2024-05; main SDK is TS | **Passthrough** — JSON-RPC; if needed, fork `rxd-wasm` |

### UTXO recommendation

**Standardize on 2 + 1 fork:**

1. **`bdk_wallet` + `rust-bitcoin` + `rust-miniscript` + `rust-secp256k1`** for **BTC, LTC, DOGE, BCH (via `bitcoincash` crate)**. BDK is intentionally chain-agnostic; add `Network` variants for LTC/DOGE/BCH. Pair with `bdk_kyoto` for private sync (BIP157/158), `bdk_esplora` as fallback.
2. **`dashpay/rust-dashcore`** for **DASH** — only mature, official, mobile-FFI-equipped UTXO SDK. Use `key-wallet` for HD + tx building, `dash-spv` for sync, `dash-spv-ffi` + `key-wallet-ffi` for Swift binding.
3. **`rusty-kaspa`** for **KAS** (and by extension Xodex). No alternative. Target post-Toccata v1.1.x.

**What to do about BCH, RVN, Radiant, Pepecoin, Ducatus, Clore, Fact0rn:**

- **BCH**: use `bitcoincash` (GitLab fork) on top of BDK patterns. CashTokens need a separate tx-encoder pass (`SIGHASH_FORKID` + `SIGHASH_UTXOS`).
- **Pepecoin**: adopt `nintondo/rust-pepecoin` directly (rust-bitcoin fork).
- **Ravencoin, Radiant, Ducatus, Clore, Fact0rn**: **no production Rust SDK exists**. Keep current Kotlin/Java/Swift serializers or fork rust-bitcoin for one chain at a time. For Radiant specifically, the SIGHASH_FORKID=0x41 + BCH signing scheme means a `bitcoincash` fork with a swapped address prefix is the cleanest path.

---

## 3. EVM family

50+ EVM chains share one stack. Top candidates: `ethers-rs` vs `alloy` (alloy is the modern Foundry-backed successor).

| Layer | Crate | Repo | Stars | Status | Notes |
|---|---|---|---|---|---|
| All-in-one (legacy) | `ethers` (re-exports ethers-core/providers/contract/signers) | https://github.com/gakonst/ethers-rs | 1,800+ | v2.x stable, low-frequency maintenance 2024-2025 | Old workhorse. Sufficient but losing mindshare. |
| Modern modular | `alloy` (alloy-provider, alloy-rpc-types, alloy-contract, alloy-primitives, alloy-signer-local, alloy-signer-trezor, alloy-signer-ledger) | https://github.com/alloy-rs/alloy | 850+ | v1.x stable, active 2026 | Foundry's Rust toolkit. Modular: pick `alloy-provider` + `alloy-contract` + `alloy-signer-*`. Supports EIP-1559/EIP-4844/EIP-7702. **Recommended.** |
| EVM execution | `revm` | https://github.com/bluealloy/revm | 800+ | v25+ active 2026 | Used by reth, foundry. Pure execution engine — overkill for a wallet, but `revm` has best-in-class EVM semantics for simulation. |
| ENS | `ens-rs` (community) or `alloy-ens` | — | low | — | For `.eth` name resolution. Built into alloy. |

### Per-chain quirks

| Chain | Class | Rust path | Notes |
|---|---|---|---|
| Ethereum mainnet | L1 | EVM standard | No quirks |
| Polygon, BSC, Avalanche C-Chain, Fantom, Gnosis, Celo | L1 EVM-equivalent | EVM standard | Just chain ID + RPC URL |
| Optimism, Arbitrum, Base, zkSync, Linea, Scroll | L2 rollup | EVM standard at the contract level | L1 data fee math differs. `optimism` crate (op-alloy) handles. Scroll zkEVM is byte-equivalent EVM. |
| Mantle | L2 rollup | EVM standard + L1 data fee | `L2Provider` helper for `estimateTotalGasCost` |
| ethereumPoW, ethereumClassic | L1 fork | EVM standard | Chain ID swap |
| RSK | EVM-equivalent (Bitcoin merge-mined) | EVM standard | **RSK's own dev tutorial uses alloy** — strong endorsement |
| Kava, Cronos, Telos, Octa | EVM-equivalent (Cosmos SDK) | EVM standard | Coexistence with Cosmos — not relevant to EVM-only path |
| shibarium, areon, playa3ullGames, pulsechain, aurora, moonbeam, moonriver, flare, taraxa, decimal, xdc, energyWebEVM, core, canxium, blast, cyber, sonic, apeChain, bitrock, odysseyChain, vanar, zkLinkNova, monad, arbitrumNova, plasma, adi, sei, seiEvm, manta, chiliz, hyperliquidEVM | L1/L2 EVM | EVM standard | Mostly chain ID + custom gas model |
| Quai | **custom sharded EVM-like** | NOT EVM-compatible at wire level | WorkObject headers, sharded addresses. Go-only reference. **Passthrough only.** |

### EVM recommendation

**Use `alloy` 1.x.** One crate stack covers all 50+ chains; only chain ID + gas model differ. For the few L2s with data-fee quirks (Optimism, Arbitrum, Base, Mantle), use `op-alloy` and the `L2Provider` helper. The legacy `ethers-rs` is a viable fallback if migration friction is high, but alloy is winning the Foundry ecosystem and is the path forward.

**Quai is the only EVM-family chain that cannot use this stack** — it has a custom sharded wire format. Treat as JSON-RPC passthrough or write a bespoke `quai` crate.

---

## 4. Solana + non-EVM majors

| Chain | Primary Rust crate | Repo | Stars | Last commit / release | Mobile? | Notes |
|---|---|---|---|---|---|---|
| **Solana** | `solana-sdk` v4.0.1 (MSRV 1.85, 13.3M downloads) + `solana-client` + `solana-program` + `spl-token` v9 + `spl-token-2022-interface` + `spl-associated-token-account` v8.0.0 (Oct 2025) + `solana-keychain` | https://github.com/anza-xyz/solana-sdk, https://github.com/solana-program, https://github.com/anza-xyz/agave | 252 (sdk repo) | master commit **Aug 4 2026 (12 h ago)**; v4.0.1 Feb 17 2026; v3.0.0 Aug 18 2025 | Yes — MWA via [solana-mobile/mobile-wallet-adapter](https://github.com/solana-mobile/mobile-wallet-adapter) (328★, v2.2.0-nostr-beta1 Jun 8 2026) — Android Kotlin/Java, Flutter, React Native, Swift, Unity | Official Agave-forked SDK. v3 was breaking. `solana-keychain` adds Vault/Privy/Turnkey/AWS-KMS/Fireblocks/GCP-KMS/CDP/Para/Dfns backends. |
| **Tron** | `tronic` (alloy-style, gRPC + JSON-RPC, TRC-20 typed) and `tronz` (gRPC + SolidityNode, `signer-mnemonic` BIP-39/44 coin-type 195) — both community | https://github.com/39george/tronic, https://github.com/throgxyz/tronz | low (community) | tronic Jul 20 2025; tronz active (42 examples) | Partial — pure-Rust gRPC, no native C libs | No official `tronprotocol/tron-rust` is published for wallet use. `andelf/rust-tron` is a CLI daemon. **Risk: maintainers are individuals.** |
| **XRP (Ripple)** | `xrpl-rust` v1.2.0 (ISC, `#![no_std]`-capable, embassy-rt + tokio-rt, bip39, JSON-RPC + WebSocket) | https://github.com/XRPLF/xrpl-rust | 41 | **Jul 23 2026 (2 wks)** | Yes — `embassy-rt` feature for embedded/mobile | XRPLF grant winner. AMM + NFT + Payment + AccountSet builders. 186 open issues, 415 commits. Low star count is the main risk. |
| **Stellar** | `stellar-sdk` (HTTP + Horizon) for wallet; `soroban-sdk` v27.0.3 (Jul 28 2026) for contracts | soroban: https://github.com/stellar/rs-soroban-sdk (official); wallet-side `stellar-rs` is community | soroban-sdk: high; wallet Rust: low | soroban-sdk v27.0.3 **Jul 28 2026** | Soroban contracts: `wasm32v1-none` target only, Rust 1.84+ | `soroban-sdk` is **contract-side only**. Wallet-grade Rust SDK is community (`tristanltd/stellar-sdk`). SDF publishes official SDKs in JS/Java/Go/Python/Swift only. **Risk: no wallet-grade official Rust crate.** |
| **Cardano** | `cardano-multiplatform-lib` (CML, dcSpark fork of Emurgo's CSL, Rust + WASM + NPM + mobile) | https://github.com/dcSpark/cardano-multiplatform-lib, https://github.com/Emurgo/cardano-serialization-lib | 105 (CML); Emurgo CSL: ~330 | CML: **Jul 30 2026 (1 wk)**; CSL: May 25 2026 | Yes — Ionic + Capacitor or WASM bindings recommended by dcSpark | CIP-1852 derivation, native tokens, multi-era, Plutus. **CML is the actively maintained successor** (Emurgo upstream is slower). |
| **Algorand** | `algonaut` (only viable Rust SDK) | https://github.com/manuelmauro/algonaut | 70 | **Jun 12 2026 (2 mo)**; 0.9.0 pre-1.0 | Yes — wasm32 builds need no C toolchain | Pre-1.0. Full algod/kmd/indexer clients, ARC-4 + ARC-56 contract! macro, `AtomicGroupBuilder` typestate, HSM/WalletConnect Signer trait. Algorand Foundation does not maintain an official Rust SDK. |
| **Tezos** | `tezos-rust-sdk` (airgap-it / spruceid maintained: `tezos-core`, `tezos-michelson`, `tezos-operation`, `tezos-rpc`, `tezos-contract`). `tezedge` (node) effectively abandoned. | https://github.com/airgap-it/tezos-rust-sdk | low (community) | tezos-rust-sdk maintained; tezedge: "dev has ceased" | Yes — pure-Rust, no C deps | `taquito` is JS only. Tezos Foundation doesn't ship a wallet-grade Rust SDK. |
| **Hedera (HBAR)** | `hedera` v0.43.0 (renamed from `hedera-sdk` to Hiero branding) | https://github.com/hiero-ledger/hiero-sdk-rust | 58 | crate updated **Jan 8 2026**; mirror of official SDK | No native mobile bindings (mirror-node + consensus-node HTTP/gRPC) | Mirrors `hedera-sdk-java`. 70k+ downloads. Async-first Tokio. Maintained by LaunchBadge + Swirlds Labs. |
| **NEAR** | `near-api-rs` (crate `near-api`, wallet-side); `near-sdk-rs` (contract-side, `#[near]` macro) | https://github.com/near/near-api-rs, https://github.com/near/near-sdk-rs | near-api-rs: 25; near-sdk-rs: high | near-api-rs **Jul 13 2026 (3 wks)**; near-sdk-rs active | No native mobile bindings | near-api-rs is the official NEAR Foundation wallet lib; near-sdk-rs MSRV 1.93. Includes Ledger + system-keystore signers, builder pattern, FT/NFT/storage-deposit/staking. |
| **TON** | `tonlib-rs` (Ston-fi) + `ton-rs` (Ston-fi, lower-level) | https://github.com/ston-fi/tonlib-rs, https://github.com/ston-fi/ton-rs | tonlib-rs: 273 | tonlib-rs **Jun 3 2026 (2 mo)** | Yes — FFI bindings to tonlibjson native lib; **requires C lib build (libsodium, secp256k1, lz4)** | Both wrap the C++ TON `tonlibjson` via `tonlib-sys`. Heavy native dep — mobile builds need cross-compiled .a. Wallet v3/v4 supported. **5-10 MB native lib per platform.** |
| **VeChain** | `thor-devkit` (sterliakov community; **not** the official VeChain repo) | https://github.com/sterliakov/thor-devkit.rs | low | crate v0.1.0 **Apr 1 2025**; 0 dependents on crates.io | Pure-Rust, no C deps | No official VeChain team Rust SDK exists. Only wallet-grade lib is this community crate. **Risk: maintained by one person, low adoption.** |
| **Filecoin** | `forest` v0.35.0 (full node in Rust, ChainSafe) | https://github.com/ChainSafe/forest | high (~1.6k) | crate **Jul 24 2026**; very active | Possible (pure Rust) but full-node binary, not wallet-lib | `forest` is a **full node + wallet CLI** (`forest-wallet new secp256k1`), not an embeddable library. Wallet-grade embeddable Rust SDK for mobile = **effectively none**. |
| **ICP (Internet Computer)** | `ic-agent` (official DFINITY) + `ic-utils` + `icx-cert` | https://github.com/dfinity/agent-rs | 144 | **Aug 3 2026 (2 days)**; 552 commits | No native mobile bindings; runs anywhere with HTTP | DFINITY-official. `ic-agent` is for **canister-side / client-to-ICP** interaction. ICP uses **chain-key (threshold) ECDSA/Schnorr signing** — not a normal private-key wallet. To sign for Bitcoin/Ethereum/etc. from ICP, query `sign_with_ecdsa`/`sign_with_schnorr` on the management canister. **Wallet model: completely different.** |
| **Polkadot / Substrate** | `subxt` (Parity) + `subxt-signer` + `parity-scale-codec` | https://github.com/paritytech/subxt | 488 | **Aug 4 2026 (17 h ago)**; 1,271 commits, 295 releases | Yes — compiles to WASM, FFI to Node/Python | Most active project in this group. Subxt-cli downloads chain metadata, `#[subxt::subxt]` macro generates type-safe API. sr25519/ed25519 signers. Polkadot/AssetHub/Kusama/Substrate parachains. Production-quality. |

### Solanas + non-EVM majors — recommendation

**Tier 1 — adopt directly (production-grade, active, official or de-facto official):**

- **Solana** → `solana-sdk` 4.x + `solana-client` + `spl-*-interface` crates. Tangem's existing pattern is embed-and-broadcast — skip MWA unless shipping a dApp companion.
- **Polkadot** → `subxt` + `subxt-signer` + `parity-scale-codec`. Tangem-quality Rust, first-class.
- **Cardano** → `cardano-multiplatform-lib` (dcSpark). Not the older Emurgo CSL.
- **Hedera** → `hedera` 0.43 (Hiero branding).

**Tier 2 — adopt but pin and watch (community-maintained, sufficient quality):**

- **XRP** → `xrpl-rust`. XRPLF-backed, low star count but official grant.
- **NEAR** → `near-api-rs` (near-api crate). Official NEAR Foundation. Low star count.
- **Algorand** → `algonaut`. Pre-1.0 but no alternative.
- **Tezos** → `tezos-rust-sdk` (airgap-it). No official alternative.
- **TON** → `tonlib-rs` or `ton-rs`. Accept the C++ `tonlibjson` FFI dependency; budget for cross-compilation.
- **Tron** → `tronz` (typed TRC-20 facade with signer-mnemonic) or `tronic` (alloy-style). **Risk: maintainers are individuals.**

**Tier 3 — fall back to JSON-RPC passthrough (no viable Rust SDK):**

- **Stellar** wallet-side — only community `stellar-rs` exists; defer Stellar to JSON-RPC unless committing to maintaining a fork.
- **VeChain** — only `thor-devkit` 0.1.0 by one maintainer. **JSON-RPC passthrough**.
- **Filecoin** — `forest` is a node, not a wallet library. **JSON-RPC passthrough** to Lotus/Forest.
- **ICP** — `ic-agent` is the official client, but ICP's wallet model (chain-key threshold signing via canister) is fundamentally different from self-custodial Tangem. **JSON-RPC + signing-relay**; the on-device signer doesn't hold the key.

---

## 5. Move + Cosmos + Substrate

| Chain | Primary Rust crate | Repo | Stars | Last commit | crates.io | Mobile? | Notes |
|---|---|---|---|---|---|---|---|
| **Aptos** | `aptos-sdk` v0.6.0 | https://github.com/aptos-labs/aptos-rust-sdk, legacy https://github.com/aptos-labs/aptos-core | 13 / 6,438 | 2026-08-04 (new) / 2026-08-05 (legacy) | 23,374 total, 776 recent | Yes (no_std-friendly core) but full feature set pulls tokio + BCS + reqwest | v2 SDK feature-parity with TS SDK. Multi-key: ed25519, secp256k1, secp256r1, BLS12-381. Sponsored tx / fee payer / keyless (OIDC zkLogin). Bundle ~5-10 MB after feature trimming. |
| **Sui** | `sui-rust-sdk` (modular: `sui-sdk-types`, `sui-crypto`, `sui-rpc`, `sui-transaction-builder`, `sui-graphql`) | https://github.com/MystenLabs/sui-rust-sdk, legacy https://github.com/MystenLabs/sui/tree/main/crates/sui-sdk | 86 / 7,730 | 2026-07-27 (new) / 2026-08-05 (mono) | `sui-sdk-types` 0.3.2 (230k dl, 99k recent), `sui-crypto` 0.3.1 (67k dl, 25k recent), `sui-sdk` 0.0.0 (2.8k dl) | Partial — `sui-crypto` and `sui-sdk-types` are usable; full `sui-sdk` needs tokio | Skip legacy. Use `sui-sdk-types` + `sui-crypto` directly. Programmable tx blocks. zkLogin/passkey: in TS, not yet in Rust — flag a gap. |
| **Cosmos Hub** | `cosmrs` 0.22.0 | https://github.com/cosmos/cosmos-rust | 346 | 2025-09-18 (stale-ish) | 1.34M total, 182k recent | Marginal — protobuf + tendermint-rpc + grpc heavy; core tx signing is portable | Canonical Rust SDK. Bech32 + secp256k1. Bank, staking, distribution, slashing, feegrant, vesting, CosmWasm, IBC message types. |
| **Polkadot / Kusama** | `subxt` 0.50.2 + `subxt-signer` | https://github.com/paritytech/subxt | 488 | 2026-08-04 | 7.24M total, 1.25M recent | Not recommended — `subxt-cli` metadata download required, full client is ~50 MB+ | Metadata-driven. Signer supports sr25519 + ed25519. Alternative `substrate-api-client` 1.21.0 (no_std) for mobile; `apex-sdk-substrate` 0.1.6 turnkey wrapper with XCM. |
| **Koinos** | NONE OFFICIAL | ref: https://github.com/koinos/koinos-sdk-cpp (4★), https://github.com/joticajulian/koilib (11★, JS) | — | — | — | Not viable — Rust port would be from scratch | Custom VM, **canonical protobuf** (not standard libprotobuf — non-deterministic), secp256k1 over sha256 of canonical tx id, WIF keys, bech32-ish base58 addresses. Adopting Koinos in Rust = ~6 weeks from-scratch. |
| **Alephium** | `alephium-web3` 2.0.7 (community, NOT official) | https://github.com/abuvanth/alephium-web3 (crates.io owner) | low | 2025-11-01 | 45 total downloads, 8 recent | Crate is incomplete — Phase 1/5 done, signer+tx+contracts roadmap unfinished | Reference impl is Scala. The maintained SDK is TypeScript. Custom sUTXO DAG model (per-shard/group index, not global). Blake2b prefixed messages. Schnorr/ECDSA. Ralph is a custom VM. **LGPL-3.0** is license-incompatible with most mobile app stores without dynamic linking. |

### Move + Cosmos + Substrate — recommendation

| Chain | Verdict | Rationale |
|---|---|---|
| **Aptos** | **ADOPT** | `aptos-sdk` 0.6.0 on crates.io is official, multi-sig ready, sponsored tx + keyless supported. Use `default-features = false, features = ["ed25519"]` for Tangem's ed25519-only flow. |
| **Sui** | **ADOPT** (modular) | Skip legacy. `sui-sdk-types` + `sui-crypto` directly. ~250k monthly downloads. zkLogin/passkey unsupported in Rust — flag a gap. JSON-RPC client layer needs Tangem-side code or pull `sui-rpc`. |
| **Cosmos Hub** | **ADOPT** | Only mature Rust Cosmos SDK. 1.3M downloads. Mobile bloat from `tendermint-rpc` + `cosmos-sdk-proto` — feature-gate to tx-only. |
| **Polkadot/Kusama** | **SKIP Rust on mobile** | Substrate on Rust mobile is rough. Polkadot.js is the working path. If forced: `substrate-api-client` (no_std) over `subxt`. Don't ship parachain metadata generation on-device. |
| **Koinos** | **DEFER** | No Rust SDK exists. Canonical proto serialization, secp256k1, WIF — all doable in Rust but ~6 weeks from-scratch. Recommend deferring until community Rust port appears. |
| **Alephium** | **DEFER / MONITOR** | `alephium-web3` Rust crate is a 45-download community port, Phase 2/5 incomplete, LGPL-3.0 risky for mobile. Reference is Scala + TS. Track abuvanth's roadmap; revisit in 6 months. |

### Custom Rust work required if any deferred chains are forced

1. **Koinos** — port `koinos-proto` to Rust with **canonical serialization** (not standard protobuf), then build tx-builder + secp256k1 signer + base58 address codec. ~6 weeks for one engineer.
2. **Alephium** — build proper Rust SDK by either (a) extending abuvanth's port under MIT/Apache OR (b) porting `alephium/api` Scala types and reusing `k256` + `blake2b` crates. DAG UTXO selection logic is the hard part.
3. **Sui zkLogin/passkey** — TS has implementations; Rust has neither. If Tangem needs these flows, ~3-4 weeks porting the JWT/JWK proof verification + secp256r1 signature scheme.
4. **Substrate parachain metadata** — if Tangem ever ships a parachain, runtime metadata must be bundled or fetched. Not a Rust SDK problem — an architectural one.

---

## 6. Long-tail chains

| Chain | Rust crate (if exists) | Status | Decision |
|---|---|---|---|
| **Chia** | `chia-sdk-driver` + `chia-sdk-coinset` + `chia-protocol` (xch-dev/chia-wallet-sdk) | Mature, actively maintained 2026-07, mobile-friendly (WASM bindings), 9k+ org stars | **Adopt** — full native Rust tx build/signing |
| **Fact0rn** | none | C++ node + Python miner | **Passthrough** — `rust-bitcoincore-rpc` against factornd RPC |
| **Ducatus** | none (legacy). DUCX is separate EVM-on-BSC token | Legacy chain dormant; new project is DUCX (ERC-20 on BSC) | **Passthrough** — Rust just builds BTC-like envelope; for DUCX use EVM framework as plain ERC-20 |
| **Pepecoin** | `nintondo/rust-pepecoin` (rust-bitcoin fork) | 970 dl, 1 dependent, 2025-09 | **Adopt** — fork of rust-bitcoin, fits UTXO pattern |
| **Clore** | none. Token migrated to ERC-20 on Ethereum (2025-12) | Legacy PoW chain deprecated | **Passthrough** — handle as ERC-20 on Ethereum |
| **Xodex** | none | No SDK, no public RPC docs | **Passthrough** — JSON-RPC only |
| **Ravencoin** | none native; `bitcoin-explorer-raven` is decode-only (no signing) | Reference is C++; rust-bitcoin could be forked but no maintained fork exists | **Passthrough** — RVN JSON-RPC; minor rust-bitcoin patch only if assets needed |
| **Radiant** | `rxd-wasm` (BSV.WASM fork, Rust → WASM) | 0 stars, 4 contributors, last push 2024-05 | **Passthrough** — JSON-RPC; if needed, fork `rxd-wasm` (it builds tx) |
| **Decimal** | `cosmrs` (cosmos-rust, generic Cosmos SDK) | Mature; Decimal is Cosmos SDK fork so tx format matches with msg-type swaps | **Adopt via cosmrs** — custom msg-type bindings for decimal-chain |
| **XDC** | `xdc3_rust` (tiny) + ethers-rs works (EVM, chain ID 50) | XDC is fully EVM (modern geth-based); only difference is chain ID + gas model | **Adopt via EVM framework** — set chain ID 50, custom fee estimator; ignore `xdc3_rust` |
| **RSK** | none native; **official RSK dev tutorial uses Alloy** | RSK is EVM-compatible, official path is `alloy-rs` | **Adopt via Alloy** |
| **Chiliz** | none native; ethers-rs / Alloy works (BSC fork, EVM, PoSA) | EVM-compatible, chain ID 88888 | **Adopt via EVM framework** |
| **Quai** | none — Go-only reference (`go-quai`, 2.4k stars) | Quai is custom sharded EVM-like, NOT Ethereum-compatible at the wire level | **Passthrough** — JSON-RPC only; signing path is unique, no Rust SDK exists |
| **Mantle** | none native; ethers-rs works (EVM L2, optimistic rollup) | Fully EVM; only quirk is L1-data-fee in `estimateTotalGasCost` | **Adopt via EVM framework** |
| **Scroll** | none native; ethers-rs works (Rust in repo is for zkEVM provers, not wallet SDK) | zkEVM = byte-equivalent EVM; chain ID 534352 mainnet | **Adopt via EVM framework** |
| **Hyperliquid** | `quicknode-hyperliquid-sdk` (QuikNode Labs) + `hyperliquid-rust-sdk` community forks | Active 2026-02; covers order building, signing, builder fees, priority fees | **Adopt** — only Rust SDK with chain-specific signing logic (builder fees, priority fees, HIP-1 deploys) |

### Long-tail recommendation

- **First-class Rust signing paths (3):** Chia, Pepecoin, Hyperliquid.
- **Free wins via EVM framework (5):** XDC, RSK, Chiliz, Mantle, Scroll.
- **Adopt via cosmrs (1):** Decimal.
- **JSON-RPC passthrough (6):** Fact0rn, Ducatus (legacy), Clore, Xodex, Ravencoin, Quai.
- **Net result: 9 chains get real Rust signing paths, 6 are passthrough-only.** ~60/40 rewrite-to-passthrough ratio is healthy — the 6 passthroughs all fit a single trait shape `{ sign_locally: false, tx_envelope_from_rpc: true, broadcast: true }`, and only **Hyperliquid** genuinely requires chain-specific Rust code that can't be replicated via JSON-RPC.

---

## 7. Master verdict matrix

| # | Chain | Family | Tier 1 SDK | Tier 2 SDK | Verdict | Tangem current dep | Replace with |
|---|---|---|---|---|---|---|---|
| 1 | Bitcoin (BTC) | UTXO | `bdk_wallet` 3.1.0 + `rust-bitcoin` 0.32 | — | **ADOPT** | `BitcoinDevKit` (bdk-swift 2.3.1) + `TangemWalletCoreBinariesWrapper` | `bdk_wallet` 3.1.0 + `rust-bitcoin` 0.32 |
| 2 | Bitcoin Cash (BCH) | UTXO | `bitcoincash` (gitlab fork) | — | **ADOPT** | `TangemWalletCoreBinariesWrapper` | `bitcoincash` crate |
| 3 | Litecoin (LTC) | UTXO | `litecoin` 0.32.8-rc.1 | — | **ADOPT** | `TangemWalletCoreBinariesWrapper` | `litecoin` crate (rust-bitcoin fork) |
| 4 | Dogecoin (DOGE) | UTXO | `easydoge-km` (best mobile fit, UniFFI) | `dfinity/rust-dogecoin` | **ADOPT** | `TangemWalletCoreBinariesWrapper` | `easydoge-km` for key mgmt + `rust-bitcoin` primitives |
| 5 | Dash (DASH) | UTXO | `dashpay/rust-dashcore` | — | **ADOPT** | `TangemWalletCoreBinariesWrapper` | `rust-dashcore` (mobile FFI ready) |
| 6 | Kaspa (KAS) | UTXO | `rusty-kaspa` 1.1.0 | — | **ADOPT** | `TangemWalletCoreBinariesWrapper` (custom) | `rusty-kaspa` + `kaspa-wasm` |
| 7 | Ducatus | UTXO | none | — | **PASSTHROUGH** | custom | JSON-RPC to existing node |
| 8 | Pepecoin | UTXO | `nintondo/rust-pepecoin` | — | **ADOPT** | custom | rust-bitcoin fork |
| 9 | Clore | UTXO | none | — | **PASSTHROUGH** | custom | JSON-RPC |
| 10 | Xodex | UTXO | none | — | **PASSTHROUGH** | (Kaspa-derived) | JSON-RPC |
| 11 | Fact0rn | UTXO | none | — | **PASSTHROUGH** | custom | `rust-bitcoincore-rpc` |
| 12 | Ravencoin | UTXO | none | — | **PASSTHROUGH** | custom | JSON-RPC |
| 13 | Radiant | UTXO | none (fork `rxd-wasm` if needed) | — | **PASSTHROUGH** | custom | JSON-RPC |
| 14 | Ethereum mainnet | EVM | `alloy` 1.x | `ethers-rs` 2.x | **ADOPT via EVM** | `TangemWalletCoreBinariesWrapper` | `alloy` 1.x |
| 15-50 | All other EVM (36+ chains) | EVM | `alloy` 1.x | — | **ADOPT via EVM** | `TangemWalletCoreBinariesWrapper` | `alloy` 1.x (chain ID + custom fee model) |
| 51 | Solana | Solana | `solana-sdk` 4.0.1 | — | **ADOPT** | `Solana.Swift` 1.2.0-tangem19 | `solana-sdk` 4.0.1 + `spl-*-interface` |
| 52 | Tron | Non-EVM major | `tronz` (typed TRC-20) | `tronic` (alloy-style) | **ADOPT w/ caution** | custom | `tronz` |
| 53 | XRP | Non-EVM major | `xrpl-rust` 1.2.0 | — | **ADOPT** | custom | `xrpl-rust` |
| 54 | Stellar | Non-EVM major | none (community `stellar-rs` only) | — | **PASSTHROUGH** | `stellarsdk` 3.1.0-tangem1 | JSON-RPC to Horizon |
| 55 | Cardano | Non-EVM major | `cardano-multiplatform-lib` | — | **ADOPT** | custom | dcSpark CML |
| 56 | Algorand | Non-EVM major | `algonaut` 0.9.0 | — | **ADOPT** | custom | `algonaut` |
| 57 | Tezos | Non-EVM major | `tezos-rust-sdk` | — | **ADOPT** | custom | `tezos-rust-sdk` |
| 58 | Hedera (HBAR) | Non-EVM major | `hedera` 0.43.0 | — | **ADOPT** | `Hiero` 0.49.0-tangem4 | `hedera` 0.43.0 (Hiero branding) |
| 59 | NEAR | Non-EVM major | `near-api-rs` | — | **ADOPT** | custom | `near-api-rs` |
| 60 | TON | Non-EVM major | `tonlib-rs` | — | **ADOPT (note 5-10MB native)** | `ton-swift` 1.0.17-tangem1 | `tonlib-rs` (FFI to tonlibjson) |
| 61 | VeChain | Non-EVM major | `thor-devkit` (one maintainer) | — | **PASSTHROUGH** | custom | JSON-RPC |
| 62 | Filecoin | Non-EVM major | `forest` (node, not wallet lib) | — | **PASSTHROUGH** | custom | JSON-RPC to Lotus/Forest |
| 63 | ICP | Non-EVM major | `ic-agent` (chain-key signing) | — | **PASSTHROUGH** | `IcpKit` 0.1.2-tangem5 | JSON-RPC + signing-relay |
| 64 | Polkadot + parachains | Substrate | `subxt` 0.50.2 (heavy on mobile) | `substrate-api-client` 1.21.0 (no_std) | **ADOPT (Tangem-quality)** | custom | `subxt` or `substrate-api-client` |
| 65 | Aptos | Move | `aptos-sdk` 0.6.0 | — | **ADOPT** | custom | `aptos-sdk` 0.6.0 |
| 66 | Sui | Move | `sui-rust-sdk` modular (`sui-sdk-types` + `sui-crypto`) | — | **ADOPT** | custom | `sui-rust-sdk` modular crates |
| 67 | Cosmos (Hub, Kava, Cronos, Telos, Octa, TerraV1/V2) | Cosmos | `cosmrs` 0.22.0 | — | **ADOPT** | custom | `cosmrs` 0.22.0 |
| 68 | Koinos | Custom | NONE | — | **DEFER** | custom | (6-week from-scratch) |
| 69 | Alephium | Custom | `alephium-web3` 2.0.7 (LGPL, Phase 2/5) | — | **DEFER** | custom | monitor abuvanth |
| 70 | Chia | Long-tail | `chia-sdk-driver` + `chia-sdk-coinset` | — | **ADOPT** | custom | xch-dev Wallet SDK |
| 71 | Hyperliquid | Long-tail | `quicknode-hyperliquid-sdk` | — | **ADOPT** | custom | QuickNode SDK (note vendor lock) |
| 72 | Casper | Custom | (out of research scope, lower priority) | — | **PASSTHROUGH** (assumed) | custom | — |
| 73 | Binance (legacy) | Custom | (out of scope) | — | **PASSTHROUGH** | `BinanceChain` 0.0.18 | — |

**Totals:** 95+ chains, 70 documented. ~50 ADOPT, 12 PASSTHROUGH, 2 DEFER, balance = other/variants.

---

## 8. Cross-cutting caveats

1. **Hyperliquid's QuickNode SDK is vendor-locked** (hosted RPC). Confirm QuickNode is acceptable before adopting; otherwise fall back to `info` + `exchange` JSON-RPC which loses the typed builder-fee/priority-fee ergonomics.

2. **Alephium's `alephium-web3` crate is LGPL-3.0** — license-incompatible with most mobile app stores without dynamic linking. The community crate is also Phase 2/5 complete (signer+tx+contracts missing). Defer adoption until either the project adopts MIT/Apache or Tangem forks it.

3. **TON's FFI native-lib cost is ~5-10 MB per platform** (libsodium, secp256k1, lz4 from `tonlibjson`). For a hardware-wallet app with strict binary size budgets, prefer JSON-RPC to a node running `tonlibjson` server-side rather than embedding it.

4. **Sui zkLogin/passkey not in Rust yet** — TS has implementations; Rust has neither. If Tangem ships zkLogin flows, ~3-4 weeks porting work.

5. **Stellar wallet-side has no official Rust SDK** — SDF publishes in JS/Java/Go/Python/Swift. Community `stellar-rs` exists but is small. Defer or accept the maintenance burden.

6. **Polkadot/Substrate mobile story is rough** — `subxt` + `subxt-cli` metadata generation is heavy. For Polkadot-only, `apex-sdk-substrate` is a turnkey wrapper with XCM. For parachains, runtime metadata must be bundled.

7. **ICP wallet model is fundamentally different** — chain-key threshold signing via the management canister. The on-device signer doesn't hold the key. Treat as a remote-signer pattern, not a self-custodial pattern.

8. **Koinos canonical protobuf** — standard `prost`/libprotobuf serialization is non-deterministic and breaks signing. Custom canonical serializer required. Non-trivial from-scratch.

9. **Tangem's hardware signing lives in `TangemSdk` Swift, not BlockchainSdk.** The Rust rewrite only handles tx building, address generation, fee estimation, broadcast, and chain state. The `TangemSdk` boundary stays Swift-side: host signs hash bytes, Rust attaches signature to the tx.

10. **Star counts on the `bdk_wallet` repo (50) are low** because the crate moved to its own repo in 2025-04 — the ecosystem traffic is on the `bdk` monorepo (1,057★). The active development is on `bdk_wallet`.

---

## 9. Forward-looking: what this enables

Per user direction, **the long-term goal is a full rewrite of `BlockchainSdk` in Rust.** This research is the input for that rewrite. Concrete next steps (NOT in scope for this doc, listed for future planning):

- Decide FFI strategy (Mozilla UniFFI is the leading candidate — proven in Breez SDK, LDK-node, Spark SDK; supports Swift + Kotlin + Python + Flutter from one UDL/proc-macro set).
- Decide API compat strategy (drop-in wrapper vs clean-break Rust-native API).
- Decide phase plan: which chains go in v1 of the Rust rewrite (likely BTC + EVM + Solana = ~70% of wallet usage) vs deferred.
- Resolve Hyperliquid vendor lock, TON native lib size, Alephium license before locking architecture.
- Pick the FFI error-handling pattern (thiserror + `#[derive(uniffi::Error)]` is the standard).
- Pick the iOS packaging pipeline (`cargo-xcodebuild` vs manual `xcodebuild -create-xcframework`).

Each of these is a separate, scoped follow-up decision.

---

## 10. Companion notes

- Existing prior research in this repo covers Lightning and wallet apps. See:
  - `docs/lightning/2026-08-05-ldk-rust-core.md`
  - `docs/lightning/2026-08-05-ldk-for-wallet-apps.md`
  - `docs/lightning/2026-08-05-rust-lightning-sdks-mobile.md`
  - `docs/wallets/2026-08-05-rust-core-wallet-apps.md`
  - `docs/2026-08-04-bcs-bitcoin.md` + `2026-08-04-bcs-bitcoin-implementation-reference.md` (bcs-bitcoin v1 reference, complements the BTC row above)
- Methodology precedent: per-chain table with crate, repo, stars, last commit, status, mobile fit, notes. End with explicit recommendation.
- This doc is research only. No rewrite architecture design. No API contract. Implementation plan is a separate future deliverable.
