# Architecture: Multi-Chain Rust Wallet App (`rust-wallet-app`)

**Date:** 2026-08-06
**Status:** Draft — umbrella spec, supersedes nothing, extends Bitcoin single-chain spec
**Scope:** Phase 2 of `BlockchainSdk` Rust rewrite. Multi-chain umbrella. Replaces per-chain modules in `tangem-app-ios/Modules/BlockchainSdk/Blockchains/`.
**Source research:** [`../../blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md`](../../blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md) (commit `0c20f77`)
**Source Bitcoin spec:** [`2026-08-05-rust-bitcoin-wallet-design.md`](2026-08-05-rust-bitcoin-wallet-design.md)
**Source Bitcoin arch:** [`2026-08-06-rust-bitcoin-wallet-architecture.md`](2026-08-06-rust-bitcoin-wallet-architecture.md)
**Source plan (Bitcoin):** [`../plans/2026-08-05-rust-bitcoin-wallet.md`](../plans/2026-08-05-rust-bitcoin-wallet.md)
**Source ADR (signing):** [`../../wallets/2026-08-05-adr-0001-signing-model.md`](../../wallets/2026-08-05-adr-0001-signing-model.md)
**Source comparison:** [`../../wallets/2026-08-05-tangem-vs-btc-wallet-comparison.md`](../../wallets/2026-08-05-tangem-vs-btc-wallet-comparison.md)

---

## 1. System overview

**`rust-wallet-app`** = umbrella Rust workspace that hosts per-chain wallet modules, replacing Tangem iOS's `tangem-app-ios/Modules/BlockchainSdk/` (95+ chains, 46 source dirs, 105,000 Swift LOC, 1,187 files per research §1).

**Target shape:** Cargo workspace `rust-wallet-app/`. One crate per chain family. **`bitcoin-wallet-core`** (committed v0.1) becomes the v1.0 first child of the umbrella. **`rust-wallet-app`** itself is a thin orchestrator that:

1. Owns a single shared mnemonic (one seed → many coins via BIP-44 coin-type).
2. Dispatches `ChainId` to the matching per-chain crate.
3. Provides cross-chain UI: balance aggregation, address book, tx history across all chains.
4. Wraps per-chain signing behind a common `Signer` trait (per ADR 0001).
5. Holds no chain-specific state — each per-chain crate owns its own DB, signer, RPC client.

**Out of scope (deferred to follow-up specs):** 95-chain reach (per-chain opt-in below), hardware-wallet integration (Phase 3 via UniFFI per ADR 0001), FFI surface for iOS (Phase 3), Lightning (separate spec), staking/yield flows (separate spec).

---

## 2. Chain family taxonomy

Per the Tangem research (§1-§6), 95+ chains fall into 4 family patterns + long-tail. The umbrella inherits the family boundaries — each family maps to a distinct SDK pattern + signing curve + address format.

| Family | Model | Signing curve | Address format | Representative chains |
|---|---|---|---|---|
| **UTXO** (Bitcoin family) | UTXO set, script-based, transaction in/out | secp256k1 (ECDSA, Schnorr) | Bech32/Bech32m, Base58 | BTC, BCH, LTC, DOGE, DASH, KAS, Pepecoin |
| **EVM** (Ethereum family) | Account-based, EVM bytecode, sequential nonce | secp256k1 (ECDSA, EIP-7702) | EIP-55 hex, ICAP | ETH, Polygon, BSC, Arbitrum, Optimism, Base, ~50 chains |
| **Solana + non-EVM majors** | Account-based, parallel runtime, per-instruction signer | ed25519 (Solana, XRP, NEAR, Aptos, Sui, Algorand); sr25519 + ed25519 (Polkadot); BLS12-381 (Aptos multi-key) | Base58+ed25519 pubkey, Bech32m (Cardano), chain-specific | SOL, XRP, NEAR, ADA, ALGO, XTZ, HBAR, DOT, SUI, APT, ALPH |
| **Move + Cosmos** | Account-based, message-passing | ed25519 (Sui, Aptos), secp256k1 (Cosmos) | Bech32, normalized hash | Sui, Aptos, Cosmos Hub, Koinos |
| **Long-tail** | Various | Various | Various | ~6 chains with no viable Rust SDK (per research §6: **Fact0rn, Ducatus, Clore, Xodex, Ravencoin, Quai**) |

**In-scope v1 (umbrella-first cut):** BTC + ETH + SOL. Three families, three SDK patterns, one chain each. Validates the umbrella pattern before scaling to 95+ chains.

**Recommended rollout (per chain maturity + dependency risk):**

| Release | Chains | Crates |
|---|---|---|
| **v0.1** (already done) | BTC | `bdk_wallet` 3.1, `rust-bitcoin` 0.32, `bdk_esplora` 0.22, `bdk_file_store` 0.15 |
| **v0.2** (umbrella cut) | + ETH + SOL | `alloy` 1.x, `solana-sdk` 4.x |
| **v0.3** | + LTC + DOGE + BCH | `rust-litecoin` 0.32, `nintondo/rust-dogecoin`, `bitcoincash` 0.32 |
| **v0.4** | + DASH + KAS | `dashpay/rust-dashcore`, `rusty-kaspa` |
| **v0.5** | + Aptos + Sui + Cardano + Hedera | `aptos-sdk`, `sui-rust-sdk`, `cardano-multiplatform-lib`, `hedera` |
| **v1.0** | + ICP + Stellar + others | JSON-RPC passthrough per research §6 |
| **v1.5** | + XRP + NEAR + Algorand + Tezos + TON + Tron | `xrpl-rust`, `near-api-rs`, `algonaut`, `tezos-rust-sdk`, `tonlib-rs` (C dep), `tronz` |
| **v2.0** | + Pepecoin + remaining long-tail | `nintondo/rust-pepecoin`, JSON-RPC passthrough |

**Long-tail deferral:** Per research §6, Stellar wallet-side, VeChain, Filecoin, ICP, Ravencoin, Radiant, Clore, Ducatus, Fact0rn, Xodex = JSON-RPC passthrough only. No first-class Rust SDK. Umbrella treats them as `ChainKind::Passthrough` with the same trait surface but a custom RPC adapter.

---

## 3. Component model

### 3.1 Layered view

```text
┌────────────────────────────────────────────────────────────────────┐
│ HOST LAYER (not in this repo)                                       │
│   iOS (UniFFI) | Android (UniFFI) | CLI | test harness                │
└────────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────────┐
│ UMBRELLA (rust-wallet-app) — single source of truth for cross-chain │
│   ┌──────────────┬──────────────┬──────────────┬──────────────┐     │
│   │  Mnemonic    │  Signer      │  AddressBook │  History     │     │
│   │  (shared)    │  trait       │  (per-chain  │  (cross-     │     │
│   │              │              │   alias map) │   chain)     │     │
│   └──────────────┴──────────────┴──────────────┴──────────────┘     │
└────────────────────────────────────────────────────────────────────┘
                          │       │       │       │
                          ▼       ▼       ▼       ▼
┌────────────────────────────────────────────────────────────────────┐
│ PER-CHAIN CRATES (one per chain family / chain)                      │
│   bitcoin-wallet-core (v0.1 done)                                  │
│   ethereum-wallet-core  (v0.2)  → alloy                           │
│   solana-wallet-core    (v0.2)  → solana-sdk                       │
│   litecoin-wallet-core  (v0.3)  → rust-litecoin                    │
│   dogecoin-wallet-core  (v0.3)  → nintondo/rust-dogecoin           │
│   ... (50+ more, see §2 table)                                     │
└────────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────────┐
│ SHARED SIGNER TRAIT (per ADR 0001)                                  │
│   v0.1: software signing (per-chain Keypair)                        │
│   v1.0: hardware via UniFFI (iOS host provides TangemSdk impl)      │
└────────────────────────────────────────────────────────────────────┘
```

### 3.2 Dependency rules

- **No upward deps.** Per-chain crates never import `rust-wallet-app`. Umbrella composes per-chain crates; per-chain crates don't know about umbrella.
- **No sideways deps.** Two per-chain crates never import each other. Cross-chain logic (e.g., swap BTC → ETH) lives in the umbrella, not in either per-chain crate.
- **Per-chain owns its own state.** Mnemonic file is shared (read-only after unlock). Each per-chain crate owns its own SQLite, RPC client, signer.
- **Single shared trait crate (proposed):** `chain-traits` defines the `ChainWallet` trait all per-chain crates implement. Lives at workspace root. No per-chain crate depends on another.

### 3.3 Crate layout (proposed)

```text
rust-wallet-app/                    (workspace root)
├── Cargo.toml                      (workspace)
├── crates/
│   ├── chain-traits/               (defines ChainWallet trait)
│   ├── bitcoin-wallet-core/        (v0.1 done — already committed)
│   ├── ethereum-wallet-core/        (v0.2)
│   ├── solana-wallet-core/          (v0.2)
│   ├── litecoin-wallet-core/        (v0.3)
│   ├── dogecoin-wallet-core/        (v0.3)
│   ├── bitcoincash-wallet-core/     (v0.3)
│   ├── dash-wallet-core/            (v0.4)
│   ├── kaspa-wallet-core/           (v0.4)
│   ├── polkadot-wallet-core/        (v0.5)
│   ├── cardano-wallet-core/         (v0.5)
│   ├── hedera-wallet-core/          (v0.5)
│   ├── sui-wallet-core/             (v0.5)
│   ├── aptos-wallet-core/           (v0.5)
│   ├── xrp-wallet-core/             (v1.5)
│   ├── near-wallet-core/            (v1.5)
│   ├── algorand-wallet-core/        (v1.5)
│   ├── tezos-wallet-core/           (v1.5)
│   ├── ton-wallet-core/             (v1.5) — 5-10 MB native lib
│   ├── tron-wallet-core/            (v1.5)
│   ├── rust-wallet-app/             (umbrella, this repo's deliverable)
│   ├── rust-wallet-cli/             (clap CLI like `btc` but multi-chain)
│   └── rust-wallet-server/          (REST, deferred v2 per Bitcoin spec)
```

---

## 4. Common trait surface (the `ChainWallet` trait)

The umbrella defines one trait every per-chain crate implements. Mirrors the Bitcoin `Wallet` API (§3 of Bitcoin arch) but generic.

```rust
// crates/chain-traits/src/lib.rs
use async_trait::async_trait;

pub trait ChainWallet: Send + Sync {
    type Address: Display + FromStr;
    type Amount: Copy + PartialOrd + Default;
    type Txid: Display + FromStr + Eq + Hash;
    type Balance;
    type FeeEstimate;
    type UnsignedTx: Serialize + DeserializeOwned;
    type SignedTx: Serialize + DeserializeOwned;

    /// Construction
    async fn from_mnemonic(m: &Secret<Mnemonic>, config: &ChainConfig) -> Result<Self>
    where Self: Sized;
    async fn open(config: &ChainConfig, db: &Path) -> Result<Self>
    where Self: Sized;

    /// Read (all sync; async I/O is the caller's responsibility)
    fn network(&self) -> ChainNetwork;
    fn balance(&self) -> Result<Self::Balance>;
    fn address(&self) -> Result<Self::Address>;
    fn transactions(&self) -> Result<Vec<TransactionRecord<Self::Txid, Self::Address, Self::Amount>>>;
    fn fee_estimate(&self) -> Result<Self::FeeEstimate>;

    /// Write — note: `&self` not `&mut self`. Per-chain crate uses interior
    /// mutability (e.g., `std::sync::Mutex<BdkWallet>` for BTC, `tokio::sync::Mutex`
    /// for ETH/SOL) to satisfy `Sync` for the umbrella's `Arc<dyn ChainWallet>`.
    /// Lock-across-await discipline: per-chain crate MUST release the lock
    /// before any `.await` (mirrors Bitcoin arch §6 invariant).
    async fn sync(&self) -> Result<()>;
    fn build_tx(&self, params: TxParams<Self::Address, Self::Amount>) -> Result<Self::UnsignedTx>;
    fn sign(&self, tx: Self::UnsignedTx) -> Result<Self::SignedTx>;
    async fn broadcast(&self, tx: &Self::SignedTx) -> Result<Self::Txid>;
}
```

**Why `&self` (not `&mut self`):** `Arc<dyn ChainWallet>` is `Sync` (umbrella callers hold the Arc across multiple operations). `&mut self` on `dyn ChainWallet` is not dyn-compatible (Rust 1.85). The trade-off: per-chain crate wraps state in interior `Mutex` instead of relying on the trait method's `&mut`. Cost: one extra `Mutex` per chain crate. Benefit: trait can be `dyn`-safe, no per-call Arc clone.

**Why `Amount: Copy + PartialOrd + Default` (not `Into<u64>`):** ETH amounts are wei (u256) — far exceeds u64. Solana lamports fit u64 but SPL token decimals (some >9) don't. Trait says "ordered + default"; per-chain implements conversion to its native type (u64 for BTC, U256 for ETH, u64 for SOL lamports, u64 for SPL with decimals shifted).

**Why `UnsignedTx`/`SignedTx` (not `PsbtLike`):** PSBT is BIP-174, BTC-only. ETH uses RLP-encoded typed transactions; SOL uses Solana's `Transaction`. Per-chain associated types let each chain return its native type. `Serialize + DeserializeOwned` bounds let the umbrella persist / inspect without knowing the type.

**Why `async_trait`:** `Arc<dyn ChainWallet>` requires dyn-compatibility. Native `async fn in trait` (Rust 1.75+) is NOT dyn-compatible. `async_trait` macro provides dyn-safe async methods. Trade-off: one `Box::pin` per future per call. Acceptable for the umbrella's per-chain dispatch surface.

**Why a trait, not a sum type:** Each per-chain crate has wildly different state (UTXO set vs account state). Sum type (e.g., `enum ChainWallet { Btc(BitcoinWallet), Eth(EthereumWallet) }`) requires every call site to `match`, which leaks the per-chain shape into umbrella callers. A trait + `dyn ChainWallet` keeps per-chain types opaque. v0.1 `dyn` overhead is acceptable; v0.2 can switch to type-erased `Any` if needed.

**Why a trait per release:** v0.1 (Bitcoin) doesn't need this. v0.2 (umbrella) introduces it. Bitcoin's `Wallet` struct can be retro-fitted to implement `ChainWallet` with minimal changes (one impl block + type aliases).

---

## 5. State management

### 5.1 Umbrella-level state (cross-chain)

```rust
pub struct WalletApp {
    mnemonic: Secret<Mnemonic>,                    // shared single seed (BIP-39)
    chain_wallets: RwLock<HashMap<ChainId, Arc<dyn ChainWallet>>>,
    address_book: AddressBook,                     // per-chain alias map
    cross_chain_history: CrossChainHistory,        // optional aggregator
}
```

- One `Secret<Mnemonic>` shared across all chains (BIP-44 coin-type derivation).
- Each chain has its own `ChainWallet` instance (separate state, separate DB).
- `RwLock<HashMap>` for the map (reads frequent, writes rare — add/remove chain at runtime).

### 5.2 Per-chain state (existing Bitcoin spec §4.1)

Each per-chain crate inherits the single-chain pattern from Bitcoin arch §4.1:

```rust
pub struct BitcoinWallet {           // bitcoin-wallet-core
    bdk: std::sync::Mutex<BdkWallet>,
    esplora: EsploraClient,
    config: WalletConfig,
}
```

Per Ethereum:

```rust
pub struct EthereumWallet {           // ethereum-wallet-core
    inner: Mutex<alloy::providers::Provider>,    // alloy state
    signer: Arc<dyn Signer>,                    // per ADR 0001
    config: EthConfig,
}
```

Per Solana:

```rust
pub struct SolanaWallet {             // solana-wallet-core
    rpc: solana_client::RpcClient,
    signer: Arc<dyn Signer>,
    config: SolConfig,
}
```

### 5.3 Multi-chain persistence

```text
data_dir/
├── mnemonic.enc               # v0.2: Argon2id + AES-256-GCM (shared across chains)
├── btc/
│   └── bdk.sqlite             # bdk_file_store (BDK ships persistence)
├── eth/
│   └── eth_index.sled         # custom sled index keyed by block+tx hash
├── sol/
│   └── sol_index.redb         # custom redb index of signatures per address
└── address_book.json          # umbrella-level cross-chain alias map
```

**Storage model:** BDK ships `bdk_file_store` for BTC. **Alloy and solana-sdk do NOT include persistent storage** (research §3 / §4) — they are pure RPC + signing libraries. The umbrella must ship per-chain tx-history + balance index layers. v0.2 picks:
- ETH: `sled` (per research: "embedded, pure Rust, no C deps", fast for key-value)
- SOL: `redb` (per research: "embedded, ACID, pure Rust")

Both are umbrella-level deps, used by `ethereum-wallet-core` and `solana-wallet-core` respectively. Alternative: each per-chain crate ships its own index. Decision deferred to implementation; current default is per-crate-ships-own to keep chain crates self-contained.

Single mnemonic file (shared) + per-chain SQLite/sled/redb DBs (isolated). Mnemonic loaded once at app start by the umbrella; passed to each per-chain `from_mnemonic` as `&Secret<Mnemonic>`. Each chain derives its own key tree from the shared seed using its chain's standard (see §7.5 below).

**Derivation standard per chain:**

| Chain | Standard | Coin type | Notes |
|---|---|---|---|
| BTC | BIP-44 | 0 | m/84'/0'/0' (default Native SegWit per Bitcoin arch) |
| ETH | BIP-44 | 60 | m/44'/60'/0'/0/0 |
| SOL | BIP-44 (SLIP-0044 extension) | 501 | m/44'/501'/0'/0' |
| LTC | BIP-44 | 2 | m/84'/2'/0' |
| DOGE | BIP-44 | 3 | m/44'/3'/0' |
| BCH | BIP-44 | 145 | m/44'/145'/0' |
| DASH | BIP-44 | 5 | m/44'/5'/0' |
| KAS | (TBD) | 111111 (testnet) | rusty-kaspa uses custom derivation; spec team must confirm before v0.4 |
| ADA | CIP-1852 | 1815 | m/1852'/1815'/0'/0/0 (NOT BIP-44) |
| DOT | SLIP-0010 + Substrate | 354 | NOT BIP-44; uses sr25519 + custom path |
| XRP | BIP-44 + ed25519 | 144 | m/44'/144'/0'/0/0 (BIP-44 path, ed25519 curve) |
| NEAR | BIP-44 + ed25519 | 397 | m/44'/397'/0' (SLIP-0010 derivation in practice) |
| SUI | SLIP-0010 | 784 | ed25519 |
| APT | SLIP-0010 | 637 | ed25519 (BIP-44 path, ed25519 curve) |
| HBAR | SLIP-0010 | 3030 | ed25519 |
| ALGO | BIP-44 | 283 | m/44'/283'/0'/0/0 (BIP-44 path, ed25519 curve) |
| XTZ | BIP-44 + ed25519 (BIP-32-Ed25519) | 1729 | Ed25519-HD (BIP32-Ed25519) |
| TON | (TBD) | 607 | TON uses custom v3r2/v4 wallet standard; spec team to confirm |
| ICP | (chain-key) | n/a | Threshold ECDSA via management canister; no seed-derived key |

**Implementation rule:** per-chain `from_mnemonic` owns the derivation path for its chain. The umbrella does not assume BIP-44 universally. Per-chain `ChainConfig` carries the derivation path string (or a chain-specific enum).

### 5.4 Concurrency

- `WalletApp.chain_wallets: RwLock<HashMap<...>>` — outer lock.
- `Arc<dyn ChainWallet>` per entry — shared, multi-thread.
- Per-chain `std::sync::Mutex` (for `!Send` types like `BdkWallet`) or `tokio::sync::Mutex` (for `Send` types like `alloy::Provider`) — per-chain crate decides.
- Lock-ordering rule: outer (`chain_wallets`) → inner (per-chain mutex). Never reversed. Two-lock acquisition only at umbrella API surface.

---

## 6. Data flow (universal send)

The send flow is family-dependent. Three universal stages, family-specific implementations:

```text
                ┌──────────────────────────┐
                │ STAGE 1 — BUILD          │
                │   Inputs: UTXOs/account  │
                │   Outputs: recipient +   │
                │           fee + change   │
                │   Family-specific:       │
                │     UTXO: TxBuilder      │
                │     EVM: alloy tx       │
                │     SOL: solana-tx      │
                └──────────────────────────┘
                              │ unsigned tx
                              ▼
                ┌──────────────────────────┐
                │ STAGE 2 — SIGN           │
                │   Family-specific:       │
                │     UTXO: PSBT + ECDSA   │
                │     EVM: typed tx + ECDSA│
                │     SOL: msg + ed25519  │
                │   Signer trait per ADR   │
                └──────────────────────────┘
                              │ signed tx
                              ▼
                ┌──────────────────────────┐
                │ STAGE 3 — BROADCAST      │
                │   Family-specific:       │
                │     UTXO: Esplora /tx    │
                │     EVM: eth_sendRawTx   │
                │     SOL: sendTransaction│
                └──────────────────────────┘
```

The umbrella exposes one `WalletApp::send(chain_id, recipient, amount, fee) -> Result<Txid>` that dispatches to the right per-chain implementation. The host (iOS, CLI) doesn't see family differences.

---

## 7. Storage architecture

### 7.1 Mnemonic (shared, cross-chain)

Per ADR 0001 + mnemonic-handling-decision:

| Version | Format | Encryption |
|---|---|---|
| v0.2 | `magic(4) \|\| ver(1) \|\| salt(16) \|\| nonce(12) \|\| ciphertext(N+16)` | Argon2id (m=256 MiB, t=10, p=4) + AES-256-GCM |
| v1.0 | iOS Keychain (via Swift host) | Swift `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` |

Loaded once at app start by the umbrella. Umbrella passes `&Secret<Mnemonic>` to each per-chain `from_mnemonic` constructor (one Argon2id unlock per app start, ~500ms per mnemonic-handling-decision wall-clock calibration). Per-chain crate clones the secret's bytes (or holds a reference + derives without copying); the secret's `ZeroizeOnDrop` runs when the umbrella's `Secret<Mnemonic>` is dropped at app exit. Per-chain `from_mnemonic` MUST NOT clone the secret outside its own scope.

**Unlock lifetime:** Argon2id unlock on app start. `Secret<Mnemonic>` lives in process memory for the app session. v0.2 re-prompts after lock-screen inactivity (configurable, default 15 min). Cleared on graceful shutdown. v1.0 moves unlock to Swift side via iOS Keychain.

### 7.2 Per-chain databases

Each per-chain crate owns its own SQLite DB under `data_dir/{chain_id}/`. Format varies per SDK:
- BTC: `bdk_file_store` (existing).
- ETH: alloy's storage (or custom).
- SOL: `solana-sdk` storage (or custom).

DB isolation = blast-radius reduction. A corrupted ETH DB doesn't affect BTC.

### 7.3 Address book (umbrella-level)

`address_book.json` — per-chain alias map. Shared across all chains. Syncs to iOS Contacts if `ContactIntegration` feature enabled (deferred v2).

### 7.4 Cross-chain history (umbrella-level, optional v1.0)

Aggregator view: list transactions across all chains sorted by timestamp. Each per-chain crate exposes `transactions()`; umbrella collects + merges. Indexed by `Txid` (which is chain-specific; prefix with `ChainId`).

---

## 8. Security model (per ADR 0001, extended to multi-chain)

| Layer | v0.2 | v0.5 | v1.0 |
|---|---|---|---|
| **Signing** | Software per-chain: secp256k1 (BTC, ETH), ed25519 (SOL, SUI, APT) | + sr25519 (DOT), + Ed25519-HD (XTZ), + BLS12-381 (APT multi-key) | Hardware via UniFFI per chain (iOS host provides TangemSdk impl) |
| **Mnemonic at rest** | Argon2id 256 MiB / 500ms + AES-256-GCM (shared file) | Same | iOS Keychain (Swift-side, shared) |
| **Mnemonic in memory** | `Secret<Mnemonic>` + `ZeroizeOnDrop` | + `mlock` (Unix) | Swift `Data` + `kSecAttrAccessible` |
| **In-process key per chain** | Raw `Keypair` (or per-chain equivalent) | Same | Never in process (signing on card per chain) |
| **Network** | Testnet default per chain; mainnet opt-in | Same | Mainnet per chain |
| **Threat model coverage** | "Stolen disk image" | + per-chain audit | + "Coercion" + "In-memory extraction" |

**Per-release × per-curve signing matrix** (locks the "some" handwave from prior draft):

| Chain | v0.2 | v0.5 | v1.0 | Curve |
|---|---|---|---|---|
| BTC | secp256k1 | secp256k1 | secp256k1 (Tangem card) | secp256k1 |
| ETH | secp256k1 | secp256k1 | secp256k1 (Tangem card) | secp256k1 |
| SOL | ed25519 | ed25519 | ed25519 (Tangem card) | ed25519 |
| ADA | — | ed25519 (BIP32-Ed25519) | ed25519 | ed25519 |
| HBAR | — | ed25519 | ed25519 | ed25519 |
| DOT | — | sr25519 | sr25519 | sr25519 |
| SUI | — | ed25519 | ed25519 | ed25519 |
| APT | — | ed25519 (single-sig) + BLS12-381 (multi-sig) | same | ed25519 / BLS12-381 |
| Others | — | TBD per-chain | TBD | TBD |

**Shared `Signer` trait** (defines the v0.5+ signer interface for multi-curve):

```rust
pub trait Signer: Send + Sync {
    fn curve(&self) -> CurveId;
    fn sign(&self, msg: &[u8]) -> Result<Signature, SignError>;
    fn public_key(&self) -> PublicKeyBytes;
}

pub enum CurveId { Secp256k1, Ed25519, Sr25519, Bls12381 }

pub struct Signature { pub curve: CurveId, pub bytes: Vec<u8> }
```

The `CurveId` tag lets the umbrella and per-chain crates identify which signature scheme to apply (e.g., EIP-191 signing prefix for ETH ECDSA vs Solana's `SigningDomain::Sign` prefix for ed25519). Each curve has its own canonical signature byte length (secp256k1 = 64, ed25519 = 64, sr25519 = 64, BLS12-381 = 96).

v1.0 hardware via UniFFI: the iOS host (TangemSdk) provides one `Signer` impl per curve. Per-chain crate wraps the host-provided Signer with its own curve-aware prefix logic. Multi-curve support is built into the Tangem card itself (Tangem supports secp256k1 + ed25519 + BLS12-381; sr25519 is Polkadot-specific and may need a separate signer).

**Migration triggers:**
- v0.2 → v0.5: add multi-chain support; mnemonic + v0.1 hygiene from Bitcoin carries over unchanged. Shared `Signer` trait introduced with per-curve impls.
- v0.5 → v1.0: hardware-backed per chain via UniFFI; per-curve Signer impls swap to TangemSdk-backed. `sign_with_external_signer(&psbt, &impl Signer)` per chain (mirrors Bitcoin plan Task 28).

---

## 9. Key design decisions (locked)

| # | Decision | Rationale | Rejected | Source |
|---|---|---|---|---|
| 1 | **Umbrella owns no chain state** | Each per-chain crate owns its DB + signer; umbrella only composes. Reduces coupling, enables independent chain rollout. | Sum type (`enum ChainWallet`) — leaks per-chain types into callers. Direct dispatch (no trait) — requires `match` per call site. | spec §3.2; research §1 architecture pattern (Tangem uses `WalletManagerAssembly` dispatch) |
| 2 | **Common `ChainWallet` trait** | One surface for host; per-chain crate provides the impl. Host doesn't see family differences. | Sum type: leak + match everywhere. | spec §4 |
| 3 | **Single shared mnemonic across chains** | BIP-44 coin-type derivation is the standard multi-coin model. One seed = all chains. | Per-chain mnemonic: user must manage N seeds. Per-chain xprv file: same problem. | spec §5.1; ADR 0001 |
| 4 | **Per-chain SQLite DB** | Blast-radius isolation. Per-chain schema independent. | Shared DB: corruption spreads. Single-file storage: schema migration risk across families. | spec §7.2 |
| 5 | **v1 cut: BTC + ETH + SOL** | One UTXO + one Account-EVM + one Account-non-EVM. Three families, three SDK patterns, minimum viable umbrella. | All 95 chains at once: scope explosion. BTC only: doesn't test multi-family dispatch. | spec §2; research §2-§6 |
| 6 | **alloy 1.x for EVM** | Modern Foundry-backed successor. Modular, mobile-friendly. 50+ EVM chains covered. | ethers-rs 2.x: legacy, losing mindshare. Bespoke per EVM chain: scope explosion. | research §3 |
| 7 | **solana-sdk 4.x for Solana** | Official Agave fork. 13M downloads, weekly commits. Mobile + MWA. | Bespoke SOL client: re-implementing official work. solana-program (on-chain only): wrong layer. | research §4 |
| 8 | **bdk_wallet 3.1 for BTC only**; BCH/LTC/DOGE use rust-bitcoin-fork crates | BDK 3.x is BTC-specific. BCH = `bitcoincash` 0.32 (CashTokens + SIGHASH_FORKID); LTC = `rust-litecoin` 0.32; DOGE = `nintondo/rust-dogecoin` (Scrypt + AuxPow). All are rust-bitcoin 0.32 forks sharing ~80% of the BTC type system. | Single `bdk_wallet` covers all UTXO: research §2 explicitly says BDK 3.x is BTC-only. | research §2 |
| 9 | **Long-tail = JSON-RPC passthrough** | ~6 chains with no viable Rust SDK. Don't block 95% of users on 10% of chains. | Bespoke SDK per chain: 6 weeks per chain, indefinite maintenance. | research §6 |
| 10 | **Single `Signer` trait per chain** | Per ADR 0001 v0.1 software / v1.0 hardware. Same pattern in multi-chain. | Per-chain concrete signer: harder to swap implementations. Per-SDK signer: leaks SDK types. | ADR 0001; spec §8 |
| 11 | **CLI: one binary `btc` already exists, multi-chain CLI = `wallet`** | Bitcoin `btc` CLI is the v0.1 single-chain cut. Multi-chain `wallet` CLI (v0.2) wraps all chains. | Per-chain CLI binary per chain: N binaries to install, distribute, learn. | Bitcoin plan Task 16 |
| 12 | **No FFI in v0.2; UniFFI in v1.0** | FFI surface is Phase 3 (mobile migration). v0.2 = CLI + library only. | UniFFI in v0.2: pulls in Swift binding generation, iOS host code, complicates CI. | Bitcoin plan Global Constraint; research §1 (Tangem is iOS app, not the rewrite target) |
| 13 | **No server in v0.2** | Same as Bitcoin: server deferred. `rust-wallet-server` placeholder only. | Server in v0.2: REST auth + per-chain endpoint design = scope creep. | Bitcoin arch §9 row 1 |
| 14 | **MIT license, matching all deps** | Per Bitcoin decision. No copyleft. | Apache-2.0: no differentiator. | spec §13.2 (Bitcoin) |
| 15 | **MSRV 1.85** | Matches Bitcoin + BDK + solana-sdk MSRV. | Lower MSRV: drops modern Rust features. | Bitcoin plan; research |
| 16 | **Testnet default per chain** | Extends Bitcoin testnet default. Mainnet opt-in per chain. | Mainnet default: v0.1 unsafe + testnet chains have no mainnet. | Bitcoin arch §9 |
| 17 | **v0.2 umbrella scope = 3 chains** (BTC + ETH + SOL) | Validates umbrella pattern with minimum surface. Defers 47+ chains. | All chains at v0.2: scope too large for first cut. | spec §2 |
| 18 | **Per-chain signing curve** (secp256k1 vs ed25519 vs sr25519) | Different families need different curves. Per-chain `Signer` trait abstracts. | Single curve (secp256k1): doesn't fit Solana/Aptos/Sui. | research §2-§5 |

---

## 10. Cross-cutting concerns

### 10.1 Error handling

Per-chain crate defines its own `Error` enum (thiserror). Umbrella defines a top-level `AppError` enum that uses a generic `Chain(ChainId, Box<dyn Error + Send + Sync>)` variant instead of one variant per chain. Avoids 95-variant explosion when more chains ship.

```rust
#[derive(thiserror::Error)]
pub enum AppError {
    #[error("chain {0:?} error: {1}")]
    Chain(ChainId, Box<dyn std::error::Error + Send + Sync>),
    #[error("chain not enabled: {0:?}")]
    ChainNotEnabled(ChainId),
    #[error("mnemonic error: {0}")]
    Mnemonic(String),
    #[error("cross-chain error: {0}")]
    CrossChain(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("storage error: {0}")]
    Storage(String),
}
```

Per-chain errors are boxed + downcast at boundaries (host or CLI) when chain-specific error handling matters. CLI uses `anyhow` for `main`; library returns `Result<T, AppError>`. Cross-cutting variants (Mnemonic, CrossChain, Config, Storage) handle the 80% case; the boxed `Chain` catches the 20%.

### 10.2 Logging

`tracing` per Bitcoin arch §10.2. Per-chain crate uses `tracing::info!` at module boundaries. No sensitive data (mnemonic, xprv, private key) ever logged. Umbrella adds a per-chain context span: `tracing::info_span!("chain", id = ?chain_id)`.

### 10.3 Configuration

`config.toml` (umbrella-level) lists enabled chains + per-chain overrides:

```toml
[chains.btc]
network = "testnet"
esplora_url = "https://blockstream.info/testnet/api"

[chains.eth]
network = "sepolia"
rpc_url = "https://ethereum-sepolia-rpc.publicnode.com"

[chains.sol]
network = "devnet"
rpc_url = "https://api.devnet.solana.com"
```

`Config::load()` reads + validates + dispatches to per-chain `ChainConfig::default()`.

### 10.4 Dependency policy

- `cargo deny check` in CI: no copyleft, no unmaintained, no advisories.
- `cargo +nightly udeps` in slow lane.
- MSRV = 1.85.
- `cargo miri test` for soundness on per-chain crates that use `unsafe` (mostly none — secp256k1 FFI is the only FFI).

---

## 11. Crate footprint (initial v0.2 cut)

| Crate | Version | Why | Source |
|---|---|---|---|
| **Shared** | | | |
| `chain-traits` | (this repo) | Defines `ChainWallet` trait | spec §4 |
| `bip39` | (re-exported) | Mnemonic via `bdk_wallet::keys::bip39` (feature `keys-bip39`) | Bitcoin arch §11 |
| `bip32` | 0.6 | HD derivation (BIP-44 coin-type) — direct dep needed (BDK doesn't re-export) | research |
| `thiserror` | 1 | Per-chain + umbrella errors | spec §10.1 |
| `anyhow` | 1 | CLI only | spec §10.1 |
| `tracing` | 0.1 | Logging | spec §10.2 |
| `async-trait` | 0.1 | Async trait support (until Rust 1.75 native async traits) | spec §4 |
| `serde` + `serde_json` | 1 | Address book + config | spec §10.3 |
| `clap` | 4 | CLI | spec §10.3 |
| `zeroize` | 1 | `Secret<Mnemonic>` zeroize | mnemonic-handling-decision |
| `argon2` + `aes-gcm` | 0.5 / 0.10 | v0.2 mnemonic encryption | mnemonic-handling-decision |
| **Bitcoin** | | | |
| `bdk_wallet` | 3.1 | BTC core | Bitcoin spec |
| `bdk_esplora` | 0.22 | BTC chain source | Bitcoin spec |
| `bdk_file_store` | 0.15 | BTC SQLite | Bitcoin spec |
| `bitcoin` | 0.32 | BTC primitives | Bitcoin spec |
| **Ethereum** | | | |
| `alloy` | 1.x | EVM primitives + provider + signer | research §3 |
| `alloy-provider` | 1.x | RPC + failover | research §3 |
| `alloy-signer-local` | 1.x | Software signing v0.2 | research §3 |
| **Solana** | | | |
| `solana-sdk` | 4.x | SOL primitives + signer | research §4 |
| `solana-client` | 4.x | RPC client | research §4 |
| `spl-token` | 9 | SPL token (SOL stablecoins, NFTs) | research §4 |

**Net v0.2 direct deps (umbrella + 3 chain crates):** ~35 (rough estimate; will recalc after Bitcoin plan lands in v0.2).

---

## 12. Open questions

| # | Question | Decision path | Status |
|---|---|---|---|
| 1 | Per-chain feature flag pattern? | Each chain crate behind a workspace feature (`chain-btc`, `chain-eth`, `chain-sol`). v0.2 enables all three. v0.5+ per chain. | Open |
| 2 | Single `Signer` trait per chain or per family? | Per chain is simpler; per family reduces trait count but requires blanket impls. Recommend per chain. | Open |
| 3 | Mnemonic file format: v0.2 single-file with multi-coin flag, or one file per chain? | Single file (shared seed). Per-chain = N seeds. Decision: single file. | Decided (spec §5.1) |
| 4 | Address book: JSON file, SQLite, or per-chain embedded? | JSON file (umbrella-level). Lightweight, no migration needed. | Decided (spec §7.3) |
| 5 | Cross-chain history: aggregate on read or eager-index? | Aggregate on read. Each chain's `transactions()` is already persisted; umbrella calls N chain methods on demand. | Decided (spec §7.4) |
| 6 | Hardware signing per chain: one UniFFI trait or per-chain trait? | **Decided** (spec §8): one shared `Signer` trait with `CurveId` dispatch. Host provides one impl per curve (not per chain). Per-chain crate wraps the curve-impl with chain-specific signing prefix logic. | Open |
| 7 | Cross-chain swap (BTC → ETH) in scope? | Not in v0.2. Defer to v1.0+ (requires DEX integration, separate spec). | Deferred |
| 8 | Staking/yield in scope? | Not in v0.2. Defer to v1.0+ (chain-specific, separate specs per chain). | Deferred |
| 9 | ICP chain-key signing model? | Treat as JSON-RPC passthrough per research §4. Umbrella uses `ChainKind::Passthrough` variant. | Decided (spec §2 long-tail) |
| 10 | TON native lib cross-compilation CI? | 5-10 MB native lib per platform. CI must cross-compile libsodium + secp256k1 + lz4. v1.5 — defer CI work until v1.5 scope. | Deferred |

---

## 13. Verification plan

| Release | Tests | Source |
|---|---|---|
| **v0.2** (umbrella + ETH + SOL) | `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test`; regtest for each chain family (BTC regtest + ETH anvil + SOL test validator); cross-chain integration (load mnemonic → derive keys for all 3 chains → check addresses match reference vectors) | research §1 architecture pattern + Bitcoin spec §9 |
| **v0.3** | + LTC + DOGE + BCH; per-chain regtest + cross-chain | research §2 |
| **v0.4** | + DASH + KAS; SPV sync for DASH, full-node for KAS | research §2 |
| **v0.5** | + Polkadot + Aptos + Sui + Cardano + Hedera; per-family mock chain tests | research §4-§5 |
| **v1.0** | + miri; + e2e FFI tests (iOS host); + cross-chain swap prototype (BTC → SOL via Jupiter); + property-based tests on `ChainWallet` trait | spec §8 + research §1 |
| **v1.5** | + ICP + Stellar + XRP + NEAR + others | research §6 |
| **v2.0** | + Pepecoin + remaining long-tail; + TON native lib CI | research §6 |

Cross-cutting: property-based tests on `ChainWallet` trait surface (per release). Each per-chain crate implements a `chain-traits::test::MockChain` for non-production testing.

---

## 14. References

### Canonical source documents

- **This spec's source research:** [`../../blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md`](../../blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md) (commit `0c20f77`)
- **Bitcoin spec (the v0.1 first child):** [`2026-08-05-rust-bitcoin-wallet-design.md`](2026-08-05-rust-bitcoin-wallet-design.md)
- **Bitcoin arch (this spec's template):** [`2026-08-06-rust-bitcoin-wallet-architecture.md`](2026-08-06-rust-bitcoin-wallet-architecture.md)
- **Bitcoin plan:** [`../plans/2026-08-05-rust-bitcoin-wallet.md`](../plans/2026-08-05-rust-bitcoin-wallet.md)
- **ADR 0001 (signing model):** [`../../wallets/2026-08-05-adr-0001-signing-model.md`](../../wallets/2026-08-05-adr-0001-signing-model.md)
- **Mnemonic decision:** [`../../wallets/2026-08-05-mnemonic-handling-decision.md`](../../wallets/2026-08-05-mnemonic-handling-decision.md)
- **Tangem vs Bitcoin comparison:** [`../../wallets/2026-08-05-tangem-vs-btc-wallet-comparison.md`](../../wallets/2026-08-05-tangem-vs-btc-wallet-comparison.md)

### External

- Tangem iOS source (read-only research material): `tangem-app-ios/Modules/BlockchainSdk/`
- BIP-44 (multi-coin HD derivation): https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki

### Crate registries (verify current versions before pinning)

- [bdk_wallet](https://crates.io/crates/bdk_wallet) | [alloy](https://github.com/alloy-rs/alloy) | [solana-sdk](https://crates.io/crates/solana-sdk) | [subxt](https://github.com/paritytech/subxt)

---

## 15. Revision log

| Date | Change | Source |
|---|---|---|
| 2026-08-06 | Initial draft. Synthesizes Tangem SDK research (chain list + per-chain SDK choices), Bitcoin spec/arch (umbrella template + v0.1 first child), ADR 0001 (multi-chain signing model). | All listed in §14 |
| 2026-08-06 | Applied ecc:architect review. 6 criticals fixed: (1) Decision 8 — BDK = BTC only; BCH/LTC/DOGE use rust-bitcoin-fork crates. (2) `Amount` bound loosened from `Into<u64>` to `Copy + PartialOrd + Default` (ETH wei = u256). (3) `sync` signature changed to `&self` + interior mutability (Arc<dyn> dyn-compat). (4) `PsbtLike` removed — replaced with `UnsignedTx`/`SignedTx` associated types (Serialize+DeserializeOwned). (5) Multi-curve `Signer` trait defined with `CurveId` enum + per-release × per-curve matrix. (6) `AppError` changed from 95-variant to generic `Chain(ChainId, Box<dyn Error>)`. 5 majors: corrected long-tail list (Fact0rn/Ducatus/Clore/Xodex/Ravencoin/Quai), removed Polkadot from v0.5 (research §5 "skip Rust on mobile"), fixed async_trait comment (dyn-compat not Rust 1.75), added chain→derivation-standard table (CIP-1852 for ADA, SLIP-0010 for SUI/APT, BIP-44+ed25519 for XRP/NEAR/ALGO/XTZ), specified mnemonic read model (umbrella-owned). | ecc:architect review (aa5a1f1e689135da7) |
