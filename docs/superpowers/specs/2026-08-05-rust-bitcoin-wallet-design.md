# Design: Standalone Rust Bitcoin Wallet Core (Phase 1 of BlockchainSdk rewrite)

**Date:** 2026-08-05
**Status:** Draft — awaiting user approval
**Scope:** Phase 1 of the full BlockchainSdk Rust rewrite. Covers **only** the Bitcoin module replacement. Other UTXO chains (BCH, LTC, DOGE, DASH, KAS) ship in later specs reusing the same crate family.

---

## 1. Goal & non-goals

### Goal

Replace `tangem-app-ios/Modules/BlockchainSdk/Blockchains/Bitcoin/` (~2,070 Swift LOC, 20 files) with a **standalone Rust Bitcoin wallet** — no Swift host, no `TangemSdk`, no hardware-card integration. Rust owns the entire stack: BIP-39 mnemonic, BIP-32 derivation, secp256k1 signing, transaction building, UTXO selection, broadcast, fee estimation, and chain state.

The deliverable is a Rust **workspace** with three crates:

1. **`bitcoin-wallet-core`** (library) — reusable, host-agnostic wallet engine.
2. **`btc`** (CLI binary) — manual smoke-test + dev tool.
3. **`btc-server`** (REST server) — HTTP interface for non-Rust consumers (curl, any HTTP client).

### Non-goals (explicit deferral)

- **No hardware-wallet integration** (no Tangem card, no Ledger, no Trezor). Pure software signing.
- **No multi-sig** (single-sig only in v1; `miniscript` is in the dependency tree so v2 is straightforward).
- **No Lightning** (separate spec).
- **No other UTXO chains** (BCH/LTC/DOGE/DASH/KAS get their own specs after this lands).
- **No mobile/FFI target** (this spec is library + CLI + server only; UniFFI/Swift binding is a later spec).
- **No watch-only / read-only mode in v1** (add later; trivial — pass `Option<Mnemonic>`).
- **No silent payments / ZK features** in v1 (BIP-352 deferred).
- **No mempool.space or external oracle** in v1 (fee estimation from mempool + Esplora fallback).

---

## 2. Crate layout (workspace)

```text
bitcoin-wallet-rs/                    (workspace root)
├── Cargo.toml                        (workspace)
├── README.md
├── LICENSE                           (MIT)
├── crates/
│   ├── bitcoin-wallet-core/          (library — primary deliverable)
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── wallet/                # Wallet, WalletBuilder, WalletConfig
│   │   │   │   ├── mod.rs
│   │   │   │   ├── builder.rs
│   │   │   │   ├── sync.rs            # BDK sync logic
│   │   │   │   ├── balance.rs
│   │   │   │   └── addresses.rs
│   │   │   ├── tx/                    # Transaction building
│   │   │   │   ├── mod.rs
│   │   │   │   ├── builder.rs         # BDK TxBuilder wrapper
│   │   │   │   ├── psbt.rs            # PSBT v2 handling
│   │   │   │   ├── sighash.rs         # Sighash construction (rust-bitcoin)
│   │   │   │   └── fee.rs             # Fee estimation (Esplora tiers)
│   │   │   ├── keys/                  # Key management
│   │   │   │   ├── mod.rs
│   │   │   │   ├── mnemonic.rs        # BIP-39
│   │   │   │   ├── derivation.rs      # BIP-32/44/49/84/86
│   │   │   │   └── signer.rs          # secp256k1 Signer trait impl
│   │   │   ├── script/                # Bitcoin script
│   │   │   │   ├── mod.rs
│   │   │   │   ├── builder.rs         # ScriptBuilder
│   │   │   │   ├── parser.rs
│   │   │   │   └── opcodes.rs         # OP_CODE enum (rust-bitcoin already has)
│   │   │   ├── address/               # Address encoding
│   │   │   │   ├── mod.rs
│   │   │   │   ├── legacy.rs          # P2PKH / P2SH
│   │   │   │   ├── segwit.rs          # Bech32 / Bech32m
│   │   │   │   └── taproot.rs
│   │   │   ├── chain/                 # Network + RPC
│   │   │   │   ├── mod.rs
│   │   │   │   ├── network.rs         # Network enum (mainnet/testnet/regtest/signet)
│   │   │   │   ├── esplora.rs         # bdk_esplora client
│   │   │   │   ├── electrum.rs        # bdk_electrum client (fallback)
│   │   │   │   └── rpc.rs             # JSON-RPC error handling
│   │   │   ├── error.rs               # top-level Error enum (thiserror)
│   │   │   └── config.rs              # WalletConfig struct
│   │   └── tests/                     # Integration tests (regtest)
│   │       ├── regtest.rs
│   │       └── vectors.rs
│   ├── btc/                            # CLI binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       └── commands/              # clap subcommands
│   │           ├── mod.rs
│   │           ├── wallet.rs          # create, import, list
│   │           ├── address.rs         # new, list
│   │           ├── balance.rs
│   │           ├── send.rs
│   │           ├── tx.rs              # history
│   │           └── fee.rs
│   └── btc-server/                     # REST server
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── routes/
│           │   ├── mod.rs
│           │   ├── wallet.rs
│           │   ├── address.rs
│           │   ├── tx.rs
│           │   └── fee.rs
│           ├── state.rs                # AppState (one Wallet per wallet_id)
│           └── openapi.rs              # utoipa-generated OpenAPI spec
├── tests/
│   └── e2e/                            # REST server end-to-end (reqwest)
│       └── api.rs
├── docker/
│   ├── Dockerfile
│   └── docker-compose.yml              # regtest bitcoind + esplora
├── .github/
│   └── workflows/
│       ├── ci.yml                      # cargo test, clippy, fmt, deny
│       └── release.yml                 # cross-compile + publish
└── deny.toml                           # cargo-deny config
```

---

## 3. Core dependencies (Cargo.toml)

```toml
[workspace.dependencies]
# Bitcoin primitives
bdk_wallet        = "3.1"          # wallet, tx building, UTXO selection
bdk_chain         = "3.1"          # chain index
bdk_esplora       = "0.22"         # Esplora HTTP backend
bdk_electrum      = "0.24"         # Electrum fallback
bdk_kyoto         = "0.3"          # BIP-157/158 client (optional, mobile-grade)
rust-bitcoin      = "0.32"         # Script, PSBT, primitives
rust-secp256k1    = "0.30"         # signing
rust-miniscript   = "12"           # descriptor policy (v2 prep)
bip39             = "2"            # mnemonic
bip32             = "0.6"          # HD derivation (or use rust-bitcoin's key_expression)

# Async + HTTP
tokio             = { version = "1", features = ["full"] }
reqwest           = { version = "0.12", features = ["json", "rustls-tls"] }

# Error handling
thiserror         = "1"
anyhow            = "1"            # CLI only

# Serialization
serde             = { version = "1", features = ["derive"] }
serde_json        = "1"
toml              = "0.8"

# Logging
tracing           = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# CLI
clap              = { version = "4", features = ["derive"] }

# Server
axum              = "0.7"
tower             = "0.5"
tower-http        = { version = "0.6", features = ["trace", "cors"] }
utoipa            = "5"            # OpenAPI generation
utoipa-swagger-ui = "8"

# Testing
proptest          = "1"
mockall           = "0.13"
```

MSRV: **1.85** (matches BDK 3.x).

---

## 4. Module architecture

### 4.1 `bitcoin-wallet-core` library

Public API surface (top-level):

```rust
// Public types
pub struct Wallet { /* opaque */ }
pub struct WalletBuilder { /* opaque */ }
pub struct WalletConfig {
    pub network: Network,                       // mainnet | testnet | regtest | signet
    pub descriptor: Descriptor<DescriptorPublicKey>,  // BDK descriptor (BIP-44/49/84/86)
    pub esplora_url: String,
    pub electrum_url: Option<String>,           // fallback
}
pub struct AddressInfo {
    pub address: Address,
    pub derivation_path: DerivationPath,
    pub index: u32,
    pub used: bool,
}
pub struct Balance {
    pub confirmed: u64,                         // satoshis
    pub unconfirmed: i64,                       // can be negative (spent)
    pub immature: u64,
}
pub struct TransactionRecord {
    pub txid: Txid,
    pub received: u64,
    pub sent: u64,
    pub fee: Option<u64>,
    pub confirmation_time: Option<u64>,         // block height
}
pub struct FeeEstimate {
    pub fastest: FeeRate,
    pub half_hour: FeeRate,
    pub hour: FeeRate,
    pub economy: FeeRate,
    pub minimum: FeeRate,
}
pub enum AddressType { Legacy, NestedSegwit, NativeSegwit, Taproot }

// Error type — top-level, no anyhow in the library
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    #[error("invalid derivation path: {0}")]
    InvalidDerivationPath(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("esplora error: {0}")]
    Esplora(String),
    #[error("electrum error: {0}")]
    Electrum(String),
    #[error("insufficient funds: needed {needed} sat, have {available} sat")]
    InsufficientFunds { needed: u64, available: u64 },
    #[error("transaction build error: {0}")]
    TxBuild(String),
    #[error("signing error: {0}")]
    Sign(String),
    #[error("psbt error: {0}")]
    Psbt(String),
    #[error("address derivation error: {0}")]
    AddressDerivation(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not initialized: {0}")]
    NotInitialized(String),
    #[error(transparent)]
    Bitcoin(#[from] bitcoin::consensus::encode::Error),
    #[error(transparent)]
    Bdk(#[from] bdk_wallet::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
```

`Wallet` API (immutable where possible; mut only for `sync`, `new_address`):

```rust
impl Wallet {
    // Construction
    pub fn builder() -> WalletBuilder;
    pub async fn from_mnemonic(mnemonic: &Mnemonic, passphrase: &str, config: WalletConfig) -> Result<Self>;
    pub async fn from_descriptor(descriptor: &str, network: Network, esplora_url: &str) -> Result<Self>;
    pub async fn open(config: WalletConfig, db: &Path) -> Result<Self>;  // persisted wallet

    // Read operations
    pub fn network(&self) -> Network;
    pub fn balance(&self) -> Result<Balance>;
    pub fn address(&self, address_type: AddressType) -> Result<AddressInfo>;
    pub fn new_address(&mut self, address_type: AddressType) -> Result<AddressInfo>;
    pub fn addresses(&self) -> Result<Vec<AddressInfo>>;
    pub fn transactions(&self) -> Result<Vec<TransactionRecord>>;
    pub fn fee_estimate(&self) -> Result<FeeEstimate>;

    // Write operations
    pub async fn sync(&mut self) -> Result<()>;                    // full chain sync
    pub fn build_tx(&self, params: TxParams) -> Result<Psbt>;     // returns PSBT
    pub fn sign(&self, psbt: &mut Psbt) -> Result<Transaction>;   // sign + finalize
    pub async fn broadcast(&self, tx: &Transaction) -> Result<Txid>;
    pub fn bump_fee(&self, txid: &Txid, new_rate: FeeRate) -> Result<Psbt>;
}
```

### 4.2 Derivation paths supported (BIP-44/49/84/86)

| Path | Address type | Use case |
|---|---|---|
| `m/44'/0'/0'/0/x` | Legacy (P2PKH/P2SH) | legacy wallets |
| `m/49'/0'/0'/0/x` | Nested SegWit (P2SH-P2WPKH) | backward-compat |
| `m/84'/0'/0'/0/x` | Native SegWit (P2WPKH) | default modern |
| `m/86'/0'/0'/0/x` | Taproot (P2TR) | newest |

Default for new wallets: `m/84'/0'/0'` (BIP-84). Taproot opt-in.

### 4.3 PSBT flow

`build_tx` returns a **BIP-174 PSBT v2** (rust-bitcoin `Psbt` type). For v1 the v1 spec is fine; v2 preferred for forward-compat.

`sign` takes `&mut Psbt` — Rust signs internally (no external signer) because hardware signing is explicitly out of scope.

`broadcast` posts the finalized `Transaction` to Esplora `/tx` endpoint.

---

## 5. Data flow

### 5.1 Create wallet

```text
CLI:    $ btc wallet create --network testnet --type native-segwit
  ↓
WalletBuilder::with_mnemonic(Mnemonic::generate(12)?)
  ↓
Descriptor::new_bip84(secp256k1, key_expression, network)?
  ↓
WalletConfig { network, descriptor, esplora_url }
  ↓
Wallet::from_mnemonic() {
  1. BDK Wallet::new(descriptor, *, network)
  2. Persist to disk (SQLite via bdk_file_store)
  3. Initial sync from esplora
}
  ↓
Print: "Wallet created. Address: tb1q..."
```

### 5.2 Send transaction

```text
CLI:    $ btc send --to tb1q... --amount 0.001 --fee fastest
  ↓
wallet.sync()?  // ensure chain state is fresh
  ↓
wallet.build_tx(TxParams {
    recipients: vec![(address, Amount::from_sat(100_000))],
    fee_rate: FeeRate::from_sat_per_vb(20),
}) -> Psbt
  ↓
wallet.sign(&mut psbt)?  // internal secp256k1 signing
  ↓
wallet.broadcast(&tx)?  // POST to Esplora
  ↓
Print: "Sent. txid: abc..."
```

### 5.3 REST flow

```text
POST /v1/wallets
{ "mnemonic": "word1 word2 ...", "passphrase": "", "network": "testnet", "address_type": "native-segwit" }
→ 201 { "wallet_id": "uuid", "address": "tb1q..." }

GET /v1/wallets/{id}/balance
→ 200 { "confirmed": 12345, "unconfirmed": 0, "immature": 0 }

POST /v1/wallets/{id}/sync
→ 200 { "synced_to_height": 2500123, "tx_count": 42 }

POST /v1/wallets/{id}/tx
{ "to": "tb1q...", "amount_sat": 100000, "fee_rate_sat_per_vb": 20 }
→ 200 { "txid": "abc..." }
```

---

## 6. REST API (axum + utoipa)

OpenAPI spec generated via `utoipa::ToSchema` derive on all DTOs. Swagger UI at `/swagger-ui/`. Versioned at `/v1/`.

Endpoints:

| Method | Path | Purpose |
|---|---|---|
| POST | `/v1/wallets` | Create wallet (mnemonic OR descriptor) |
| GET | `/v1/wallets` | List wallet IDs in this server |
| GET | `/v1/wallets/{id}` | Wallet metadata (network, descriptor fingerprint, address) |
| DELETE | `/v1/wallets/{id}` | Forgets wallet (data removed from server state — does NOT delete on-chain) |
| GET | `/v1/wallets/{id}/balance` | Confirmed + unconfirmed |
| GET | `/v1/wallets/{id}/addresses` | All derived addresses |
| POST | `/v1/wallets/{id}/addresses` | Generate new address (advances index) |
| GET | `/v1/wallets/{id}/txs` | Transaction history (paginated) |
| POST | `/v1/wallets/{id}/sync` | Force a chain sync |
| POST | `/v1/wallets/{id}/tx` | Build + sign + broadcast (all-in-one) |
| POST | `/v1/wallets/{id}/tx/build` | Build PSBT only (returns base64 PSBT) — for inspection |
| POST | `/v1/wallets/{id}/tx/broadcast` | Broadcast a pre-built tx hex |
| POST | `/v1/wallets/{id}/tx/bump-fee` | RBF (returns new PSBT) |
| GET | `/v1/fee-estimate` | Current fee rates (Esplora) |

**Authentication in v1:** none. Server is local-only by design. v2: bearer token via `tower-http::auth`.

**State persistence:** wallets kept in memory + on disk under `data_dir/{wallet_id}/` (SQLite via `bdk_file_store`). Server can restart; wallets survive.

**Single-process concurrency:** `tokio::sync::Mutex<Wallet>` per wallet. Adequate for a personal wallet server.

---

## 7. CLI (`btc`)

`clap` subcommands:

```text
$ btc --help
Bitcoin wallet CLI

USAGE:
    btc <SUBCOMMAND>

SUBCOMMANDS:
    wallet       Create, import, list, show, delete wallets
    address      Generate new addresses, list all
    balance      Show confirmed/unconfirmed balance
    sync         Force chain sync
    send         Build, sign, broadcast a transaction
    tx           Show transaction history, get one by txid
    fee          Show current fee estimates
    config       Show/edit CLI config (network, esplora URL)
```

Examples:

```bash
# Create new wallet (generates 12-word BIP-39 mnemonic)
btc wallet create --network testnet --type native-segwit

# Import existing wallet
btc wallet import --mnemonic "word1 word2 ..." --network mainnet

# List wallets
btc wallet list

# Show balance
btc balance --wallet my-wallet

# Send
btc send --wallet my-wallet --to tb1q... --amount 0.001 --fee fastest

# Inspect a built tx (dry run)
btc send --wallet my-wallet --to tb1q... --amount 0.001 --fee fastest --dry-run
```

CLI config (TOML at `~/.config/btc/config.toml`):

```toml
[default]
network = "testnet"
esplora_url = "https://blockstream.info/testnet/api"
electrum_url = "blockstream.info:700"
db_path = "~/.local/share/btc/db"

[wallets.my-wallet]
network = "mainnet"
esplora_url = "https://blockstream.info/api"
mnemonic = "..."  # encrypted at rest with passphrase (v2)
```

---

## 8. Error handling

- `thiserror::Error` everywhere in the library.
- Errors map 1:1 to axum `IntoResponse` impls that produce RFC 7807 `application/problem+json` responses.
- `anyhow` is **CLI-only** (top-level error printer).
- Errors never panic across API boundary. Every `Result<T, Error>` returned to caller.

```rust
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Error::InvalidMnemonic(_) | Error::InvalidDerivationPath(_) => (StatusCode::BAD_REQUEST, "invalid_input"),
            Error::NotInitialized(_) => (StatusCode::NOT_FOUND, "not_found"),
            Error::InsufficientFunds { .. } => (StatusCode::UNPROCESSABLE_ENTITY, "insufficient_funds"),
            Error::Network(_) | Error::Esplora(_) | Error::Electrum(_) => (StatusCode::BAD_GATEWAY, "upstream"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        (status, Json(json!({
            "type": format!("https://docs.btc-wallet.rs/errors/{code}"),
            "title": code,
            "status": status.as_u16(),
            "detail": self.to_string(),
        }))).into_response()
    }
}
```

---

## 9. Testing

### 9.1 Unit tests (in each module)

- `bip39_mnemonic::generate` produces 12 / 24-word mnemonics with valid checksum.
- `derivation::bip44_path(0)` returns `m/44'/0'/0'/0/0`.
- `address::p2wpkh` matches `rust-bitcoin` reference.
- `script::parse` decodes OP_CODE sequences correctly.
- Property-based with `proptest` for script building round-trips.

### 9.2 Integration tests (`tests/regtest.rs`)

Spin up `bitcoind` regtest via `bitcoind` crate (Rust binding), fund a wallet, send roundtrip:

```rust
#[tokio::test]
async fn regtest_send_roundtrip() {
    let bitcoind = bitcoind::BitcoinD::new(...).await;
    bitcoind.client.generate_to_address(101, &miner_address).await.unwrap();  // mature
    let wallet = Wallet::from_mnemonic(&mnemonic, "", regtest_config(bitcoind)).await.unwrap();
    let balance = wallet.balance().unwrap();
    assert!(balance.confirmed > 0);
    let psbt = wallet.build_tx(TxParams::new(recipient, Amount::from_sat(1000))).unwrap();
    let tx = wallet.sign(&mut psbt).unwrap();
    let txid = wallet.broadcast(&tx).await.unwrap();
    bitcoind.client.generate_to_address(1, &miner_address).await.unwrap();
    wallet.sync().await.unwrap();
    let txs = wallet.transactions().unwrap();
    assert!(txs.iter().any(|t| t.txid == txid));
}
```

Covered: taproot send, segwit send, RBF bump, fee estimation sync, descriptor backup/restore.

### 9.3 End-to-end REST tests (`tests/e2e/api.rs`)

`reqwest` against running `btc-server` instance:

```rust
#[tokio::test]
async fn e2e_create_send_balance() {
    let client = reqwest::Client::new();
    let resp = client.post(url("/v1/wallets"))
        .json(&json!({ "mnemonic": "...", "network": "regtest" }))
        .send().await.unwrap();
    let wallet_id = resp.json::<WalletCreated>().await.unwrap().wallet_id;
    let resp = client.post(url(&format!("/v1/wallets/{wallet_id}/tx")))
        .json(&json!({ "to": recipient, "amount_sat": 1000, "fee_rate_sat_per_vb": 1 }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);
}
```

### 9.4 CI

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test` (unit + integration)
- `cargo test --test e2e -- --ignored` (e2e, requires Docker)
- `cargo deny check` (licenses, advisories, sources, bans)
- `cargo +nightly udeps` (unused deps)
- `cargo miri test` (soundness, slow lane)

---

## 10. Build, CI, release

### Local build

```bash
# Lib
cargo build --release -p bitcoin-wallet-core

# CLI
cargo build --release -p btc

# Server
cargo build --release -p btc-server

# Cross-compile (Linux x86_64 server)
cargo build --release -p btc-server --target x86_64-unknown-linux-gnu

# Size-optimized
RUSTFLAGS="-C opt-level=z -C lto=fat -C strip=symbols -C panic=abort" \
  cargo build --release -p btc-server
```

### Docker

Multi-stage Dockerfile. Final image ~30 MB.

```bash
docker build -t btc-server:dev .
docker run -p 8080:8080 -v $PWD/data:/data btc-server:dev
```

### CI matrix

- Linux x86_64 stable + nightly
- Linux aarch64 stable
- macOS x86_64 stable (skip nightly)
- Windows MSRV (1.85) + stable

### Release

`release-plz` workflow. Publish `bitcoin-wallet-core` to crates.io. CLI + server are binaries (no publish).

---

## 11. Phase plan (week-by-week)

### Week 1 — Skeleton + keys

- Workspace + 3 crates scaffolded.
- `keys::mnemonic` (BIP-39 generate, validate, to/from entropy).
- `keys::derivation` (BIP-32 via `bip32` crate, BIP-44/49/84/86 paths).
- `keys::signer` (secp256k1 Signer trait impl, secp256k1::Keypair wrapper).
- `error` enum skeleton.
- Tests: mnemonic roundtrip, derivation paths match reference vectors, sign/verify ECDSA + Schnorr.
- **No wallet code yet — just primitives.**

### Week 2 — Script + address

- `script::builder` (P2PKH, P2SH, P2WPKH, P2WSH, P2TR scripts).
- `script::parser` (decode raw scripts to opcode stream).
- `address::legacy` (Base58Check P2PKH/P2SH).
- `address::segwit` (Bech32 / Bech32m).
- `address::taproot` (BIP-86 key-path, tweaked key).
- Tests: each address type round-trips through `rust-bitcoin` reference.

### Week 3 — Wallet + chain sync

- `chain::network` enum + config.
- `chain::esplora` (bdk_esplora client wrapper, retry + circuit-breaker).
- `chain::electrum` (bdk_electrum fallback).
- `wallet::Wallet::from_mnemonic` (build BDK descriptor, open wallet, persist).
- `wallet::sync` (initial sync, incremental sync).
- `wallet::balance` (confirmed/unconfirmed/immature).
- `wallet::addresses` (multi-address via xpub).
- Integration test: regtest roundtrip (mine, sync, balance > 0).
- **No tx building yet — read-only wallet works end-to-end.**

### Week 4 — Transactions

- `tx::builder` (BDK TxBuilder wrapper, recipient list, fee rate, drain_to, change).
- `tx::psbt` (BIP-174 v2 construction, serialization to base64).
- `tx::sighash` (SIGHASH_ALL / SIGHASH_SINGLE / SIGHASH_ANYONECANPAY, taproot key-path + script-path).
- `tx::sign` (in-process secp256k1 signing, finalize PSBT).
- `tx::fee` (Esplora fee estimate with 4-tier fallback).
- `tx::broadcast` (POST to Esplora `/tx`).
- `tx::bump_fee` (RBF, build new PSBT, return).
- Integration test: regtest send, mine 1 block, see confirmed.

### Week 5 — CLI

- `btc` binary, all subcommands.
- Config file (`~/.config/btc/config.toml`).
- Mnemonic generation + display (with security warning).
- Integration test: `assert_cmd` runs CLI end-to-end against regtest.

### Week 6 — REST server

- `btc-server` binary, axum + utoipa.
- All endpoints from §6.
- State persistence (SQLite via bdk_file_store).
- utoipa OpenAPI spec, Swagger UI.
- E2E tests: full create → sync → send → confirm via HTTP.

### Week 7 — Hardening

- Property-based tests for script + address modules.
- miri on `bitcoin-wallet-core` soundness.
- `cargo-deny` (no copyleft, no unmaintained, no advisories).
- Memory audit: zero unsafe in user code (only via secp256k1 FFI which is audited).
- Size audit: server binary ≤ 15 MB stripped.
- Fuzzing (cargo-fuzz) on script parser.

### Week 8 — Docs + release

- README, CONTRIBUTING, CHANGELOG, SECURITY.md.
- API docs published to docs.rs.
- Docker image published to GHCR.
- v0.1.0 published to crates.io.

**Total: 8 weeks, 1 engineer.**

---

## 12. Out-of-scope for this spec (handled by later specs)

- **Multi-sig** — `miniscript` is in the dep tree, just not exposed. v2.
- **Hardware-wallet integration** (Ledger, Trezor, Tangem) — explicit non-goal for v1.
- **Lightning** — separate spec (see `docs/lightning/`).
- **Other UTXO chains** (BCH, LTC, DOGE, DASH, KAS) — separate specs, same crate family.
- **Mobile/FFI** (UniFFI to Swift) — separate spec for the full BlockchainSdk rewrite.
- **Watch-only / read-only** — trivial v1.1 addition.
- **Silent payments (BIP-352)** — defer to v2.
- **Atomic swaps, Lightning channel opens, DLCs** — out of scope.

---

## 13. Resolved decisions (locked 2026-08-05)

1. **Mnemonic storage on disk** — **Plain (v1) with strong security warning at creation.** v1.1 adds passphrase-encrypted storage.
2. **License** — **MIT.** Matches `rust-bitcoin`, `bdk`, `bip39`, `bip32` all of which are MIT. Maximum reuse.
3. **Database** — **`bdk_file_store` (SQLite).** Stable, audited, used by BDK itself. Survives server restart.
4. **Default fee strategy** — **Half-hour (3-block target).** User can override per send with `--fee fastest|half_hour|hour|economy` (CLI) or `fee_rate_sat_per_vb` in JSON body (REST).
5. **Multi-wallet** — **Multi-wallet per server.** Wallets addressed by `wallet_id` UUID. State held per wallet under `data_dir/{wallet_id}/`.
