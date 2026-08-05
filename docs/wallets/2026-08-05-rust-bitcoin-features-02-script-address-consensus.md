# rust-bitcoin 0.32.11 — Script, Address, Consensus Encode

Spike notes enumerating the public API of `bitcoin` 0.32.11 for the three
modules a BDK-style wallet layer needs to wrap: script construction and
inspection, address parsing/formatting, and consensus byte encoding.

This is **research output only** — no code changes. BDK 3.1 already re-exports
many of these via the `bitcoin` crate, so most of the column is "yes, exposed
by BDK". Where BDK does not surface a rust-bitcoin function, the column says
"no / verify in Task 31 spike".

## Sources

- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/script/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/script/borrowed.rs.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/script/owned.rs.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/script/builder.rs.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/script/push_bytes.rs.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/script/enum.Instruction.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/opcodes.rs.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/struct.Script.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/struct.ScriptBuf.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/script/struct.Builder.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/struct.Address.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/address/mod.rs.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/enum.AddressType.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/encode/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/encode.rs.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/encode/trait.Encodable.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/encode/trait.Decodable.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/encode/trait.ReadExt.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/encode/trait.WriteExt.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/encode/struct.VarInt.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/encode/enum.Error.html`

---

## 1. Script module — `bitcoin::blockdata::script`

Source root: `src/bitcoin/blockdata/script/mod.rs.html` (lines 3–756).
Three sibling types form the user-facing API:

| Type     | Kind                  | Role                                                                |
| -------- | --------------------- | ------------------------------------------------------------------- |
| `Script` | `?Sized` / `&Script`  | Borrowed slice of script bytes. Immutable, cheap to pass around.    |
| `ScriptBuf` | owned, `Deref<Target=Script>` | Owning counterpart. All `new_*` constructors live here.   |
| `Builder` | builder-pattern     | Fluent API to assemble a script; ends with `.into_script() -> ScriptBuf`. |

`Script` is to `ScriptBuf` as `str` is to `String`. Most predicates and
accessors are on `Script` (and auto-exposed on `ScriptBuf` via `Deref`).

### 1.1 Summary table — script constructors and predicates

| Function / method                       | Signature (Rust)                                                                                                  | BDK 3.1 status            | Notes                                                                                            |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------- | ------------------------------------------------------------------------------------------------ |
| `Script::new`                           | `pub const fn new() -> &'static Script`                                                                            | yes                       | Empty script singleton — `borrowed.rs.html#84`.                                                  |
| `Script::from_bytes`                    | `pub fn from_bytes(bytes: &[u8]) -> &Script`                                                                       | yes                       | Zero-copy cast — `borrowed.rs.html#88-94`.                                                       |
| `Script::from_bytes_mut`                | `pub fn from_bytes_mut(bytes: &mut [u8]) -> &mut Script`                                                           | yes                       | `borrowed.rs.html#98-106`.                                                                       |
| `Script::builder`                       | `pub fn builder() -> Builder`                                                                                      | yes                       | Entry point to the builder — `borrowed.rs.html#110`.                                             |
| `ScriptBuf::new`                        | `pub const fn new() -> Self`                                                                                       | yes                       | Empty owned script — `owned.rs.html#39`.                                                         |
| `ScriptBuf::with_capacity`              | `pub fn with_capacity(capacity: usize) -> Self`                                                                    | yes                       | `owned.rs.html#42`.                                                                              |
| `ScriptBuf::from_hex`                   | `pub fn from_hex(s: &str) -> Result<Self, HexToBytesError>`                                                        | yes                       | `owned.rs.html#180`.                                                                             |
| `ScriptBuf::from_bytes`                 | `pub fn from_bytes(bytes: Vec<u8>) -> Self`                                                                        | yes                       | `owned.rs.html#185`.                                                                             |
| `ScriptBuf::builder`                    | `pub fn builder() -> Builder`                                                                                      | yes                       | `owned.rs.html#72`.                                                                              |
| `ScriptBuf::new_p2pk`                   | `pub fn new_p2pk(pubkey: &PublicKey) -> Self`                                                                       | yes                       | Legacy P2PK — `owned.rs.html#75`.                                                                |
| `ScriptBuf::new_p2pkh`                  | `pub fn new_p2pkh(pubkey_hash: &PubkeyHash) -> Self`                                                                | yes                       | `owned.rs.html#78`.                                                                              |
| `ScriptBuf::new_p2sh`                   | `pub fn new_p2sh(script_hash: &ScriptHash) -> Self`                                                                 | yes                       | `owned.rs.html#81`.                                                                              |
| `ScriptBuf::new_p2wpkh`                 | `pub fn new_p2wpkh(pubkey_hash: &WPubkeyHash) -> Self`                                                              | yes                       | `owned.rs.html#86`.                                                                              |
| `ScriptBuf::new_p2wsh`                  | `pub fn new_p2wsh(script_hash: &WScriptHash) -> Self`                                                              | yes                       | `owned.rs.html#97`.                                                                              |
| `ScriptBuf::new_p2tr`                   | `pub fn new_p2tr<C: Verification>(secp: &Secp256k1<C>, internal_key: UntweakedPublicKey, merkle_root: Option<TapNodeHash>) -> Self` | yes                       | `owned.rs.html#106`.                                                                             |
| `ScriptBuf::new_p2tr_tweaked`           | `pub fn new_p2tr_tweaked(output_key: TweakedPublicKey) -> Self`                                                    | yes                       | `owned.rs.html#119`.                                                                             |
| `ScriptBuf::new_p2a`                    | `pub fn new_p2a() -> Self`                                                                                         | yes (added 0.32)          | P2A anchor output — `owned.rs.html#130`.                                                         |
| `ScriptBuf::new_witness_program`        | `pub fn new_witness_program(wp: &WitnessProgram) -> Self`                                                          | yes                       | `owned.rs.html#136`.                                                                             |
| `ScriptBuf::new_op_return`              | `pub fn new_op_return<T: AsRef<PushBytes>>(data: T) -> Self`                                                       | yes                       | `owned.rs.html#169`.                                                                             |
| `ScriptBuf::p2wpkh_script_code`         | `pub fn p2wpkh_script_code(wpkh: WPubkeyHash) -> ScriptBuf`                                                        | yes                       | Used for sighash — `owned.rs.html#141`.                                                          |
| `Script::to_p2sh`                       | `pub fn to_p2sh(&self) -> ScriptBuf`                                                                               | yes                       | Wrap redeem script as P2SH — `borrowed.rs.html#386-395`.                                        |
| `Script::to_p2wsh`                      | `pub fn to_p2wsh(&self) -> ScriptBuf`                                                                              | yes                       | `borrowed.rs.html#135`.                                                                          |
| `Script::to_p2tr`                       | `pub fn to_p2tr<C: Verification>(&self, secp: &Secp256k1<C>, internal_key: UntweakedPublicKey) -> ScriptBuf`        | yes                       | Single-script Tapscript — `borrowed.rs.html#157-165`.                                            |
| `Script::p2wpkh_script_code`            | `pub fn p2wpkh_script_code(&self) -> Option<ScriptBuf>`                                                            | yes                       | Reverse of `Address::p2wpkh` — `borrowed.rs.html#403-414`.                                      |
| `Script::is_p2pk`                       | `pub fn is_p2pk(&self) -> bool`                                                                                    | yes                       | `borrowed.rs.html#250-252`.                                                                      |
| `Script::p2pk_public_key`               | `pub fn p2pk_public_key(&self) -> Option<PublicKey>`                                                                | yes                       | Returns `None` even when `is_p2pk()` is true if key not strict — `borrowed.rs.html#273-317`.     |
| `Script::is_p2pkh`                      | `pub fn is_p2pkh(&self) -> bool`                                                                                   | yes                       | `borrowed.rs.html#223-235`.                                                                      |
| `Script::is_p2sh`                       | `pub fn is_p2sh(&self) -> bool`                                                                                    | yes                       | `borrowed.rs.html#209-216`.                                                                      |
| `Script::is_p2wpkh`                     | `pub fn is_p2wpkh(&self) -> bool`                                                                                  | yes                       | `borrowed.rs.html#341-345`.                                                                      |
| `Script::is_p2wsh`                      | `pub fn is_p2wsh(&self) -> bool`                                                                                   | yes                       | `borrowed.rs.html#333-337`.                                                                      |
| `Script::is_p2tr`                       | `pub fn is_p2tr(&self) -> bool`                                                                                    | yes                       | `borrowed.rs.html#349-354`.                                                                      |
| `Script::is_multisig`                   | `pub fn is_multisig(&self) -> bool`                                                                                | yes                       | Bare `m-of-n` — `borrowed.rs.html#321`.                                                         |
| `Script::is_op_return`                  | `pub fn is_op_return(&self) -> bool`                                                                               | yes                       | `borrowed.rs.html#365-377`.                                                                      |
| `Script::is_provably_unspendable`       | `pub fn is_provably_unspendable(&self) -> bool`                                                                     | yes but deprecated        | Use `is_op_return` — `borrowed.rs.html#380`.                                                     |
| `Script::is_push_only`                  | `pub fn is_push_only(&self) -> bool`                                                                               | yes                       | `borrowed.rs.html#242`.                                                                          |
| `Script::is_witness_program`            | `pub fn is_witness_program(&self) -> bool`                                                                         | yes                       | `borrowed.rs.html#325-329`.                                                                      |
| `Script::witness_version`               | `pub fn witness_version(&self) -> Option<WitnessVersion>`                                                           | yes                       | `borrowed.rs.html#200-205`.                                                                      |
| `Script::redeem_script`                 | `pub fn redeem_script(&self) -> Option<&Script>`                                                                   | yes                       | BIP16 last-item push — `borrowed.rs.html#419`.                                                  |
| `Script::dust_value`                    | `pub fn dust_value(&self) -> Amount`                                                                               | yes but deprecated        | Replaced by `minimal_non_dust` — `borrowed.rs.html#430-432`.                                     |
| `Script::minimal_non_dust`              | `pub fn minimal_non_dust(&self) -> Amount`                                                                         | yes                       | 3 sat/vB default — `borrowed.rs.html#445-447`.                                                  |
| `Script::minimal_non_dust_custom`       | `pub fn minimal_non_dust_custom(&self, fee_rate: FeeRate) -> Amount`                                               | yes                       | `borrowed.rs.html#486`.                                                                          |
| `Script::count_sigops`                  | `pub fn count_sigops(&self) -> usize`                                                                              | yes                       | Accurate (post-BIP147) — `borrowed.rs.html#500`.                                                 |
| `Script::count_sigops_legacy`           | `pub fn count_sigops_legacy(&self) -> usize`                                                                       | yes                       | `borrowed.rs.html#550-552`.                                                                      |
| `Script::script_hash`                   | `pub fn script_hash(&self) -> ScriptHash`                                                                          | yes                       | HASH160 of script bytes — `borrowed.rs.html#121`.                                               |
| `Script::wscript_hash`                  | `pub fn wscript_hash(&self) -> WScriptHash`                                                                        | yes                       | SHA256 of script bytes — `borrowed.rs.html#125`.                                                |
| `Script::tapscript_leaf_hash`           | `pub fn tapscript_leaf_hash(&self) -> TapLeafHash`                                                                 | yes                       | `borrowed.rs.html#129-131`.                                                                      |
| `Script::first_opcode`                  | `pub fn first_opcode(&self) -> Option<Opcode>`                                                                     | yes                       | `borrowed.rs.html#633-641`.                                                                      |
| `Script::fmt_asm`                       | `pub fn fmt_asm(&self, f: &mut dyn Write) -> core::fmt::Result`                                                    | yes                       | `borrowed.rs.html#590-594`.                                                                      |
| `Script::to_asm_string`                 | `pub fn to_asm_string(&self) -> String`                                                                            | yes                       | `borrowed.rs.html#601`.                                                                          |
| `Script::to_hex_string`                 | `pub fn to_hex_string(&self) -> String`                                                                            | yes                       | Lowercase hex — `borrowed.rs.html#604-606`.                                                      |
| `Script::verify`                        | `pub fn verify(&self, index: usize, amount: Amount, spending_tx: &[u8]) -> Result<(), BitcoinconsensusError>`       | yes (behind `bitcoinconsensus` feature) | `consensus/validation.rs.html#120-127`.                                  |
| `Script::verify_with_flags`             | `pub fn verify_with_flags<F: Into<u32>>(&self, index, amount, spending_tx, flags) -> Result<(), BitcoinconsensusError>` | yes (feature-gated)       | `consensus/validation.rs.html#138-146`.                                                          |

### 1.2 Builder API (`bitcoin::blockdata::script::Builder`)

All builder methods take `self` and return `Self`, so calls chain without
`&mut`. Source: `src/bitcoin/blockdata/script/builder.rs.html`.

| Method                                  | Signature                                                                                                  | Notes                                                                                  |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `Builder::new`                          | `pub const fn new() -> Self`                                                                                | `builder.rs#22`.                                                                       |
| `Builder::len`                          | `pub fn len(&self) -> usize`                                                                                | `builder.rs#25`.                                                                       |
| `Builder::is_empty`                     | `pub fn is_empty(&self) -> bool`                                                                            | `builder.rs#28`.                                                                       |
| `Builder::push_int`                     | `pub fn push_int(self, data: i64) -> Builder`                                                               | Minimal CScriptNum — `builder.rs#34-48`.                                               |
| `Builder::push_slice`                   | `pub fn push_slice<T: AsRef<PushBytes>>(self, data: T) -> Builder`                                            | Minimal push opcode chosen — `builder.rs#60-64`.                                       |
| `Builder::push_key`                     | `pub fn push_key(self, key: &PublicKey) -> Builder`                                                         | `builder.rs#67-73`.                                                                    |
| `Builder::push_x_only_key`              | `pub fn push_x_only_key(self, x_only_key: &XOnlyPublicKey) -> Builder`                                       | 32-byte x-only push — `builder.rs#76-78`.                                              |
| `Builder::push_opcode`                  | `pub fn push_opcode(self, data: Opcode) -> Builder`                                                         | `builder.rs#81-85`.                                                                    |
| `Builder::push_verify`                  | `pub fn push_verify(self) -> Builder`                                                                       | Adds `OP_VERIFY` or rewrites last opcode to its VERIFY form — `builder.rs#97-106`.      |
| `Builder::push_lock_time`               | `pub fn push_lock_time(self, lock_time: LockTime) -> Builder`                                               | `builder.rs#109-111`.                                                                  |
| `Builder::push_sequence`                | `pub fn push_sequence(self, sequence: Sequence) -> Builder`                                                 | `builder.rs#114-116`.                                                                  |
| `Builder::into_script`                  | `pub fn into_script(self) -> ScriptBuf`                                                                     | `builder.rs#119`.                                                                      |
| `Builder::into_bytes`                   | `pub fn into_bytes(self) -> Vec<u8>`                                                                        | `builder.rs#122`.                                                                      |
| `Builder::as_script`                    | `pub fn as_script(&self) -> &Script`                                                                        | `builder.rs#125`.                                                                      |
| `Builder::as_bytes`                     | `pub fn as_bytes(&self) -> &[u8]`                                                                           | `builder.rs#128`.                                                                      |

BDK 3.1 status: the entire `Builder` API is exposed under
`bitcoin::blockdata::script::Builder` and used by `bdk_wallet` for descriptor
script construction.

### 1.3 Opcodes — `bitcoin::opcodes::all`

Source: `src/bitcoin/blockdata/opcodes.rs.html` (lines 78–338). This is a
**constants-only** module — every item is an `Opcode` constant starting with
`OP_`. Designed for `use bitcoin::opcodes::all::*;` wildcard imports so you
get every opcode in scope without also pulling `Opcode`, `Class`, etc.

| Constant group                  | Examples                                                                                                        |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Push data                       | `OP_PUSHBYTES_0`..`OP_PUSHBYTES_75`, `OP_PUSHDATA1`, `OP_PUSHDATA2`, `OP_PUSHDATA4`                              |
| Small integers                  | `OP_PUSHNUM_NEG1`, `OP_PUSHNUM_1`..`OP_PUSHNUM_16`                                                              |
| Stack ops                       | `OP_DUP`, `OP_DROP`, `OP_SWAP`, `OP_PICK`, `OP_ROLL`, `OP_SIZE`, `OP_DEPTH`, `OP_TOALTSTACK`, `OP_FROMALTSTACK`   |
| Arithmetic                      | `OP_ADD`, `OP_SUB`, `OP_1ADD`, `OP_1SUB`, `OP_NEGATE`, `OP_ABS`, `OP_MIN`, `OP_MAX`, `OP_BOOLAND`, `OP_BOOLOR`, `OP_NOT` |
| Comparisons                     | `OP_EQUAL`, `OP_EQUALVERIFY`, `OP_NUMEQUAL`, `OP_LESSTHAN`, `OP_GREATERTHAN`, `OP_WITHIN`                        |
| Crypto                          | `OP_SHA256`, `OP_SHA1`, `OP_RIPEMD160`, `OP_HASH160`, `OP_HASH256`                                              |
| Flow control                    | `OP_IF`, `OP_NOTIF`, `OP_ELSE`, `OP_ENDIF`, `OP_VERIFY`, `OP_RETURN`, `OP_CODESEPARATOR`                         |
| Signatures                      | `OP_CHECKSIG`, `OP_CHECKSIGVERIFY`, `OP_CHECKMULTISIG`, `OP_CHECKMULTISIGVERIFY`, `OP_CHECKSIGADD`               |
| Locktime                        | `OP_CLTV`, `OP_CSV`                                                                                             |
| Nops                            | `OP_NOP`, `OP_NOP1`..`OP_NOP10`                                                                                 |
| Reserved/disabled               | `OP_RESERVED`, `OP_RESERVED1`, `OP_RESERVED2`, `OP_VER`, `OP_VERIF`, `OP_VERNOTIF`, `OP_INVALIDOPCODE`, and the `OP_RETURN_*` family |
| Disabled (cat-style)            | `OP_CAT`, `OP_MUL`, `OP_DIV`, `OP_MOD`, `OP_LSHIFT`, `OP_RSHIFT`, `OP_INVERT`, `OP_AND`, `OP_OR`, `OP_XOR`, `OP_2MUL`, `OP_2DIV`, `OP_LEFT`, `OP_RIGHT`, `OP_SUBSTR` |

Note that `OP_0`/`OP_1`/`OP_FALSE`/`OP_TRUE` are **not** aliased in `all`; use
`OP_PUSHBYTES_0` for `OP_0`/`OP_FALSE` and `OP_PUSHNUM_1` for `OP_1`/`OP_TRUE`.

BDK 3.1 status: re-exported under `bitcoin::opcodes::all`. No additional
wrapper.

### 1.4 Instruction iterator and types

| Item                                   | Signature / shape                                                              | Source                                       |
| -------------------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------- |
| `Script::instructions`                 | `pub fn instructions(&self) -> Instructions<'_>`                                 | `borrowed.rs.html#559-561`                    |
| `Script::instructions_minimal`         | `pub fn instructions_minimal(&self) -> Instructions<'_>`                         | `borrowed.rs.html#571-573`                    |
| `Script::instruction_indices`          | `pub fn instruction_indices(&self) -> InstructionIndices<'_>`                   | `borrowed.rs.html#580-582`                    |
| `Script::instruction_indices_minimal`  | `pub fn instruction_indices_minimal(&self) -> InstructionIndices<'_>`           | `borrowed.rs.html#585-587`                    |
| `enum Instruction`                     | `Op(Opcode)` \| `PushBytes(&'a PushBytes)`                                       | `blockdata/script/enum.Instruction.html`      |
| `struct PushBytes`                     | DST wrapper; `&PushBytes` requires bytes ≤ 2³²                                  | `blockdata/script/push_bytes.rs`              |
| `struct PushBytesBuf`                  | Owned, growable counter-part to `PushBytes`                                     | `blockdata/script/struct.PushBytesBuf.html`   |
| `struct ScriptHash`                    | 20-byte hash, payload of P2SH                                                    | `blockdata/script/struct.ScriptHash.html`     |
| `struct WScriptHash`                   | 32-byte SHA256 hash, payload of P2WSH                                           | `blockdata/script/struct.WScriptHash.html`    |

The `Instruction` enum in 0.32.x has only **two** variants (`Op`, `PushBytes`)
— there is **no** `PushInt(n)` variant. To decode an `OP_PUSHNUM_*` push as an
integer, callers use the free functions `read_scriptint`, `read_scriptint_non_minimal`
(see below) on the pushed bytes.

```rust
// borrowed.rs.html#242 (paraphrased)
pub fn is_push_only(&self) -> bool
```

Free script-int helpers — `src/bitcoin/blockdata/script/mod.rs.html`:

| Function                | Purpose                                                       |
| ----------------------- | ------------------------------------------------------------- |
| `read_scriptbool`       | Decode a `Script` item as a Bitcoin boolean.                  |
| `read_scriptint`        | Decode an integer in minimal CScriptNum format (errors on non-minimal). |
| `read_scriptint_non_minimal` | Decode an integer in script format without non-minimal error. |
| `write_scriptint`       | Encode an integer in minimal CScriptNum format.               |

BDK 3.1 status: all of these are exposed by the `bitcoin` crate and used
internally by `bdk_script`.

---

## 2. Address module — `bitcoin::Address`

Source root: `src/bitcoin/address/mod.rs.html`. The Address type moved out of
`bitcoin::blockdata::address` in 0.32 and now lives at the crate root.

The defining signature is:

```rust
pub struct Address<V = NetworkChecked>(/* private fields */)
where
    V: NetworkValidation;
```

`Address` is parameterized by a phantom marker indicating whether the network
has been verified. `Address` (the bare name) is a type alias for
`Address<NetworkChecked>`. Methods that produce or require a verified address
are gated on the marker.

### 2.1 Summary table — address constructors and methods

All constructors live on `Address` (the `NetworkChecked` alias).

| Method                                  | Signature                                                                                                                          | BDK 3.1 status | Source                                 |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | -------------- | -------------------------------------- |
| `Address::p2pkh`                        | `pub fn p2pkh(pk: impl Into<PubkeyHash>, network: impl Into<NetworkKind>) -> Address`                                                | yes            | `address/mod.rs.html#410-416`          |
| `Address::p2sh`                         | `pub fn p2sh(script: &Script, network: impl Into<NetworkKind>) -> Result<Address, P2shError>`                                        | yes            | `address/mod.rs.html#424-426`          |
| `Address::p2sh_from_hash`               | `pub fn p2sh_from_hash(hash: ScriptHash, network: impl Into<NetworkKind>) -> Address`                                                | yes            | `address/mod.rs.html#431-434`          |
| `Address::p2wpkh`                       | `pub fn p2wpkh(pk: &CompressedPublicKey, hrp: impl Into<KnownHrp>) -> Self`                                                         | yes            | `address/mod.rs.html#439-443`          |
| `Address::p2shwpkh`                     | `pub fn p2shwpkh(pk: &CompressedPublicKey, network: impl Into<NetworkKind>) -> Address`                                              | yes            | `address/mod.rs.html#446-449`          |
| `Address::p2wsh`                        | `pub fn p2wsh(script: &Script, hrp: impl Into<KnownHrp>) -> Address`                                                                | yes            | `address/mod.rs.html#454-458`          |
| `Address::p2shwsh`                      | `pub fn p2shwsh(script: &Script, network: impl Into<NetworkKind>) -> Address`                                                       | yes            | `address/mod.rs.html#461-469`          |
| `Address::p2tr`                         | `pub fn p2tr<C: Verification>(secp: &Secp256k1<C>, internal_key: UntweakedPublicKey, merkle_root: Option<TapNodeHash>, hrp: impl Into<KnownHrp>) -> Address` | yes            | `address/mod.rs.html#472-475`          |
| `Address::p2tr_tweaked`                 | `pub fn p2tr_tweaked(output_key: TweakedPublicKey, hrp: impl Into<KnownHrp>) -> Address`                                            | yes            | `address/mod.rs.html#481-484`          |
| `Address::from_witness_program`         | `pub fn from_witness_program(program: WitnessProgram, hrp: impl Into<KnownHrp>) -> Address`                                          | yes            | `address/mod.rs.html#492-509`          |
| `Address::from_script`                  | `pub fn from_script(script: &Script, params: impl AsRef<Params>) -> Result<Address, FromScriptError>`                                | yes            | `address/mod.rs.html#593-604`          |
| `Address::address_type`                 | `pub fn address_type(&self) -> Option<AddressType>`                                                                                  | yes            | `address/mod.rs.html#512-520`          |
| `Address::pubkey_hash`                  | `pub fn pubkey_hash(&self) -> Option<PubkeyHash>`                                                                                   | yes            | `address/mod.rs.html#533-540`          |
| `Address::script_hash`                  | `pub fn script_hash(&self) -> Option<ScriptHash>`                                                                                   | yes            | `address/mod.rs.html#543-550`          |
| `Address::witness_program`              | `pub fn witness_program(&self) -> Option<WitnessProgram>`                                                                            | yes            | `address/mod.rs.html#565`              |
| `Address::is_spend_standard`            | `pub fn is_spend_standard(&self) -> bool`                                                                                           | yes            | `address/mod.rs.html#568-590`          |
| `Address::script_pubkey`                | `pub fn script_pubkey(&self) -> ScriptBuf`                                                                                          | yes            | `address/mod.rs.html#633`              |
| `Address::to_qr_uri`                    | `pub fn to_qr_uri(&self) -> String`                                                                                                 | yes            | `address/mod.rs.html#640-648`          |
| `Address::is_related_to_pubkey`         | `pub fn is_related_to_pubkey(&self, pubkey: &PublicKey) -> bool`                                                                    | yes            | `address/mod.rs.html#654-656`          |
| `Address::is_related_to_xonly_pubkey`   | `pub fn is_related_to_xonly_pubkey(&self, xonly_pubkey: &XOnlyPublicKey) -> bool`                                                   | yes            | `address/mod.rs.html#660-671`          |
| `Address::matches_script_pubkey`        | `pub fn matches_script_pubkey(&self, script: &Script) -> bool`                                                                       | yes            | `address/mod.rs.html#692-789`          |
| `Address::assume_checked`               | `pub fn assume_checked(self) -> Address` (on `Address<NetworkUnchecked>`)                                                           | yes            | `address/mod.rs.html#788`              |
| `Address::assume_checked_ref`           | `pub fn assume_checked_ref(&self) -> &Address` (on `Address<NetworkUnchecked>`)                                                     | yes            | `address/mod.rs.html#721-728`          |
| `Address::require_network`              | `pub fn require_network(self, required: Network) -> Result<Address, ParseError>` (on `Address<NetworkUnchecked>`)                   | yes            | `address/mod.rs.html#773-779`          |
| `Address::is_valid_for_network`         | `pub fn is_valid_for_network(&self, n: Network) -> bool` (on `Address<NetworkUnchecked>`)                                           | yes            | `address/mod.rs.html#773-779`          |
| `Address::as_unchecked`                 | `pub fn as_unchecked(&self) -> &Address<NetworkUnchecked>`                                                                          | yes            | `address/mod.rs.html#386-388`          |
| `Address::into_unchecked`               | `pub fn into_unchecked(self) -> Address<NetworkUnchecked>`                                                                          | yes            | `address/mod.rs.html#391`              |
| `FromStr for Address<NetworkUnchecked>` | `fn from_str(s: &str) -> Result<Address<NetworkUnchecked>, ParseError>`                                                             | yes            | `address/mod.rs.html#815-862`          |
| `From<Address> for ScriptBuf`           | `fn from(a: Address) -> Self`                                                                                                       | yes            | `address/mod.rs.html#791-793`          |
| `impl Display for Address`              | (only when `V = NetworkChecked`)                                                                                                    | yes            | `address/mod.rs.html#797-799`          |

### 2.2 `AddressType` enum

Source: `src/bitcoin/address/mod.rs.html#66-79`. The enum is
`#[non_exhaustive]` — match arms must include a wildcard.

| Variant    | Meaning              |
| ---------- | -------------------- |
| `P2pkh`    | Pay to pubkey hash   |
| `P2sh`     | Pay to script hash   |
| `P2wpkh`   | Pay to witness PKH   |
| `P2wsh`    | Pay to witness SH    |
| `P2tr`     | Pay to taproot       |
| `P2a`      | Pay to anchor        |

### 2.3 Companion types and constants

| Type / constant                  | Definition                                                                                  | Source                                          |
| -------------------------------- | ------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| `PubkeyHash`                     | 20-byte HASH160 of compressed pubkey                                                        | `bitcoin/struct.PubkeyHash.html`                |
| `ScriptHash` (alias `hash::ScriptHash`) | 20-byte HASH160 of script                                                             | `blockdata/script/struct.ScriptHash.html`       |
| `WPubkeyHash`                    | 20-byte SegWit V0 P2WPKH payload                                                            | `bitcoin/struct.WPubkeyHash.html`               |
| `WScriptHash`                    | 32-byte SegWit V0 P2WSH payload                                                             | `blockdata/script/struct.WScriptHash.html`      |
| `WitnessProgram`                 | SegWit program (version + payload)                                                          | `bitcoin/struct.WitnessProgram.html`            |
| `WitnessVersion`                 | SegWit version byte (BIP141)                                                                | `blockdata/script/witness_version/index.html`   |
| `TapNodeHash`                    | Tagged hash used in Taproot tree                                                             | `bitcoin/struct.TapNodeHash.html`               |
| `NetworkKind`                    | `{ Main, Test }` (covers mainnet + the three testnet flavors as one)                        | `bitcoin/enum.NetworkKind.html`                 |
| `Network` (full)                 | `{ Bitcoin, Testnet, Testnet4, Signet, Regtest, ... }`                                      | `bitcoin/enum.Network.html`                     |
| `KnownHrp`                       | Bech32 HRP per network (`B` for mainnet, `tb` for testnet, etc.)                            | `bitcoin/enum.KnownHrp.html`                    |
| `CompressedPublicKey`            | secp256k1 compressed pubkey wrapper                                                         | `bitcoin/struct.CompressedPublicKey.html`       |
| `TweakedPublicKey`, `UntweakedPublicKey` | Taproot key wrappers                                                                  | `bitcoin/key/`                                  |
| `ParseError`, `P2shError`, `FromScriptError` | Error enums for address parsing/construction                                       | `bitcoin/address/enum.*.html`                   |

### 2.4 Parsing and Display flow

```rust
// address/mod.rs.html#815-862 (paraphrased)
let addr_unchecked: Address<NetworkUnchecked> = "bc1q...".parse()?;
let addr: Address<NetworkChecked> = addr_unchecked.require_network(Network::Bitcoin)?;
let s: String = addr.to_string();      // Display only on NetworkChecked
```

`Display` is only implemented for `Address<NetworkChecked>`. To print an
unchecked address, call `.assume_checked()` (unsafe in semantics — see
`address/mod.rs.html#788` doc-comment).

BDK 3.1 status: every method in this section is reachable from BDK 3.1 via
`use bitcoin::{Address, ...}`. The two-phase parsing flow is used by
`bdk_wallet`'s `Address::from_str` wrapper.

---

## 3. Consensus encode module — `bitcoin::consensus::encode`

Source root: `src/bitcoin/consensus/encode.rs.html` (lines 3–1326). This is
the byte-level encoding used for anything that must be identical across all
Bitcoin nodes.

### 3.1 Summary table — traits, free functions, types

| Item                                       | Signature                                                                                          | BDK 3.1 status | Source                                |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------- | -------------- | ------------------------------------- |
| `trait Encodable`                          | `fn consensus_encode<W: Write + ?Sized>(&self, writer: &mut W) -> Result<usize, Error>;`             | yes            | `encode.rs.html#328`                  |
| `trait Decodable`                          | (no required methods; both methods have defaults)                                                   | yes            | `encode.rs.html#332-383`              |
| `Decodable::consensus_decode_from_finite_reader` | `fn consensus_decode_from_finite_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self, Error>;` | yes            | `encode.rs.html#362-369`              |
| `Decodable::consensus_decode`              | `fn consensus_decode<R: Read + ?Sized>(reader: &mut R) -> Result<Self, Error>;`                     | yes            | `encode.rs.html#380-382`              |
| `fn serialize`                             | `pub fn serialize<T: Encodable + ?Sized>(data: &T) -> Vec<u8>`                                       | yes            | `encode.rs.html#154-159`              |
| `fn deserialize`                           | `pub fn deserialize<T: Decodable>(data: &[u8]) -> Result<T, Error>`                                 | yes            | `encode.rs.html#168-177`              |
| `fn deserialize_partial`                   | `pub fn deserialize_partial<T: Decodable>(data: &[u8]) -> Result<(T, usize), Error>`                 | yes            | `encode.rs.html#189-195`              |
| `fn serialize_hex`                         | `pub fn serialize_hex<T: Encodable + ?Sized>(data: &T) -> String`                                    | yes            | `encode.rs.html` (encode module)      |
| `fn deserialize_hex`                       | `pub fn deserialize_hex<T: Decodable>(s: &str) -> Result<T, Error>`                                 | yes            | `encode.rs.html` (encode module)      |
| `struct VarInt(pub u64)`                   | Bitcoin CompactSize                                                                                 | yes            | `encode.rs.html#387`                  |
| `VarInt::size`                             | `pub const fn size(&self) -> usize` (1/3/5/9 bytes)                                                  | yes            | `encode.rs.html#450-457`              |
| `impl Encodable for VarInt`                | `fn consensus_encode<W: Write + ?Sized>(&self, w: &mut W) -> Result<usize, Error>;`                 | yes            | `encode.rs.html#475-500`              |
| `impl Decodable for VarInt`                | `fn consensus_decode<R: Read + ?Sized>(r: &mut R) -> Result<Self, Error>;`                          | yes            | `encode.rs.html#502-534`              |
| `struct CheckedData`                       | Data + 4-byte checksum                                                                              | yes            | `encode/struct.CheckedData.html`      |
| `const MAX_COMPACT_SIZE`                   | `usize`, max value encodable as a CompactSize                                                       | yes            | `encode/constant.MAX_COMPACT_SIZE.html` |
| `const MAX_VEC_SIZE`                       | `usize`, byte cap on deserialized vectors                                                          | yes            | `encode/constant.MAX_VEC_SIZE.html`   |
| `enum Error`                               | `#[non_exhaustive]`, variants below                                                                 | yes            | `encode.rs.html#42-69`                |
| `enum FromHexError`                        | Hex deserialization error                                                                          | yes            | `encode/enum.FromHexError.html`       |
| `trait WriteExt`                           | Extension methods on `Write` (see §3.3)                                                             | yes            | `encode.rs.html#198-222`              |
| `trait ReadExt`                            | Extension methods on `Read` (see §3.3)                                                              | yes            | `encode.rs.html#227-249`              |

### 3.2 `encode::Error` variants

| Variant                         | Shape                                                                    | Notes                                              |
| ------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------- |
| `Io(io::Error)`                  | wraps `bitcoin_io::error::Error`                                         | Bubbles up from the underlying writer/reader.      |
| `OversizedVectorAllocation { requested, max }` | `usize, usize`                                                | Triggered when decoding a CompactSize length > `MAX_VEC_SIZE`. |
| `InvalidChecksum { expected, actual }` | `[u8; 4], [u8; 4]`                                                | From `CheckedData`.                                |
| `NonMinimalVarInt`              | unit                                                                     | CompactSize not minimally encoded.                 |
| `OversizedVarInt`               | unit                                                                     | Reserved since 0.32.9, reverted in 0.32.10.        |
| `ParseFailed(&'static str)`     | string                                                                   | Generic parse error.                               |
| `UnsupportedSegwitFlag(u8)`     | byte                                                                     | Witness version/flag unknown to the decoder.       |

The enum is `#[non_exhaustive]` so callers must include a wildcard arm.

### 3.3 `ReadExt` and `WriteExt` methods

These add Bitcoin consensus-aware typed reads/writes on top of
`bitcoin_io::{Read, Write}`. They are the building blocks of every manual
`consensus_encode`/`consensus_decode` impl.

`WriteExt` — `encode.rs.html#198-222`:

| Method          | Signature                                                       | Source line |
| --------------- | --------------------------------------------------------------- | ----------- |
| `emit_u64`      | `fn emit_u64(&mut self, v: u64) -> Result<(), Error>`           | `#200`      |
| `emit_u32`      | `fn emit_u32(&mut self, v: u32) -> Result<(), Error>`           | `#202`      |
| `emit_u16`      | `fn emit_u16(&mut self, v: u16) -> Result<(), Error>`           | `#204`      |
| `emit_u8`       | `fn emit_u8(&mut self, v: u8) -> Result<(), Error>`             | `#206`      |
| `emit_i64`      | `fn emit_i64(&mut self, v: i64) -> Result<(), Error>`           | `#209`      |
| `emit_i32`      | `fn emit_i32(&mut self, v: i32) -> Result<(), Error>`           | `#211`      |
| `emit_i16`      | `fn emit_i16(&mut self, v: i16) -> Result<(), Error>`           | `#213`      |
| `emit_i8`       | `fn emit_i8(&mut self, v: i8) -> Result<(), Error>`             | `#215`      |
| `emit_bool`     | `fn emit_bool(&mut self, v: bool) -> Result<(), Error>`          | `#218`      |
| `emit_slice`    | `fn emit_slice(&mut self, v: &[u8]) -> Result<(), Error>`        | `#221`      |

`ReadExt` — `encode.rs.html#227-249`:

| Method          | Signature                                                       | Source line |
| --------------- | --------------------------------------------------------------- | ----------- |
| `read_u64`      | `fn read_u64(&mut self) -> Result<u64, Error>`                  | `#227`      |
| `read_u32`      | `fn read_u32(&mut self) -> Result<u32, Error>`                  | `#229`      |
| `read_u16`      | `fn read_u16(&mut self) -> Result<u16, Error>`                  | `#231`      |
| `read_u8`       | `fn read_u8(&mut self) -> Result<u8, Error>`                    | `#233`      |
| `read_i64`      | `fn read_i64(&mut self) -> Result<i64, Error>`                  | `#236`      |
| `read_i32`      | `fn read_i32(&mut self) -> Result<i32, Error>`                  | `#238`      |
| `read_i16`      | `fn read_i16(&mut self) -> Result<i16, Error>`                  | `#240`      |
| `read_i8`       | `fn read_i8(&mut self) -> Result<i8, Error>`                    | `#242`      |
| `read_bool`     | `fn read_bool(&mut self) -> Result<bool, Error>`                | `#245`      |
| `read_slice`    | `fn read_slice(&mut self, slice: &mut [u8]) -> Result<(), Error>` | `#248`      |

`ReadExt` is **not** dyn-compatible (object-safe). `WriteExt` is.

### 3.4 `consensus_encode_len` / length-prefixed encoding

There is no single free function named `consensus_encode_len` in this module.
Length-prefixed encoding is done two ways:

1. The caller encodes the length with `VarInt` (or a `CompactSize` impl on the
   type) and then the bytes — e.g. `Vec<T>` impl in `encode.rs.html` shows the
   canonical pattern.
2. `MAX_VEC_SIZE` and `MAX_COMPACT_SIZE` constants cap the size that the
   default `Decodable::consensus_decode` will accept (`encode.rs.html`,
   `consensus_decode_from_finite_reader` wrapper).

If a "consensus_encode_len" helper is needed, **verify in Task 31 spike** —
the closest match in 0.32.x is the inherent `Encodable` impls on standard
container types.

---

## What's NOT in rust-bitcoin 0.32 (for these 3 modules)

Items the task brief listed that we **did not find** in 0.32.11, or that have
been replaced by something else.

### Script

- **`Script::p2pkh(pubkey_hash)`, `Script::p2sh(script_hash)`, `Script::p2wpkh(pubkey_hash)`, `Script::p2wsh(script_hash)`, `Script::p2tr(x_only_pubkey)`** — **NOT** inherent methods on `Script`. The constructors all live on `ScriptBuf` as `new_p2pkh`, `new_p2sh`, `new_p2wpkh`, `new_p2wsh`, `new_p2tr` (see §1.1). For conversion the other direction, `Address` has `script_pubkey(&self) -> ScriptBuf`.
- **`Script::to_p2pk(pubkey) -> Script`** — Not present. The `ScriptBuf::new_p2pk(pubkey: &PublicKey) -> Self` is the correct replacement (returns a `ScriptBuf`, not a `Script`).
- **`Instruction::PushInt(n)`** — **Not in 0.32.11**. The enum has exactly two variants, `Op(Opcode)` and `PushBytes(&'a PushBytes)`. Decode integers via `read_scriptint` / `read_scriptint_non_minimal` free functions.
- **`Script::from_hex_string` / `Script::to_hex_string`** — `to_hex_string(&self) -> String` exists on `Script`. The parser is `ScriptBuf::from_hex(s: &str) -> Result<Self, HexToBytesError>`; there is no `Script::from_hex_string` (use `ScriptBuf::from_hex` then `&*buf`).
- **`Script::dust_value`** — Exists but **deprecated since 0.32.0**; use `minimal_non_dust` (or `minimal_non_dust_custom(fee_rate)`).
- **`Script::is_provably_unspendable`** — Exists but **deprecated since 0.32.0**; use `is_op_return`.

### Address

- **`Address::network` field** — **Not present**. `Address` has no public
  `network` field. The network is encoded into the `NetworkValidation` marker
  (`Address<NetworkChecked>` / `Address<NetworkUnchecked>`), and the HRP/witness
  version are stored inside the private payload. To recover network context use
  `address_type()` plus the HRP from the witness program, or call
  `is_valid_for_network(Network)` on an `Address<NetworkUnchecked>`.
- **`Address::requires_clean_stack()`** — **Not present**. There is no
  predicate with this name. The closest semantic helper is
  `is_spend_standard()` (returns `bool`) and the various `is_p2*` predicates
  on `Script`. Verify in Task 31 spike if a strict equivalent is required.
- **`Address::from_str(s)`** — Not a method. `FromStr` is implemented for
  `Address<NetworkUnchecked>`, so callers do `s.parse::<Address<_>>()?` or
  `Address::<NetworkUnchecked>::from_str(s)`. The result is then
  `.require_network(Network)`-ed.
- **`AddressType` is exhaustive** — **Wrong**. It is `#[non_exhaustive]` —
  `match` arms must include `_ => ...`.
- **`WPubkeyHash` payload access on `Address`** — Use
  `address.pubkey_hash()` (returns `Option<PubkeyHash>`) for P2PKH and
  `address.witness_program()` for SegWit variants; there is no
  `address.wpubkey_hash()` dedicated accessor.

### Consensus encode

- **`consensus_encode_len` named helper** — Not present as a single
  free function. Length-prefix encoding is done via `VarInt` (`size()` +
  `consensus_encode`) plus a manual `emit_slice` (see §3.4).
- **`encode::deserialize_stream` / streaming deserialize** — Not a single
  function. The streaming variant is `deserialize_partial`, which returns
  `Result<(T, usize), Error>` so callers can resume after the bytes consumed.
- **`Address` `Encodable` / `Decodable`** — `Address` is not `Encodable` /
  `Decodable` directly (only `Address<N>: Serialize/Deserialize` with the
  `serde` feature, see `address/mod.rs.html#370-379`). To consensus-encode
  an address, use `Address::script_pubkey()` and encode the resulting
  `ScriptBuf` (which **is** `Encodable`).
- **`Script` `Encodable` / `Decodable`** — `ScriptBuf` is `Encodable` /
  `Decodable` (via `impl Encodable for ScriptBuf`). `Script` (the unsized
  slice) is not — you encode the owned `ScriptBuf` or call
  `Script::to_bytes()` first.

---

## Cross-references

- BDK 3.1 re-exports all the types above via `bitcoin::{Script, ScriptBuf,
  Builder, Address, Network, VarInt, ...}` and the
  `bitcoin::consensus::encode` module, so a wallet layer built on BDK does not
  need to depend on `bitcoin` directly for these surfaces.
- The `script` and `consensus::encode` modules are **fully documented**
  (100% coverage per docs.rs page banner), so any function not listed here can
  be looked up directly without source spelunking.
- For deep source review, the canonical URL pattern is
  `https://docs.rs/bitcoin/0.32.11/src/bitcoin/<path>.rs.html`, e.g.
  `https://docs.rs/bitcoin/0.32.11/src/bitcoin/blockdata/script/builder.rs.html`.

## Open items / verify in Task 31 spike

- Whether `consensus_encode_len` exists under any name (e.g. on container
  types). The 0.32 surface I scanned did not surface a single function with
  that name.
- Whether BDK 3.1's `bdk_script` re-exports the `Builder` methods directly or
  only via descriptor compilation. The `Builder` type itself is reachable
  regardless.
- Whether `Address::is_spend_standard` is the right replacement for any
  "requires_clean_stack" semantics the task brief hinted at.
