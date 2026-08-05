# BDK Wallet 3.1 — Deep API Reference: Categories 10-14

> Scope: `keys` module, `errors`, `utilities`, `network`, integration with other crates.
> Crate: `bdk_wallet = "3.1"`. Compiled against `bitcoin = "0.32.8"` (resolved `0.32.100`),
> `miniscript = "12.3.5"` (resolved `12.3.7"`), `bdk_chain = "0.23.3"`.
>
> Verified against `https://docs.rs/bdk_wallet/latest/bdk_wallet/...` (snapshot 2026-08-05).
> Anything not confirmed on docs.rs is flagged **verify in Task 31 spike**.

---

## 10. Keys module (`bdk_wallet::keys`)

### Summary table

| Item | Kind | Header | Notes |
|------|------|--------|-------|
| `keys::bip39` | module | `keys-bip39` feature | Re-exports from the `bip39 ^2.2.2` crate |
| `keys::bip39::Mnemonic` | struct (re-export) | `keys-bip39` | Wraps a BIP-39 phrase; implements `DerivableKey` + `GeneratableKey` in BDK |
| `keys::bip39::Error` | enum (re-export) | `keys-bip39` | From `bip39::Error` |
| `keys::bip39::Language` | enum (re-export) | `keys-bip39` | `English` + a small set of BIP-39 languages |
| `keys::bip39::WordCount` | enum (re-export) | `keys-bip39` | 12 / 15 / 18 / 21 / 24 |
| `keys::bip39::MnemonicWithPassphrase` | type alias | `keys-bip39` | `(Mnemonic, Option<String>)` |
| `keys::GeneratableKey<Ctx>` | trait | — | `generate`, `generate_with_aux_rand`, `generate_with_entropy` |
| `keys::GeneratableDefaultOptions` | trait | — | Default-options helper for `GeneratableKey` |
| `keys::DerivableKey` | trait | — | Marker for keys that can BIP-32-derive |
| `keys::IntoDescriptorKey` | trait | — | Bridge from raw key to `DescriptorKey` |
| `keys::DescriptorKey` | enum | — | Container for public or secret key (`Public` / `Secret`) |
| `keys::DescriptorSecretKey` | enum | — | `Single` / `XPrv` / `MultiXPrv` |
| `keys::DescriptorPublicKey` | enum | — | `Single` / `XPub` / `MultiXPub`. Re-export of `miniscript::...::DescriptorPublicKey` |
| `keys::ExtendedKey` | enum | — | `XPrv` / `XPub` wrapper |
| `keys::KeyError` | enum | — | 6 variants (see §11) |
| `keys::ScriptContext` | trait | — | BDK alias for `miniscript::ScriptContext` |
| `keys::ScriptContextEnum` | enum | — | Enum representation of the valid `ScriptContext`s |
| `keys::SinglePriv` / `SinglePub` | struct | — | Single (non-extended) key with optional origin |
| `keys::SinglePubKey` | enum | — | Compressed vs X-only |
| `keys::SortedMultiVec` | struct | — | Contents of a `sortedmulti(...)` descriptor |
| `keys::GeneratedKey<K, Ctx>` | struct | — | Output of `GeneratableKey::generate*` |
| `keys::PrivateKeyGenerateOptions` | struct | — | Options for `PrivateKey` generation |
| `keys::XprivGenerateOptions` | struct | — | Options for `Xpriv` generation |
| `keys::KeyMap` | type alias | — | `BTreeMap<DescriptorPublicKey, DescriptorSecretKey>` |
| `keys::ValidNetworkKinds` | type alias | — | Set of valid `NetworkKind`s for a key |
| `keys::any_network_kind()` / `mainnet_network_kind()` / `test_network_kind()` | fn | — | Set builders |
| `keys::intersect_network_kinds(a, b)` | fn | — | Set intersection |

There is **no `keys::ValidNetworks` type**. Use `ValidNetworkKinds` (`BTreeSet<NetworkKind>`) instead.

### `bdk_wallet::keys::bip39::Mnemonic` (re-export of `bip39::Mnemonic`)

Construction:

```rust
impl Mnemonic {
    pub fn from_entropy(entropy: &[u8]) -> Result<Mnemonic, Error>;       // English only
    pub fn from_entropy_in(language: Language, entropy: &[u8]) -> Result<Mnemonic, Error>;
    pub fn parse(s: &str) -> Result<Mnemonic, Error>;                       // English only
    pub fn parse_in(language: Language, s: &str) -> Result<Mnemonic, Error>;
    pub fn parse_normalized(s: &str) -> Result<Mnemonic, Error>;            // requires unicode-normalization feature (off by default in bdk_wallet)
    pub fn parse_in_normalized(language: Language, s: &str) -> Result<Mnemonic, Error>;
    pub fn parse_in_normalized_without_checksum_check(language, s) -> Result<...>;
    pub fn language_of(s: &str) -> Result<Language, Error>;
}
```

Seed derivation (BIP-39 → BIP-32 seed):

```rust
impl Mnemonic {
    pub fn to_seed(&self, passphrase: &str) -> [u8; 64];           // PBKDF2-HMAC-SHA512
    pub fn to_seed_normalized(&self, passphrase: &str) -> [u8; 64]; // requires unicode-normalization
    pub fn to_entropy(&self) -> Vec<u8>;                            // requires alloc
    pub fn to_entropy_array(&self) -> ([u8; 64], usize);
}
```

Generic params of `GeneratableKey<Ctx>` when implemented on `Mnemonic`:

```text
Entropy = [u8; 32]
Options = (WordCount, Language)
Error   = bip39::Error
```

Generation entry points (provided by BDK, wrapped around `bip39`):

```rust
// `Generate` uses thread-local RNG (requires `std`).
fn generate(opts: (WordCount, Language)) -> Result<GeneratedKey<Mnemonic, Ctx>, bip39::Error>;
fn generate_with_entropy(opts, entropy) -> Result<_, _>;
fn generate_with_aux_rand(opts, rng: &mut (impl CryptoRng + RngCore)) -> Result<_, _>;
```

**`MnemonicType`** does **not** exist in 3.1. Use the `WordCount` enum: `Mnemonic12 / Mnemonic15 / Mnemonic18 / Mnemonic21 / Mnemonic24`.

### `keys::DescriptorSecretKey`

Variants (re-exported from `miniscript`):

```rust
pub enum DescriptorSecretKey {
    Single(SinglePriv),
    XPrv(DescriptorXKey<Xpriv>),
    MultiXPrv(DescriptorMultiXKey<Xpriv>),
}
```

Methods:

```rust
pub fn to_public<C: Verification>(&self, secp: &Secp256k1<C>)
    -> Result<DescriptorPublicKey, DescriptorKeyParseError>;

pub fn is_multipath(&self) -> bool;
pub fn into_single_keys(self) -> Vec<DescriptorSecretKey>;
```

### `keys::DescriptorPublicKey`

```rust
pub enum DescriptorPublicKey {
    Single(SinglePub),
    XPub(DescriptorXKey<Xpub>),
    MultiXPub(DescriptorMultiXKey<Xpub>),
}

impl DescriptorPublicKey {
    pub fn master_fingerprint(&self) -> Fingerprint;
    pub fn full_derivation_path(&self) -> Option<DerivationPath>;
    pub fn full_derivation_paths(&self) -> Vec<DerivationPath>;

    #[deprecated(note = "use has_wildcard")]
    pub fn is_deriveable(&self) -> bool;
    pub fn has_wildcard(&self) -> bool;
    pub fn has_hardened_step(&self) -> bool;

    #[deprecated(note = "use at_derivation_index")]
    pub fn derive(self, idx: u32) -> Result<DefiniteDescriptorKey, ConversionError>;
    pub fn at_derivation_index(self, idx: u32) -> Result<DefiniteDescriptorKey, ConversionError>;

    pub fn is_multipath(&self) -> bool;
    pub fn into_single_keys(self) -> Vec<DescriptorPublicKey>;
}
```

Implements `MiniscriptKey`, `IntoDescriptorKey<Ctx>`, `Display`, `FromStr`, `From<PublicKey>`,
`From<XOnlyPublicKey>`, `From<DefiniteDescriptorKey>`, `Serialize`/`Deserialize` (serde feature).

### `keys::GeneratableKey` trait

```rust
pub trait GeneratableKey<Ctx: ScriptContext>: Sized {
    type Entropy: AsMut<[u8]> + Default;
    type Options;
    type Error: Debug;

    fn generate_with_entropy(
        options: Self::Options,
        entropy: Self::Entropy,
    ) -> Result<GeneratedKey<Self, Ctx>, Self::Error>;

    // Provided:
    fn generate(opts: Self::Options)
        -> Result<GeneratedKey<Self, Ctx>, Self::Error>;       // requires `std`
    fn generate_with_aux_rand(
        opts: Self::Options,
        rng: &mut (impl CryptoRng + RngCore),
    ) -> Result<GeneratedKey<Self, Ctx>, Self::Error>;
}
```

`GeneratableDefaultOptions` (companion): gives a no-argument `generate_default()` for implementations
that want to skip the `Options` boilerplate.

---

## 11. Errors

> **Important shape change vs. BDK 0.x**: there is **no single `bdk_wallet::Error` enum**.
> The crate publishes 5 sibling error enums under `bdk_wallet::error::*`, each scoped to a
> lifecycle stage (`CreateTxError`, `BuildFeeBumpError`, `LoadError`, plus `LoadMismatch` and
> `MiniscriptPsbtError`). Plus the per-module enums in `keys`, `descriptor`, and the
> re-exported `chain` ones.

### 11.1 `bdk_wallet::error::CreateTxError` (18 variants)

| # | Variant | Fields | Raised when |
|---|---------|--------|-------------|
| 1 | `Descriptor` | `DescriptorError` | Descriptor parse / policy derivation failed |
| 2 | `Policy` | `PolicyError` | Policy / extraction failed |
| 3 | `SpendingPolicyRequired` | `KeychainKind` | Psbt can't be signed by the wallet policy |
| 4 | `Version0` | — | Tx version 0 was requested but not allowed |
| 5 | `Version1Csv` | — | `version=1` requested but `OP_CSV` needs v2 |
| 6 | `LockTime` | `{ requested: LockTime, required: LockTime }` | Requested locktime < required |
| 7 | `RbfSequenceCsv` | `{ sequence: Sequence, csv: Sequence }` | RBF can't be enabled with required CSV |
| 8 | `FeeTooLow` | `{ required: Amount }` | Bump tx fee below original |
| 9 | `FeeRateTooLow` | `{ required: FeeRate }` | Bump tx fee rate below requirement |
| 10 | `NoUtxosSelected` | — | `manually_selected_only` flag set, no utxo passed |
| 11 | `OutputBelowDustLimit` | `usize` (output index) | Output under dust threshold (default 546 sats) |
| 12 | `CoinSelection` | `InsufficientFunds` | BnB / coin selection ran out of money (see below) |
| 13 | `NoRecipients` | — | Empty `TxBuilder::add_recipient` chain |
| 14 | `Psbt` | `bitcoin::psbt::Error` | PSBT construction failed |
| 15 | `MissingKeyOrigin` | `String` | Extended key missing explicit origin info (`[...]`) |
| 16 | `UnknownUtxo` | — | Spending a UTXO not in `Wallet.local_chain().tx_graph` |
| 17 | `MissingNonWitnessUtxo` | `OutPoint` | Foreign utxo needs `non_witness_utxo` set |
| 18 | `MiniscriptPsbt` | `MiniscriptPsbtError` | (see below) |

`InsufficientFunds` (re-exported from `bdk_wallet::coin_select` via `CreateTxError::CoinSelection`):

```rust
pub struct InsufficientFunds {
    pub needed: u64,     // target (recipient + fee) — verify field name in Task 31 spike
    pub available: u64, // selected coin amount
}
```

Trait: `Debug`, `Display`, `Error`. Implements `From<DescriptorError>`, `From<bitcoin::psbt::Error>`,
`From<InsufficientFunds>`, `From<MiniscriptPsbtError>`, `From<PolicyError>`. Send + Sync; **not** UnwindSafe.

### 11.2 `bdk_wallet::error::BuildFeeBumpError` (6 variants)

| # | Variant | Fields | Raised when |
|---|---------|--------|-------------|
| 1 | `UnknownUtxo` | `OutPoint` | Bumping an input we don't know |
| 2 | `TransactionNotFound` | `Txid` | Original tx not in chain source |
| 3 | `TransactionConfirmed` | `Txid` | Trying to bump an already-confirmed tx |
| 4 | `IrreplaceableTransaction` | `Txid` | Sequence ≥ `0xFFFFFFFE` |
| 5 | `FeeRateUnavailable` | — | No fee-rate oracle data |
| 6 | `InvalidOutputIndex` | `OutPoint` | Input references bad output index |

### 11.3 `bdk_wallet::error::LoadError` (5 variants)

| # | Variant | Fields | Raised when |
|---|---------|--------|-------------|
| 1 | `Descriptor` | `DescriptorError` | Persisted descriptor malformed |
| 2 | `MissingNetwork` | — | Stored data lacks network field |
| 3 | `MissingGenesis` | — | Stored data lacks genesis hash |
| 4 | `MissingDescriptor` | `KeychainKind` | Stored data lacks descriptor for that keychain |
| 5 | `Mismatch` | `LoadMismatch` | Stored data fails validation (see below) |

`From<LoadMismatch> for LoadError` is implemented.

### 11.4 `bdk_wallet::error::LoadMismatch` (3 variants)

| Variant | Fields |
|---------|--------|
| `Network` | `loaded: Network`, `expected: Network` |
| `Genesis` | `loaded: BlockHash`, `expected: BlockHash` |
| `Descriptor` | `keychain: KeychainKind`, `loaded: Option<Box<ExtendedDescriptor>>`, `expected: Option<Box<ExtendedDescriptor>>` |

### 11.5 `bdk_wallet::error::MiniscriptPsbtError` (3 variants)

| Variant | Wraps |
|---------|-------|
| `Conversion` | `miniscript::descriptor::ConversionError` |
| `UtxoUpdate` | `miniscript::psbt::UtxoUpdateError` |
| `OutputUpdate` | `miniscript::psbt::OutputUpdateError` |

`From<MiniscriptPsbtError> for CreateTxError`.

### 11.6 `bdk_wallet::keys::KeyError` (6 variants)

| Variant | Inner type | When |
|---------|-----------|------|
| `InvalidScriptContext` | — | Key not compatible with the script context (e.g. uncompressed in Segwit) |
| `InvalidNetworkKind` | — | xprv/xpub network doesn't match the wallet network |
| `InvalidChecksum` | — | Descriptor checksum mismatch |
| `Message` | `String` | Catch-all string error |
| `Bip32` | `bitcoin::bip32::Error` | BIP-32 derivation error |
| `Miniscript` | `miniscript::Error` | Miniscript satisfaction / translation error |

### 11.7 `bdk_wallet::descriptor` errors

`descriptor::DescriptorError` re-exports `miniscript::descriptor::DescriptorError`. Verify field
list in Task 31 spike — typical variants include `InvalidAddress`, `Tr(String)`, `BareDescriptor`,
`MultiA`, `PushTooSmall`, `Key`, `InvalidVersion`, `NoChecksum`, `Verify`, `Sepc256k1` (sic),
`MissingHdKeyOrigin`, etc. **Do not enumerate here without re-checking.**

### 11.8 `bdk_wallet::chain` errors (re-exported from `bdk_chain`)

| Type | Module | Notes |
|------|--------|-------|
| `chain::tx_graph::CalculateFeeError` | `bdk_chain::tx_graph` | Couldn't compute tx fee (missing ancestor / `TxOutNoCache`) |
| `chain::tx_graph::OrphanListError` | `bdk_chain::tx_graph` | Returns `Vec<Txid>` of still-orphaned txids after `try_list_canonical` |
| `chain::local_chain::CannotExtendChainWithBlock` | `bdk_chain::local_chain` | `Update` exceeds `LocalChain` tip |
| `chain::persisted::PersistWithSingleSyncError` | `bdk_chain::persisted` | Persistence backing error |

(Both `chain::*` enums are re-exported through `bdk_wallet::chain`.)

---

## 12. Utilities

| Item | Signature | Where | Purpose |
|------|-----------|-------|---------|
| `wallet_name_from_descriptor` | `pub fn wallet_name_from_descriptor<T: IntoWalletDescriptor>(descriptor: T, change_descriptor: Option<T>, network_kind: NetworkKind, secp: &Secp256k1<All>) -> Result<String, Error>` | `bdk_wallet::wallet_name_from_descriptor` (`src/bdk_wallet/wallet/mod.rs:2851–2871`) | Deterministic wallet name built from descriptor checksum(s) |
| `version` | `pub fn version() -> &'static str` | `bdk_wallet::version` | Runtime version string of the `bdk_wallet` crate (matches `Cargo.toml`) |
| `Wallet::descriptor_checksum` | `pub fn descriptor_checksum(&self, keychain: KeychainKind) -> String` | `bdk_wallet::Wallet` (source at `wallet/mod.rs:2267–2282`) | Public-descriptor checksum for a keychain |
| `Wallet::network` | `pub fn network(&self) -> Network` | `bdk_wallet::Wallet` (`wallet/mod.rs:593–595`) | Underlying `bitcoin::Network` |
| `Wallet::public_descriptor` (used internally by `descriptor_checksum`) | `pub fn public_descriptor(&self, keychain: KeychainKind) -> Option<ExtendedDescriptor>` | `Wallet` | Returns the descriptor the wallet signs with |
| `IntoWalletDescriptor` | trait | `bdk_wallet::descriptor` | Lets a `&str` or `Descriptor`/`Descriptor<PublicKey>` slide into the API above |

**There is no `Wallet::to_string` override.** `Wallet` only implements `fmt::Debug`. Export the
descriptor via `wallet.public_descriptor(...)`, `wallet.descriptor_checksum(...)`, or
`wallet_name_from_descriptor(...)`.

The `Error` returned by `wallet_name_from_descriptor` is the **crate-root `Error` alias** —
verify in spike whether this is a wrapper over `DescriptorError` or the older `Error` trait
newtype. (docs.rs path: `bdk_wallet::Error`; sources `src/bdk_wallet/wallet/mod.rs:2851`.)

---

## 13. Network

### 13.1 `Wallet::network` returns `bitcoin::Network`

```rust
pub fn network(&self) -> Network   // bitcoin::network::Network
```

BDK does **not** define its own `Network` enum — it uses `bitcoin 0.32`'s exhaustive enum.

### 13.2 `bitcoin::Network` variants (5)

```rust
pub enum Network {
    Bitcoin,
    Testnet,
    Testnet4,
    Signet,
    Regtest,
}
```

`bitcoin 0.32` warns this enum is **exhaustive**; new networks require a breaking change.
The forward-compat path is via `Network::Into<Params>` → `Params`.

### 13.3 Per-network address format (BIP-84 bech32)

| Network | Native segwit HRP | P2PKH prefix | P2SH prefix |
|---------|-------------------|--------------|-------------|
| Bitcoin | `bc` (`bc1q...`) | `1` | `3` |
| Testnet | `tb` (`tb1q...`) | `m`/`n` | `2` |
| Testnet4 | `tb` (`tb1q...`) | `m`/`n` | `2` |
| Signet | `tb` (`tb1q...`) | `m`/`n` | `2` |
| Regtest | `bcrt` (`bcrt1q...`) | `m`/`n` | `2` |

(Generated by `bitcoin::Address::from_str` + the network's bech32 `KnownHrp`.)

### 13.4 `CreateParams` — network setter

Builder on `bdk_wallet::CreateParams` (`src/bdk_wallet/wallet/params.rs:61–70`):

```rust
impl CreateParams {
    pub fn new_single(descriptor: impl IntoWalletDescriptor) -> Self;
    pub fn new(descriptor: ..., change_descriptor: ...) -> Self;
    pub fn new_two_path(two_path_descriptor: ...) -> Self;

    pub fn network(self, network: Network) -> Self;   // default: Network::Bitcoin
    pub fn genesis_hash(self, hash: BlockHash) -> Self;
    pub fn lookahead(self, lookahead: u8) -> Self;
    pub fn keymap(self, kc: KeychainKind, km: KeyMap) -> Self;
    pub fn use_spk_cache(self, on: bool) -> Self;

    pub fn create_wallet(self, persister: impl WalletPersister) -> Result<PersistedWallet, CreateError>;
    pub fn create_wallet_async<P: AsyncWalletPersister>(self, persister: P) -> Result<PersistedWallet, CreateError>;
    pub fn create_wallet_no_persist(self) -> Result<Wallet, DescriptorError>;
}
```

### 13.5 `LoadParams` — network check, not storage

```rust
impl LoadParams {
    pub fn new() -> Self;
    pub fn keymap(self, ...) -> Self;
    pub fn descriptor(self, ...) -> Self;
    pub fn two_path_descriptor(self, ...) -> Self;
    pub fn check_network(self, network: Network) -> Self;
    pub fn check_genesis_hash(self, hash: BlockHash) -> Self;
    pub fn lookahead(self, u8) -> Self;
    pub fn extract_keys(self, bool) -> Self;
    pub fn use_spk_cache(self, bool) -> Self;

    pub fn load_wallet(self, persister: impl WalletPersister) -> Result<Option<PersistedWallet>, LoadError>;
    pub fn load_wallet_async(self, persister: ...) -> Result<...>;
    pub fn load_wallet_no_persist(self) -> Result<Option<Wallet>, LoadError>;
}
```

The wallet reads the network from the stored data; `check_network` is a sanity assertion.

### 13.6 Default ports / URLs

BDK has **no notion of default RPC / P2P ports**. P2P/P2W port selection is delegated to
`bdk_esplora` (configurable URL, see §14). Wallet callers pick the URL string.

---

## 14. Integration with other crates

### 14.1 Workspace dependency table (verified on docs.rs `bdk_wallet-3.1.0` page)

| Crate | Cargo.toml | Resolved | Surface used by `bdk_wallet` |
|-------|------------|----------|-------------------------------|
| `bitcoin` (rust-bitcoin) | `^0.32.8` (normal) | `0.32.100` | Re-exported at `bdk_wallet::bitcoin`; gives `Network`, `Transaction`, `TxOut`, `Address`, `ScriptBuf`, `WScriptHash`, `Amount`, `FeeRate`, `psbt::Psbt`, `bip32::Xpriv`/`Xpub`/`Fingerprint`, `secp256k1::Secp256k1`, … |
| `miniscript` | `^12.3.5` (normal) | `12.3.7` | Re-exported at `bdk_wallet::miniscript`; gives `Descriptor`, `Translator`, `policy::Concrete`, `PsbtExt`, `ConversionError`, `TranslatePk` |
| `bdk_chain` | `^0.23.3` (normal) | `0.23.3` | Re-exported at `bdk_wallet::chain`; aliased to `chain`; gives `IndexedTxGraph`, `TxGraph`, `LocalChain`, `CheckPoint`, `Balance`, `SpkIterator`, `KeychainTxoutIndex` |
| `bdk_file_store` | `^0.22.0` (optional) | `0.22.0` | Re-exported at `bdk_wallet::file_store`; gives `Store<C>` (append-only flat-file changeset store) |
| `bip39` | `^2.2.2` (optional) | `2.2.2` | Re-exported at `bdk_wallet::keys::bip39` *only when `keys-bip39` feature is enabled*. Includes `Mnemonic`, `Language`, `WordCount`, `Error` |
| `bdk_esplora` | dev-dep `^0.22.1` | `0.22.2` | Not re-exported; extension traits `EsploraExt`, `EsploraAsyncExt` over `esplora_client` |
| `bdk_electrum` | dev-dep `^0.23.2` | `0.24.0` (transitive) | Not re-exported; wraps `electrum_client` with `BdkElectrumClient` |
| `bdk_bitcoind_rpc` | dev-dep `^0.22.0` | `0.22.0` | Not re-exported; provides `BitcoindClient` against `bitcoind` JSON-RPC |
| `rand_core` | `^0.6.4` (normal) | — | `RngCore` bound on `GeneratableKey::generate_with_aux_rand` |
| `serde` / `serde_json` | `^1` (normal) | `1.0.228` / `1.0.150` | `Descriptor<Xpub>` & `PersistedWallet` JSON persistence |
| `tempfile` | dev + optional | `^3.26.0` | Test fixtures |
| `anyhow` | optional + dev | `^1` | Convenience error conversions |

**`secp256k1` is not a top-level re-export of `bdk_wallet`.** Callers must depend on it
themselves (a compatible version, currently `0.29.x` to match `bitcoin 0.32`).

### 14.2 `bdk_chain ^0.23.3` — what we actually consume

```rust
use bdk_wallet::chain::{
    indexed_tx_graph::IndexedTxGraph,
    local_chain::LocalChain,
    spk_client::{SyncRequest, FullScanRequest},
    tx_graph::TxGraph,
    CheckPoint, BlockId, ConfirmationBlockTime, TxPosInBlock, TxUpdate,
    Balance, ChainPosition, ObservedIn, CanonicalReason,
    Anchor, ChainOracle, DescriptorExt,
};
```

All chain I/O returns `chain::ChainPosition::{Confirmed(..), Unconfirmed(..)}`.

### 14.3 `bdk_esplora ^0.22.2` (dev-dep at the wallet level)

```rust
use esplora_client::{BlockingClient, AsyncClient};   // from the esplora-client re-export
use bdk_esplora::EsploraExt;            // adds .sync(req), .full_scan(req, stop_gap) for blocking
use bdk_esplora::EsploraAsyncExt;       // async counterpart
```

Usage:

```rust
let client = BlockingClient::new("https://mempool.example.com/api")?;
let response = client.sync(req, None)?;        // returns SyncResponse
let response = client.full_scan(req, stop_gap)?; // returns FullScanResponse
```

Note: at docs.rs the live crate version is 0.22.2 (the wallet declares `^0.22.1`).

### 14.4 `bdk_electrum` (dev-dep at the wallet level)

```rust
use bdk_electrum::BdkElectrumClient;
let client = BdkElectrumClient::new("ssl://electrum.example.com:50002")?;
let response = client.sync(req, /* stop_gap */ None)?;
let response = client.full_scan(req, /* stop_gap */ 10, /* batch */ None)?;
```

`BdkElectrumClient::new` takes anything implementing `electrum_client::ElectrumApi`.

### 14.5 `bdk_file_store ^0.22` (optional re-export as `bdk_wallet::file_store`)

```rust
use bdk_wallet::file_store::Store;
let store = Store::new("/var/lib/wallet/store.bin")?;
store.append(&changeset)?;                    // C: PersistedWallet::changeset
let iter = Store::iter()?;
```

Carve-out: docs.rs notes this backend "does not natively support backwards compatible BDK
version upgrades" and is a development/testing store — not production.

### 14.6 `rust-bitcoin` re-exports

`bdk_wallet::bitcoin` is `pub use bitcoin;` plus it is depended on for:

- `Address`, `AddressType`
- `Amount`, `FeeRate`, `SignedAmount`
- `BlockHash`, `BlockHeader`, `Network`, `Params`
- `OutPoint`, `Psbt`, `Sequence`, `Transaction`, `TxIn`, `TxOut`, `Txid`, `Witness`
- `WPubkeyHash`, `WScriptHash`, `ScriptBuf`
- `bip32::{Xpriv, Xpub, DerivationPath, Fingerprint, ChildNumber}`
- `CompressedPublicKey`, `PublicKey`, `XOnlyPublicKey`, `PrivateKey`
- `secp256k1::{Secp256k1, All, VerifyOnly, Verification}` (re-exposed via `bitcoin::secp256k1`)

### 14.7 `rust-miniscript` re-exports

`bdk_wallet::miniscript` is `pub use miniscript;`. Plus `bdk_wallet::descriptor` re-exports:

- `Descriptor`, `DescriptorType`, `DescriptorPublicKey`, `DefiniteDescriptorKey`
- `Legacy`, `Segwitv0`, `Tap` (via `ScriptContextEnum`)
- `policy::Concrete` / `Satisfiable`
- `Translator`, `TranslatePk`
- `DescriptorError`, `ConversionError`, `Error`
- `calc_checksum`, `verify_checksum`

### 14.8 `secp256k1`

- **Not re-exported** by `bdk_wallet` at top level.
- Reachable via `bdk_wallet::bitcoin::secp256k1` (because `bitcoin 0.32` re-exports it).
- Compatible version is `secp256k1 = "0.29"` in your `Cargo.toml` (matches `bitcoin 0.32`).
- Construct contexts with `Secp256k1::new()` (verification) or `Secp256k1::signing_only()`
  depending on whether you'll sign.

---

## What's NOT in BDK 3.1 (for these 5 categories)

- **No top-level `bdk_wallet::Error` enum.** Errors are split into five domain enums in
  `bdk_wallet::error::*`. The legacy `0.x` umbrella `Error` is gone; treat each subsystem's
  enum as the canonical error.
- **No `bdk_wallet::Network` enum.** BDK 3.1 uses `bitcoin::Network` directly.
- **No `keys::ValidNetworks`.** Use `keys::ValidNetworkKinds` (a `BTreeSet<NetworkKind>`).
- **No `MnemonicType`.** Replaced by `bip39::WordCount` (`Mnemonic12/15/18/21/24`).
- **No `DescriptorSecretKey` or `DescriptorPublicKey` constructors.** Use
  `ExtendedPrivKey::from_str` / `ExtendedPubKey::from_str` and wrap via `From` impls.
- **No `Wallet::to_string`.** Only `Debug`. Derive a string via `descriptor_checksum`,
  `wallet_name_from_descriptor`, or `public_descriptor` + `Display`.
- **No notion of default RPC ports / P2P ports.** URLs are user-supplied.
- **No `secp256k1` direct re-export.** Add it as your own dependency.
- **No IPv4Tor or proxy address kinds** in BDK itself — those live in client crates
  (`esplora-client`, `electrum-client`).
- **No `bdk_wallet::descriptor::WalletPolicy` named that way.** Spending policy lives under
  `bdk_wallet::miniscript::policy` (re-exported `Concrete`).
- **No address-prefix constants exported.** They're defined inside `bitcoin::network::constants::params`.
- **`Wallet::descriptor_checksum` only takes a single `KeychainKind`**; there is no convenience
  to dump both checksums — fetch `public_descriptor(...).to_string()` if you need both strings.

---

## What's still uncertain (verify in Task 31 spike)

- `InsufficientFunds { needed, available }` — the doc page summarised shape but did not pin
  the field names; confirm with a `cargo doc`-generated dump.
- `DescriptorError` variant list — re-export from `miniscript`, fetch the up-to-date list
  before reporting.
- `secp256k1` exact resolved version — pull from `cargo tree` on a fresh spike project.
- Whether `Error` returned by `wallet_name_from_descriptor` is the legacy umbrella or a new
  `bdk_wallet::error::Error` alias (`src/bdk_wallet/wallet/mod.rs:2851` line is the source of truth).

---

## Sources (2026-08-05 snapshot)

- https://docs.rs/bdk_wallet/latest/bdk_wallet/keys/index.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/keys/bip39/index.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/keys/bip39/struct.Mnemonic.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/keys/enum.DescriptorSecretKey.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/keys/enum.DescriptorPublicKey.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/keys/enum.KeyError.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/keys/trait.GeneratableKey.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/error/index.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/error/enum.CreateTxError.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/error/enum.BuildFeeBumpError.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/error/enum.LoadError.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/error/enum.LoadMismatch.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/error/enum.MiniscriptPsbtError.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/descriptor/index.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/fn.wallet_name_from_descriptor.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/fn.version.html
- https://docs.rs/bdk_wallet/latest/bdk_wallet/index.html
- https://docs.rs/bdk_chain/latest/bdk_chain/index.html
- https://docs.rs/bdk_esplora/latest/bdk_esplora/index.html
- https://docs.rs/bdk_electrum/latest/bdk_electrum/index.html
- https://docs.rs/bdk_file_store/latest/bdk_file_store/index.html
- https://docs.rs/bitcoin/0.32.100/x86_64-unknown-linux-gnu/bitcoin/network/enum.Network.html
