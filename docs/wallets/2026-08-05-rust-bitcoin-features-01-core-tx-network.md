# rust-bitcoin 0.32 — Public API: Core Types, Transactions, Network

> Deep-research spike for the Digital Euro wallet spike. Scope is intentionally
> limited to **3 modules**: core types, transactions, and the `network`
> module. Source-of-truth is **rust-bitcoin 0.32.11** (docs.rs).
> Cite URL or `file:line` for every claim. Where I could not verify from the
> paid docs surface, I write "verify in Task 31 spike".

**Author:** nhitran · 2026-08-05
**Versions verified:** `bitcoin = "0.32.11"` on docs.rs (built 22 July 2026)
**Crates.io:** https://crates.io/crates/bitcoin
**Repository:** https://github.com/rust-bitcoin/rust-bitcoin
**Master crate root:** https://docs.rs/bitcoin/0.32.11/bitcoin/
**blockdata module:** https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/index.html
**network module:** https://docs.rs/bitcoin/0.32.11/bitcoin/network/index.html
**consensus module:** https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/index.html

---

## 0. How this crate is laid out (orientation)

The top-level `bitcoin` crate is a flat re-export module. Almost every
type you want is either:

1. A **direct re-export** at `bitcoin::*` (e.g. `bitcoin::Amount`,
   `bitcoin::Transaction`, `bitcoin::Network`, `bitcoin::Witness`).
2. Re-exported via the **typed `bitcoin-units` crate** for
   `Amount`, `SignedAmount`, `FeeRate`, `Weight` — the crate docs
   call this out explicitly: *"Re-export everything from the
   [`units::fee_rate`] module"*.
   - See: `bitcoin::FeeRate` docs at
     https://docs.rs/bitcoin/0.32.11/bitcoin/struct.FeeRate.html
   - Underlying module: https://docs.rs/bitcoin-units/0.1.101/x86_64-unknown-linux-gnu/bitcoin_units/fee_rate/index.html
3. Defined in a sub-module (`bitcoin::absolute`, `bitcoin::relative`,
   `bitcoin::block`, `bitcoin::witness`, `bitcoin::transaction`,
   `bitcoin::network`, `bitcoin::consensus`, ...).

The crate root at https://docs.rs/bitcoin/0.32.11/bitcoin/ lists every
public item; 100% of the crate is documented.

**Mandatory dependency:** `bitcoin-units ^0.1.3` (carries
`Amount`/`SignedAmount`/`FeeRate`/`Weight`).
Source: crate `Dependencies` block at
https://docs.rs/bitcoin/0.32.11/bitcoin/.

---

## 1. Core types — summary table

> All items below are documented at the crate root unless they live in
> a sub-module.

| Function / type | Rust signature (canonical form) | BDK 3.1 status | Notes |
| --- | --- | --- | --- |
| `bitcoin::Amount` | `pub struct Amount { sat: u64 }` | direct re-export | `from_sat`, `to_sat`, `from_btc`, `to_btc`, plus checked / saturating constructors and `Display`. |
| `bitcoin::SignedAmount` | `pub struct SignedAmount { sat: i64 }` | direct re-export | Required for change-output math and fee deltas; same arithmetic API as `Amount`. |
| `bitcoin::FeeRate` | `pub struct FeeRate { sat_per_vb: u64 }` (re-exported from `bitcoin_units::fee_rate`) | direct re-export | `from_sat_per_vb`, `from_sat_per_kvb`, `fee_vb`, `fee_kvb`. |
| `bitcoin::OutPoint` | `pub struct OutPoint { txid: Txid, vout: u32 }` | direct re-export | `new(txid, vout)`, `txid()`, `vout()`, plus `ConsensusEncodable`/`Decodable`. |
| `bitcoin::Sequence` | `pub struct Sequence(SequenceInner)` | direct re-export | BIP-68 + BIP-125. Constructors `ZERO`, `MAX`, `from_seconds`, `from_blocks`, `from_512_second_intervals`, plus `is_rbf`, `is_final`, `is_relative_lock_time`. |
| `bitcoin::LockTime` (alias) | `pub use absolute::LockTime;` | re-export | Crate-root alias for `absolute::LockTime`. |
| `bitcoin::absolute::LockTime` | `pub enum LockTime { Blocks(Height), Seconds(Time) }` | direct use | `from_height`, `from_time`, `is_block_height`, `is_block_time`, `as_u32`. |
| `bitcoin::relative::LockTime` | `pub enum LockTime { Blocks(Height), Seconds(Time), Seconds512(Time) }` | direct use | BIP-68 relative lock; `from_height`, `from_seconds`, `from_512_second_intervals`. |
| `bitcoin::Witness` | `pub struct Witness { ... }` | direct re-export | SegWit witness stack; `new()`, `push()`, `is_empty()`, `len()`, `Iter` + `IntoIter`. |
| `bitcoin::WitnessVersion` | `pub enum WitnessVersion { V0, V1, V2, V3, ... V16 }` | direct re-export | BIP-141 version byte; `to_fe()` returns the `u5` for script encoding. |
| `bitcoin::VarInt` | `pub struct VarInt { value: u64 }` | direct re-export | Bitcoin consensus compact size; `len()` returns serialized size. |
| `bitcoin::Block` | `pub struct Block { header: Header, txdata: Vec<Transaction> }` | direct re-export | Block header + tx list; `header()`, `txdata()`, `block_hash()`. |
| `bitcoin::BlockHash` | `pub struct BlockHash(...); Hash type` | direct re-export | Tagged hash of the 80-byte header. |
| `bitcoin::Header` | `pub struct Header { version: Version, prev_blockhash: BlockHash, merkle_root: TxMerkleNode, time: u32, bits: CompactTarget, nonce: u32 }` | direct re-export (in `block` module) | `block_hash()` → `BlockHash`. |
| `bitcoin::Wtxid` | `pub struct Wtxid(...); Hash type` | direct re-export | BIP-141 witness txid (SHA-256 of the **full** serialized tx incl. witness). |

### 1.1 `bitcoin::Amount` — satoshi ↔ BTC

- Defined at the crate root (re-export of `bitcoin_units::amount::Amount`).
  Source: https://docs.rs/bitcoin/0.32.11/bitcoin/struct.Amount.html
- Underlying module: https://docs.rs/bitcoin-units/0.1.101/x86_64-unknown-linux-gnu/bitcoin_units/amount/index.html

```rust
pub struct Amount { /* private */ }

impl Amount {
    pub const ZERO: Amount;
    pub const MAX: Amount; // 21_000_000 * COIN

    pub fn from_sat(satoshi: u64) -> Amount;
    pub fn to_sat(self) -> u64;

    pub fn from_btc(btc: f64) -> Result<Amount, ParseAmountError>;
    pub fn to_btc(self) -> f64;

    // saturating / checked arithmetic
    pub fn checked_add(self, rhs: Amount) -> Option<Amount>;
    pub fn checked_sub(self, rhs: Amount) -> Option<Amount>;
    pub fn saturating_add(self, rhs: Amount) -> Amount;
    pub fn saturating_sub(self, rhs: Amount) -> Amount;
}
```

Docstring (summary): *"Amount in satoshis. Wrapper type for `u64`
representing an absolute amount of bitcoin (1 BTC == 100 000 000
sats)."* (verify in Task 31 spike for exact wording — only the type
shape is verified from the crate root).

### 1.2 `bitcoin::FeeRate` — sat/kvB math

- Source: https://docs.rs/bitcoin/0.32.11/bitcoin/struct.FeeRate.html
- Underlying: `bitcoin_units::fee_rate::FeeRate`
  https://docs.rs/bitcoin-units/0.1.101/x86_64-unknown-linux-gnu/bitcoin_units/fee_rate/index.html

```rust
pub struct FeeRate { sat_per_vb: u64 } // re-exported

impl FeeRate {
    pub const ZERO: FeeRate;
    pub const MIN: FeeRate;
    pub const MAX: FeeRate;

    pub fn from_sat_per_vb(sat_per_vb: u64) -> Self;
    pub fn from_sat_per_kvb(sat_per_kvb: u64) -> Self; // 1 sat/vB == 1000 sat/kvB
    pub fn from_btc_per_kvb(btc_per_kvb: u64) -> Self;

    pub fn sat_per_vb(self) -> u64;
    pub fn sat_per_kvb(self) -> u64;

    pub fn fee_vb(self, vbytes: u64) -> Amount;
    pub fn fee_kvb(self, kvbytes: u64) -> Amount; // ⚠ rounding — check
}
```

> **Verify in Task 31 spike**: the exact field name on the struct
> (`sat_per_vb` vs `satoshis_per_vbyte`). I only confirmed the public
> method names from the docs.rs re-export page. Field names are
> private in 0.32 (this changed in earlier versions), so callers
> should use the methods.

### 1.3 `bitcoin::SignedAmount`

- Same page as `Amount`, but `i64` inner.
- Used for fee deltas and change-output math where the result can be
  negative (rare; BDK itself uses `Amount` and explicit `u64` arithmetic).

### 1.4 `bitcoin::OutPoint`

```rust
pub struct OutPoint { /* private fields */ }

impl OutPoint {
    pub fn new(txid: Txid, vout: u32) -> Self;
    pub fn txid(&self) -> Txid;
    pub fn vout(&self) -> u32;
}

impl OutPoint {
    pub const NULL: OutPoint; // (zero txid, vout 0xffff_ffff)
}
```

- Implements `Encodable + Decodable` via the consensus codec.
- `OutPoint::NULL` is the sentinel "anyone-can-spend" reference used
  in coinbase inputs.

### 1.5 `bitcoin::Sequence`

```rust
pub struct Sequence(/* private */);

impl Sequence {
    pub const ZERO: Sequence;
    pub const MAX: Sequence;       // 0xffffffff
    pub const FINAL: Sequence;     // == MAX
    pub const ENABLE_RBF_NO_LOCKTIME: Sequence; // 0xffff_fffe

    pub fn from_seconds(s: u32) -> Option<Sequence>;
    pub fn from_blocks(b: u16) -> Option<Sequence>;
    pub fn from_512_second_intervals(intervals: u16) -> Option<Sequence>;
    pub fn from_consensus(s: u32) -> Sequence;

    pub fn is_final(&self) -> bool;
    pub fn is_rbf(&self) -> bool;
    pub fn is_relative_lock_time(&self) -> bool;
    pub fn to_consensus_u32(self) -> u32;
}
```

- BIP-68: bit 31 cleared, low 16 bits encode blocks/512s/seconds.
- BIP-125 RBF: any value < `0xfffffffe` (i.e. < `MAX`) signals RBF.

### 1.6 `bitcoin::LockTime` (absolute)

`bitcoin::LockTime` is a re-export of `bitcoin::absolute::LockTime`.

```rust
// bitcoin::absolute
pub enum LockTime {
    Blocks(Height),
    Seconds(Time),
}

impl LockTime {
    pub const ZERO: LockTime; // always mineable

    pub fn from_height(height: u32) -> Self;
    pub fn from_time(unix_seconds: u32) -> Self;
    pub fn from_consensus(consensus: u32) -> Option<Self>;

    pub fn is_block_height(&self) -> bool;
    pub fn is_block_time(&self) -> bool;
    pub fn to_consensus_u32(self) -> u32;
}
```

> **Verify in Task 31 spike**: the exact variant names
> (`Blocks` / `Seconds` vs `BlockHeight` / `Time`). The `LockTime`
> type exists at the path
> `bitcoin::absolute::LockTime`
> (https://docs.rs/bitcoin/0.32.11/bitcoin/absolute/enum.LockTime.html),
> but I did not pay-fetch the variant page in this spike.

### 1.7 `bitcoin::relative::LockTime` (BIP-68)

```rust
// bitcoin::relative
pub enum LockTime {
    Blocks(Height),
    Seconds(Time),
    Seconds512(Time),
}

impl LockTime {
    pub const ZERO: LockTime;

    pub fn from_height(height: u16) -> Self;
    pub fn from_seconds_ceil(seconds: u32) -> Self;
    pub fn from_512_second_intervals(intervals: u16) -> Self;
    pub fn to_consensus_u32(self) -> u32;
}
```

Used together with `Sequence::is_relative_lock_time` to enforce CSV
(OP_CHECKSEQUENCEVERIFY) on a per-input basis.

### 1.8 `bitcoin::Witness`, `bitcoin::WitnessVersion`

```rust
pub struct Witness { /* stack of byte slices */ }

impl Witness {
    pub fn new() -> Self;
    pub fn push(&mut self, item: &[u8]);
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> Iter;
    pub fn into_iter(self) -> IntoIter;
    pub fn clear(&mut self);
    pub fn size(&self) -> usize; // serialised size in bytes
}
```

```rust
pub enum WitnessVersion {
    V0, V1, V2, V3, V4, V5, V6, V7,
    V8, V9, V10, V11, V12, V13, V14, V15, V16,
}

impl WitnessVersion {
    pub fn to_fe(self) -> u8; // BIP-141 version byte
}
```

> **Verify in Task 31 spike**: variant count (V0..V16). I confirmed
> the enum exists and the `to_fe` method (BIP-141) from the crate
> root.

### 1.9 `bitcoin::VarInt`

```rust
pub struct VarInt { /* private */ }

impl VarInt {
    pub fn new(value: u64) -> Self;
    pub fn get(&self) -> u64;
    pub fn len(&self) -> usize; // serialised length: 1..9
}
```

- Implements `Encodable + Decodable`. Serialised length follows the
  Bitcoin compact-size rules (1, 3, 5, 9 bytes).

### 1.10 `bitcoin::Block`, `BlockHash`, `Header`

```rust
pub struct Block {
    pub header: Header,
    pub txdata: Vec<Transaction>,
}

pub struct BlockHash(/* tagged hash */); // 32 bytes

pub struct Header {
    pub version: Version,                  // i32
    pub prev_blockhash: BlockHash,
    pub merkle_root: TxMerkleNode,
    pub time: u32,
    pub bits: CompactTarget,
    pub nonce: u32,
}

impl Block {
    pub fn block_hash(&self) -> BlockHash;
    pub fn header(&self) -> &Header;
    pub fn txdata(&self) -> &[Transaction];
}
```

- `Block` is in `bitcoin::block` (re-exported at crate root).
- `BlockHash` derives `Hash` from `bitcoin_hashes` and implements
  `Display`, `FromStr` (the natural "0…f" hex form used in P2P /
  block explorers).

### 1.11 `bitcoin::Wtxid`

```rust
pub struct Wtxid(/* tagged hash */);
```

- BIP-141: `SHA-256(SHA-256(serialised tx incl. witness))`.
- Used for witness-merkle-root commitment in segwit-era block
  headers. `Transaction::compute_wtxid()` produces it.

---

## 2. Transactions — summary table

| Function / type | Rust signature (canonical form) | BDK 3.1 status | Notes |
| --- | --- | --- | --- |
| `bitcoin::Transaction` | `pub struct Transaction { version: Version, lock_time: absolute::LockTime, input: Vec<TxIn>, output: Vec<TxOut> }` | direct re-export | `compute_txid()`, `compute_wtxid()`, `input()`, `output()`, `version()`, `lock_time()`, `is_coinbase()`. |
| `bitcoin::TxIn` | `pub struct TxIn { previous_output: OutPoint, script_sig: ScriptBuf, sequence: Sequence, witness: Witness }` | direct re-export | Fields are public. `TxIn::default()` == coinbase sentinel (`OutPoint::NULL`, empty sig/witness, `Sequence::MAX`). |
| `bitcoin::TxOut` | `pub struct TxOut { value: Amount, script_pubkey: ScriptBuf }` | direct re-export | `value` is now `Amount` (was `u64` before 0.31). |
| `bitcoin::Version` | `pub struct Version(pub i32);` | direct re-export | Transaction version (1, 2 for Taproot, etc.). `TX_VERSION_1`, `TX_VERSION_2` constants. |
| `bitcoin::absolute::LockTime` | (see §1.6) | direct use | `bitcoin::LockTime` is a re-export of this. |
| `bitcoin::relative::LockTime` | (see §1.7) | direct use | BIP-68. |
| `bitcoin::consensus::serialize(&tx)` | `pub fn serialize<T: Encodable + ?Sized>(data: &T) -> Vec<u8>` | direct use | Consensus-correct byte-encoding. |
| `bitcoin::consensus::deserialize::<Transaction>(&bytes)` | `pub fn deserialize<T: Decodable>(bytes: &[u8]) -> Result<T, DecodeError>` | direct use | Fails on trailing bytes. |
| `bitcoin::consensus::deserialize_partial` | same but tolerates trailing bytes | direct use | Stream decoder for P2P. |
| `bitcoin::consensus::Encodable` | `trait Encodable { fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, io::Error>; }` | direct use | Implemented by `Transaction`, `TxIn`, `TxOut`, `OutPoint`, `Witness`, `Block`, `Header`, `ScriptBuf`, `VarInt`, etc. |
| `bitcoin::consensus::Decodable` | `trait Decodable { fn consensus_decode<R: Read>(r: &mut R) -> Result<Self, DecodeError>; }` | direct use | Counterpart of `Encodable`. |
| `tx.compute_txid()` | `pub fn compute_txid(&self) -> Txid` | direct use | Hashes the **non-witness** serialised tx (legacy txid). |
| `tx.compute_wtxid()` | `pub fn compute_wtxid(&self) -> Wtxid` | direct use | Hashes the **witness** serialised tx (BIP-141). For non-segwit txs it equals `compute_txid`. |
| `bitcoin::consensus::verify_transaction` / `verify_transaction_with_flags` | requires `bitcoinconsensus` feature | direct use | FFI into Bitcoin Core's script interpreter. |
| `bitcoin::consensus::verify_script` / `verify_script_with_flags` | requires `bitcoinconsensus` feature | direct use | Verify an input script + witness against prev scriptPubKey. |

> **Verify in Task 31 spike**: `TxIn` field visibility. In rust-bitcoin
> 0.31 / 0.32 the fields of `TxIn`, `TxOut`, and `Transaction` are
> public (no `pub(crate)` privacy like in 0.30), which is how BDK and
> LDK mutate transactions. I confirmed the struct exists at the
> crate root but did not pay-fetch every field's exact access level
> in this spike.

### 2.1 `bitcoin::Transaction`

```rust
pub struct Transaction {
    pub version: Version,
    pub lock_time: absolute::LockTime,
    pub input: Vec<TxIn>,
    pub output: Vec<TxOut>,
}

impl Transaction {
    pub fn compute_txid(&self) -> Txid;
    pub fn compute_wtxid(&self) -> Wtxid;
    pub fn is_coinbase(&self) -> bool;
    pub fn version(&self) -> Version;
    pub fn lock_time(&self) -> absolute::LockTime;
    pub fn input(&self) -> &[TxIn];
    pub fn output(&self) -> &[TxOut];
}
```

- Source: https://docs.rs/bitcoin/0.32.11/bitcoin/struct.Transaction.html
  (77 KB scrape — confirmed existence; full body requires the
  Task 31 spike to dump the API surface line-by-line).

### 2.2 `bitcoin::TxIn`

```rust
pub struct TxIn {
    pub previous_output: OutPoint,
    pub script_sig: ScriptBuf,
    pub sequence: Sequence,
    pub witness: Witness,
}

impl TxIn {
    pub const DEFAULT_SEQUENCE: Sequence = Sequence::MAX;
    pub fn new(prev_out: OutPoint, script_sig: ScriptBuf) -> Self;
    pub fn new_with_sequence(prev_out: OutPoint, script_sig: ScriptBuf, sequence: Sequence) -> Self;
    pub fn coinbase() -> Self; // OutPoint::NULL, empty sig/witness, SEQ_MAX
    pub fn is_coinbase(&self) -> bool;
}
```

### 2.3 `bitcoin::TxOut`

```rust
pub struct TxOut {
    pub value: Amount,
    pub script_pubkey: ScriptBuf,
}

impl TxOut {
    pub fn new(value: Amount, script_pubkey: ScriptBuf) -> Self;
}
```

### 2.4 `bitcoin::Version`

```rust
pub struct Version(pub i32);
impl Version {
    pub const ONE: Version;
    pub const TWO: Version;   // Taproot
    pub fn from_consensus(v: i32) -> Self;
    pub fn to_consensus_u32(self) -> u32;
}
```

> **Verify in Task 31 spike**: the exact associated constant names
> (`ONE` vs `TX_VERSION_1`). Older docs used `TX_VERSION_1`/`TWO`;
> 0.32 renamed them. Confirm.

### 2.5 Consensus codec — `bitcoin::consensus::encode`

The `encode` module lives under `bitcoin::consensus`. Source:
https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/encode/index.html

```rust
// bitcoin::consensus::encode
pub fn serialize<T: Encodable + ?Sized>(data: &T) -> Vec<u8>;
pub fn deserialize<T: Decodable>(bytes: &[u8]) -> Result<T, DecodeError>;
pub fn deserialize_partial<T: Decodable>(bytes: &[u8]) -> Result<T, DecodeError>;
```

```rust
pub trait Encodable {
    fn consensus_encode<W: Write>(&self, w: &mut W) -> Result<usize, io::Error>;
}

pub trait Decodable: Sized {
    fn consensus_decode<R: Read>(r: &mut R) -> Result<Self, DecodeError>;
}
```

Plus the `ReadExt` / `WriteExt` extension traits and `VarInt`
encoding helpers in the same module.

The `bitcoin-io ^0.1.1` crate replaces `std::io` for the codec so
that `bitcoin` stays `no_std`-compatible.

### 2.6 The `Transaction` codec round-trip

```rust
use bitcoin::{Transaction};
use bitcoin::consensus::{serialize, deserialize};

let bytes: Vec<u8> = serialize(&tx);              // 200-byte non-witness, larger w/ witness
let tx: Transaction = deserialize(&bytes)?;       // strict (trailing bytes → DecodeError)
let tx2: Transaction = deserialize_partial(&bytes)?; // tolerant (recommended for P2P)
```

---

## 3. Network — summary table

| Function / type | Rust signature (canonical form) | BDK 3.1 status | Notes |
| --- | --- | --- | --- |
| `bitcoin::Network` | `pub enum Network { Bitcoin, Testnet, Testnet4, Signet, Regtest }` | direct re-export | Plus `Bitcoin`, `Testnet` deprecated aliases removed in 0.32 (verify). |
| `Network::from_magic(magic: u32) -> Option<Network>` | reverse-lookup of the 4-byte p2p magic | direct use | `0xF9BEB4D9` for `Bitcoin`. |
| `Network::magic(self) -> u32` | the canonical 4-byte magic | direct use | Example in docs.rs: `serialize(&network.magic()) == [0xF9,0xBE,0xB4,0xD9]`. |
| `Network::is_mainnet(self) -> bool` | `true` for `Bitcoin` and `Signet` | direct use | Source: docs.rs listing of `Network` enum. |
| `Network::is_testnet(self) -> bool` | `true` for `Testnet`, `Testnet4`, `Regtest` | direct use | |
| `NetworkKind` | `pub enum NetworkKind { Mainnet, Testnet }` | direct re-export | Used by miniscript, descriptor, and policy code; collapses `Bitcoin`/`Signet` to `Mainnet` and `Testnet`/`Testnet4`/`Regtest` to `Testnet`. |
| `bitcoin::KnownHrp` | `pub enum KnownHrp { Mainnet, Testnet }` | direct re-export | Used for bech32 address parsing; the actual HRP strings live in the `address` module. |
| `bitcoin::params::Params` | per-network chain params (alias of `consensus::Params`) | direct use | (See §3.5 below.) |
| Per-network HRP strings | `bc` (mainnet), `tb` (testnet+testnet4), `bcrt` (regtest), `tb` (signet) | direct use | Defined in `bitcoin::address` and `KnownHrp`. |

### 3.1 `bitcoin::Network` — the enum

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Network {
    Bitcoin,
    Testnet,
    Testnet4,    // BIP-94
    Signet,
    Regtest,
}

impl Network {
    pub fn from_magic(magic: u32) -> Option<Network>;
    pub fn magic(self) -> u32;
    pub fn is_mainnet(self) -> bool;
    pub fn is_testnet(self) -> bool;
    pub fn from_core_arg(arg: &str) -> Result<Network, ParseNetworkError>;
    pub fn to_core_arg(self) -> &'static str;       // via `as_core_arg` submodule
    pub fn chain_hash(self) -> [u8; 32];            // genesis block hash (hex via UnknownChainHashError)
}
```

Example from the docs.rs `network` module page (canonical test):

```rust
use bitcoin::Network;
use bitcoin::consensus::encode::serialize;

let network = Network::Bitcoin;
let bytes = serialize(&network.magic());
assert_eq!(&bytes[..], &[0xF9, 0xBE, 0xB4, 0xD9]);
```

- Source: https://docs.rs/bitcoin/0.32.11/bitcoin/network/index.html

### 3.2 Magic bytes per network

| Network | Magic (u32, BE) | Bytes |
| --- | --- | --- |
| `Bitcoin` | `0xF9_BE_B4_D9` | `f9 be b4 d9` |
| `Testnet` | `0x0B_11_09_07` | `0b 11 09 07` |
| `Testnet4` | `0x1C_16_3F_68` | `1c 16 3f 68` (verify — BIP-94) |
| `Signet` | `0x0A_03_CF_40` | `0a 03 cf 40` |
| `Regtest` | `0xFA_BF_B5_DA` | `fa bf b5 da` |

> **Verify in Task 31 spike** (specifically the exact magic for
> `Testnet4` — BIP-94 has a known typo that changed the value). The
> `magic()` method exists but I did not enumerate every constant in
> this spike.

### 3.3 `bitcoin::NetworkKind` (miniscript / descriptor)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NetworkKind {
    Mainnet,
    Testnet,
}

impl NetworkKind {
    pub fn from(network: Network) -> Self {
        if network.is_mainnet() { Self::Mainnet } else { Self::Testnet }
    }
}
```

- `NetworkKind` collapses the five real networks into two: `Bitcoin`
  + `Signet` → `Mainnet`; `Testnet` + `Testnet4` + `Regtest` →
  `Testnet`. Used by miniscript and BDK descriptor parsing when the
  caller needs a "what BIP-380 context are we in" answer.

### 3.4 Bech32 / bech32m HRPs per network

HRPs (human-readable parts) live on the `address` side but are
tied to `Network`:

| Network | HRP (P2WPKH / P2WSH) | HRP (Taproot, P2TR) |
| --- | --- | --- |
| `Bitcoin` | `bc` | `bc` |
| `Testnet` | `tb` | `tb` |
| `Testnet4` | `tb` | `tb` |
| `Signet` | `tb` | `tb` |
| `Regtest` | `bcrt` | `bcrt` |

The `KnownHrp` enum is the typed key used by the bech32 parser:

```rust
pub enum KnownHrp {
    Mainnet, // "bc"
    Testnet, // "tb"
}
```

> **Verify in Task 31 spike**: BDK 3.1 uses `Address::network` /
> `Address::is_valid_for_network` rather than directly reading
> `KnownHrp`; verify the wire between the two.

### 3.5 `bitcoin::consensus::Params` — per-network consensus params

- Source: https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/struct.Params.html

```rust
pub struct Params { /* private */ }

impl Params {
    pub const BITCOIN: Params;
    pub const TESTNET: Params;
    pub const TESTNET4: Params;
    pub const SIGNET: Params;
    pub const REGTEST: Params;

    pub fn new(network: Network) -> Self;

    pub fn network(&self) -> Network;
    pub fn max_block_weight(&self) -> Weight;
    pub fn max_block_sigops_cost(&self) -> usize;
    pub fn min_transaction_weight(&self) -> Weight;
    pub fn subsidy(&self, height: u32) -> Amount;
    pub fn block_subsidy(&self, height: u32) -> Amount;
    pub fn halving_interval(&self) -> u32;
    pub fn pow_limit(&self) -> Target;
    pub fn pow_limit_bits(&self) -> CompactTarget;
    pub fn bip34_height(&self) -> u32;
    pub fn bip65_height(&self) -> u32;
    pub fn bip66_height(&self) -> u32;
    pub fn csv_height(&self) -> u32;        // BIP-68 / CSV
    pub fn segwit_height(&self) -> u32;     // BIP-141
    pub fn taproot_height(&self) -> u32;    // BIP-341
    pub fn target_timespan(&self) -> u32;   // blocks (10*2016 on mainnet)
    pub fn target_spacing(&self) -> u32;    // seconds (600 on mainnet)
}
```

The crate root also exposes `bitcoin::params` as an alias.

---

## 4. What's NOT in rust-bitcoin 0.32 (for these 3 modules)

Items a wallet might expect that this crate does **not** ship:

| Need | Status | Where to find it |
| --- | --- | --- |
| BIP-32 HD key derivation (`ExtendedPrivKey`, derivation paths, xprv/xpub) | Exists (`bitcoin::bip32`) but **not in scope of this spike** — see `bitcoin::bip32` module. | https://docs.rs/bitcoin/0.32.11/bitcoin/bip32/index.html |
| Miniscript (structured script) | Exists in separate `miniscript` crate | https://docs.rs/miniscript/ |
| Descriptors (BIP-380/381) | Exists in separate `miniscript` crate | https://docs.rs/miniscript/ |
| PSBT construction | Exists (`bitcoin::psbt`) but **not in scope of this spike** | https://docs.rs/bitcoin/0.32.11/bitcoin/psbt/index.html |
| Wallet persistence / chain tracking | **Not** in `bitcoin` — that's `bdk`, `bip157`, etc. | n/a |
| Coin selection | **Not** in `bitcoin` — use `bdk`/`coinselect`. | n/a |
| RBF fee bumping helpers (`bump_fee` machinery) | **Not** in `bitcoin` — `bdk` re-exports helpers but the logic is in BDK. | n/a |
| Schnorr signature primitives | Exists (`bitcoin::key::TapTweak`, `bitcoin::taproot`) but **not in scope of this spike**. | `bitcoin::taproot` module |
| Fee estimation against mempool | **Not** in `bitcoin`. | n/a |
| `Address::p2wpkh(...)`, `Address::p2wsh(...)`, `Address::p2tr(...)` | **Not in scope** but worth noting: live in `bitcoin::address` module. | https://docs.rs/bitcoin/0.32.11/bitcoin/address/index.html |
| `Address::from_script(script, network)` | **Not in scope** but used everywhere. | https://docs.rs/bitcoin/0.32.11/bitcoin/address/index.html |
| SLIP-132 `yprv`/`zprv` version bytes | **Not** in `bitcoin` — `bip32` crate uses SLIP-10 key formats, custom versions are layered on top. | n/a |
| Taproot control-block construction | Lives in `bitcoin::taproot::TapTreeBuilder` and `bitcoin::taproot::LeafScript` | not in this spike's scope |
| Mempool / RBF policy helpers | Not in `bitcoin` (only the `Sequence` / `LockTime` math). | n/a |

---

## 5. Error types in this area

All confirmed via the `consensus` and `network` module indices on
docs.rs (https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/index.html,
https://docs.rs/bitcoin/0.32.11/bitcoin/network/index.html):

| Error | Where | Use |
| --- | --- | --- |
| `bitcoin::consensus::DecodeError` | `bitcoin::consensus` | Wrap of `io::Error` for consensus decoding failures (trailing bytes, EOF, etc.). |
| `bitcoin::network::ParseNetworkError` | `bitcoin::network` | From-string parse failures for `Network`. |
| `bitcoin::network::UnknownChainHashError` | `bitcoin::network` | From-chain-hash parse failures for `Network`. |
| `bitcoin::consensus::validation::TransactionError` | `bitcoin::consensus::validation` | `verify_transaction` failure causes. |
| `bitcoin::amount::error::ParseAmountError` | `bitcoin::amount` | `Amount::from_btc` / `SignedAmount` parse failures. |
| `io::Error` | `std::io` (re-exported via `bitcoin-io`) | Underlying I/O errors from `Encodable`/`Decodable`. |
| `bitcoin::transaction::VersionConversionError` | `bitcoin::transaction` | Some `Version` roundtrips. *(verify — not in this spike)* |
| `bitcoin::absolute::ConversionError` | `bitcoin::absolute` | Invalid `LockTime` consensus roundtrip. *(verify)* |
| `bitcoin::relative::ConversionError` | `bitcoin::relative` | Invalid `relative::LockTime` consensus roundtrip. *(verify)* |
| `bitcoin::witness_program::Error` / `bitcoin::witness_version::Error` | `bitcoin::witness_*` | Bad witness version / program. *(verify)* |
| `bitcoin::block::ValidationError` | `bitcoin::block` | Block header / coinbase validation. *(verify)* |

> **Verify in Task 31 spike**: the precise error enum variants per
> error type — only the existence was confirmed from the index page.

---

## 6. Sources cited

| URL | Used for |
| --- | --- |
| https://docs.rs/bitcoin/0.32.11/bitcoin/ | Crate root index of structs/enums/functions |
| https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/index.html | `blockdata` module index |
| https://docs.rs/bitcoin/0.32.11/bitcoin/network/index.html | `network` module index + magic-bytes example |
| https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/index.html | `consensus` module index (`Params`, `Encodable`, `Decodable`, `serialize`, `deserialize`, `verify_*`) |
| https://docs.rs/bitcoin/0.32.11/bitcoin/struct.Transaction.html | `Transaction` struct page (77 KB scrape) |
| https://docs.rs/bitcoin/0.32.11/bitcoin/struct.Amount.html | `Amount` struct page |
| https://docs.rs/bitcoin/0.32.11/bitcoin/struct.FeeRate.html | `FeeRate` struct page (re-exported from `bitcoin_units`) |
| https://docs.rs/bitcoin-units/0.1.101/x86_64-unknown-linux-gnu/bitcoin_units/fee_rate/index.html | `FeeRate` actual definition |
| https://docs.rs/bitcoin/0.32.11/bitcoin/absolute/enum.LockTime.html | `absolute::LockTime` enum page |
| https://docs.rs/bitcoin/0.32.11/bitcoin/relative/enum.LockTime.html | `relative::LockTime` enum page |
| https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/struct.Params.html | `consensus::Params` page |
| https://github.com/rust-bitcoin/rust-bitcoin | Source repository |
| https://crates.io/crates/bitcoin | Crate page |

---

## 7. Verification checklist for Task 31 spike (Rust API source code)

Each item below should be confirmed by `git clone` + `cargo doc` +
file:line citation:

- [ ] `Amount::from_sat` exact signature & visibility
- [ ] `Amount::from_btc` error type (`ParseAmountError` vs `ParseFloatError`)
- [ ] `FeeRate` field name (`sat_per_vb` vs `sat_per_vbyte`)
- [ ] `FeeRate::from_sat_per_kvb` rounding behaviour
- [ ] `Sequence::ENABLE_RBF_NO_LOCKTIME` value (0xffff_fffe)
- [ ] `Sequence::from_seconds` / `from_blocks` exact `Option` semantics
- [ ] `LockTime` variant names: `Blocks(Height)` / `Seconds(Time)` (absolute) and
      `Blocks(Height)` / `Seconds(Time)` / `Seconds512(Time)` (relative)
- [ ] `WitnessVersion::to_fe` returns `u5` or `u8`?
- [ ] `VarInt::new(0)` and the encoded size table
- [ ] `Block::txdata` field vs accessor
- [ ] `Wtxid` == `Txid` for non-segwit txs (verify in code)
- [ ] `Transaction::compute_txid` excludes witness; `compute_wtxid` includes it
- [ ] `TxIn::coinbase()` exact sentinel (`OutPoint::NULL`, `Sequence::MAX`)
- [ ] `TxOut::value` type (now `Amount`, not `u64`)
- [ ] `Version::from_consensus` / `to_consensus_u32`
- [ ] `consensus::serialize` signature with `Encodable + ?Sized`
- [ ] `consensus::deserialize` strict vs `deserialize_partial`
- [ ] `Network::from_magic` magic bytes per network (Testnet4 magic verify)
- [ ] `Network::from_core_arg` accepts `"main"`, `"test"`, `"testnet4"`, `"signet"`, `"regtest"`
- [ ] `NetworkKind::from(Network)` mapping
- [ ] `KnownHrp::Mainnet` HRP == `"bc"`, `Testnet` HRP == `"tb"`
- [ ] `consensus::Params::subsidy(height)` halving schedule
- [ ] `consensus::Params::taproot_height` value for each network (Bitcoin=709_632, Testnet=2_516_992 etc.)
- [ ] All error type variants in §5 (run `cargo doc` and click each)

---

## 8. Summary for the wallet-spike team

For the Digital Euro wallet in this spike, the three modules above
give us:

1. **Strong primitives**: every value type is non-fungible
   (`Amount`, `Sequence`, `LockTime`, `Network`). Wallet code cannot
   accidentally mix sat/BTC, mainnet/testnet, or block-height/time
   locktimes.
2. **Direct, predictable consensus codec**: `consensus::serialize` /
   `deserialize` produce bytes that match Bitcoin Core and Electrum.
   `compute_txid()` / `compute_wtxid()` are local; no RPC needed.
3. **Five-network support out of the box**: `Bitcoin`, `Testnet`,
   `Testnet4` (BIP-94), `Signet`, `Regtest`. Regtest is the
   recommended local-wallet network for this spike's tests; Signet
   for staging; Testnet4 (not Testnet3) for QA faucets.
4. **What we still need from BDK**: PSBT, descriptors, coin selection,
   chain sync, fee bumping. Those are the **next** spikes (BDK
   3.1 inventory already collected in
   `2026-08-05-bdk-features-*.md`).