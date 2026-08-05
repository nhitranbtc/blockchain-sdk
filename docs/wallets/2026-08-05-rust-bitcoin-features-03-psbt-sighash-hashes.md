# rust-bitcoin 0.32.11 — Features 03: PSBT, sighash, hashes

> Source of truth: <https://docs.rs/bitcoin/0.32.11/bitcoin/> and
> <https://docs.rs/bitcoin_hashes/0.14.0/bitcoin_hashes/>
> Repo: <https://github.com/rust-bitcoin/rust-bitcoin>
> Date verified: 2026-08-05
> Scope: this document enumerates the public API of three modules only —
> `bitcoin::psbt`, `bitcoin::sighash`, and the re-exported `bitcoin_hashes`
> crate (the hashes live in their own crate since rust-bitcoin 0.31).

---

## 0. How hashes are organised in 0.32

In rust-bitcoin 0.32 the `hashes` types are **not** a `bitcoin::hashes`
module — they are re-exported from the standalone `bitcoin_hashes ^0.14`
crate ([docs.rs/bitcoin_hashes/0.14.0/bitcoin_hashes/](https://docs.rs/bitcoin_hashes/0.14.0/bitcoin_hashes/)).

```text
bitcoin 0.32.11  ──►  bitcoin_hashes 0.14.0
                       ├── trait Hash
                       ├── trait HashEngine
                       ├── sha256, sha256d, sha256t
                       ├── hash160, ripemd160
                       ├── sha1, sha384, sha512, sha512_256
                       ├── siphash24
                       ├── hmac
                       └── cmp (fixed_time_eq)
```

All hash types are accessible as `bitcoin::hashes::*` because the
top-level crate re-exports the crate:

```rust
use bitcoin::hashes::{sha256, Hash};   // works, re-exported
use bitcoin_hashes::{sha256, Hash};    // also works
```

Confirmed via crate dependency listing on docs.rs/bitcoin/0.32.11.

---

## 1. PSBT module — `bitcoin::psbt`

BIP-174 Partially Signed Bitcoin Transaction. Defined in
`bitcoin/src/psbt/mod.rs` (2 473 lines) — the module docstring explicitly
states: *"Implementation of BIP174 Partially Signed Bitcoin Transaction
Format … except we define PSBTs containing non-standard sighash types as
invalid."*

### 1.1 Module summary table

| Function / type             | Rust signature (0.32.11)                                                                                                                                                                                                          | BDK 3.1 status | Notes |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ----- |
| `Psbt` struct               | `pub struct Psbt { pub unsigned_tx: Transaction, pub version: u32, pub xpub: BTreeMap<Xpub, KeySource>, pub proprietary: BTreeMap<ProprietaryKey, Vec<u8>>, pub unknown: BTreeMap<Key, Vec<u8>>, pub inputs: Vec<Input>, pub outputs: Vec<Output> }` | Wrapped (TxGraph + `Psbt` re-exports) | Source: `psbt/mod.rs:47-66`. `version` defaults to `0` if omitted on the wire (BIP-174). |
| `Psbt::from_unsigned_tx`    | `pub fn from_unsigned_tx(tx: Transaction) -> Result<Self, Error>`                                                                                                                                                              | Wrapped        | Source: `psbt/mod.rs:141-143`. Errors if transaction is not unsigned. |
| `Psbt::serialize`           | `pub fn serialize(&self) -> Vec<u8>`                                                                                                                                                                                             | Wrapped        | Source: `psbt/serialize.rs:53-76`. Binary BIP-174 encoding. |
| `Psbt::serialize_to_writer` | `pub fn serialize_to_writer(&self, w: &mut impl Write) -> Result<usize>`                                                                                                                                                         | Wrapped        | Source: `psbt/serialize.rs:79-81`. Streaming variant. |
| `Psbt::serialize_hex`       | `pub fn serialize_hex(&self) -> String`                                                                                                                                                                                         | Wrapped        | Source: `psbt/serialize.rs:46-50`. |
| `Psbt::deserialize`         | `pub fn deserialize(bytes: &[u8]) -> Result<Self, Error>`                                                                                                                                                                       | Wrapped        | Inverse of `serialize`. |
| `Psbt::from_str`            | `impl FromStr for Psbt` → `type Err = PsbtParseError; fn from_str(s: &str) -> Result<Self, Self::Err>`                                                                                                                            | Wrapped        | Base64 round-trip. |
| `Psbt::to_string`           | `impl Display for Psbt` → base64 string                                                                                                                                                                                          | Wrapped        | |
| `Psbt::extract_tx`          | `pub fn extract_tx(self) -> Result<Transaction, ExtractTxError>`                                                                                                                                                                | Wrapped        | Finalises inputs (writes `final_script_sig` / `final_script_witness` into the unsigned tx). |
| `Psbt::combine`             | `pub fn combine(&mut self, other: Self) -> Result<(), Error>`                                                                                                                                                                    | Wrapped        | BIP-174 combine. |
| `Psbt::sign`                | `pub fn sign<K, I>(&mut self, k: K, sighash: I) -> Result<SigningErrors, SignError> where K: GetKey, I: IntoIterator<Item = (usize, Sign)>; type SigningErrors = BTreeMap<usize, SignError>`                                    | Wrapped        | Per-input signer driver. |
| `Psbt::fee`                 | `pub fn fee(&self) -> Result<Amount, _>`                                                                                                                                                                                        | Wrapped        | input − output. |
| `Psbt::iter_funding_utxos`  | `pub fn iter_funding_utxos(&self) -> impl Iterator<Item = TxOut>`                                                                                                                                                                | Wrapped        | |
| `Input` struct              | 21 public fields (see §1.3)                                                                                                                                                                                                      | Wrapped        | Source: `psbt/map/input.rs:71-133`. |
| `Output` struct             | `pub struct Output { pub redeem_script: Option<ScriptBuf>, pub witness_script: Option<ScriptBuf>, pub bip32_derivation: BTreeMap<PublicKey, KeySource>, pub proprietary: BTreeMap<ProprietaryKey, Vec<u8>>, pub unknown: BTreeMap<Key, Vec<u8>> }` | Wrapped        | Source: `psbt/map/output.rs`. |
| `Input::sighash_type`       | `pub sighash_type: Option<PsbtSighashType>`                                                                                                                                                                                      | Wrapped        | `None` ⇒ use default. |
| `Input::ecdsa_hash_ty`      | `pub fn ecdsa_hash_ty(&self) -> Result<EcdsaSighashType, NonStandardSighashTypeError>`                                                                                                                                            | Wrapped        | Defaults to `EcdsaSighashType::All`. |
| `Input::taproot_hash_ty`    | `pub fn taproot_hash_ty(&self) -> Result<TapSighashType, InvalidSighashTypeError>`                                                                                                                                               | Wrapped        | Defaults to `TapSighashType::Default`. |
| `Input::combine`            | `pub fn combine(&mut self, other: Self)`                                                                                                                                                                                         | Wrapped        | BIP-174 combine. |
| `PsbtSighashType` struct    | `pub struct PsbtSighashType(u32)`; newtype over `u32`                                                                                                                                                                            | Wrapped        | Bit-flag compatible with `EcdsaSighashType`/`TapSighashType`. |
| `GetKey` trait              | `pub trait GetKey { type Error; fn get_key(&self, key_request: KeyRequest) -> Result<Option<PrivateKey>, Self::Error>; }`                                                                                                         | Wrapped        | Implementor supplies keys to `Psbt::sign`. |
| `PartiallySignedTransaction` | **Not in 0.32.11.** No re-export alias.                                                                                                                                                                                          | n/a            | The `Psbt` name is canonical since 0.28. |
| PSBT v2 (BIP-370)           | **Partial** — see §1.5.                                                                                                                                                                                                          | n/a            | |

BDK 3.1 wraps every row marked "Wrapped" by re-exporting the `bitcoin`
PSBT types through `bdk::bitcoin::psbt` and exposing `bdk::Psbt` as a
type alias. See `2026-08-05-rust-bitcoin-features-03-bdk-psbt.md` (to be
written) for the exact BDK re-export list.

### 1.2 `Psbt` struct — fields

```text
pub struct Psbt {
    pub unsigned_tx: Transaction,                    // psbt/mod.rs:48
    pub version: u32,                                // psbt/mod.rs:51
    pub xpub: BTreeMap<Xpub, KeySource>,             // psbt/mod.rs:54
    pub proprietary: BTreeMap<ProprietaryKey, Vec<u8>>,
    pub unknown: BTreeMap<Key, Vec<u8>>,
    pub inputs: Vec<Input>,                          // psbt/mod.rs:62
    pub outputs: Vec<Output>,                        // psbt/mod.rs:65
}
```

`unsigned_tx` must have empty scriptSigs and witnesses when constructing a
PSBT. The `version` field is documented as *"If omitted, the version
number is 0"* (psbt/mod.rs:51 docstring).

### 1.3 `Input` struct — fields (21 in total)

Source: `psbt/map/input.rs:71-133`.

```text
pub struct Input {
    pub non_witness_utxo:        Option<Transaction>,
    pub witness_utxo:            Option<TxOut>,
    pub partial_sigs:            BTreeMap<PublicKey, Signature>,
    pub sighash_type:            Option<PsbtSighashType>,
    pub redeem_script:           Option<ScriptBuf>,
    pub witness_script:          Option<ScriptBuf>,
    pub bip32_derivation:        BTreeMap<PublicKey, KeySource>,
    pub final_script_sig:        Option<ScriptBuf>,
    pub final_script_witness:    Option<Witness>,
    pub ripemd160_preimages:     BTreeMap<Hash, Vec<u8>>,
    pub sha256_preimages:        BTreeMap<Hash, Vec<u8>>,
    pub hash160_preimages:       BTreeMap<Hash, Vec<u8>>,
    pub hash256_preimages:       BTreeMap<Hash, Vec<u8>>,
    pub tap_key_sig:             Option<Signature>,
    pub tap_script_sigs:         BTreeMap<(XOnlyPublicKey, TapLeafHash), Signature>,
    pub tap_scripts:             BTreeMap<ControlBlock, (ScriptBuf, LeafVersion)>,
    pub tap_key_origins:         BTreeMap<XOnlyPublicKey, (Vec<TapLeafHash>, KeySource)>,
    pub tap_internal_key:        Option<XOnlyPublicKey>,
    pub tap_merkle_root:         Option<TapNodeHash>,
    pub proprietary:             BTreeMap<ProprietaryKey, Vec<u8>>,
    pub unknown:                 BTreeMap<Key, Vec<u8>>,
}
```

### 1.4 `PsbtSighashType`

```text
pub struct PsbtSighashType(u32);   // newtype, psbt/serialize.rs (re-exports)
```

The PSBT-level sighash type encodes any bitwise combination of
`SIGHASH_ALL` (0x01), `SIGHASH_NONE` (0x02), `SIGHASH_SINGLE` (0x03),
and `SIGHASH_ANYONECANPAY` (0x80). The Bitcoin library rejects PSBTs
whose sighash type would not be relayable (the module docs say:
*"we define PSBTs containing non-standard sighash types as invalid"*).

Convert to the canonical typed enums via:

```text
Input::ecdsa_hash_ty(&self)   -> Result<EcdsaSighashType, NonStandardSighashTypeError>
Input::taproot_hash_ty(&self) -> Result<TapSighashType, InvalidSighashTypeError>
```

### 1.5 PSBT v2 (BIP-370) status in 0.32.11

**Answer: PSBT v2 is NOT supported in bitcoin 0.32.11.**

Evidence (multi-source):

1. The `Psbt` struct in 0.32.11 exposes only `unsigned_tx`, `version`,
   `xpub`, `proprietary`, `unknown`, `inputs`, `outputs`. The BIP-370
   fields `tx_modifiable`, `global_scalar`, and the new per-input
   `previous_txid` / `spent_outputs` / `tap_key_sig` are absent.
2. The `unsigned_tx` field is `Transaction` (non-optional), while BIP-370
   requires it to be optional (a v2 PSBT must reconstruct the unsigned
   tx from `inputs[].previous_txid` + `inputs[].spent_outputs`).
3. PR [#3424 "Add PSBT v2 fields"](https://github.com/rust-bitcoin/rust-bitcoin/pull/3424)
   by `tcharding`, dated 2024-09-29, explicitly adds the BIP-370
   fields and notes *"Add the new fields for BIP-370 PSBT version 2.
   For code that uses the `unsigned_tx` which is now optional just
   `unwrap` and document panic if called with v2."* This work has not
   landed in 0.32.x — it was superseded by the rewrite in commits
   `38dd041` ("Add PSBT v2 fields") and `fea379b` ("Add PSBTv2 fields",
   2025-02-24) which targeted `master`/future releases.
4. The latest `bitcoin::psbt::Psbt` docs page (which is from
   `master`, not 0.32.11) confirms the v2 work landed there:
   `pub struct Psbt { pub unsigned_tx: Transaction, pub version: u32, … }` —
   but `version` only persists a `u32`; full v2 round-trip and v0/v2
   mutual conversion remain a work in progress on master.

**Practical impact for BDK / Tangem work**: 0.32.11 PSBTs are BIP-174
(v0) only. Anything that says it is "PSBT v2" in a wallet context
cannot be parsed or written by 0.32.11 — verify in Task 31 spike.

---

## 2. Sighash module — `bitcoin::sighash`

Source: `bitcoin/src/crypto/sighash.rs` (2 219 lines). Module docstring:
*"Signature hash implementation … BIP341, BIP143, and legacy"*.

### 2.1 Module summary table

| Item                                   | Rust signature (0.32.11)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | BDK 3.1 status | Notes |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------- | ----- |
| `SighashCache<T: Borrow<Transaction>>` | `pub struct SighashCache<T: Borrow<Transaction>> { /* private fields */ }`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Wrapped        | Defined at `sighash.rs:75-89`. All signing is built around this cache. |
| `SighashCache::new`                    | `pub fn new(tx: R) -> Self` where `R: Borrow<Transaction>`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Wrapped        | `sighash.rs:584`. Constructs from owned tx or `&tx`. |
| `SighashCache::transaction`            | `pub fn transaction(&self) -> &Transaction`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Wrapped        | `sighash.rs:587`. |
| `SighashCache::into_transaction`       | `pub fn into_transaction(self) -> R`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Wrapped        | `sighash.rs:591`. Consumes cache, recovers tx. |
| `SighashCache::taproot_signature_hash` | `pub fn taproot_signature_hash<T: Borrow<TxOut>>(&mut self, input_index: usize, prevouts: &Prevouts<'_, T>, annex: Option<Annex<'_>>, leaf_hash_code_separator: Option<(TapLeafHash, u32)>, sighash_type: TapSighashType) -> Result<TapSighash, TaprootError>`                                                                                                                                                                                                                                                                                                                                                                                       | Wrapped        | `sighash.rs:733-750`. BIP-341 — universal entry. |
| `SighashCache::taproot_key_spend_signature_hash` | `pub fn taproot_key_spend_signature_hash<T: Borrow<TxOut>>(&mut self, input_index: usize, prevouts: &Prevouts<'_, T>, sighash_type: TapSighashType) -> Result<TapSighash, TaprootError>`                                                                                                                                                                                                                                                                                                                                                                                                                                                | Wrapped        | `sighash.rs:756-774`. Key-path only. |
| `SighashCache::taproot_script_spend_signature_hash` | `pub fn taproot_script_spend_signature_hash<S: Into<TapLeafHash>, T: Borrow<TxOut>>(&mut self, input_index: usize, prevouts: &Prevouts<'_, T>, leaf_hash: S, sighash_type: TapSighashType) -> Result<TapSighash, TaprootError>`                                                                                                                                                                                                                                                                                                                                                                                                       | Wrapped        | `sighash.rs:782-834`. Assumes `OP_CODESEPARATOR = 0xFFFFFFFF`; use `taproot_encode_signing_data_to` for custom values. |
| `SighashCache::taproot_encode_signing_data_to` | `pub fn taproot_encode_signing_data_to<W: Write + ?Sized, T: Borrow<TxOut>>(&mut self, writer: &mut W, input_index: usize, prevouts: &Prevouts<'_, T>, annex: Option<Annex<'_>>, leaf_hash_code_separator: Option<(TapLeafHash, u32)>, sighash_type: TapSighashType) -> Result<(), SigningDataError<TaprootError>>`                                                                                                                                                                                                                                                                                                                | Wrapped        | `sighash.rs:711-730`. Lowest-level taproot encoder. |
| `SighashCache::segwit_v0_encode_signing_data_to` | `pub fn segwit_v0_encode_signing_data_to<W: Write + ?Sized>(&mut self, writer: &mut W, input_index: usize, script_code: &Script, value: Amount, sighash_type: EcdsaSighashType) -> Result<(), SigningDataError<InputsIndexError>>`                                                                                                                                                                                                                                                                                                                                                                                                            | Wrapped        | `sighash.rs:840-859`. |
| `SighashCache::p2wpkh_signature_hash` | `pub fn p2wpkh_signature_hash(&mut self, input_index: usize, script_pubkey: &Script, value: Amount, sighash_type: EcdsaSighashType) -> Result<SegwitV0Sighash, P2wpkhError>`                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | Wrapped        | `sighash.rs:862-879`. Use `Script::p2wpkh_script_code(&pk)` for `script_pubkey` here. |
| `SighashCache::p2wsh_signature_hash`  | `pub fn p2wsh_signature_hash(&mut self, input_index: usize, witness_script: &Script, value: Amount, sighash_type: EcdsaSighashType) -> Result<SegwitV0Sighash, InputsIndexError>`                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Wrapped        | `sighash.rs:903-1006`. BIP-143 for P2WSH. |
| `SighashCache::legacy_signature_hash` | `pub fn legacy_signature_hash(&self, input_index: usize, script_pubkey: &Script, sighash_type: u32) -> Result<LegacySighash, InputsIndexError>`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Wrapped        | `sighash.rs:1103-1135`. **NOTE: takes `u32`, not `EcdsaSighashType`** (legacy hashes 4 bytes; only the low byte is appended to the sig). Handles the SIGHASH_SINGLE bug by returning the "one array". |
| `SighashCache::legacy_encode_signing_data_to` | `pub fn legacy_encode_signing_data_to<W: Write + ?Sized, U: Into<u32>>(&self, writer: &mut W, input_index: usize, script_pubkey: &Script, sighash_type: U) -> EncodeSigningDataResult<SigningDataError<InputsIndexError>>`                                                                                                                                                                                                                                                                                                                                                                                                                  | Wrapped        | `sighash.rs:1027-1042`. Low-level legacy encoder. **Does NOT handle the SIGHASH_SINGLE bug** — caller must check `EncodeSigningDataResult::is_sighash_single_bug()`. |
| `SighashCache::witness_mut`           | `pub fn witness_mut(&mut self, input_index: usize) -> Option<&mut Witness>` (impl `<R: BorrowMut<Transaction>>`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | Wrapped        | `sighash.rs:1132-1134`. Lets you write signatures back into the witness after signing. |
| `Annex` (struct, BIP-341)             | `pub struct Annex<'a>(pub &'a [u8]);` — wrapper that enforces first byte is `0x50`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | Wrapped        | Construct via `Annex::new(&bytes)?` (returns `AnnexError`). |
| `AnnexError`                           | `pub enum AnnexError { InvalidPrefix /* … */ }`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Wrapped        | |
| `Prevouts<'a, T>` (enum)               | `pub enum Prevouts<'a, T: Borrow<TxOut>> { One(TxOut), All(&'a [T]) }`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Wrapped        | `Prevouts::One` may be used when sighash is `SIGHASH_ANYONECANPAY`. |
| `ScriptPath` (struct)                  | `pub struct ScriptPath<'a> { pub script: &'a Script, pub leaf_hash: TapLeafHash, pub leaf_version: LeafVersion, /* … */ }`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Wrapped        | Used by script-path spend signing. |
| `TapSighash` (struct)                  | `pub struct TapSighash(pub Hash);` — tagged hash (tag = `"TapSighash"`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Wrapped        | 32-byte output. Pass to `secp256k1::Message::from_digest(*tapsighash.as_ref())` for BIP-340 signing. |
| `TapSighashTag`                        | `pub struct TapSighashTag;`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Wrapped        | Tag used to construct `TapSighash`. |
| `TapSighashType` (enum)                | `pub enum TapSighashType { Default = 0x00, All = 0x01, None = 0x02, Single = 0x03, AllPlusAnyoneCanPay = 0x81, NonePlusAnyoneCanPay = 0x82, SinglePlusAnyoneCanPay = 0x83 }`                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Wrapped        | "Fixed values so they can be cast as integer types for encoding." |
| `EcdsaSighashType` (enum)              | `pub enum EcdsaSighashType { All = 0x01, None = 0x02, Single = 0x03, AllPlusAnyoneCanPay = 0x81, NonePlusAnyoneCanPay = 0x82, SinglePlusAnyoneCanPay = 0x83 }`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Wrapped        | Same flag bits as `PsbtSighashType` low byte. |
| `SegwitV0Sighash` (struct)             | `pub struct SegwitV0Sighash(pub Hash);`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Wrapped        | 32-byte output of `p2wpkh_signature_hash` / `p2wsh_signature_hash`. |
| `LegacySighash` (struct)               | `pub struct LegacySighash(pub Hash);`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Wrapped        | 32-byte output. |
| `TapLeafHash`                          | Re-exported from `bitcoin` crate root: `pub struct TapLeafHash(pub Sha256);`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Wrapped        | Defined in `bitcoin/src/taproot.rs`. |
| `secp256k1::Message::from_digest`      | `impl Message { pub fn from_digest(digest: [u8; 32]) -> Self; }` (from `secp256k1 ^0.29`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Wrapped        | Pass `*segwit_sighash.to_byte_array()` / `*tapsighash.to_byte_array()` here. There is **no** `From<SegwitV0Sighash>` blanket impl — you must call `to_byte_array()` yourself. |
| `tx_signature_hash`                    | **Not present in 0.32.11.**                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | n/a            | Replaced by the per-script-type methods above. |

### 2.2 Recommended signing pattern

```rust
use bitcoin::hashes::Hash;
use bitcoin::sighash::{SighashCache, EcdsaSighashType};
use bitcoin::secp256k1::{Message, Secp256k1};
use bitcoin::Witness;

let mut cache = SighashCache::new(&tx);
let sighash = cache.p2wpkh_signature_hash(
    input_index,
    &utxo.script_pubkey,
    utxo.value,
    EcdsaSighashType::All,
)?;
let msg = Message::from_digest(*sighash.to_byte_array());
let sig = secp.sign_ecdsa(&msg, &sk);

// Write the signature back into the witness.
*cache.witness_mut(input_index).unwrap() = Witness::p2wpkh(&sig, &pk);
```

Source: `sighash/struct.SighashCache.html` example block.

### 2.3 Sighash return-type gotcha (0.32.x)

Unlike 0.31 which returned a `Message`, in 0.32 the methods return
*newtype structs* (`SegwitV0Sighash`, `TapSighash`, `LegacySighash`).
Each implements `Hash` (from `bitcoin_hashes`), so the standard
sighash-byte-array conversion is:

```text
let bytes: [u8; 32] = *sighash.to_byte_array();
```

The page-level "WARN: no blanket `From<Sighash> for Message`" is the
most common porting bug when moving code from rust-bitcoin <0.31 to
≥0.31. Verify in Task 31 spike against the actual `secp256k1` 0.29
imports BDK pins.

---

## 3. Hashes module — `bitcoin::hashes` (re-export of `bitcoin_hashes ^0.14`)

Crate: <https://docs.rs/bitcoin_hashes/0.14.0/bitcoin_hashes/>
Crate version bundled by `bitcoin 0.32.11`: `bitcoin_hashes ^0.14.0`
(verified in `bitcoin` crate dependencies).

### 3.1 Module summary table

| Item                                       | Rust signature (bitcoin_hashes 0.14)                                                                                                                                                                                                                                                                                                                              | BDK 3.1 status | Notes |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ----- |
| `Hash` trait                               | `pub trait Hash: sealed::Sealed + Hash + Eq + Ord + Copy + Clone + Hash + Display + FromStr + Borrow<[u8]> + Index<usize, Output = u8> + Index<RangeFull, Output = [u8]> { type Engine: HashEngine; const LEN: usize; fn from_byte_array(bytes: [u8; LEN]) -> Self; fn to_byte_array(self) -> [u8; LEN]; fn from_engine(engine: Self::Engine) -> Self; /* hash() default via engine */ }` | Wrapped        | The root trait every hash implements. `LEN` is fixed per type. |
| `Hash::hash` (default)                     | `fn hash(bytes: &[u8]) -> Self { … engine … }`                                                                                                                                                                                                                                                                                                                    | Wrapped        | One-shot; concrete impls may override. |
| `Hash::hash_engine`                        | `fn hash_engine() -> Self::Engine { … }`                                                                                                                                                                                                                                                                                                                          | Wrapped        | Fresh incremental hasher. |
| `Hash::from_slice`                         | `fn from_slice(sl: &[u8]) -> Result<Self, FromSliceError>`                                                                                                                                                                                                                                                                                                         | Wrapped        | Errors if `sl.len() != Self::LEN`. |
| `Hash::to_hex` / `from_hex`                | via `hex-conservative` re-export                                                                                                                                                                                                                                                                                                                                  | Wrapped        | |
| `HashEngine` trait                         | `pub trait HashEngine: io::Write + Default + Clone { type Hash: Hash; fn finalize(self) -> Self::Hash; fn n(&self) -> usize; fn internal_byte_length(&self) -> usize; }`                                                                                                                                                                                              | Wrapped        | Drives incremental hashing (write bytes → finalize). |
| `sha256::Hash` / `HashEngine`              | `pub struct Hash(pub [u8; 32]); pub struct HashEngine(sha256::State);`                                                                                                                                                                                                                                                                                              | Wrapped        | 32-byte SHA-256. |
| `sha256d::Hash` (double SHA-256)           | `pub struct Hash(pub [u8; 32]);`                                                                                                                                                                                                                                                                                                                                   | Wrapped        | Used for txid, block hash, merkle nodes. |
| `sha256t::Hash<TAG>` (tagged SHA-256)      | `pub struct Hash<T: Tag>(pub [u8; 32]);` — `sha256t_hash_newtype!` macro                                                                                                                                                                                                                                                                                            | Wrapped        | Used for TapLeafHash, TapBranchHash, etc. |
| `hash160::Hash`                            | `pub struct Hash(pub [u8; 20]);`                                                                                                                                                                                                                                                                                                                                   | Wrapped        | RIPEMD160(SHA256(x)) — 20 bytes. Used for P2PKH/P2SH inside P2WPKH. |
| `ripemd160::Hash`                          | `pub struct Hash(pub [u8; 20]);`                                                                                                                                                                                                                                                                                                                                   | Wrapped        | 20-byte RIPEMD-160. |
| `sha1::Hash`                               | `pub struct Hash(pub [u8; 20]);`                                                                                                                                                                                                                                                                                                                                   | Wrapped        | Only present for legacy interop. |
| `sha512::Hash`                             | `pub struct Hash(pub [u8; 64]);`                                                                                                                                                                                                                                                                                                                                   | Wrapped        | Not used in consensus. |
| `sha512_256::Hash`                         | `pub struct Hash(pub [u8; 32]);`                                                                                                                                                                                                                                                                                                                                   | Wrapped        | SHA-512 truncated to 256 — used by Lightning / Elements. |
| `sha384::Hash`                             | `pub struct Hash(pub [u8; 48]);`                                                                                                                                                                                                                                                                                                                                   | Wrapped        | Not used in consensus. |
| `siphash24::Hash`                          | `pub struct Hash(pub u64);`                                                                                                                                                                                                                                                                                                                                        | Wrapped        | 8-byte SipHash 2-4 (used by Bitcoin Core for its in-memory keyset hash). |
| `hmac::Hmac<T: Hash>`                      | `pub struct Hmac<T: Hash>(pub T); pub struct HmacEngine<T: Hash>(engine: T::Engine);`                                                                                                                                                                                                                                                                               | Wrapped        | Generic HMAC. Use `HmacEngine::new(key)` → `.write_all(...)` → `Hmac::from_engine(engine)`. Drives BIP-32 HMAC-SHA512, BIP-39 mnemonic seed, and ChaCha20-Poly1305. |
| `cmp::fixed_time_eq`                       | `pub fn fixed_time_eq(a: &[u8], b: &[u8]) -> bool`                                                                                                                                                                                                                                                                                                                  | Wrapped        | Constant-time byte slice equality. |
| `FromSliceError`                           | `pub struct FromSliceError { pub expected: usize, pub actual: usize }`                                                                                                                                                                                                                                                                                              | Wrapped        | Returned by `Hash::from_slice`. |
| `hash_newtype!` macro                      | `pub use bitcoin_hashes::hash_newtype;`                                                                                                                                                                                                                                                                                                                            | Wrapped        | Declares a `pub struct Foo(Inner);` and impls `Hash`/`Display`/`FromStr`. |
| `sha256t_hash_newtype!` macro              | `pub use bitcoin_hashes::sha256t_hash_newtype;`                                                                                                                                                                                                                                                                                                                    | Wrapped        | For tagged-hash newtypes. |
| `hex_fmt_impl!` / `borrow_slice_impl!`     | helpers for trait implementations                                                                                                                                                                                                                                                                                                                                  | Wrapped        | |
| `serde_impl!`                              | Serde impls for byte-newtypes (feature-gated)                                                                                                                                                                                                                                                                                                                       | Wrapped        | |

### 3.2 Common usage patterns

```rust
use bitcoin::hashes::{sha256, hash160, Hash, HashEngine, hmac, ripemd160};

// One-shot.
let txid = sha256d::Hash::hash(&serialised_tx);

// Incremental.
let mut e = sha256::HashEngine::default();
e.write_all(b"hello").unwrap();
e.write_all(b" world").unwrap();
let h = sha256::Hash::from_engine(e);

// RIPEMD160(SHA256(x)).
let pkh = hash160::Hash::hash(&pubkey_bytes);

// HMAC-SHA512 — BIP-32.
let mut mac = hmac::HmacEngine::<sha512::Hash>::new(b"seed");
mac.write_all(b"input").unwrap();
let out = hmac::Hmac::<sha512::Hash>::from_engine(mac);

// Constant-time compare.
use bitcoin::hashes::cmp::fixed_time_eq;
assert!(fixed_time_eq(a, b));
```

---

## 4. What's NOT in rust-bitcoin 0.32 (for these three categories)

| Missing                                       | Where you'd find it                                                                                                                                                          |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PartiallySignedTransaction` type alias      | Removed long before 0.32; the canonical name is `Psbt`. If a downstream project needs the old name, define `type PartiallySignedTransaction = Psbt;` in your own crate.     |
| PSBT v2 (BIP-370)                             | **Not implemented** in 0.32.11. The v2 fields (`tx_modifiable`, `global_scalar`, per-input `previous_txid`, `spent_outputs`, etc.) only landed on `master` via PR #3424 / commits `38dd041` / `fea379b`. Anything BIP-370 in 0.32 will panic on `unwrap()` of the still-mandatory `unsigned_tx`. |
| `tx_signature_hash` (universal)               | Removed pre-0.32. Use the per-script-type methods (`p2wpkh_signature_hash`, `p2wsh_signature_hash`, `legacy_signature_hash`, `taproot_*_signature_hash`).                     |
| `From<Sighash> for secp256k1::Message` blanket| Not present in 0.32. You must call `sighash.to_byte_array()` and then `Message::from_digest(*bytes)`.                                                                       |
| `SchnorrSighashType`                          | Renamed to `TapSighashType` in 0.31 (the docs explicitly call this out). Old code referencing `SchnorrSighashType` will fail to compile.                                      |
| `sha256d::Hash::hash()` convenience           | Not a free function — use `sha256d::Hash::hash(bytes)` (one-shot) which internally composes two SHA-256 engines. There is no `bitcoin::hashes::sha256d()` shortcut.            |
| `siphash48::Hash`                             | Only `siphash24` is exported by `bitcoin_hashes 0.14`. SipHash-2-4 only.                                                                                                     |
| `bitcoin::hashes` module path                 | The module is not re-defined inside `bitcoin 0.32`; you access it via the **crate re-export**. Writing `use bitcoin::hashes;` works because `bitcoin` re-exports the crate, but `mod hashes` does not exist. |

---

## 5. Sources / verification trail

- `https://docs.rs/bitcoin/0.32.11/bitcoin/psbt/index.html` — module index, struct list.
- `https://docs.rs/bitcoin/0.32.11/bitcoin/psbt/struct.Psbt.html` — `Psbt` fields and methods, source: `psbt/mod.rs:47-66`, `psbt/mod.rs:141-143`, `psbt/serialize.rs:43-129`.
- `https://docs.rs/bitcoin/0.32.11/bitcoin/psbt/struct.Input.html` — all 21 fields, source: `psbt/map/input.rs:71-133`.
- `https://docs.rs/bitcoin/0.32.11/bitcoin/sighash/index.html` — module index (struct/enum list).
- `https://docs.rs/bitcoin/0.32.11/bitcoin/sighash/struct.SighashCache.html` — `SighashCache` fields, methods, signatures, sources (`sighash.rs:75-89`, `sighash.rs:573-1135`).
- `https://docs.rs/bitcoin_hashes/0.14.0/bitcoin_hashes/` — crate root (re-exports `Hash`, `HashEngine`, all submodules, macros).
- `https://github.com/rust-bitcoin/rust-bitcoin/pull/3424` — PSBT v2 work, dated 2024-09-29.
- `https://github.com/rust-bitcoin/rust-bitcoin/commit/38dd041` and `fea379b` — PSBT v2 commits on master (not in 0.32.11).
- `https://github.com/rust-bitcoin/rust-bitcoin` — repo.

**Verification caveat**: PSBT v2 status was inferred by combining (a)
the absence of BIP-370 fields in the 0.32.11 `Psbt` struct definition,
(b) the module-level docstring still being BIP-174-only, and (c) the
existence of in-flight PRs targeting master. A direct compile of a
BIP-370 PSBT against 0.32.11 has not been performed in this research —
verify in Task 31 spike before committing to "no v2 support".

---

## 6. Hand-off summary for BDK / Tangem work

1. **PSBT (0.32.11)**: BIP-174 only. Use `Psbt::from_unsigned_tx` →
   mutate `inputs[i]` → `Psbt::sign(..., sighash)` →
   `Psbt::serialize()` / `extract_tx()`.
2. **PSBT v2**: not available. Plan a follow-up against `bitcoin`
   master or `rust-psbt` crate (see `tcharding/psbt-v2` discussion in
   PR #3424).
3. **Sighash**: always go through `SighashCache`. Use the typed
   `SegwitV0Sighash` / `TapSighash` / `LegacySighash` return types and
   `to_byte_array()` → `Message::from_digest`. Watch the legacy
   signature-hash single-bug path.
4. **Hashes**: prefer `bitcoin::hashes::*` re-exports, not the
   underlying `bitcoin_hashes` crate directly, so dependency pinning
   stays single-sourced.
