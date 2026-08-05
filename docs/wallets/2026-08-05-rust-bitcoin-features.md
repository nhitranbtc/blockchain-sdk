# Rust-Bitcoin 0.32 — Complete API Surface

**Date:** 2026-08-05
**Source:** Live docs.rs (`bitcoin 0.32.11`, June 2026) + GitHub source paths in `rust-bitcoin/rust-bitcoin` monorepo at tag `bitcoin-0.32.11`.
**Companion docs (split for parallel research):**
- [Part 1: core types + transactions + network](2026-08-05-rust-bitcoin-features-01-core-tx-network.md)
- [Part 2: script + address + consensus](2026-08-05-rust-bitcoin-features-02-script-address-consensus.md)
- [Part 3: PSBT + sighash + hashes](2026-08-05-rust-bitcoin-features-03-psbt-sighash-hashes.md)
- [Part 4: secp256k1 + key + taproot](2026-08-05-rust-bitcoin-features-04-secp256k1-key-taproot.md)

## TL;DR

`rust-bitcoin 0.32` is the **primitive layer** below `bdk_wallet 3.1`. BDK re-exports all the types via `bdk_wallet::bitcoin::*` (same crate, same version). The two are **not alternatives** — they are **layers**. `rust-bitcoin` provides the types and primitive operations; `bdk_wallet` provides the wallet engine that uses them. The plan touches `rust-bitcoin` directly for **4 things BDK does not do**: script building, address encoding, sighash computation, and PSBT serialization.

## Use cases `rust-bitcoin 0.32` handles natively (no `bdk_wallet` involvement)

These are user-facing capabilities that `rust-bitcoin` provides out of the box. The `bitcoin-wallet-core` library wraps each one with a thin Rust API; the `btc` CLI exposes it as a subcommand. No `bdk_wallet` call needed for these — direct type operations only.

| Use case | API | Code we write |
|---|---|---|
| **Build a transaction from scratch** (UTXO + recipient + fee) | `Transaction` + `TxIn` + `TxOut` (Task 11 input) | full tx construction with `Amount` + `FeeRate` |
| **Build any Bitcoin script** (P2PKH / P2SH / P2WPKH / P2WSH / P2TR / OP_RETURN / custom multisig / timelocks) | `ScriptBuf::new_p2pkh/p2sh/p2wpkh/p2wsh/p2tr` + `Builder::push_*` (Task 5) | script construction for any policy |
| **Parse any script to opcode stream** (decode raw scripts for analysis) | `Script::instructions()` returns `Instruction` iterator (Task 5) | script parsing for verification + feature detection |
| **Encode any standard address** (legacy P2PKH / nested segwit / native segwit / taproot) | `Address::p2pkh/p2sh/p2wpkh/p2wsh/p2tr/p2tr_tweaked` (Task 6) | address generation for any address type |
| **Decode any standard address** (parse base58/bech32/bech32m string to `Address`) | `Address::from_str(s)` + `Address::assume_checked()` (Task 6) | address validation |
| **Serialize any Bitcoin type to bytes** (tx, psbt, block, etc.) | `consensus::serialize(&t) -> Vec<u8>` (Task 12) | P2P / disk format |
| **Deserialize any Bitcoin type from bytes** | `consensus::deserialize::<T>(bytes) -> Result<T, _>` (Task 12) | P2P / disk format |
| **Compute sighash for any input type** (legacy / segwit v0 / taproot key-path / taproot script-path) | `SighashCache::p2wpkh_signature_hash` / `legacy_signature_hash` / `taproot_signature_hash` (Task 12) | sighash extraction for hardware-signer integration (Task 28) |
| **Build a PSBT v1** (BIP-174) | `Psbt::from_unsigned_tx` + `Input`/`Output` builders (Task 12) | full PSBT construction |
| **Sign a PSBT in place** (ECDSA or Schnorr) | `Psbt::sign` or `wallet.sign(&mut psbt, ...)` (Task 13) | transaction finalization |
| **Finalize a PSBT to extract the raw tx** | `Psbt::extract_tx() -> Result<Transaction, _>` (Task 13) | broadcast-ready bytes |
| **Build a Taproot output** (BIP-86 single key OR BIP-341 script tree) | `TaprootBuilder::add_leaf_with_ver` + `finalize` + `Address::p2tr_tweaked` (Task 6) | any Taproot construction |
| **Sign ECDSA over a 32-byte hash** (BIP-143 / BIP-146 anti-grinding / low-R) | `secp256k1::Secp256k1::sign_ecdsa` / `sign_ecdsa_low_r` (Task 4 + Task 29) | raw ECDSA signing |
| **Sign Schnorr (BIP-340)** with optional aux randomize | `secp.sign_schnorr` / `secp.sign_schnorrno_aux_rand` (Task 4) | Taproot signing |
| **Verify signatures** (ECDSA or Schnorr) | `secp.verify_ecdsa` / `secp.verify_schnorr` (Task 4) | signature validation |
| **Convert to/from any Bitcoin hash** (sha256, hash160, ripemd160, sha256d) | `bitcoin::hashes::{sha256, hash160, ripemd160}::Hash` (Task 29 for BIP-137) | non-tx hashing (message signing, address derivation) |
| **Compute HMAC-SHA512** (BIP-32 master derivation, BIP-39 seed) | `bitcoin::hashes::hmac::Hmac<sha512::Hash>` (used by standalone `bip32` + `bip39` crates) | HD derivation building blocks |
| **Multi-network support** (5 networks) | `Network` enum + `from_magic` / `magic` / `is_mainnet` / `is_testnet` (Task 7 config) | network parameter everywhere |
| **PSBT base64 round-trip** | `Psbt::from_str` / `to_string` (Task 12) | PSBT export/import for `--dry-run` + descriptor exchange |
| **Txid + wtxid computation** | `Transaction::compute_txid()` / `compute_wtxid()` (Task 17 RBF + Task 18 BIP-137) | txid for RBF target + wtxid for BIP-137 |
| **Sequence number manipulation** (BIP-125 RBF signaling + BIP-68 relative locktime) | `Sequence(u32)` newtype with constructors (Task 17) | RBF flag setting + locktime |

**Use cases rust-bitcoin does NOT handle** (delegated to other layers):

| Use case | Layer |
|---|---|
| Wallet construction from descriptor | `bdk_wallet::Wallet::create` |
| Chain sync (Esplora/Electrum) | `bdk_esplora` / `bdk_electrum` |
| UTXO selection (BnB, Knapsack, etc.) | `bdk_wallet::coin_selection` |
| TxBuilder (recipient list, fee selection, change gen) | `bdk_wallet::TxBuilder` |
| RBF / CPFP | `bdk_wallet::build_fee_bump` |
| Manual UTXO selection | `bdk_wallet::TxBuilder::add_utxo` |
| Signing of a PSBT (in-process, with descriptor key) | `bdk_wallet::Wallet::sign` |
| External signer interface | `bdk_wallet::add_signer` + `TransactionSigner` trait |
| Balance / transaction history / UTXO queries | `bdk_wallet::Wallet` |
| Multi-address via xpub | `bdk_wallet::Wallet::reveal_next_address` |
| Watch-only wallets | `bdk_wallet::Wallet::new_single(public_descriptor)` |
| Persistence (atomic staged changes) | `bdk_wallet::take_staged` + `bdk_file_store` |
| Error introspection (5 sibling enums) | `bdk_wallet::error::*` |
| Descriptor export + checksum | `bdk_wallet::Wallet::public_descriptor` + `descriptor_checksum` |
| HD derivation (BIP-32) | standalone `bip32 0.6` |
| BIP-39 mnemonic | `bdk_wallet::keys::bip39::Mnemonic` (re-export) |
| Encrypted mnemonic at rest | `argon2` + `aes-gcm` + `zeroize` |
| Descriptor parsing (BIP-380) | `bdk_wallet::descriptor!()` (miniscript) |
| Lightning | `rust-lightning` |

**Net:** `rust-bitcoin 0.32` handles **~21 use cases** — the **primitive operations** layer. `bdk_wallet 3.1` handles **~20 use cases** — the **wallet engine** layer. Same crate family, same dependency tree, no overlap.

## Master index — 12 modules × public surface

| # | Module | Public surface highlights | Plan uses it for | BDK re-exports? |
|---|---|---|---|---|
| 1 | `bitcoin` (crate root) | `Amount`, `FeeRate`, `OutPoint`, `Sequence`, `LockTime`, `Witness`, `VarInt`, `Block` | throughout | ✅ re-exported as `bdk_wallet::bitcoin` |
| 2 | `bitcoin::transaction` | `Transaction`, `TxIn`, `TxOut`, `Version`, `absolute::LockTime`, `relative::LockTime` | serialization, TxOut, signature hash | ✅ via `bdk_wallet::bitcoin::Transaction` |
| 3 | `bitcoin::network` | `Network` enum (Bitcoin/Testnet/Testnet4/Signet/Regtest), `NetworkKind`, `consensus::Params` | config, Esplora URL builder | ✅ via `bdk_wallet::bitcoin::Network` |
| 4 | `bitcoin::blockdata::script` | `Script` / `ScriptBuf` / `Builder`; `new_p2pkh/p2sh/p2wpkh/p2wsh/p2tr/p2tr_tweaked/op_return`; opcodes via `bitcoin::opcodes::all::*` | Task 5 (script building) | ❌ (not in BDK surface — BDK only consumes scripts) |
| 5 | `bitcoin::Address` | `Address<NetworkChecked>` (alias `Address`); `p2pkh/p2sh/p2wpkh/p2wsh/p2tr/p2tr_tweaked/from_witness_program/from_script`; `address_type`, `script_pubkey`, `matches_script_pubkey` | Task 6 (address encoding) | ❌ (not in BDK surface — BDK only consumes addresses) |
| 6 | `bitcoin::consensus::encode` | `Encodable`/`Decodable` traits; `serialize`/`deserialize`/`deserialize_partial`/`serialize_hex`/`deserialize_hex`; `VarInt`; `Error` enum (7 variants) | Task 12 (PSBT round-trip) | partial (BDK has `Psbt::serialize/deserialize`) |
| 7 | `bitcoin::psbt` | `Psbt` (BIP-174 only — NO v2 in 0.32.11); `Input` (21 fields, Taproot + preimage + BIP-32); `Output`; `ecdsa_hash_ty`/`taproot_hash_ty`; `extract_tx`; `combine`; `sign` | Task 12 (PSBT), Task 13 (extract_tx) | ✅ via `bdk_wallet::bitcoin::psbt::Psbt` |
| 8 | `bitcoin::sighash` | `SighashCache`; per-script-type methods (`p2wpkh_signature_hash`, `legacy_signature_hash`, `taproot_signature_hash`); typed sighashes (`SegwitV0Sighash`, `TapSighash`, `LegacySighash`); `Annex`; `Prevouts`; `ScriptPath`; `EcdsaSighashType`; `TapSighashType` | Task 12 (sighash extraction) | ❌ (not in BDK surface — BDK computes internally) |
| 9 | `bitcoin::hashes` (re-exported from `bitcoin_hashes ^0.14`) | `Hash`/`HashEngine` traits; `sha256`, `sha256d`, `hash160`, `ripemd160`, `sha1`, `sha512`, `sha512_256`, `sha384`, `siphash24`; `hmac::Hmac`; `cmp::fixed_time_eq`; `hash_newtype!` macro | Task 29 (BIP-137 hash: `sha256(SHA256("\x19Bitcoin..."))`) | ❌ (BPK has `hashes::Hash` but we use it directly for non-Bitcoin-tx purposes) |
| 10 | `bitcoin::secp256k1` (re-exported from `secp256k1 0.30.0`) | `Secp256k1<Verification/Signing/All>`; `Keypair::from_secret_key`; `Message::from_digest`; `ecdsa::Signature` (64-byte compact); `XOnlyPublicKey`; `schnorr::Signature`; `sign_ecdsa`/`sign_schnorr`/`verify_ecdsa`/`verify_schnorr` | Task 4 (signer), Task 29 (BIP-137 ECDSA) | partial — `bdk_wallet::bitcoin::secp256k1::*` available but we add `secp256k1 0.30` as direct dep for clearer import paths |
| 11 | `bitcoin::key` | `PublicKey` (wrapper around `secp256k1::PublicKey` with Bitcoin methods); `new`, `pubkey_hash`, `wpubkey_hash`, `p2wpkh_script_code`, `verify`; **NO** `from_secret_key` (use `from_private_key` instead — takes `PrivateKey`); **NO** `serialize`/`is_compressed`/`is_fully_valid` — access via `pk.inner.serialize()`/`.is_full_valid()` | Task 4 (signer), Task 6 (address), Task 9 (wallet) | ✅ via `bdk_wallet::bitcoin::key::PublicKey` |
| 12 | `bitcoin::taproot` | `TaprootBuilder` (DFS order with `depth: u8`); `add_leaf_with_ver`, `add_hidden_node`; `finalize<C>(secp, internal_key: UntweakedPublicKey) -> Result<TaprootSpendInfo, TaprootBuilder>`; `TaprootSpendInfo` (all fields private, accessors); `LeafVersion` (TapScript + Future(FutureLeafVersion)); **NO** `TaprootSigInfo` enum (sighash byte encoded in `taproot::Signature`) | Task 6 (Taproot address), future P2TR script-path (deferred to v1.0) | ❌ (not in BDK surface for v0.1) |

## Critical findings (changes plan design)

1. **PSBT v2 NOT supported in 0.32.11.** The v2 fields (`tx_modifiable`, `previous_txid`, `spent_outputs`, etc.) only landed on `master` via PR #3424 (2024-09-29) and commits `38dd041` / `fea379b`. **Our plan must use PSBT v1 (BIP-174) for v0.1.** No impact on user-facing features — our dry-run prints base64 PSBT v1.

2. **PublicKey's `from_secret_key` does NOT exist.** It was renamed to `from_private_key(secp, &PrivateKey)` in 0.31. The plan's Task 4 example code (`PublicKey::from_secret_key(&secp, &sk)`) is wrong. Must be `PublicKey::from_private_key(&secp, &PrivateKey::new(secret_key, network))` — and `PrivateKey::new(secp, sk, network)` is the canonical constructor. **Verify in Task 31 spike.**

3. **Sighash newtypes need `.to_byte_array()` before `Message::from_digest()`.** `SighashCache::p2wpkh_signature_hash(...)` returns `SegwitV0Sighash` (a typed newtype), NOT `Message`. Must call `.to_byte_array() -> [u8; 32]` then `Message::from_digest(*bytes)`. No blanket `From<Sighash> for Message` impl. **Common 0.31 → 0.32 porting gotcha.**

4. **TxOut::value is `Amount` (not raw `u64`)** since 0.31. Plan code that does `txout.value` and treats it as `u64` will fail to compile. Use `.to_sat()` or `.to_btc()`.

5. **Script constructors live on `ScriptBuf`, not `Script`.** `Script::p2pkh(...)` does NOT exist. Use `ScriptBuf::new_p2pkh(pubkey_hash)`. The plan's Task 5 may have used the wrong receiver type — verify in spike.

6. **`Address` is `Address<NetworkChecked>`.** `Address::network` field is NOT public; the type is parameterized by a marker. Use `Address::assume_checked()` only if you've already validated.

7. **`bdk_wallet::key` does NOT re-export `Signing` and `All`.** Only `Verification`, `Secp256k1`, `Keypair`, `Parity`, `XOnlyPublicKey`, `constants`. The `Signing` context for `Keypair::from_secret_key` is what we want — the standalone `secp256k1 0.30` direct dep gives us full control.

8. **`bitcoin_hashes` is a separate crate, re-exported as `bitcoin::hashes`.** It's NOT a `mod` — it doesn't use the `bitcoin_unstable` flag. Stable API since 0.14. Plan's Task 29 (`Hmac`, `sha256::Hash`) works through the re-export.

## What's NOT in rust-bitcoin 0.32 (explicit gaps)

| Missing | Why | What we do |
|---|---|---|
| **PSBT v2** (BIP-370) | not yet released on `bitcoin 0.32.11` tag (only on master) | use PSBT v1 for v0.1; revisit in v1.0 |
| **TaprootSigInfo** enum | consolidated into `taproot::Signature` | use `Signature::randomize(aux)` for BIP-341 sighash |
| **`From<Sighash> for secp256k1::Message`** blanket | typed newtypes need explicit conversion | call `.to_byte_array() -> Message::from_digest(*bytes)` |
| **`siphash48`** | only `siphash24` (Bitcoin block-level) | not needed for v0.1 |
| **`PartiallySignedTransaction`** type alias | canonical name is `Psbt` | use `Psbt` |
| **`PublicKey::from_secret_key(&secp, &sk)`** | renamed to `from_private_key(secp, &PrivateKey::new(sk, network))` | use `PrivateKey::new(secp, sk, network)` then `PublicKey::from_private_key(&secp, &privkey)` |
| **`PublicKey::serialize()` / `is_compressed()` / `is_fully_valid()`** | use `pk.inner.serialize()` / `is_full_valid()` | deref to inner `secp256k1::PublicKey` |
| **`Script::p2pkh/p2sh/...`** constructors | moved to `ScriptBuf::new_*` | use `ScriptBuf::new_p2pkh(...)` |
| **`Instruction::PushInt`** | removed in 0.32 | decode via `Op(OP_PUSHNUM_N)` or `Op(OP_PUSHBYTES_N)` then parse |
| **`consensus_encode_len` named helper** | inline `size` calculation | compute manually: `8 + 1 + 33 + 1 + varint(output_count) + 1 + 4 + 4 + varint(input_count)` |
| **`Address::network` field** | type-parameterized marker | use `Address::assume_checked()` if you need it after validation |
| **HD derivation (BIP-32)** | not in scope | standalone `bip32` crate |
| **BIP-39 mnemonic** | not in scope | BDK's `keys::bip39` re-export (or standalone `bip39`) |
| **Coin selection algorithms** | not in scope | `bdk_wallet` provides them |
| **Chain sync** (Esplora/Electrum) | not in scope | `bdk_esplora` / `bdk_electrum` |
| **Descriptor parsing** | not in scope | `bdk_wallet::descriptor!()` (miniscript) |
| **Lightning** | not in scope | `rust-lightning` |

## Plan impact (concrete corrections)

| Plan task | Original assumption | Corrected |
|---|---|---|
| Task 4 (signer) | `PublicKey::from_secret_key(&secp, &sk)` | `PublicKey::from_private_key(&secp, &PrivateKey::new(sk, network))` |
| Task 5 (script) | `Script::p2pkh(pubkey_hash)` | `ScriptBuf::new_p2pkh(pubkey_hash)` |
| Task 6 (address) | `Address::network` | use `Address::assume_checked()` after validation, or pass network explicitly to constructor |
| Task 12 (psbt) | PSBT v2 in 0.32 | PSBT v1 only; revisit in v1.0 |
| Task 12 (sighash) | `SighashCache::..._signature_hash(...) -> Message` | `SighashCache::..._signature_hash(...) -> SegwitV0Sighash` (or similar typed newtype) → `.to_byte_array() -> Message::from_digest(*bytes)` |
| Task 11 (tx::builder) | `txout.value` as `u64` | `.to_sat()` or `.to_btc()` (it's `Amount`) |

## Full-task verification list for Task 31 (BDK API spike — extended with rust-bitcoin checks)

The spike must validate these specific assumptions **in addition to** the 24-item BDK list from `docs/wallets/2026-08-05-bdk-wallet-features.md`:

25. `Script::p2pkh` does NOT exist — use `ScriptBuf::new_p2pkh(pubkey_hash)`.
26. `PublicKey::from_secret_key` does NOT exist — use `PublicKey::from_private_key(secp, &PrivateKey::new(sk, network))`.
27. `PublicKey::serialize()` does NOT exist on the wrapper — use `pk.inner.serialize()`.
28. `Address::p2pkh(pubkey, network)` does exist (it validates the network).
29. `TxOut::value` is `Amount` not `u64`.
30. `SighashCache::p2wpkh_signature_hash(...)` returns `SegwitV0Sighash`, not `Message`.
31. `Message::from_digest(*bytes)` requires the `[u8; 32]` from `.to_byte_array()`.
32. `Psbt` in 0.32.11 is v1 only — no v2 fields.
33. `bitcoin::secp256k1::Secp256k1::new()` returns `Secp256k1<All>` (the `All` zero-sized type tag).
34. `bitcoin::opcodes::all::OP_DUP` etc. are module-level constants, not enum variants.

If any fail, the fix is one of: use the v0.32.11-correct name, fall back to `bdk_wallet::bitcoin::*` re-export path, or document the gap in the relevant part doc.

## Sources

All 4 part docs cite:
- `https://docs.rs/bitcoin/0.32.11/bitcoin/`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/script/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/blockdata/address/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/consensus/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/psbt/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/sighash/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/hashes/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/secp256k1/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/key/index.html`
- `https://docs.rs/bitcoin/0.32.11/bitcoin/taproot/index.html`
- `https://github.com/rust-bitcoin/rust-bitcoin` at tag `bitcoin-0.32.11`

## How to use these 5 docs

| Reader | Path |
|---|---|
| Engineer starting a task that uses rust-bitcoin | Read the per-module "Plan uses it for" column in this index, then drill into the relevant part doc (1-4) for full signatures |
| Code reviewer | Cross-check that the implementation uses the rust-bitcoin 0.32 method (not 0.30/0.31) — the "Critical findings" section flags all the renames/removals |
| Task 31 spike engineer | Use the 10-item "Plan impact" list to validate each API assumption. Add to the 24-item BDK verification list (docs/wallets/2026-08-05-bdk-wallet-features.md §"Full-task verification list") for a 34-item combined checklist. |
| Plan editor | Use the "Critical findings" section to update plan task bodies where the example code uses a 0.30/0.31 method |
