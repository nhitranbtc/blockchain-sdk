# Task → Rust SDK Map

**Date:** 2026-08-05
**Purpose:** Companion to `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`. Maps every task in the plan to the specific Rust crates it uses, the API surface, and the rationale.
**Use this when:** starting work on a task — you need to know which crates to import and which functions to call. The plan describes the WHAT; this doc describes the WITH-WHAT.

## How to read this map

- **"Crates" column:** the production dependencies the task imports.
- **"API surface" column:** the specific functions / types / traits the task calls.
- **"Rationale" column:** why this crate (vs an alternative), if non-obvious.

## Task-by-task map

### Task 1: Workspace + CI scaffold
- **Crates:** none (just workspace + `rust-toolchain.toml` + CI yaml)
- **API surface:** `cargo new`, `cargo build`
- **Rationale:** scaffolding only; no Rust code yet. The `bdk_wallet = { version = "3.1", features = ["keys-bip39"] }` is declared in workspace Cargo.toml here.

### Task 2: Error enum (thiserror)
- **Crates:** `thiserror 1`
- **API surface:** `#[derive(Debug, thiserror::Error)]`, `#[error("...")]` attribute, `#[from] bitcoin::consensus::encode::Error`, `#[from] bdk_wallet::Error`
- **Rationale:** standard for typed error enums in Rust.

### Task 3: keys::mnemonic (BIP-39)
- **Crates:** `bdk_wallet::keys::bip39` (re-export behind `keys-bip39` feature) — no standalone `bip39` dep
- **API surface:** `bdk_wallet::keys::bip39::{Language, Mnemonic, MnemonicType}`, `Mnemonic::generate(12)`, `Mnemonic::parse_in(Language::English, s)`, `Mnemonic::from_entropy_in`, `Mnemonic::to_seed(passphrase)`
- **Rationale:** BDK re-exports the same `bip39` crate. Use the re-export to drop 1 direct dependency. **BDK does NOT have a `Wallet::from_mnemonic` shortcut** — the flow is always Mnemonic → xprv → descriptor string → `Wallet::create`.

### Task 4: keys::derivation + keys::signer
- **Crates:** `bip32 0.6` + `bdk_wallet::bitcoin::secp256k1` (re-export, no direct dep)
- **API surface:** `bip32::{XPrv, DerivationPath, DerivationPath::from_str}`, `XPrv::derive_from_path(seed, path)`, `XPrv::derive_path(path)`, `XPrv::to_string()`; `bdk_wallet::bitcoin::secp256k1::{Secp256k1, Keypair, Message, SecretKey}`, `Keypair::from_secret_key(&secp, &sk)`, `secp.sign_ecdsa(&msg, &kp)`
- **Rationale:** BDK does NOT re-export bip32; we keep it as a direct dep. `secp256k1` is reachable via `bdk_wallet::bitcoin::secp256k1` (re-exported from `bitcoin ^0.32`). Standalone `secp256k1 0.30` direct dep is NOT needed.

### Task 5: script::builder + script::parser
- **Crates:** `rust-bitcoin 0.32` (`bitcoin::script` + `bitcoin::opcodes` + `bitcoin::PublicKey`)
- **API surface:** `bitcoin::script::Builder`, `Builder::new().op_opcode(OP_DUP).push_slice(...).into_script()`, `Script::instructions()`, `bitcoin::PublicKey::pubkey_hash()` (NOT on `bitcoin::secp256k1::PublicKey`!), `wpubkey_hash()`, `XOnlyPublicKey::from(pk)`
- **Rationale:** `rust-bitcoin` is the reference Bitcoin script library. **Critical:** `.pubkey_hash()` and `.wpubkey_hash()` are methods on `bitcoin::PublicKey`, NOT on `bitcoin::secp256k1::PublicKey` (the secp256k1 type is the raw curve point, not a Bitcoin public key). Use `bitcoin::PublicKey::from(secp_pk).pubkey_hash()`.

### Task 6: address (legacy, segwit, taproot)
- **Crates:** `rust-bitcoin 0.32`
- **API surface:** `bitcoin::Address`, `Address::p2pkh(&payload, network)`, `Address::p2wpkh(&payload, network)`, `Address::p2tr(&secp, xonly, None, network)`, `bitcoin::WPubkeyHash`
- **Rationale:** all address encoding is in `rust-bitcoin`. Network enum `bitcoin::Network`.

### Task 7: chain::network + config
- **Crates:** none (just `std` + `serde`)
- **API surface:** `#[derive(Serialize, Deserialize)]` on `WalletConfig`, `std::path::PathBuf`
- **Rationale:** config-only.

### Task 8: chain::esplora + chain::electrum
- **Crates:** `bdk_esplora 0.22` + `bdk_electrum 0.24` + `reqwest 0.12` (transitive)
- **API surface:** `bdk_esplora::esplora_client::Builder::new(url).build_blocking()`, `EsploraExt::full_scan`, `EsploraExt::broadcast`, `EsploraExt::get_fee_estimates`
- **Rationale:** `bdk_esplora` is the official Esplora client.

### Task 9: wallet (from_mnemonic + sync + balance)
- **Crates:** `bdk_wallet 3.1` (with `keys-bip39` feature) + `bip32 0.6` + `bdk_esplora 0.22` + `bdk_file_store 0.15`
- **API surface:** `bdk_wallet::Wallet::create(descriptor, change_descriptor).network(network).create_wallet_no_persist()?`, `bdk_wallet::Wallet::create_single(...).create_wallet_no_persist()?`, `bdk_wallet::KeychainKind::External`, `wallet.peek_address(KeychainKind::External, index)`, `wallet.reveal_next_address(KeychainKind::External)`, `wallet.next_derivation_index(KeychainKind::External)`, `wallet.apply_update(update)`, `wallet.persist()`, `wallet.balance()`, `bdk_esplora::esplora_client::Builder::new(url).build_blocking()`, `bdk_esplora::EsploraExt::full_scan`
- **Rationale:** central task. BDK's `Wallet` is the wallet shell. Our 4-crate flow ends here.

### Task 10: addresses (multi-address via xpub)
- **Crates:** `bdk_wallet 3.1`
- **API surface:** `wallet.peek_address(KeychainKind::External, index).address`, `wallet.reveal_next_address(KeychainKind::External)`, `wallet.next_derivation_index(KeychainKind::External)`
- **Rationale:** BDK handles the address index internally.

### Task 11: tx::builder
- **Crates:** `bdk_wallet 3.1` (specifically `bdk_wallet::TxBuilder`)
- **API surface:** `wallet.build_tx()`, `bdk_wallet::bitcoin::Psbt`, `bdk_wallet::TxBuilder::add_recipient(script, amount)`, `.fee_rate(rate)`, `.drain_to(addr)`, `.finish()`
- **Rationale:** BDK's TxBuilder is the canonical UTXO selection + fee calculation + change generation.

### Task 12: tx::psbt + tx::sighash
- **Crates:** `rust-bitcoin 0.32` (`bitcoin::Psbt`, `bitcoin::sighash::SighashCache`)
- **API surface:** `Psbt::serialize()`, `Psbt::deserialize()`, `base64::encode/decode`, `psbt.inputs[i].sighash`
- **Rationale:** PSBT serialization is in `rust-bitcoin`. Sighash extraction is in `rust-bitcoin::sighash::SighashCache`.

### Task 13: tx::sign + tx::broadcast
- **Crates:** `bdk_wallet 3.1` (sign) + `bdk_esplora 0.22` (broadcast)
- **API surface:** `wallet.sign(&mut psbt, SignOptions::default())`, `psbt.extract_tx()`, `EsploraExt::broadcast(&tx)`
- **Rationale:** BDK's `sign` handles sighash + ECDSA + finalization.

### Task 14: tx::fee + tx::bump_fee
- **Crates:** `bdk_esplora 0.22` (fee) + `bdk_wallet 3.1` (bump_fee)
- **API surface:** `EsploraClient::get_fee_estimates().await` (returns `HashMap<String, f64>`), `bdk_wallet::bitcoin::FeeRate::from_sat_per_vb(rate)`, `wallet.build_fee_bump(txid).fee_rate(new_rate).finish()`
- **Rationale:** fee estimates are HTTP-only; BDK's `build_fee_bump` is the canonical RBF implementation.

### Task 15: regtest integration test
- **Crates (dev-dep only):** `bitcoind 0.36` + `bitcoind-async-client 0.36` + `tempfile 3`
- **API surface:** `bitcoind::BitcoinD::new(bitcoin_data_dir, exe_path)`, `client.create_wallet("name", ...)`, `client.generate_to_address(101, &addr)`, `client.send_to_address(&addr, amount)`, `bitcoind::exe_path()`
- **Rationale:** `bitcoind` crate spawns + controls a real `bitcoind` binary for regtest.

### Task 16: btc CLI scaffold + wallet/address/balance/sync commands
- **Crates:** `clap 4` (with `derive` feature) + `anyhow 1` + `tracing 0.1` + `tracing-subscriber 0.3` + `bitcoin-wallet-core 0.1` (workspace path)
- **API surface:** `clap::{Parser, Subcommand}`, `#[derive(Parser)]`, `#[derive(Subcommand)]`, `tracing_subscriber::fmt::init()`, `anyhow::Result`
- **Rationale:** `clap` derive is standard. `anyhow` is CLI top-level only.

### Task 17: btc send + tx + fee + config commands
- **Crates:** same as Task 16 + `serde_json 1` (for `--json` output) + `tokio 1` (async runtime)
- **API surface:** `serde_json::json!()`, `serde_json::to_string_pretty(&record)`, `tokio::main`, `tokio::time::sleep`, `tokio::fs::read_to_string`
- **Rationale:** `serde_json` is standard. `tokio` is standard async runtime.

### Task 18: btc end-to-end CLI test
- **Crates (dev-dep only):** `assert_cmd 2` + `predicates 3`
- **API surface:** `assert_cmd::Command::cargo_bin("btc").unwrap()`, `cmd.arg("--help")`, `cmd.assert().success().stdout(predicate::str::contains("..."))`
- **Rationale:** `assert_cmd` is standard for testing Rust binaries.

### Task 19: proptest + miri
- **Crates (dev-dep only):** `proptest 1` + `cargo +nightly miri` (compiler flag)
- **API surface:** `proptest::proptest!` macro, `proptest::prelude::*` (`prop_assert!`, `prop_assert_eq!`), `cargo +nightly miri test`
- **Rationale:** `proptest` is the standard property-based testing crate.

### Task 20: cargo-deny + cargo-fuzz
- **Crates:** `cargo-deny` (tool) + `cargo-fuzz` (tool) + `libfuzzer-sys` (only inside fuzz crate)
- **API surface:** `cargo deny check`, `cargo fuzz init`, `cargo fuzz run script_parser`, `fuzz_target!(|data: &[u8]| {...})`
- **Rationale:** `cargo-deny` enforces license/advisory/duplicate-deps policy. `cargo-fuzz` is standard libfuzzer wrapper.

### Task 21: Docker + size audit
- **Crates:** none (Dockerfile + size check)
- **API surface:** `docker build -t btc:dev -f docker/Dockerfile .`, `RUSTFLAGS="-C opt-level=z -C lto=fat -C strip=symbols -C panic=abort" cargo build --release -p btc`
- **Rationale:** pure build / packaging.

### Task 22: README
- **Crates:** none (markdown only)
- **API surface:** markdown
- **Rationale:** docs.

### Task 23: CONTRIBUTING + CHANGELOG + SECURITY
- **Crates:** none
- **API surface:** markdown
- **Rationale:** docs.

### Task 24: CI workflows
- **Crates:** none
- **API surface:** GitHub Actions YAML
- **Rationale:** CI configuration.

### Task 25: v0.1.0 release
- **Crates:** none
- **API surface:** `git tag v0.1.0`, `git push origin main --tags`, `cargo publish -p bitcoin-wallet-core`, `cargo login <token>`
- **Rationale:** release process.

### Task 26: tx::dust
- **Crates:** `rust-bitcoin 0.32` (specifically `bitcoin::ScriptBuf`)
- **API surface:** `ScriptBuf::len()`, threshold calculation per script type
- **Rationale:** no `dust` crate in Rust ecosystem; we implement the Bitcoin Core `CFeeRate` dust rule (3 * minRelayFee per vbyte).

### Task 27: chain::explorer
- **Crates:** `rust-bitcoin 0.32` (`bitcoin::Network`, `bitcoin::Txid`, `bitcoin::Address`)
- **API surface:** `format!("{base}/tx/{txid}", ...)`, `format!("{base}/address/{addr}", ...)`
- **Rationale:** pure `String` formatting. Blockstream + mempool.space are just websites.

### Task 28: tx::sign_external (Signer trait)
- **Crates:** `secp256k1 0.30` (specifically `secp256k1::ecdsa::Signature`)
- **API surface:** `trait Signer { fn sign_ecdsa(&self, hash: &[u8; 32]) -> Result<ecdsa::Signature>; fn public_key(&self) -> bdk_wallet::bitcoin::secp256k1::PublicKey; }`
- **Rationale:** this is the Phase 2 UniFFI hook. Trait returns the same `secp256k1::ecdsa::Signature` type that BDK's signing produces. iOS Swift wraps `TangemSdk` in this trait.

### Task 29: btc CLI bump-fee + sign-message
- **Crates:** same as Tasks 16/17 + `rust-bitcoin 0.32::hashes::sha256` (re-exported, no new dep) for BIP-137
- **API surface:** `wallet.build_fee_bump(&txid)`, `bdk_wallet::bitcoin::hashes::sha256::Hash::engine()`, `bdk_wallet::bitcoin::secp256k1::secp.sign_ecdsa(&msg, &kp)`
- **Rationale:** BIP-137 = SHA256(SHA256("\x19Bitcoin Signed Message:\n" || varint(len) || message)). The hash primitives are already in `rust-bitcoin::hashes`.

### Task 30: keys::encrypted_mnemonic (Argon2id + AES-256-GCM)
- **Crates:** `argon2 0.5` + `aes-gcm 0.10` + `zeroize 1` + `rand 0.8` (for the salt/nonce)
- **API surface:** `Argon2::new(Algorithm::Argon2id, Version::V0x13, params)`, `argon.hash_password_into(passphrase, salt, &mut key)`, `Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key))`, `cipher.encrypt(nonce, plaintext)`, `cipher.decrypt(nonce, payload)`, `Zeroizing::zeroize(&mut key)`
- **Rationale:** all from RustCrypto, all audited. m=256MiB/t=10/p=4 calibrated to 500ms wall-clock on first run. AES-256-GCM is AEAD.

### Task 31: BDK 3.1 API spike
- **Crates:** `bdk_wallet 3.1` (with `keys-bip39` feature) + `bdk_chain 3.1` + `bdk_esplora 0.22` + `bdk_file_store 0.15` + `bitcoin 0.32`
- **API surface:** `Wallet::new_single(descriptor)`, `wallet.peek_address`, `wallet.reveal_next_address`, `wallet.persist(&store)`, `Store::open_or_create_new("name", b"magic")`
- **Rationale:** throwaway spike. Validates the 9 specific assumptions listed in `docs/wallets/2026-08-05-feature-sdks-support.md`.

### Task 32: wallet::xpub_watch_only
- **Crates:** `bdk_wallet 3.1` + `bdk_file_store 0.15` (no new crate)
- **API surface:** `Wallet::new_single(descriptor)`, public-only descriptor (no xprv in input), `KeychainKind::External`, `wallet.peek_address`, `wallet.reveal_next_address`
- **Rationale:** BDK handles watch-only natively — public-only descriptors don't need signing infrastructure.

### Task 33: CI testnet integration test
- **Crates:** none (CI yaml only — uses the `btc` binary built in earlier jobs)
- **API surface:** `btc wallet create`, `btc sync`, `btc balance`, `btc send`, `btc tx list`
- **Rationale:** exercise the real CLI against real testnet.

## Summary table — Rust crates per release

| Release | Production deps | Dev-deps |
|---|---|---|
| **v0.1** | bdk_wallet 3.1 (keys-bip39) + rust-bitcoin 0.32 + bip32 0.6 + clap 4 + tokio 1 + reqwest 0.12 + thiserror 1 + tracing 0.1 + tracing-subscriber 0.3 + serde 1 + serde_json 1 + anyhow 1 + base64 0.22 + zeroize 1 | bitcoind 0.36 + bitcoind-async-client 0.36 + tempfile 3 + assert_cmd 2 + predicates 3 + proptest 1 |
| **v0.2** (adds) | argon2 0.5 + aes-gcm 0.10 + libc 0.2 | — |
| **v1.0** (mobile adds) | (no new Rust deps; iOS side has Swift packages) | — |

## Crates that were considered and rejected (for reference)

- `miniscript` — in workspace deps as future-proofing for multi-sig (v1.0+). Not used in v0.1.
- `rand` 0.8 — not a direct dep; BDK's `bip39` re-export handles randomness via the `rand` feature.
- `wagyu` / `wazir-cash` — niche PSBT libs; `rust-bitcoin::Psbt` is standard.
- `rust-lightning` — tied to Lightning, not Bitcoin core.
- `age` — single-author; `aes-gcm` (RustCrypto) is standard.
- `bdk_bitcoind_rpc` — 2024-era, unmaintained; use `bitcoind` crate directly.
- `bitcoin-savings` — community fork; `rust-bitcoin` is canonical.

## How to use this map

When starting a task:
1. Read the task's "Crates" row — know what to import.
2. Read the "API surface" row — know the specific function names to call.
3. If the API is unclear, cross-reference the BDK 3.1 docs.rs page for the function and run the Task 31 spike to validate.
4. If the API is fundamentally different from what this map says, **update this map** in the same commit as the task implementation. The map is the source of truth for "which crate does what".

## See also

- `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` — the plan itself, with task bodies.
- `docs/wallets/2026-08-05-bitcoin-rust-sdks-deep-dive.md` — why these 4 crates and not others.
- `docs/wallets/2026-08-05-feature-sdks-support.md` — feature × crate coverage matrix.
- `docs/wallets/2026-08-05-tangem-vs-btc-wallet-comparison.md` — per-feature SDK column.
