# rust-bitcoin 0.32 — secp256k1, key, taproot

Scope: enum the **public API surface** of three modules in `rust-bitcoin`
v0.32.11 (BDK 3.1 baseline) and record what is *not* there but might be
expected from an earlier (0.29/0.30) mental model. Source of truth is
docs.rs/bitcoin/0.32.11 + the v0.32.11 source tree.

> Git tag used: **`bitcoin-0.32.11`** (the tag name has the
> `bitcoin-` prefix in the rust-bitcoin monorepo; the bare `v0.32.11`
> tag does not exist). All `file:line` references are against
> `bitcoin-0.32.11`.

---

## 1. `secp256k1` re-export (`bitcoin::secp256k1`)

### 1.1 What bitcoin actually re-exports

The whole `secp256k1` crate is exposed as `pub extern crate secp256k1;`
in `bitcoin/src/lib.rs:83`. There is **no** `bitcoin/src/secp256k1.rs`
wrapper module file. The `bitcoin::key` module additionally
re-exports a curated subset:

```rust
// bitcoin/src/crypto/key.rs:18
#[rustfmt::skip]                // Keep public re-exports separate.
pub use secp256k1::{constants, Keypair, Parity, Secp256k1, Verification, XOnlyPublicKey};

#[cfg(feature = "rand-std")]
pub use secp256k1::rand;
```

So:

| Path                                | What                                                                                  |
| ----------------------------------- | ------------------------------------------------------------------------------------- |
| `bitcoin::secp256k1::*`             | The entire `secp256k1 0.30.0` crate (BIP-340, ecdsa, Keypair, Message, All, etc.).    |
| `bitcoin::key::Secp256k1`           | Alias of `secp256k1::Secp256k1`.                                                      |
| `bitcoin::key::Verification`        | Alias of `secp256k1::Verification` (marker trait).                                    |
| `bitcoin::key::Keypair`             | Alias of `secp256k1::Keypair`.                                                        |
| `bitcoin::key::Parity`              | Alias of `secp256k1::Parity`.                                                         |
| `bitcoin::key::XOnlyPublicKey`      | Alias of `secp256k1::XOnlyPublicKey`.                                                 |
| `bitcoin::key::constants`           | Re-export of `secp256k1::constants` module (curve order, generator, etc.).            |
| `bitcoin::key::rand`                | Re-export of `secp256k1::rand` (`#[cfg(feature = "rand-std")]`).                      |

Notably **NOT** re-exported through `bitcoin::key`:

- `bitcoin::key::Signing` (use `secp256k1::Signing` directly).
- `bitcoin::key::All` (use `secp256k1::All` directly).
- `bitcoin::key::Message` (use `secp256k1::Message` directly).
- `bitcoin::key::SecretKey` (use `secp256k1::SecretKey` directly).
- `bitcoin::key::ecdsa` / `bitcoin::key::schnorr` modules.

If you only have `bitcoin` in `Cargo.toml`, you reach the `secp256k1`
0.30 surface via `bitcoin::secp256k1::*`. Pull in the standalone
`secp256k1 0.30` crate only when you want a different version than the
one bundled inside `bitcoin` 0.32.11.

### 1.2 secp256k1 0.30 surface used by BDK code

Reference: https://docs.rs/secp256k1/0.30.0/secp256k1/ (this is the
exact version re-exported). All signatures below come from that page
or from the public `secp256k1` 0.30 API contract.

| Function                                       | Signature                                                                                              | BDK 3.1 status | Notes |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------ | -------------- | ----- |
| `Secp256k1::new()`                             | `pub fn new() -> Self`                                                                                 | yes            | Returns `Secp256k1<All>` (full precomputed tables; ~10 ms one-shot cost). |
| `Secp256k1::verification_only()`               | `pub fn verification_only() -> Secp256k1<VerifyOnly>`                                                 | yes            | Cheap context — only precomputes the *verifying* table. |
| `Secp256k1::signing_only()`                    | `pub fn signing_only() -> Secp256k1<SignOnly>`                                                         | yes            | Cheap context — only precomputes the *signing* table. |
| `Keypair::from_secret_key`                     | `pub fn from_secret_key<C: Signing>(secp: &Secp256k1<C>, sk: &SecretKey) -> Keypair`                   | yes            | BIP-340 ready keypair from a 32-byte secret. |
| `Keypair::public_key`                          | `pub fn public_key(&self) -> PublicKey`                                                                | yes            | Returns the compressed ECDSA `PublicKey`. |
| `SecretKey::from_slice`                        | `pub fn from_slice(data: &[u8]) -> Result<SecretKey, Error>`                                           | yes            | 32-byte big-endian, must be < curve order. |
| `PublicKey::from_slice`                        | `pub fn from_slice(data: &[u8]) -> Result<PublicKey, Error>`                                           | yes            | Accepts 33-byte compressed (0x02/0x03 prefix) or 65-byte uncompressed (0x04 prefix). |
| `PublicKey::from_secret_key`                   | `pub fn from_secret_key<C: Signing>(secp: &Secp256k1<C>, sk: &SecretKey) -> PublicKey`                 | yes            | Derives compressed pubkey. |
| `Message::from_digest`                         | `pub fn from_digest(digest: [u8; 32]) -> Message`                                                      | yes            | 32-byte fixed array; no hashing performed. |
| `Message::from_digest_slice`                   | `pub fn from_digest_slice(digest: &[u8]) -> Result<Message, Error>`                                    | yes            | Runtime-length-checked slice. |
| `ecdsa::Signature::serialize_compact`          | `pub fn serialize_compact(&self) -> [u8; 64]`                                                           | yes            | R \|\| S, big-endian. |
| `ecdsa::Signature::from_compact`               | `pub fn from_compact(data: &[u8]) -> Result<Signature, Error>`                                         | yes            | |
| `ecdsa::Signature::from_der`                   | `pub fn from_der(data: &[u8]) -> Result<Signature, Error>`                                             | yes            | Strict DER, 70-72 bytes. |
| `XOnlyPublicKey::from`                         | `impl From<PublicKey> for XOnlyPublicKey` (drops the parity bit)                                       | yes            | |
| `XOnlyPublicKey::from_slice`                   | `pub fn from_slice(data: &[u8]) -> Result<XOnlyPublicKey, Error>`                                      | yes            | 32 bytes, must be on curve, may be infinity. |
| `XOnlyPublicKey::serialize`                    | `pub fn serialize(&self) -> [u8; 32]`                                                                  | yes            | BIP-340 serialization. |
| `schnorr::Signature`                           | struct with `serialize() -> [u8; 64]`                                                                   | yes            | BIP-340 64-byte signature. |
| `schnorr::Signature::randomize`                | `pub fn randomize(&self, aux: [u8; 32]) -> Self`                                                       | yes            | BIP-341 deterministic nonce commitment when `aux` is the BIP-341 `tap_tweak` digest. |
| `Secp256k1::sign_ecdsa`                        | `pub fn sign_ecdsa(&self, msg: &Message, sk: &Keypair) -> Signature`                                   | yes            | Requires `C: Signing`. Non-deterministic by default. |
| `Secp256k1::sign_ecdsa_low_r`                  | `pub fn sign_ecdsa_low_r<C: Signing>(&self, msg: &Message, sk: &Keypair) -> Signature`                 | yes            | BIP-146 anti-grinding: bias R to be < L/2. |
| `Secp256k1::verify_ecdsa`                      | `pub fn verify_ecdsa<C: Verification>(&self, msg: &Message, sig: &Signature, pk: &PublicKey) -> Result<(), Error>` | yes       | |
| `Secp256k1::sign_schnorr`                      | `pub fn sign_schnorr<C: Signing>(&self, msg: &Message, kp: &Keypair) -> Signature`                     | yes            | BIP-340 — internally mixes `aux` randomness. |
| `Secp256k1::sign_schnorr_no_aux_rand`          | `pub fn sign_schnorr_no_aux_rand<C: Signing>(&self, msg: &Message, kp: &Keypair) -> Signature`         | yes            | BIP-340 with all-zero aux — deterministic per BIP-340 spec; only safe if `kp` was produced via `Keypair::from_secret_key` (so the secret-key-derived aux is still mixed). |
| `Secp256k1::verify_schnorr`                    | `pub fn verify_schnorr<C: Verification>(&self, msg: &Message, sig: &Signature, xonly: &XOnlyPublicKey) -> Result<(), Error>` | yes | BIP-340 verification. |

> **Caveat:** the `Secp256k1<C>` generic bound means you must construct
> a context that *can* do the operation. `sign_ecdsa` will not compile
> against `Secp256k1<VerifyOnly>` — that's a compile-time gate, not a
> runtime error.

---

## 2. `bitcoin::key` module (`bitcoin/src/crypto/key.rs`)

Source: `bitcoin/src/crypto/key.rs` at tag `bitcoin-0.32.11`.

### 2.1 `PublicKey` struct (lines 39-46)

```rust
/// A Bitcoin ECDSA public key
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey {
    /// Whether this public key should be serialized as compressed
    pub compressed: bool,
    /// The actual ECDSA key
    pub inner: secp256k1::PublicKey,
}
```

Both fields are `pub`, so the `compressed` flag and the inner
`secp256k1::PublicKey` are reachable without a method. This is why
some "expected" methods simply don't exist as methods — you go
through the field.

### 2.2 `PublicKey` inherent impl (lines 48-225)

| Method                        | Signature (Rust)                                                                                                                       | BDK 3.1 status | Notes |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ----- |
| `PublicKey::new`              | `pub fn new(key: impl Into<secp256k1::PublicKey>) -> PublicKey`                                                                       | yes            | Always-compressed constructor (line 49). |
| `PublicKey::new_uncompressed` | `pub fn new_uncompressed(key: impl Into<secp256k1::PublicKey>) -> PublicKey`                                                          | yes            | Always-uncompressed constructor (line 56). |
| `PublicKey::pubkey_hash`      | `pub fn pubkey_hash(&self) -> PubkeyHash`                                                                                             | yes            | HASH160 of the *serialized* key (compressed if `compressed == true`, uncompressed otherwise) — line 65. |
| `PublicKey::wpubkey_hash`     | `pub fn wpubkey_hash(&self) -> Result<WPubkeyHash, UncompressedPublicKeyError>`                                                        | yes            | HASH160 of the compressed serialization — returns `Err` if the key is uncompressed, because segwit requires compressed (line 69). |
| `PublicKey::p2wpkh_script_code` | `pub fn p2wpkh_script_code(&self) -> Result<ScriptBuf, UncompressedPublicKeyError>`                                                 | yes            | Convenience: returns the script code used for P2WPKH sighash (line 80). |
| `PublicKey::write_into`       | `pub fn write_into<W: Write + ?Sized>(&self, writer: &mut W) -> Result<(), io::Error>`                                                | yes            | Compressed/uncompressed write based on `self.compressed` (line 87). |
| `PublicKey::read_from`        | `pub fn read_from<R: Read + ?Sized>(reader: &mut R) -> Result<Self, io::Error>`                                                       | yes            | Detects compressed vs uncompressed from the leading byte (line 96). |
| `PublicKey::to_bytes`         | `pub fn to_bytes(self) -> Vec<u8>`                                                                                                    | yes            | Allocates a `Vec<u8>` of 33 or 65 bytes depending on `compressed` (line 130). |
| `PublicKey::to_sort_key`      | `pub fn to_sort_key(self) -> SortKey`                                                                                                 | yes            | Wraps the serialization in a `SortKey` newtype for BIP-67/`sortedmulti` (line 178). |
| `PublicKey::from_slice`       | `pub fn from_slice(data: &[u8]) -> Result<PublicKey, FromSliceError>`                                                                 | yes            | Accepts 33 **or** 65 bytes; sets `compressed = (len == 33)`; validates the prefix byte 0x04 for uncompressed (line 199). |
| `PublicKey::from_private_key` | `pub fn from_private_key<C: secp256k1::Signing>(secp: &Secp256k1<C>, sk: &PrivateKey) -> PublicKey`                                   | yes            | Constructs a *compressed* `PublicKey` from a `bitcoin::PrivateKey`. Thin wrapper over `sk.public_key(secp)` (line 219). |
| `PublicKey::verify`           | `pub fn verify<C: secp256k1::Verification>(&self, secp: &Secp256k1<C>, msg: &secp256k1::Message, sig: &ecdsa::Signature) -> Result<(), secp256k1::Error>` | yes     | Forwards to `secp.verify_ecdsa(msg, &sig.signature, &self.inner)` (line 226). |

Also re-exported via `From<secp256k1::PublicKey> for PublicKey` (line
232, always-compressed) and `From<PublicKey> for XOnlyPublicKey` (line
236, drops the parity bit).

### 2.3 What is NOT on `PublicKey` (verify in spike)

These names from earlier `rust-bitcoin` 0.29 / generic Bitcoin-API
mental models are *not* methods on `bitcoin::key::PublicKey` in 0.32:

| Missing name                      | Where the functionality actually lives                                                                                                                                                            |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PublicKey::from_secret_key`      | Renamed to **`from_private_key`** in 0.32 (takes `bitcoin::PrivateKey`, not `secp256k1::SecretKey`). If you have a raw `secp256k1::SecretKey`, go through `bitcoin::PrivateKey::from(sk)` then `PublicKey::from_private_key`. |
| `PublicKey::from_compressed_slice` | Merged into **`from_slice`** — pass 33 bytes and you get a compressed key, 65 bytes and you get an uncompressed key.                                                                |
| `PublicKey::from_uncompressed_slice` | Same as above.                                                                                                                                                                          |
| `PublicKey::serialize() -> [u8; 33]` | Not a method. Use `pk.inner.serialize()` (33 bytes) or `pk.inner.serialize_uncompressed()` (65 bytes) — `self.inner` is `pub`.                                              |
| `PublicKey::serialize_uncompressed() -> [u8; 65]` | Same — `pk.inner.serialize_uncompressed()`.                                                                                                                          |
| `PublicKey::is_compressed() -> bool` | Not a method. Read the public field **`pk.compressed`**.                                                                                                                                 |
| `PublicKey::is_fully_valid() -> bool` | **Does not exist in 0.32.** Was a `secp256k1::PublicKey` method in older crates; in 0.32 reach for `pk.inner.is_full_valid(&secp)` if you need the curve check (secp256k1 0.30 still exposes it). |

### 2.4 Companion types in `bitcoin::key`

| Type                    | What                                                                                            | Source ref |
| ----------------------- | ----------------------------------------------------------------------------------------------- | ---------- |
| `CompressedPublicKey`   | Newtype wrapper (`pub struct CompressedPublicKey(pub secp256k1::PublicKey)`) that *guarantees* compressed. Has `pubkey_hash`, `wpubkey_hash`, `p2wpkh_script_code`, `write_into`, `read_from`, `to_bytes`, `from_slice`, `try_from(PublicKey)`. | `crypto/key.rs:286-...` |
| `PrivateKey`            | Wraps `secp256k1::SecretKey`. Has WIF (de)serialization, `from_slice`, `public_key(C)`, `from_random`, `sign`, `add`, `negate`, etc. — see **feature 03 (psbt/bip32)** for the full surface. | `crypto/key.rs:...` (re-exported `crypto::key::{self, PrivateKey, ...}` at `lib.rs:139`) |
| `Keypair`               | Alias of `secp256k1::Keypair` (re-exported `crypto/key.rs:18`). BIP-340 Schnorr keypair.         | `crypto/key.rs:18` |
| `PubkeyHash`, `WPubkeyHash` | `hash_newtype!` newtypes around `hash160::Hash`. `WPubkeyHash` is the segwit (witness-program) flavour. | `crypto/key.rs:281-285` |
| `SortKey`               | `ArrayVec<u8, 65>` newtype returned by `to_sort_key`.                                            | `crypto/key.rs:241` |
| `FromSliceError`        | Enum: `InvalidLength(usize)`, `InvalidKeyPrefix(u8)`, `Secp256k1(secp256k1::Error)`.             | `crypto/key.rs:...` |
| `ParsePublicKeyError`   | For `FromStr`.                                                                                  | `crypto/key.rs:...` |
| `UncompressedPublicKeyError` | Marker error type for segwit-only paths.                                                                            | `crypto/key.rs:...` |
| `XOnlyPublicKey`        | Alias of `secp256k1::XOnlyPublicKey` (re-exported `crypto/key.rs:18`).                          | `crypto/key.rs:18` |
| `Parity`                | Alias of `secp256k1::Parity` (re-exported `crypto/key.rs:18`).                                  | `crypto/key.rs:18` |
| `Verification`          | Alias of `secp256k1::Verification` marker trait (re-exported `crypto/key.rs:18`).                | `crypto/key.rs:18` |
| `Secp256k1`             | Alias of `secp256k1::Secp256k1` (re-exported `crypto/key.rs:18`).                                | `crypto/key.rs:18` |
| `UntweakedPublicKey`, `UntweakedKeypair` | Type aliases for `secp256k1::XOnlyPublicKey` / `secp256k1::Keypair` to mark untweaked input to BIP-341.    | `crypto/key.rs:...` |
| `TweakedPublicKey`, `TweakedKeypair`     | Type aliases for the BIP-340 *output* types (post-tweak).                                                          | `crypto/key.rs:...` |
| `TapTweak`              | Trait: `tap_tweak<C: Verification>(&self, secp: &Secp256k1<C>, merkle_root: Option<TapNodeHash>) -> (TweakedType, Parity)` — implemented for `XOnlyPublicKey` and `Keypair`. | `crypto/key.rs:...` |

> Note: `Signing` and `All` are **not** re-exported through `bitcoin::key`.
> Reach for them via `secp256k1::Signing` / `secp256k1::All` (i.e.
> `bitcoin::secp256k1::Signing`, `bitcoin::secp256k1::All`).

---

## 3. `bitcoin::taproot` module (`bitcoin/src/taproot/mod.rs`)

Source: `bitcoin/src/taproot/mod.rs` at tag `bitcoin-0.32.11` (1890 lines).
Module re-exports at `mod.rs:29-31`:

```rust
pub use crate::crypto::taproot::{SigFromSliceError, Signature};
pub use merkle_branch::TaprootMerkleBranch;
```

### 3.1 `TaprootBuilder` (line 348-583)

```rust
pub struct TaprootBuilder { /* private fields */ }
```

**Constructor / capacity:**

| Method                                | Signature                                                                                            | BDK 3.1 status | Notes |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------- | -------------- | ----- |
| `TaprootBuilder::new`                 | `pub fn new() -> Self`                                                                               | yes            | line 386 — empty builder. |
| `TaprootBuilder::default`             | `impl Default for TaprootBuilder { fn default() -> Self { Self::new() } }`                            | yes            | line 585. |
| `TaprootBuilder::with_capacity`       | `pub fn with_capacity(size: usize) -> Self`                                                          | yes            | line 391 — capacity hint, where `size` should be the *max depth* of the tree. |
| `TaprootBuilder::with_huffman_tree`   | `pub fn with_huffman_tree<I>(script_weights: I) -> Result<Self, TaprootBuilderError> where I: IntoIterator<Item = (u32, ScriptBuf)>` | yes            | line 416 — builds an optimal Huffman tree from script weights. Errors on empty input or depth > 128. |

**DFS-order adders:**

| Method                          | Signature                                                                                                                  | BDK 3.1 status | Notes |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------- | -------------- | ----- |
| `TaprootBuilder::add_leaf_with_ver` | `pub fn add_leaf_with_ver(self, depth: u8, script: ScriptBuf, ver: LeafVersion) -> Result<Self, TaprootBuilderError>`  | yes            | line 448 — explicit leaf version. |
| `TaprootBuilder::add_leaf`      | `pub fn add_leaf(self, depth: u8, script: ScriptBuf) -> Result<Self, TaprootBuilderError>`                                 | yes            | line 462 — defaults to `LeafVersion::TapScript`. |
| `TaprootBuilder::add_hidden_node` | `pub fn add_hidden_node(self, depth: u8, hash: TapNodeHash) -> Result<Self, TaprootBuilderError>`                       | yes            | line 468 — hidden/omitted branch (its hash is treated as opaque). |

**Inspection / consume:**

| Method                                 | Signature                                                                                                                                  | BDK 3.1 status | Notes |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | -------------- | ----- |
| `TaprootBuilder::is_finalizable`       | `pub fn is_finalizable(&self) -> bool`                                                                                                     | yes            | line 478. |
| `TaprootBuilder::try_into_node_info`   | `pub fn try_into_node_info(mut self) -> Result<NodeInfo, IncompleteBuilderError>`                                                          | yes            | line 487 — accepts hidden nodes. |
| `TaprootBuilder::try_into_taptree`     | `pub fn try_into_taptree(self) -> Result<TapTree, IncompleteBuilderError>`                                                                 | yes            | line 500 — fails if hidden nodes present. |
| `TaprootBuilder::has_hidden_nodes`     | `pub fn has_hidden_nodes(&self) -> bool`                                                                                                   | yes            | line 512. |
| `TaprootBuilder::finalize`             | `pub fn finalize<C: secp256k1::Verification>(self, secp: &Secp256k1<C>, internal_key: UntweakedPublicKey) -> Result<TaprootSpendInfo, TaprootBuilder>` | yes            | line 520. Returns the builder unchanged in `Err(TaprootBuilder)` if not finalizable — caller can keep adding leaves. |

> **Note for callers**: `finalize` takes the *untweaked* internal key
> (`UntweakedPublicKey` = `secp256k1::XOnlyPublicKey`). The
> output `TaprootSpendInfo::output_key()` is the *tweaked* `XOnlyPublicKey`.

### 3.2 `TaprootSpendInfo` (line 194-208)

```rust
pub struct TaprootSpendInfo {
    /// The BIP341 internal key.
    internal_key: UntweakedPublicKey,
    /// The merkle root of the script tree (None if there are no scripts).
    merkle_root: Option<TapNodeHash>,
    /// The sign final output pubkey as per BIP 341.
    output_key_parity: secp256k1::Parity,
    /// The tweaked output key.
    output_key: TweakedPublicKey,
    /// Map from (script, leaf_version) to (sets of) [`TaprootMerkleBranch`].
    script_map: ScriptMerkleProofMap,
}
```

**All fields are private.** Read them through the accessors on lines
256-273:

| Method                              | Signature                                                                                  | Notes |
| ----------------------------------- | ------------------------------------------------------------------------------------------ | ----- |
| `TaprootSpendInfo::with_huffman_tree` | `pub fn with_huffman_tree<C, I>(secp: &Secp256k1<C>, internal_key: UntweakedPublicKey, script_weights: I) -> Result<Self, TaprootBuilderError> where C: secp256k1::Verification, I: IntoIterator<Item = (u32, ScriptBuf)>` | line 215 — convenience wrapper over `TaprootBuilder::with_huffman_tree` + `finalize`. |
| `TaprootSpendInfo::new_key_spend`   | `pub fn new_key_spend<C: secp256k1::Verification>(secp: &Secp256k1<C>, internal_key: UntweakedPublicKey, merkle_root: Option<TapNodeHash>) -> Self` | line 239 — BIP-341 key-path-only construction; passing `None` for `merkle_root` still commits to an unspendable script path per the BIP-341 footnote. |
| `TaprootSpendInfo::tap_tweak`       | `pub fn tap_tweak(&self) -> TapTweakHash`                                                  | line 256 — returns the `tap_tweak` hash. |
| `TaprootSpendInfo::internal_key`    | `pub fn internal_key(&self) -> UntweakedPublicKey`                                        | line 261. |
| `TaprootSpendInfo::merkle_root`     | `pub fn merkle_root(&self) -> Option<TapNodeHash>`                                         | line 264. |
| `TaprootSpendInfo::output_key`      | `pub fn output_key(&self) -> TweakedPublicKey`                                            | line 267 — the key used in the script pubkey. |
| `TaprootSpendInfo::output_key_parity` | `pub fn output_key_parity(&self) -> secp256k1::Parity`                                    | line 270 — needed to negate the Schnorr signature. |
| `TaprootSpendInfo::script_map`      | `pub fn script_map(&self) -> &ScriptMerkleProofMap`                                        | line 273. |
| `TaprootSpendInfo::from_node_info`  | `pub fn from_node_info<C: secp256k1::Verification>(secp: &Secp256k1<C>, internal_key: UntweakedPublicKey, node: NodeInfo) -> Self` | line 279 — used internally and by callers that build `NodeInfo` manually. |
| `TaprootSpendInfo::control_block`   | `pub fn control_block(&self, script_ver: &(ScriptBuf, LeafVersion)) -> Option<ControlBlock>` | line 318 — builds the BIP-341 control block for a script spend. |

### 3.3 `LeafVersion` (line 1226-1232)

```rust
pub enum LeafVersion {
    /// BIP-342 tapscript.
    TapScript,
    /// Future leaf version.
    Future(FutureLeafVersion),
}
```

Methods on `LeafVersion`:

| Method                       | Signature                                                  | Notes |
| ---------------------------- | ---------------------------------------------------------- | ----- |
| `LeafVersion::from_consensus` | `pub fn from_consensus(version: u8) -> Result<Self, TaprootError>` | line 1241 — rejects odd LSB and rejects `0x50` (annex prefix). |
| `LeafVersion::to_consensus`  | `pub fn to_consensus(self) -> u8`                          | line 1251. |

Constants used above live in the same module:

| Const                       | Value      |
| --------------------------- | ---------- |
| `TAPROOT_LEAF_TAPSCRIPT`    | `0xc0`     |
| `TAPROOT_LEAF_MASK`         | `0xfe`     |
| `TAPROOT_ANNEX_PREFIX`      | `0x50`     |
| `TAPROOT_CONTROL_BASE_SIZE` | `33`       |
| `TAPROOT_CONTROL_NODE_SIZE` | `32`       |
| `TAPROOT_CONTROL_MAX_NODE_COUNT` | `128` |
| `TAPROOT_CONTROL_MAX_SIZE`  | `33 + 128*32 = 4129` |

### 3.4 `TapNodeHash`, `TapLeafHash`, `TapTweakHash` (lines 60-141)

These are tagged-hash newtypes (sha256 with a domain-separated tag).
Public constructors:

| Type            | Constructor                                                                  | Notes |
| --------------- | ---------------------------------------------------------------------------- | ----- |
| `TapNodeHash`   | `pub fn from_node_hashes(a: TapNodeHash, b: TapNodeHash) -> TapNodeHash` (line 105) — hashes two children. |
| `TapNodeHash`   | `pub fn assume_hidden(hash: [u8; 32]) -> TapNodeHash` (line 128) — bypasses the tag when you already know the tagged-hash output of a hidden branch. |
| `TapNodeHash`   | `impl From<&LeafNode> for TapNodeHash` (line 99). |
| `TapLeafHash`   | `pub fn from_script(script: &Script, ver: LeafVersion) -> TapLeafHash` (line 87). |
| `TapTweakHash`  | `pub fn from_key_and_tweak(internal_key: UntweakedPublicKey, merkle_root: Option<TapNodeHash>) -> TapTweakHash` (line 63). |
| `TapTweakHash`  | `pub fn to_scalar(self) -> Scalar` (line 79). |

> `Signature::randomize(aux)` from §1.2 is the BIP-341 signing hook:
> feed the `TapTweakHash` (or its `Scalar` form) into `randomize` to
> make the Schnorr nonce deterministic and committed to the tweak.

### 3.5 `Address::p2tr_tweaked` — single-key / script-path address

This is **not** in `bitcoin::taproot`; it lives in `bitcoin/src/address/mod.rs`.

```rust
// bitcoin/src/address/mod.rs (around line 481, exact span documented in feature 02)
pub fn p2tr_tweaked(output_key: TweakedPublicKey, hrp: impl Into<KnownHrp>) -> Address
```

Use `p2tr_tweaked(spend_info.output_key(), network.hrp())` to build a
P2TR address from a `TaprootSpendInfo`. For a *raw* internal key plus
optional script tree, use `Address::p2tr` (feature 02).

For full address-side enumeration (`p2pkh`, `p2wpkh`, `p2sh`,
`p2wsh`, `p2tr`, `p2tr_tweaked`, etc.) see `2026-08-05-rust-bitcoin-features-02-script-address-consensus.md`.

### 3.6 `taproot::Signature` (BIP-341 sighash-aware)

Re-exported from `bitcoin::crypto::taproot` (line 29):

```rust
pub use crate::crypto::taproot::{SigFromSliceError, Signature};
```

The `Signature` carries both the 64-byte Schnorr body *and* the
`schnorr_sig_hash_type` (SIGHASH_DEFAULT, SIGHASH_ALL, SIGHASH_SINGLE
+ ANYONECANPAY). See feature 02/03 for the sighash details.

---

## 4. What's NOT in rust-bitcoin 0.32 (secp256k1 / key / taproot scope)

These come up repeatedly in porting code from older BDK or from raw
secp256k1 docs; they are *absent* in v0.32 and require a different
approach:

1. **`bitcoin::key::Signing` and `bitcoin::key::All`** — *not*
   re-exported in `bitcoin::key`. Use `bitcoin::secp256k1::Signing`
   and `bitcoin::secp256k1::All`.

2. **`PublicKey::from_secret_key(secp, &secp256k1::SecretKey)`** —
   the 0.32 method is `from_private_key` and takes `bitcoin::PrivateKey`,
   *not* a raw `secp256k1::SecretKey`. To use a raw secret key, build a
   `bitcoin::PrivateKey` first: `let pk = bitcoin::PrivateKey { inner: sk, compressed: true };`.

3. **`PublicKey::serialize()` / `serialize_uncompressed()`** — gone
   as named methods. The inner `secp256k1::PublicKey` still has them,
   reachable via `pk.inner.serialize()` (33 bytes) or
   `pk.inner.serialize_uncompressed()` (65 bytes).

4. **`PublicKey::is_compressed()`** — gone. Read the `pub` field
   `pk.compressed`.

5. **`PublicKey::is_fully_valid()`** — gone. If you need to validate
   the curve point (e.g. to detect the point-at-infinity), call
   `pk.inner.is_full_valid(&secp)` (secp256k1 0.30 still has it) — or
   reach for `secp256k1::PublicKey::from_slice(...)` which already
   rejects invalid encodings.

6. **`PublicKey::from_compressed_slice` / `from_uncompressed_slice`** —
   gone. `PublicKey::from_slice` accepts both 33-byte and 65-byte
   inputs and decides `compressed` from the length.

7. **`TaprootBuilder::taproot<C>(secp, internal_key)`** — does not
   exist in this form. The 0.32 finalize path is
   `TaprootBuilder::finalize<C>(secp, internal_key)` which returns
   `Result<TaprootSpendInfo, TaprootBuilder>` (unfinalizable builder
   is returned as the `Err` payload so the caller can recover and
   keep adding leaves).

8. **`TaprootBuilder::finalize<C>(secp, output_key_parity)`** — does
   not exist. The current `finalize` takes an
   `UntweakedPublicKey` (the *untweaked* internal key) and computes
   the parity internally. You no longer pass `output_key_parity` in.

9. **`TaprootBuilder::add_leaf(ver, script, merkle_branch)`** — does
   not exist. `add_leaf` in 0.32 is `add_leaf(depth, script)` (and
   `add_leaf_with_ver(depth, script, ver)`). You provide a `depth: u8`
   in DFS-walk order; the merkle branch is derived, not passed in.

10. **`TaprootBuilder::add_hidden_node(ver, script)`** — does not
    exist. The 0.32 version is
    `add_hidden_node(depth: u8, hash: TapNodeHash)`. The `LeafVersion`
    parameter is gone (a hidden branch has no script, so no version),
    and you pass the *already-tagged* `TapNodeHash` (use
    `TapNodeHash::assume_hidden(...)` if you already have the raw bytes).

11. **`taproot::TaprootSigInfo` enum (`Sig`, `KeySpend`,
    `ScriptSpend(…)`)** — does **not** exist in `bitcoin::taproot` in
    v0.32. The parity flag is exposed as a single
    `secp256k1::Parity` field on `TaprootSpendInfo`
    (`output_key_parity()`). BIP-341 sighash + sighash type byte are
    encoded in `taproot::Signature` (re-exported from
    `crypto::taproot`).

12. **`taproot::ScriptMerkleProofMap`** — exists, but it is the
    internal map type backing `TaprootSpendInfo::script_map()`. You
    only get a `&ScriptMerkleProofMap`; you cannot construct it
    directly — populate it through `TaprootBuilder::finalize` /
    `TaprootSpendInfo::from_node_info`.

13. **`bitcoin::secp256k1::*` is the entire crate, not a curated
    subset.** Anything in `secp256k1 0.30` is reachable as
    `bitcoin::secp256k1::...` — including things bitcoin's own
    `key` module deliberately hides. If you want to bind to the same
    secp256k1 version that bitcoin uses, **don't** add a second
    `secp256k1` dep with a different version; pin the same version or
    use the re-export.

---

## Source-of-truth references

- `bitcoin::secp256k1` (re-exported crate) —
  https://docs.rs/secp256k1/0.30.0/secp256k1/
- `bitcoin::key` index — https://docs.rs/bitcoin/0.32.11/bitcoin/key/index.html
- `bitcoin::taproot` index — https://docs.rs/bitcoin/0.32.11/bitcoin/taproot/index.html
- Source tree at tag `bitcoin-0.32.11` —
  https://github.com/rust-bitcoin/rust-bitcoin/tree/bitcoin-0.32.11
- `bitcoin/src/lib.rs:83` (`pub extern crate secp256k1;`)
- `bitcoin/src/crypto/key.rs` (re-exports line 18; `PublicKey` lines 39-225)
- `bitcoin/src/taproot/mod.rs` (LeafVersion line 1226; TaprootSpendInfo line 194;
  TaprootBuilder line 348)
- `bitcoin/src/address/mod.rs` (p2tr_tweaked) — see feature 02 for the
  full signature and surrounding `p2tr` documentation.

## Verifications left for the Task 31 spike

- The exact span for `Address::p2tr_tweaked` in
  `bitcoin/src/address/mod.rs` (line ~481 in the v0.32.11 tag) — feature
  02 has the precise cite; cross-check before publishing.
- Whether `ScriptMerkleProofMap` exposes any constructor besides
  `Default` — the docs.rs page shows it as a return-type only;
  confirm via spike.
- Whether `TAPROOT_LEAF_TAPSCRIPT == 0xc0` matches the value emitted
  by `Address::p2tr` sighash scripts in the wild (it should — see
  BIP-341).
