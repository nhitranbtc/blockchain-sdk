# `tron-wallet-core` v0.1 — public API reference

**Audience:** CLI consumers (`crates/tron/`), future FFI binding (Layer 5 of the plan's PAL), and any external Rust consumer that links the crate directly.

**Source of truth:** `rust-wallet-app/crates/tron-wallet-core/src/`. This document is generated from the actual `pub` surface (verified `grep` against `src/`); the plan (`docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`) names the *module shape* but not every function signature.

**Stability:** v0.1. The crate's pub surface is *not* SemVer-frozen. The encrypted-record format (Layer 4 persistence) **is** backward-compatible by `#[serde(default)]`; the schema gained `name` / `network` / `private_key_hex` fields in `a05e585` and pre-Phase-6 blobs still unlock (`a_legacy_phrase_only_blob_still_unlocks`).

---

## Module map

The crate follows the plan's five-layer PAL design. Layers 2-5 are *consumers* of Layer 4 (the pure Rust core).

| Layer | Module(s) | Purpose |
|---|---|---|
| 4 — Core | `address`, `keys`, `tx/{builder,sign,submit,broadcast,summary}`, `crypto`, `error`, `disambig`, `config`, `tokens`, `resource`, `trc20` | Portable pure Rust. No `tokio::net`, no `reqwest`. |
| 3 — PAL traits | `platform::{storage,info,network,clock}` | 4 trait surfaces the platform layer implements. |
| 2 — Platform impls | `platform::{desktop,android,ios,test}` | Concrete `WalletStorage` / `NetworkClient` / `Clock` / `PlatformInfo` per platform. |
| 1 — Lib root | `lib.rs` | Re-exports the above via `pub mod`. |

---

## `address`

```rust
pub struct Address(TronAddress);       // wraps anychain_tron::TronAddress
```

`Address` is the only public type — constructors and methods are documented via `FromStr`, `Display`, and inherent methods on the type itself (not enumerated here; check `cargo doc --no-deps --open` for the full method list).

---

## `keys`

```rust
pub struct Mnemonic(KmsMnemonic);                              // BIP-39, 8 languages via anychain_kms
pub struct KeyPair { pub address: Address, /* sk: Zeroizing<[u8;32]> */ }
pub struct DerivationPath;                                    // m/44'/195'/0'/0/i canonical

pub fn keypair_from_secret_bytes(bytes: &[u8]) -> Result<KeyPair>;       // validates via libsecp256k1
pub fn derive_keypair(mnemonic: &Mnemonic, path: &DerivationPath) -> Result<KeyPair>;
pub fn xpub(mnemonic: &Mnemonic, passphrase: &str, path: &DerivationPath) -> Result<String>;
```

- `derive_keypair` is the `m/44'/195'/0'/0/{index}` family — SLIP-44 coin type 195 is the TRON standard.
- `xpub` returns base58check-encoded extended public key.
- `Mnemonic` constructors (`from_phrase`, `generate`) live alongside the type — not enumerated here.

---

## `crypto`

Argon2id + AES-256-GCM wallet-blob encryption.

```rust
pub struct EncryptedWallet(Vec<u8>);                          // opaque blob

pub fn random_salt() -> [u8; 16];
pub fn derive_key(passphrase: &[u8], salt: &[u8]) -> Result<Zeroizing<Vec<u8>>>;     // Argon2id
pub fn encrypt(plaintext: &[u8], passphrase: &[u8]) -> Result<EncryptedWallet>;     // fresh salt
pub fn decrypt(blob: &EncryptedWallet, passphrase: &[u8]) -> Result<Zeroizing<Vec<u8>>>;
```

`EncryptedWallet` carries the salt internally; `decrypt` is the inverse of `encrypt` with no extra arguments.

---

## `disambig`

Cross-network guard for shared address spaces (Ethereum and TRON both use hex addresses).

```rust
pub enum AddressNetwork { Ethereum, Tron }

pub fn ensure_same_network(caller: Network, addr_recorded: AddressNetwork) -> Result<()>;
pub fn ensure_tron_style_address(addr: &str) -> Result<()>;    // rejects EIP-55 / 0x prefix
```

---

## `error`

```rust
pub enum Error {
    Address(String),
    Mnemonic(String),
    Derivation(String),
    Signing(String),
    TransactionBuild(String),
    Node(String),                // transport / 5xx / connection refused
    NodeResponse(String),        // 200 but unparseable / wrong shape
    Wallet(String),              // not found, wrong passphrase, etc.
    Encryption(String),
    Config(String),
    Pal(String),
}
```

Mapped to CLI exit codes in `crates/tron/src/handlers/mod.rs::exit_code` (Node → 3, Wallet/Encryption → 4, Signing/TransactionBuild/NodeResponse → 5, else 2).

---

## `config`

```rust
pub enum Network { Mainnet, Shasta, Nile, Local, /* + test-only variants */ }
pub struct TronConfig { pub network: Network, pub rpc_url: String /* + SPKI pin option */ }

pub fn default_rpc_url(network: Network) -> &'static str;
```

`Network::tag()` returns the lowercase tag (`"mainnet"`, `"nile"`, etc.) for CLI/config serialisation.

---

## `tokens`

Bundled token registry (compiled in via `include_str!`). Phase 6 carries mainnet + Nile USDT/USDD/TUSD fixtures; live verification against Shasta/Mainnet is in `tokens/mod.rs` tests.

```rust
pub struct Token { pub symbol: String, pub address: String, pub decimals: u8 }
pub struct TestAddresses { /* per-symbol deploy address per network */ }

pub fn load(network: Network) -> &'static [Token];
pub fn by_symbol(network: Network, symbol: &str) -> Option<&'static Token>;
pub fn by_address(network: Network, address: &str) -> Option<&'static Token>;
pub fn test_addresses(network: Network) -> Option<&'static TestAddresses>;
```

---

## `trc20`

TRC-20 ABI helpers + view-call wrappers around `triggerconstantcontract`.

```rust
pub const TRANSFER_SELECTOR: [u8; 4];       // 0xa9059cbb
pub const APPROVE_SELECTOR:  [u8; 4];       // 0x095ea7b3
pub const ALLOWANCE_SELECTOR:[u8; 4];       // 0xdd62ed3e

pub fn encode_uint256_arg(value: U256) -> [u8; 32];
pub fn balance_of_args(owner_bytes: &[u8]) -> [u8; 32];
pub fn allowance_args(owner_bytes: &[u8], spender_bytes: &[u8]) -> [u8; 64];
pub fn no_args() -> Vec<u8>;

pub async fn balance_of (rpc: &TronGridClient, contract: &str, owner: &str)                -> Result<U256>;
pub async fn allowance  (rpc: &TronGridClient, contract: &str, owner: &str, spender: &str) -> Result<U256>;
pub async fn decimals   (rpc: &TronGridClient, contract: &str, owner: &str)                -> Result<u8>;
pub async fn symbol     (rpc: &TronGridClient, contract: &str, owner: &str)                -> Result<String>;
pub async fn name       (rpc: &TronGridClient, contract: &str, owner: &str)                -> Result<String>;

pub fn is_unlimited(amount: U256) -> bool;     // >= 2^217 (39 decimal digits)
```

Note: `decimals`/`symbol`/`name` take an `owner` argument because the RPC endpoint (`triggerconstantcontract`) needs *some* `owner_address` for the constant call envelope. Any owned T-address works; the call does not change state.

---

## `tx::builder`

Pure shaping — no I/O, no signing.

```rust
pub fn trx_transfer      (owner: &str, to: &str, amount_sun: u64)                  -> Result<TronTransactionParameters>;
pub fn trc20_transfer    (owner: &str, contract: &str, to: &str, amount: U256)    -> Result<TronTransactionParameters>;
pub fn trc20_approve     (owner: &str, contract: &str, spender: &str, amount: U256) -> Result<TronTransactionParameters>;

pub fn set_ref_block   (params: &mut TronTransactionParameters, height: i64, block_id_hex: &str) -> Result<()>;
pub fn set_fee_limit   (params: &mut TronTransactionParameters, fee_limit_sun: i64);
pub fn set_timestamp   (params: &mut TronTransactionParameters, ts_ms: i64);
pub fn set_expiration  (params: &mut TronTransactionParameters, ttl_ms: i64);
```

---

## `tx::sign`

```rust
pub const SIGNATURE_LEN: usize = 65;     // 64 bytes r||s + 1 byte recid
pub const MESSAGE_LEN:   usize = 32;

pub struct Signature([u8; SIGNATURE_LEN]);
pub struct RecoveryId(u8);
pub struct Signed { pub txid: [u8; MESSAGE_LEN], pub signature: Signature, pub recovery_id: RecoveryId }
pub struct SignedTransaction { pub signed_envelope_hex: String, pub txid_hex: String }

pub fn sign_hash(hash: &[u8], sk: &Zeroizing<[u8; 32]>) -> Result<(Signature, RecoveryId)>;
pub fn sign_tx(sk: &Zeroizing<[u8; 32]>, params: &TronTransactionParameters) -> Result<SignedTransaction>;
pub fn txid(raw_bytes: &[u8]) -> [u8; MESSAGE_LEN];        // single SHA-256 (Q2 correction)
```

**Single SHA-256** for txid, not double. Confirmed by live TronGrid broadcast verification in Phase 4; the vendored `anychain-tron` patch that initially proposed double-SHA was reverted in `c521d50`.

---

## `tx::broadcast` + `chain::client`

Live RPC client + types returned from TronGrid.

```rust
pub struct TronGridClient { /* ... */ }
impl TronGridClient {
    pub fn new(rpc_url: &str, spki_pin: Option<SpkiPin>) -> Result<Self>;

    pub async fn get_now_block       (&self) -> Result<BlockHeader>;
    pub async fn broadcast           (&self, signed_envelope_hex: &str) -> Result<BroadcastReceipt>;
    pub async fn get_tx_info         (&self, txid_hex: &str) -> Result<TransactionInfo>;
    pub async fn get_account         (&self, address: &str) -> Result<AccountInfo>;
    pub async fn get_transaction_by_id(&self, txid_hex: &str) -> Result<OriginalCall>;
}

pub struct BlockHeader { pub block_number: u64, pub block_id: String /* hex */ }
pub struct BroadcastReceipt { pub code: Option<String>, pub txid: Option<String>, pub message: Option<String>, pub error: Option<String> }
pub struct TransactionInfo  { pub block_number: Option<u64>, /* + receipts */ }
pub struct AccountInfo      { pub address: String, pub balance_sun: u64, pub exists: bool }
pub enum OriginalCall {
    Transfer { owner: String, to: String, amount_sun: u64 },
    TriggerSmartContract { owner: String, contract: String, data_hex: String },
}
```

`TronGridClient::new` takes an `Option<SpkiPin>`. SPKI pinning is supported end-to-end (Phase 2 Task 2.7-2.8); `None` skips pin verification. The CLI's `--rpc-url` does **not** take a `--spki-pin` flag in v0.1 — pinning is a programmatic surface only.

---

## `tx::submit` — orchestration layer

This is what makes the CLI + the future FFI share one shape.

```rust
pub const DEFAULT_EXPIRATION_MS: i64 = 60_000;     // anychain default 5min is too long

pub struct SubmitOptions {
    pub fee_limit_sun: Option<i64>,
    pub expiration_ms: Option<i64>,
}

pub struct Submitted { pub signed: SignedTransaction, pub receipt: BroadcastReceipt }

// 3-step shape (each stage callable separately for --dry-run / --sign-only):
pub async fn prepare_trx          (rpc, owner, to, amount_sun, opts) -> Result<TronTransactionParameters>;
pub async fn prepare_trc20        (rpc, owner, contract, to, amount, opts) -> Result<TronTransactionParameters>;
pub async fn prepare_trc20_approve(rpc, owner, contract, spender, amount, opts) -> Result<TronTransactionParameters>;
pub fn     sign_prepared          (sk: &Zeroizing<[u8; 32]>, params: &TronTransactionParameters) -> Result<SignedTransaction>;
pub async fn broadcast_signed     (rpc, &SignedTransaction) -> Result<BroadcastReceipt>;

// All-in-one:
pub async fn submit_trx           (rpc, sk, owner, to, amount_sun, opts) -> Result<Submitted>;
pub async fn submit_trc20         (rpc, sk, owner, contract, to, amount, opts) -> Result<Submitted>;
pub async fn submit_trc20_approve (rpc, sk, owner, contract, spender, amount, opts) -> Result<Submitted>;

// Fee-bump (TRON has no RBF — this is a *new* txid):
pub async fn submit_send_speedup  (rpc, sk, txid_hex, fee_limit_sun: i64) -> Result<Submitted>;

// Decoded calldata shape (for the speed-up path):
pub enum Trc20Call {
    Transfer  { to: String,      amount: U256 },
    Approve   { spender: String, amount: U256 },
}
pub fn decode_trc20_call(data_hex: &str) -> Result<Trc20Call>;

// Confirmation poller:
pub fn     validate_poll_interval(poll_interval: Duration) -> Result<()>;
pub async fn wait_for_confirm     (rpc, txid_hex, timeout, poll_interval) -> Result<TransactionInfo>;
```

**Speed-up semantics:** TRON has no replace-by-fee. `submit_send_speedup` rebuilds the original call with a higher fee limit and broadcasts a *new* transaction with a *new txid*. If the original later confirms, both may execute — for calls that failed on `OUT_OF_ENERGY`, not ones merely waiting.

**`decode_trc20_call`** refuses non-zero padding in address words (would invent a recipient) and refuses any selector other than `transfer`/`approve`. Anything else is `Error::TransactionBuild`, not a guess.

**`wait_for_confirm`** timeouts return `Error::Node` — never `Ok` with an empty receipt. A zero `poll_interval` is `Error::Config`.

---

## `wallet`

```rust
pub struct WalletId([u8; 16]);        // 16 random bytes, hex-encoded for CLI

pub struct WalletManager<'a> { /* storage: &'a dyn WalletStorage */ }

pub enum WalletKind { Mnemonic, PrivateKey }
pub enum WalletSecret { Mnemonic(Mnemonic), PrivateKey(Zeroizing<[u8; 32]>) }

pub struct UnlockedWallet {
    pub id: WalletId,
    pub secret: WalletSecret,
    pub name: Option<String>,
    pub network: Network,
}

pub struct WalletSummary { /* id, name?, network, address, kind, created_at */ }

impl WalletManager<'_> {
    pub fn new(storage: &dyn WalletStorage) -> Self;

    pub fn create_with_meta (storage: &dyn WalletStorage, words: u8, name: Option<&str>, network: Network, passphrase: &[u8]) -> Result<UnlockedWallet>;
    pub fn import_from_phrase(storage: &dyn WalletStorage, phrase: &str, name: Option<&str>, network: Network, passphrase: &[u8]) -> Result<UnlockedWallet>;
    pub fn import_private_key(storage: &dyn WalletStorage, secret: &[u8], name: Option<&str>, network: Network, passphrase: &[u8]) -> Result<UnlockedWallet>;

    pub fn unlock     (&self, id: WalletId, passphrase: &[u8]) -> Result<UnlockedWallet>;
    pub fn rename     (&self, id: WalletId, passphrase: &[u8], new_name: &str) -> Result<()>;
    pub fn delete     (&self, id: WalletId) -> Result<()>;
    pub fn summary    (&self, id: WalletId, passphrase: &[u8]) -> Result<WalletSummary>;
    pub fn list       (&self) -> Result<Vec<WalletSummary>>;
    pub fn list_summaries(&self) -> Result<Vec<WalletSummary>>;     // skips Error::Encryption on bad blobs
}

impl UnlockedWallet {
    pub fn keypair (&self) -> Result<KeyPair>;                       // works for both kinds
    pub fn mnemonic(&self) -> Option<&Mnemonic>;                     // None for raw-key wallets
    pub fn summary (&self) -> WalletSummary;
}
```

**Record evolution:** the encrypted `PlaintextRecord` gained `name`, `network`, `private_key_hex` as `#[serde(default)]` fields. Pre-Phase-6 blobs (phrase + id only) still unlock — covered by `tests/wallet_persistence.rs::a_legacy_phrase_only_blob_still_unlocks`.

**Raw-key wallets** carry no mnemonic; callers of `UnlockedWallet::mnemonic()` must handle `None` (CLI's `handlers::unlock` errors with `Error::Wallet`).

---

## `platform::storage` (PAL trait)

```rust
pub trait WalletStorage: Send + Sync {
    fn put_atomic(&self, id: &WalletId, ciphertext: &[u8]) -> Result<()>;
    fn get       (&self, id: &WalletId) -> Result<Vec<u8>>;
    fn delete    (&self, id: &WalletId) -> Result<()>;
    fn list_ids  (&self) -> Result<Vec<WalletId>>;
}
```

Implementations shipped:

| Type | Platform | Notes |
|---|---|---|
| `FileWalletStorage` | Desktop | `with_dir(path)`; atomic write via temp + rename. |
| `EncryptedFileWalletStorage` | Android | Wraps Android Keystore. |
| `KeychainWalletStorage` | iOS | Wraps Keychain Services. |
| `InMemoryStorage` | Tests | No persistence. |

---

## `platform::{network,clock,info}` (PAL traits)

```rust
pub trait NetworkClient: Send + Sync {
    fn post_json(&self, url: &str, body: &serde_json::Value) -> Result<serde_json::Value>;
}
pub trait Clock: Send + Sync {
    fn now_epoch_ms(&self) -> i64;
}
pub trait PlatformInfo: Send + Sync {
    fn data_dir(&self) -> PathBuf;
    fn app_name(&self) -> &str;
    fn is_mobile(&self) -> bool;
}
```

Implementations: `ReqwestClient`/`SystemClock`/`DesktopPlatformInfo` (desktop), `OSRootsClient`/`IosClock`/`IosPlatformInfo` (iOS), `OSRootsClient`/`AndroidClock`/`AndroidPlatformInfo` (Android), `MockClient`/`MockClock`/`StaticInfo` (tests).

Default-resolution helpers: `default_clock()`, `default_storage()`, `default_network_client()`, `default_platform_info()`.

---

## `resource`

Energy estimation for TRC-20 / contract calls (preview before broadcast).

```rust
pub struct EnergyEstimate { /* energy_used, energy_penalty, fee_sun */ }
pub async fn estimate_energy(rpc: &TronGridClient, owner: &str, contract: &str, selector: &[u8; 4]) -> Result<EnergyEstimate>;
pub fn scale_energy(raw: u64, max_factor: f64) -> u64;
```

Currently exposed but not surfaced via CLI in v0.1 — Phase 7 candidate (`--estimate-fee` flag on send paths).

---

## `tx::summary`

```rust
pub struct TxSummary { /* txid, block_number, contract_type, fee, result */ }
```

Decoded view of a confirmed transaction. CLI's `tx get` returns the raw `TransactionInfo` today; `TxSummary` is the typed version a future release will switch to.

---

## Layer 5 (FFI) — not yet shipped

The plan reserves `extern "C"` surface (panic-message scrubber, single-threaded tokio runtime) for Layer 5. **Not implemented in v0.1.** When it lands, it will be a *separate* crate (`tron-wallet-core-ffi`) that depends on `tron-wallet-core` and re-exports a `#[no_mangle] pub unsafe extern "C" fn ...` set.

---

## Consumers today

| Consumer | Path | What it uses |
|---|---|---|
| `tron` CLI | `crates/tron/` | Everything in this doc except the platform PAL impls (CLI uses `FileWalletStorage` directly via `platform::desktop`). |
| Mobile | not yet | Phase 7+. |

---

## What's deliberately absent in v0.1

(Plan §Phase 6 was deliberately scoped to the CLI surface. These belong to §Phase 7 and beyond — not API bugs.)

- **Staking / voting / claim** — `freezebalance`, `unfreezebalance`, `votewitness`, `withdrawbalance`.
- **Memo / data field on TRX transfer.**
- **Per-address transaction history** (no `gettransactionlistfromaddress` wrapper).
- **Multi-sig / account permissions** (`accountpermissionsupdate`).
- **TRC-10 / TRC-721 / TRC-1155.**
- **HW wallet (Ledger).**
- **Address book / watch-list persistence.**

---

## Test anchors

| Surface | Test file |
|---|---|
| `tx::submit` orchestration | `src/tx/submit.rs` unit tests (8 — `decodes_its_own_transfer_calldata`, `refuses_an_unknown_selector`, `refuses_truncated_calldata`, `refuses_non_hex_calldata`, `refuses_a_dirty_address_word`, `address_word_round_trips`, `wait_for_confirm_rejects_a_zero_interval_before_any_call`, plus the const assertion on `DEFAULT_EXPIRATION_MS`) |
| Wallet record evolution | `tests/wallet_persistence.rs::a_legacy_phrase_only_blob_still_unlocks` (plus the other 13 cases in that file) |
| `chain::client` response parsing | `src/chain/client.rs::response_decoding_tests` (8 cases: empty body, `Error` field surfaced, unknown txid, unsupported contract type, etc.) |
| CLI surface (exit codes, STDOUT/STDERR split, subcommand inventory) | `crates/tron/tests/cli.rs` (26 subprocess integration tests) |
| Decimal amount arithmetic (no `f64` rounding) | `crates/tron/src/handlers/mod.rs` unit tests (3) |

---

**Last verified:** 2026-09-07, commit `a05e585` on `tron/phase6-cli`. Re-verify against `src/` before publishing a new release; this file lags the code if signatures change.
