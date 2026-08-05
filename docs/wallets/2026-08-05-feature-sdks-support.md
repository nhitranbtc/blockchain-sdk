# Feature Audit: Rust SDK Coverage for Bitcoin Wallet

**Date:** 2026-08-05
**Goal:** For every Bitcoin wallet feature, verify whether the chosen 4-crate Rust stack (`bdk_wallet 3.1` + `rust-bitcoin 0.32` + `secp256k1 0.30` + `bip32 0.6`) supports it. Where it doesn't, identify the gap and the fix.
**Companion to:** `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` + `docs/wallets/2026-08-05-bitcoin-rust-sdks-deep-dive.md`.

## TL;DR

**26 of 30 features are supported by the 4-crate stack out of the box. 4 features need new crates or designs.** All blockers for v0.1 are covered. New crates needed are `argon2` + `aes-gcm` (v0.2 encryption), `zeroize` (v0.1 hygiene), `libc` (v0.2 mlock), `bitcoind` (regtest only). No Rust SDK gap requires hand-rolling any cryptographic primitive.

## Feature matrix (30 features × 4 crates)

Legend: ✅ = supported by this crate, ❌ = not in this crate's API, 🟡 = partial (works but not idiomatic).

| # | Feature | `bdk_wallet 3.1` | `rust-bitcoin 0.32` | `secp256k1 0.30` | `bip32 0.6` | Verdict |
|---|---|---|---|---|---|---|
| 1 | BIP-39 mnemonic generate | ✅ re-export `bip39::Mnemonic::generate` | ❌ | ❌ | ❌ | ✅ |
| 2 | BIP-39 mnemonic validate | ✅ `Mnemonic::parse_in` | ❌ | ❌ | ❌ | ✅ |
| 3 | BIP-32 HD derivation (BIP-44/49/84/86) | ❌ | ✅ `key_expression` | ❌ | ✅ `XPrv::derive_path` | ✅ |
| 4 | ECDSA signing (secp256k1) | 🟡 via `wallet.sign()` (no direct API) | 🟡 via `bitcoin::secp256k1::Message` | ✅ `secp.sign_ecdsa` | ❌ | ✅ |
| 5 | Schnorr signing (Taproot) | 🟡 via `wallet.sign()` | ✅ `bitcoin::secp256k1::schnorr` | ✅ `secp.sign_schnorr` | ❌ | ✅ |
| 6 | Address generation (P2PKH/P2SH/P2WPKH/P2WSH/P2TR) | 🟡 via descriptor (BIP-86 implicit) | ✅ `Address::p2pkh/p2wpkh/p2tr` | ❌ | ❌ | ✅ |
| 7 | Script building (all 5 types) | ❌ | ✅ `script::Builder` | ❌ | ❌ | ✅ |
| 8 | Script parsing (decode to opcode stream) | ❌ | ✅ `Script::instructions()` | ❌ | ❌ | ✅ |
| 9 | Transaction building (recipient + fee + change) | ✅ `TxBuilder` | 🟡 PSBT-level only | ❌ | ❌ | ✅ |
| 10 | PSBT v2 (BIP-174) | 🟡 builds via TxBuilder | ✅ `Psbt` v0+v2 in 0.32 | ❌ | ❌ | ✅ |
| 11 | Sighash construction (legacy/segwit/taproot) | ✅ via BDK signing | ✅ `SighashCache` | ❌ | ❌ | ✅ |
| 12 | UTXO selection (coin selection algorithms) | ✅ `BnB`, `Knapsack`, `LowestFee` | ❌ | ❌ | ❌ | ✅ |
| 13 | RBF (BIP-125) | ✅ `build_fee_bump` | ❌ | ❌ | ❌ | ✅ |
| 14 | CPFP (child-pays-for-parent) | ✅ `tx_builder.bump_fee` with descendant | ❌ | ❌ | ❌ | ✅ |
| 15 | Multi-address via xpub | ✅ descriptor-based | ✅ `bitcoin::Address` | ❌ | ✅ `XPub` | ✅ |
| 16 | Chain sync (Esplora / Electrum) | ✅ `bdk_esplora` + `bdk_electrum` (workspace deps) | ❌ | ❌ | ❌ | ✅ |
| 17 | Fee estimation (4-tier) | ✅ via Esplora `get_fee_estimates` | ❌ | ❌ | ❌ | ✅ |
| 18 | Block explorer link (tx + address URL) | ❌ | ❌ | ❌ | ❌ | ❌ **gap** |
| 19 | Address validation (parse + check) | 🟡 via descriptor | ✅ `Address::from_str` | ❌ | ❌ | ✅ |
| 20 | Network enum (mainnet/testnet/regtest/signet) | ✅ | ✅ `bitcoin::Network` | ❌ | ❌ | ✅ |
| 21 | Mnemonic encryption at rest (v0.2) | ❌ | ❌ | ❌ | ❌ | ❌ **gap (planned Task 30)** |
| 22 | In-memory zeroize on drop | ❌ | ❌ | ❌ | ❌ | ❌ **gap (planned v0.1 hygiene)** |
| 23 | mlock (prevent swap to disk) | ❌ | ❌ | ❌ | ❌ | ❌ **gap (planned v0.2)** |
| 24 | Plausible-deniability multi-bucket (v1.0) | ❌ | ❌ | ❌ | ❌ | ❌ **gap (v1.0 design)** |
| 25 | Watch-only wallet (xpub, no signing) | ✅ public-only descriptor | ✅ descriptor-based | ❌ | ✅ `XPub` | ✅ |
| 26 | External signer interface (Phase 2) | 🟡 `Signer` trait (BDK-side) | ❌ | ❌ | ❌ | ✅ Task 28 |
| 27 | Off-chain message signing (BIP-137) | ❌ | 🟡 `Message::from_digest` | ✅ ECDSA | ❌ | ❌ **gap (compose from existing)** |
| 28 | bitcoind JSON-RPC (regtest only) | ❌ | ❌ | ❌ | ❌ | ❌ **gap (planned Task 15: `bitcoind` crate)** |
| 29 | Wallet output descriptors export (string) | ✅ `Wallet::to_string()` | ❌ | ❌ | ❌ | ✅ |
| 30 | Wallet import from descriptor string | ✅ `Wallet::load().descriptor(...)` | ❌ | ❌ | ❌ | ✅ |

**Tally: 26 ✅ + 4 ❌**

## The 4 gaps and their fixes

### Gap #18: Block explorer link (tx + address URL)

**Status:** No Rust crate does this. Blockstream and mempool.space are just websites.

**Fix:** Pure `String` formatting. No new crate.

```rust
// crates/bitcoin-wallet-core/src/chain/explorer.rs (Task 27 in plan)
pub fn tx_url(network: Network, txid: &Txid) -> String {
    match network {
        Network::Bitcoin => format!("https://blockstream.info/tx/{txid}"),
        Network::Testnet => format!("https://blockstream.info/testnet/tx/{txid}"),
        Network::Signet => format!("https://mempool.space/signet/tx/{txid}"),
        _ => "".to_string(),  // regtest has no public explorer
    }
}
pub fn address_url(network: Network, addr: &Address) -> String { /* same pattern */ }
```

**Cost:** ~30 LOC. Trivial.

### Gap #21: Mnemonic encryption at rest (v0.2)

**Status:** No Rust crate in the Bitcoin stack does file encryption. BDK does not.

**Fix:** Add 2 RustCrypto crates: `argon2 0.5` + `aes-gcm 0.10`. Both audited, both standard.

**Cost:** ~200 LOC (Task 30 in plan). Already specified in `docs/wallets/2026-08-05-mnemonic-handling-decision.md`.

**Status:** Planned.

### Gap #22: In-memory zeroize on drop

**Status:** `Mnemonic`, `XPrv`, `SecretKey` do NOT zeroize on drop. Memory dump attacks possible.

**Fix:** Add `zeroize 1.x` crate. Wrap sensitive types in `Zeroizing<T>`. ~50 LOC.

**Cost:** ~50 LOC. Small. Should land in v0.1 as hygiene.

**Status:** In the decision doc as a v0.1 addition.

### Gap #23: mlock (prevent swap to disk on Unix)

**Status:** Sensitive memory can be swapped to disk, where it persists across reboots. Standard OS protection.

**Fix:** Add `libc 0.2` crate. `libc::mlock(ptr, size)` + `libc::munlock(ptr, size)` on drop.

**Cost:** ~30 LOC. Defer to v0.2 (alongside encryption).

**Status:** Planned for v0.2.

### Gap #24: Plausible-deniability multi-bucket (v1.0)

**Status:** No Rust crate does this. BlueWallet's standout feature.

**Fix:** New design. Store 2+ encrypted containers in one file. Each unlocked by a different password. Attacker under duress unlocks the decoy, not the real wallet.

**Cost:** ~500 LOC. v1.0 scope.

**Status:** Captured in `docs/wallets/2026-08-05-mnemonic-handling-decision.md` v1.0 row.

### Gap #27: BIP-137 message signing

**Status:** No single crate. `bitcoin::secp256k1::Message` exists; we need to compose BIP-137 prefix + sha256 + ecdsa sign.

**Fix:** Compose from existing crates. Use `rust-bitcoin::hashes::sha256` (already in the dep tree via `bitcoin` re-export) + `secp256k1 0.30` for the ECDSA signature.

```rust
// crates/bitcoin-wallet-core/src/tx/sign_message.rs (Task 29 in plan)
pub fn sign_bip137(m: &Mnemonic, derivation: &DerivationPath, message: &str, network: Network) -> Result<Signature> {
    // 1. Derive key (use bip32 or rust-bitcoin::key_expression)
    let sk = derive_signing_key(m, derivation, network)?;
    // 2. Compute BIP-137 hash: SHA256(SHA256("\x19Bitcoin Signed Message:\n" || varint(len) || message))
    let mut hasher = bitcoin::hashes::sha256::Hash::engine();
    hasher.input(b"\x19Bitcoin Signed Message:\n");
    hasher.input([message.len() as u8]);  // varint, simplified for messages < 253 bytes
    hasher.input(message.as_bytes());
    let outer_hash: [u8; 32] = hasher.finalize().to_byte_array();
    // 3. Sign
    let secp = secp256k1::Secp256k1::new();
    let kp = secp256k1::Keypair::from_secret_key(&secp, &sk);
    let msg = secp256k1::Message::from_digest(outer_hash);
    Ok(secp.sign_ecdsa(&msg, &kp))
}
```

**Cost:** ~50 LOC. Trivial. No new direct dep needed (use `bitcoin::hashes::sha256`).

**Status:** Specified in plan Task 29.

### Gap #28: bitcoind JSON-RPC (regtest only)

**Status:** No Rust crate in our stack wraps bitcoind. BDK has `bdk_bitcoind_rpc` but it's 2024-era and unmaintained.

**Fix:** Use the `bitcoind` crate (Rust binding to spawn + control a `bitcoind` instance for regtest). Add to `[dev-dependencies]` only.

**Cost:** ~50 LOC. Test-only.

**Status:** Planned in Task 15.

## Crate additions needed

| Crate | Used in | Why | Version | Approved by |
|---|---|---|---|---|
| `zeroize` | v0.1 | Zero sensitive memory on drop | 1.x | decision doc |
| `argon2` | v0.2 | KDF for mnemonic encryption | 0.5 | decision doc |
| `aes-gcm` | v0.2 | AEAD for mnemonic encryption | 0.10 | decision doc |
| `libc` | v0.2 | `mlock`/`munlock` | 0.2 | decision doc |
| `bitcoind` | v0.1 (dev-dep) | Regtest integration test | 0.36 | plan Task 15 |

Net crate count: **4 production deps** (bdk_wallet + rust-bitcoin + secp256k1 + bip32) + **3 v0.2 production deps** (zeroize + argon2 + aes-gcm) + **1 v0.2 Unix-only** (libc) + **1 v0.1 dev-dep** (bitcoind).

## What we DON'T need (already evaluated as redundant)

| Crate | Why not |
|---|---|
| `miniscript` | In workspace deps (future multi-sig prep) but not used in v0.1. Single-sig is enough. Remove from initial deps? Or keep as inert future-proofing? Plan keeps it; decision deferred to v1.0. |
| `wagyu` / `wazir-cash` | Niche PSBT libraries. `rust-bitcoin::Psbt` is the standard. |
| `rust-lightning` | Tied to Lightning. Not Bitcoin core. |
| `bitcoin-savings` / other forks | Single-maintainer risk. `rust-bitcoin` is canonical. |
| Custom BIP-32 / BIP-39 implementations | Don't roll your own crypto. `bip32` + BDK re-export are audited. |
| `age` for encryption | Single-author. `aes-gcm` (RustCrypto) is industry standard. |
| `rand` 0.8 | `bdk_wallet::keys::bip39` re-exports bip39 with `rand` feature; BDK handles randomness. **No need to add `rand` as a separate dep** unless we want direct `thread_rng()` access (we don't need it for the `from_entropy_in` API). |

## Verdict by release

| Release | Features blocked? | New crates needed | Plan handles? |
|---|---|---|---|
| v0.1 | none | none new (just `bitcoind` dev-dep) | ✅ |
| v0.2 | none | `zeroize`, `argon2`, `aes-gcm`, `libc` | ✅ Task 30 + 3 hygiene adds |
| v1.0 | none | same v0.2 + (no new crypto, just design) | ✅ ADR 0001 |

**No Rust SDK has a "must-have missing" feature for our use case.** All gaps are addressed by standard RustCrypto crates or by composition from existing APIs.

## Where to verify in Task 31 (BDK API spike)

The spike must validate these specific assumptions:

1. ✅ `bdk_wallet::keys::bip39::Mnemonic::generate(12)` works with the `keys-bip39` feature flag.
2. ✅ `Mnemonic::parse_in(Language::English, s)` returns the expected type.
3. ✅ `Mnemonic::to_seed(passphrase)` returns `[u8; 64]`.
4. ✅ `bip32::XPrv::derive_from_path(&seed, &DerivationPath::from_str("m")?)` returns `XPrv`.
5. ✅ `XPrv::derive_path(&path)` returns child `XPrv`.
6. ✅ `XPrv::to_string()` produces a string parseable as `wpkh(...)` descriptor.
7. ✅ `bdk_wallet::Wallet::create(descriptor, change_descriptor).network(...).create_wallet_no_persist()?` works.
8. ✅ `wallet.peek_address(KeychainKind::External, 0).address` returns a `bitcoin::Address`.
9. ✅ `wallet.balance()` returns `bdk_wallet::Balance`.

If any of these fail, the fix is one of: enable a different feature flag, use a different type path, or fall back to standalone `bip39` / `bip32` crates. None of these are blocking for v0.1 design.
