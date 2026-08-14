# Task → Rust SDK Map

**Date:** 2026-08-05 (drift fix 2026-08-14)
**Purpose:** Companion to `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`. Maps every task in the plan to the specific Rust crates it uses, the API surface, and the rationale.
**Use this when:** starting work on a task — you need to know which crates to import and which functions to call. The plan describes the WHAT; this doc describes the WITH-WHAT.

**Drift fix (2026-08-14):** API surface corrected to match actual implementation per PRs #27, #33, #34, #105, #114, #122, #123, #124, #125. Removed `EsploraExt` references (we use `EsploraClient` directly with `reqwest`, not `bdk_esplora` — see CONTEXT.md hard rule #2 + F20). Fixed `v0.2 adds` row (`argon2` + `aes-gcm` shipped in v0.1, PR #27). Added `Status as of 2026-08-14` per task. Task 33 replaced with L29 operator-driven gate. Status column uses ✅ Done / ⏸ Deferred / 🔄 In-progress / ❌ Replaced.

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

- **Status:** ✅ Done (PR #26, SHA in audit).
- **Crates:** `bip32 0.6` + `bdk_wallet::bitcoin::secp256k1` (re-export, no direct dep).
- **API surface:** `bip32::{XPrv, DerivationPath, DerivationPath::from_str}`, `XPrv::derive_from_path(&seed_bytes: AsRef<[u8]>, &path)` (takes seed bytes, NOT XPrv), `XPrv::derive_path(path)`, `XPrv::to_string()`; `bdk_wallet::bitcoin::secp256k1::{Secp256k1, Keypair, Message, SecretKey}`, `Keypair::from_secret_key(&secp, &sk)`, `secp.sign_ecdsa(&msg, &kp)`.
- **Rationale:** BDK does NOT re-export bip32; we keep it as a direct dep. `secp256k1` is reachable via `bdk_wallet::bitcoin::secp256k1` (re-exported from `bitcoin ^0.32`). Standalone `secp256k1 0.30` direct dep is NOT needed.

### Task 5: script::builder + script::parser
- **Crates:** `rust-bitcoin 0.32` (`bitcoin::script` + `bitcoin::opcodes` + `bitcoin::PublicKey`)
- **API surface:** `bitcoin::script::Builder`, `Builder::new().op_opcode(OP_DUP).push_slice(...).into_script()`, `Script::instructions()`, `bitcoin::PublicKey::pubkey_hash()` (NOT on `bitcoin::secp256k1::PublicKey`!), `wpubkey_hash()`, `XOnlyPublicKey::from(pk)`
- **Rationale:** `rust-bitcoin` is the reference Bitcoin script library. **Critical:** `.pubkey_hash()` and `.wpubkey_hash()` are methods on `bitcoin::PublicKey`, NOT on `bitcoin::secp256k1::PublicKey` (the secp256k1 type is the raw curve point, not a Bitcoin public key). Use `bitcoin::PublicKey::from(secp_pk).pubkey_hash()`.

### Task 6: address (legacy, segwit, taproot)

- **Status:** ✅ Done (PR #13 + F19 hardening).
- **Crates:** `rust-bitcoin 0.32`.
- **API surface:** `bitcoin::Address`, `Address::p2pkh(&pubkey: &PublicKey, network) -> Address` (legacy), `Address::p2wpkh(&pubkey: &PublicKey, network) -> Address` (segwit v0), `Address::p2wsh(&script, network) -> Address`, `Address::p2tr(&secp, xonly, None, network) -> Address` (taproot), `Address::parse::<Address<_>>(&str).require_network(network)` (cross-network rejection, F19), `bitcoin::WPubkeyHash`.
- **Rationale:** all address encoding is in `rust-bitcoin`. Network enum `bitcoin::Network`. F19 enforcement via `require_network` rejects "send to wrong chain" operator error.

### Task 7: chain::network + config
- **Crates:** none (just `std` + `serde`)
- **API surface:** `#[derive(Serialize, Deserialize)]` on `WalletConfig`, `std::path::PathBuf`
- **Rationale:** config-only.

### Task 8: chain::esplora + chain::electrum

- **Status:** ✅ Done (PR #34, SHA `a5df1ab` plus later F20 hardening). electrum path removed in implementation — we ship Esplora only.
- **Crates:** `reqwest 0.12` (rustls-tls) + `rustls 0.23` + `rustls-native-certs 0.7` + `webpki 0.22` + `sha2 0.10` + `x509-parser 0.16` (direct deps); `bdk_electrum` is declared in Cargo.toml but not wired. **`bdk_esplora` deliberately NOT used** (CONTEXT.md hard rule #2 — pulls in unpatched `rustls-webpki 0.101.7` per RUSTSEC-2026-0106).
- **API surface:** `EsploraClient::new(EsploraUrl, TlsPolicy)`, `EsploraClient::from_config(&WalletConfig)`, `EsploraClient::fee_estimate() -> Result<RawFeeEstimates>`, `EsploraClient::address_utxos(&Address) -> Result<Vec<EsploraUtxo>>`, `EsploraClient::get_tx(&Txid) -> Result<Transaction>`, `EsploraClient::broadcast_tx(&str) -> Result<Txid>`. **NO** `bdk_esplora::EsploraExt`.
- **Rationale:** custom SPKI-pinned `EsploraClient` enforces F20 (cert chain + SPKI pin check). Direct `reqwest` to dodge the unpatched `bdk_esplora` transitive.

### Task 9: wallet (from_mnemonic + sync + balance)

- **Status:** ✅ Done across PRs #48 (9a `from_mnemonic`), #51 (9b `sync` partial), #52 (9c `balance` partial), #122 (send compose). Full F12/F13 deferred features superseded by post-MVP stories.
- **Crates:** `bdk_wallet 3.1` (with `keys-bip39` feature) + `bip32 0.6` + `bdk_file_store 0.15` + custom `EsploraClient` (direct `reqwest`, **NOT** `bdk_esplora` per CONTEXT.md hard rule #2).
- **API surface:** `bdk_wallet::Wallet::create(descriptor, change_descriptor).network(network).create_wallet_no_persist()?`, `bdk_wallet::Wallet::create_single(...).create_wallet_no_persist()?`, `bdk_wallet::KeychainKind::External`, `wallet.peek_address(KeychainKind::External, index)`, `wallet.reveal_next_address(KeychainKind::External)`, `wallet.next_derivation_index(KeychainKind::External)`, `wallet.apply_update(update)`, `wallet.persist(&store)`, `wallet.balance()`. Sync path uses our `EsploraClient::address_utxos(&Address)` + `EsploraClient::get_tx(&Txid)` to populate `TxGraph` (per F12/F13). **NO** `bdk_esplora::EsploraExt::full_scan`.
- **Rationale:** central task. BDK's `Wallet` is the wallet shell. Our custom `EsploraClient` (F20 SPKI pin) is the only sanctioned HTTP path; direct `reqwest` dodges `bdk_esplora` RUSTSEC-2026-0106 transitive.

### Task 10: addresses (multi-address via xpub)
- **Crates:** `bdk_wallet 3.1`
- **API surface:** `wallet.peek_address(KeychainKind::External, index).address`, `wallet.reveal_next_address(KeychainKind::External)`, `wallet.next_derivation_index(KeychainKind::External)`
- **Rationale:** BDK handles the address index internally.

### Task 11: tx::builder

- **Status:** ✅ Done (PR #122, SHA `61fd0df`). Wrapped in lib module `bitcoin_wallet_core::tx::builder`.
- **Crates:** `bdk_wallet 3.1` (specifically `bdk_wallet::TxBuilder`) + `bdk_wallet::bitcoin` (re-exported).
- **API surface:** `tx::builder::build_send_tx(&mut BdkWallet, &Address, Amount, FeeRate) -> Result<Psbt>` (compose layer); underlying BDK primitives: `bdk.build_tx()`, `TxBuilder::add_recipient(script_pubkey, amount)`, `.fee_rate(rate)`, `.finish()`. Sanitize helper `sanitize_create_tx_error` maps `CreateTxError::CoinSelection(InsufficientFunds)` etc. to error strings.
- **Rationale:** BDK's TxBuilder is canonical UTXO selection + fee calculation + change generation. Our wrapper adds F19 (cross-network rejection) + F25 (signing path sanitization) + composes with `Wallet::send` (Story 5).

### Task 12: tx::psbt + tx::sighash
- **Crates:** `rust-bitcoin 0.32` (`bitcoin::Psbt`, `bitcoin::sighash::SighashCache`)
- **API surface:** `Psbt::serialize()`, `Psbt::deserialize()`, `base64::encode/decode`, `psbt.inputs[i].sighash`
- **Rationale:** PSBT serialization is in `rust-bitcoin`. Sighash extraction is in `rust-bitcoin::sighash::SighashCache`.

### Task 13: tx::sign + tx::broadcast

- **Status:** ✅ Done (PR #122, SHA `61fd0df`). Wrapped in lib modules `tx::sign` + `tx::broadcast`.
- **Crates:** `bdk_wallet 3.1` (sign) + direct `reqwest` via `EsploraClient::broadcast_tx` (NOT `bdk_esplora` per CONTEXT.md hard rule #2).
- **API surface:** `tx::sign::sign_psbt(&BdkWallet, &mut Psbt) -> Result<()>` with `SignOptions { trust_witness_utxo: true, ..Default::default() }`; `tx::sign::extract_tx(&Psbt) -> Result<Transaction>` (uses `psbt.clone().extract_tx()` to handle consume-self); `tx::broadcast::broadcast(&EsploraClient, &Transaction) -> Result<Txid>` (serializes via `consensus::encode::serialize_hex` + calls `EsploraClient::broadcast_tx`). **NO** `EsploraExt::broadcast`.
- **Rationale:** BDK's `sign` handles sighash + ECDSA + finalization. Direct Esplora POST avoids `bdk_esplora` RUSTSEC-2026-0106 transitive.

### Task 14: tx::fee + tx::bump_fee

- **Status:** 🔄 In-progress — `EsploraClient::fee_estimate()` shipped (PR #34, used by PR #124 `btc fee-estimates` CLI subcommand, SHA `d466795`). End-to-end `FeeEstimator` with target=6 default + fallback + cap deferred per issue #128; depends on #127 audit findings.
- **Crates:** direct `reqwest` via `EsploraClient` (NOT `bdk_esplora`) for fee; `bdk_wallet 3.1` for `build_fee_bump`.
- **API surface:** `EsploraClient::fee_estimate() -> Result<RawFeeEstimates>` (returns `HashMap<String, f64>` aliased as `RawFeeEstimates`); `bdk_wallet::bitcoin::FeeRate::from_sat_per_vb(rate) -> Option<FeeRate>`; CLI surface `btc fee-estimates [--network N] [--esplora-url URL] [--pin-spki HEX] [--json]` (PR #124); RBF `wallet.build_fee_bump(&txid).fee_rate(new_rate).finish()` deferred (no CLI surface yet).
- **Rationale:** fee estimates are HTTP-only. Our custom `EsploraClient` (F20 SPKI pin) is the only sanctioned HTTP path. BDK's `build_fee_bump` is canonical RBF once #128 ships.

### Task 15: regtest integration test

- **Status:** 🔄 In-progress (PR #114, SHA `a5df1ab`). `btc-regtest-smoke` testcontainers suite shipped (2/3 pass, 1 ignored). Bollard follow-up (custom Docker network) deferred per issue #115.
- **Crates (dev-dep of `btc` crate):** `testcontainers 0.23` (blocking) + `bitcoind 0.36` (feature `0_21_2`) + `reqwest 0.12` (blocking + json + rustls-tls). **`bitcoind-async-client` NOT used** — replaced by direct `reqwest` JSON-RPC.
- **API surface:** `testcontainers::Container::new(image, cmd)` (boot ephemeral bitcoind 0.21+); `reqwest::blocking::Client` for JSON-RPC (`createwallet`, `generatetoaddress`, `scantxoutset`); `bitcoind::exe_path()` only as fallback.
- **Rationale:** `testcontainers` orchestrates ephemeral containers; direct `reqwest` JSON-RPC avoids `bitcoind-async-client` (unmaintained). Bitcoin Core 0.21+ requires explicit wallet creation OR `scantxoutset` — no default wallet.

### Task 16: btc CLI scaffold + wallet/address/balance/sync commands

- **Status:** ✅ Done (PR #11 scaffold; PRs #48, #51, #52, #99, #101, #118 wallet/show/sync/balance/import/send subcommands).
- **Crates:** `clap 4` (with `derive` feature) + `anyhow 1` + `tracing 0.1` + `tracing-subscriber 0.3` + `bitcoin-wallet-core 0.1` (workspace path).
- **API surface:** `clap::{Parser, Subcommand}`, `#[derive(Parser)]`, `#[derive(Subcommand)]`, `tracing_subscriber::fmt().with_env_filter(...).with_writer(std::io::stderr).init()` (STDERR routing per L28/F49), `anyhow::Result`. Manual `Debug` impls redact mnemonic per L17 (CRITICAL #2 pattern).
- **Rationale:** `clap` derive is standard. `anyhow` is CLI top-level only. STDERR routing keeps STDOUT scriptable (wallet_id on `create`, JSON on `show`).

### Task 17: btc send + tx + fee + config commands

- **Status:** ✅ Done across PRs #105, #122, #123, #124, #125.
  - `btc config show [--json]` — PR #105, SHA `4de4ea7`
  - `btc wallet send` — PR #122, SHA `61fd0df`
  - `btc wallet send --fee-rate` — PR #123, SHA `b5e3074`
  - `btc fee-estimates [--json]` — PR #124, SHA `d466795`
  - `btc tx-list [--json] [--limit N]` — PR #125, SHA `7fe7bc7`
- **Crates:** same as Task 16 + `serde_json 1` (for `--json` output) + `tokio 1` (async runtime).
- **API surface:** `serde_json::json!()`, `serde_json::to_string_pretty(&record)`, `tokio::main`, `tokio::time::sleep`, `tokio::fs::read_to_string`.
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

- **Status:** ⏸ Deferred. No `docker/Dockerfile` in repo. CI does not produce a container image; release-time Dockerfile TBD per release prep.
- **Crates:** none (Dockerfile + size check).
- **API surface:** `docker build -t btc:dev -f docker/Dockerfile .` (when Dockerfile lands), `RUSTFLAGS="-C opt-level=z -C lto=fat -C strip=symbols -C panic=abort" cargo build --release -p btc`.
- **Rationale:** pure build / packaging.

### Task 22: README
- **Crates:** none (markdown only)
- **API surface:** markdown
- **Rationale:** docs.

### Task 23: CONTRIBUTING + CHANGELOG + SECURITY

- **Status:** 🔄 Partial — `CHANGELOG.md` ✅ Done (Keep a Changelog format, L24 cascade per PR). `CONTRIBUTING.md` ✅ Done (commit + PR workflow + ledger rules). `SECURITY.md` ⏸ Missing — threat-model disclosure policy TBD per release prep.
- **Crates:** none.
- **API surface:** markdown.
- **Rationale:** docs.

### Task 24: CI workflows
- **Crates:** none
- **API surface:** GitHub Actions YAML
- **Rationale:** CI configuration.

### Task 25: v0.1.1 release

- **Status:** ⏸ Deferred (v0.1.0 tag never cut; we are at v0.1.1 release candidate). Pending: #128 fee estimator impl + #126 L29 live testnet gates.
- **Crates:** none.
- **API surface:** `git tag v0.1.1`, `git push origin main --tags`, `cargo publish -p bitcoin-wallet-core`, `cargo login <token>`.
- **Rationale:** release process. v0.1.1 supersedes the original v0.1.0 plan due to 4 post-MVP stories (5/6/7/8) added inline.

### Task 26: tx::dust

- **Status:** ⏸ Deferred. Surface flagged by #127 fee model audit (issue opened 2026-08-14). Implementation order: #127 audit → #128 estimator → dust checks → re-defer or schedule for v0.2.
- **Crates:** `rust-bitcoin 0.32` (specifically `bitcoin::ScriptBuf`).
- **API surface:** `ScriptBuf::len()`, threshold calculation per script type.
- **Rationale:** no `dust` crate in Rust ecosystem; we implement the Bitcoin Core `CFeeRate` dust rule (3 * minRelayFee per vbyte).

### Task 27: chain::explorer

- **Status:** ✅ Done (PR #125, SHA `7fe7bc7`).
- **Crates:** `rust-bitcoin 0.32` (`bitcoin::Network`, `bitcoin::Txid`, `bitcoin::Address`).
- **API surface:** `chain::explorer::tx_url(base: &str, txid: Txid) -> String`, `chain::explorer::address_url(base: &str, address: &Address) -> String`. 3 lib unit tests cover path append + trailing-slash preservation.
- **Rationale:** pure `String` formatting. Blockstream + mempool.space are just websites.

### Task 28: tx::sign_external (Signer trait)

- **Status:** ⏸ Deferred (Phase 2 per F33/F35; UniFFI hook for iOS Swift host).
- **Crates:** `secp256k1 0.30` (specifically `secp256k1::ecdsa::Signature`).
- **API surface:** `trait Signer { fn sign_ecdsa(&self, hash: &[u8; 32]) -> Result<ecdsa::Signature>; fn public_key(&self) -> bdk_wallet::bitcoin::secp256k1::PublicKey; }`.
- **Rationale:** Phase 2 UniFFI hook. Trait returns the same `secp256k1::ecdsa::Signature` type that BDK's signing produces. iOS Swift wraps `TangemSdk` in this trait.

### Task 29: btc CLI bump-fee + sign-message

- **Status:** 🔄 In-progress — `btc message sign/verify` ✅ Done (PR #33, SHA in audit); RBF `btc wallet bump-fee` ⏸ Deferred (depends on #128 fee estimator surface).
- **Crates:** same as Tasks 16/17 + `rust-bitcoin 0.32::hashes::sha256` (re-exported, no new dep) for BIP-137.
- **API surface:** `wallet.build_fee_bump(&txid)` (RBF path), `bdk_wallet::bitcoin::hashes::sha256::Hash::engine()` (BIP-137), `bdk_wallet::bitcoin::secp256k1::secp.sign_ecdsa(&msg, &kp)`.
- **Rationale:** BIP-137 = SHA256(SHA256("\x19Bitcoin Signed Message:\n" || varint(len) || message)). The hash primitives are already in `rust-bitcoin::hashes`.

### Task 30: keys::encrypted_mnemonic (Argon2id + AES-256-GCM)

- **Status:** ✅ Done (PR #27, SHA in audit). Both crates shipped in v0.1, NOT v0.2 as originally scheduled.
- **Crates:** `argon2 0.5` + `aes-gcm 0.10` + `zeroize 1` + `rand 0.8` (for the salt/nonce).
- **API surface:** `Argon2::new(Algorithm::Argon2id, Version::V0x13, params)`, `argon.hash_password_into(passphrase, salt, &mut key)`, `Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key))`, `cipher.encrypt(nonce, plaintext)`, `cipher.decrypt(nonce, payload)`, `Zeroizing::zeroize(&mut key)`.
- **Rationale:** all from RustCrypto, all audited. m=256MiB/t=10/p=4 calibrated to 500ms wall-clock on first run. AES-256-GCM is AEAD.

### Task 31: BDK 3.1 API spike
- **Crates:** `bdk_wallet 3.1` (with `keys-bip39` feature) + `bdk_chain 3.1` + `bdk_esplora 0.22` + `bdk_file_store 0.15` + `bitcoin 0.32`
- **API surface:** `Wallet::new_single(descriptor)`, `wallet.peek_address`, `wallet.reveal_next_address`, `wallet.persist(&store)`, `Store::open_or_create_new("name", b"magic")`
- **Rationale:** throwaway spike. Validates the 9 specific assumptions listed in `docs/wallets/2026-08-05-feature-sdks-support.md`.

### Task 32: wallet::xpub_watch_only

- **Status:** ⏸ Deferred (v0.2; not in MVP scope per F33/F35).
- **Crates:** `bdk_wallet 3.1` + `bdk_file_store 0.15` (no new crate).
- **API surface:** `Wallet::new_single(descriptor)`, public-only descriptor (no xprv in input), `KeychainKind::External`, `wallet.peek_address`, `wallet.reveal_next_address`.
- **Rationale:** BDK handles watch-only natively — public-only descriptors don't need signing infrastructure.

### Task 33: Live testnet gates (operator-driven, replaces original CI testnet plan)

- **Status:** ❌ Replaced. Original CI-gated plan abandoned per L29 (live testnet is operator-driven, not CI). Live testnet prep umbrella tracked in issue #126.
- **Crates:** none (manual run scripts + testnet-faucet-funded wallet).
- **API surface:** `btc wallet create`, `btc wallet show`, `btc wallet send`, `btc fee-estimates`, `btc tx-list` (kebab-case per clap subcommand conflict resolution, PR #125). All against `--network testnet` + `https://blockstream.info/testnet/api` + F20 SPKI pin.
- **Rationale:** exercise the real CLI against real testnet. Operator runs `scripts/btc-testnet-gate.sh` (TBD per #126); CI cannot fund a real wallet or pay real fees.

## Summary table — Rust crates per release

| Release | Production deps | Dev-deps |
|---|---|---|
| **v0.1** | bdk_wallet 3.1 (keys-bip39) + bdk_chain 3.1 + bdk_electrum 0.21 (declared, not wired) + bdk_file_store 0.15 + rust-bitcoin 0.32 + bip32 0.6 + bip39 (via bdk_wallet re-export) + clap 4 + tokio 1 + reqwest 0.12 (rustls-tls) + thiserror 1 + tracing 0.1 + tracing-subscriber 0.3 + serde 1 + serde_json 1 + anyhow 1 + base64 0.22 + zeroize 1 + **argon2 0.5 + aes-gcm 0.10 + rand 0.8 + rustls 0.23 + rustls-native-certs 0.7 + webpki 0.22 + sha2 0.10 + x509-parser 0.16 + subtle 0.6 + rpassword 7 + uuid 1 + directories 5 + tempfile 3** | bitcoind 0.36 (feature `0_21_2`) + testcontainers 0.23 (blocking) + reqwest 0.12 (blocking + json) + tempfile 3 + assert_cmd 2 + predicates 3 + proptest 1 |
| **v0.2** (adds) | libc 0.2 | — |
| **v1.0** (mobile adds) | (no new Rust deps; iOS side has Swift packages) | — |

## Crates that were considered and rejected (for reference)

- `miniscript` — in workspace deps as future-proofing for multi-sig (v1.0+). Not used in v0.1.
- `rand` 0.8 — WAS rejected as direct dep in original plan; **NOW a direct dep** in v0.1 (used by Argon2id salt/nonce + Aes256Gcm nonce, see Task 30).
- `wagyu` / `wazir-cash` — niche PSBT libs; `rust-bitcoin::Psbt` is standard.
- `rust-lightning` — tied to Lightning, not Bitcoin core.
- `age` — single-author; `aes-gcm` (RustCrypto) is standard.
- `bdk_bitcoind_rpc` — 2024-era, unmaintained; use `bitcoind` crate directly.
- `bitcoin-savings` — community fork; `rust-bitcoin` is canonical.
- `bdk_esplora` — **deliberately not used** in v0.1 (CONTEXT.md hard rule #2; pulls in unpatched `rustls-webpki 0.101.7` per RUSTSEC-2026-0106). We use direct `reqwest` via our `EsploraClient` instead.
- `bitcoind-async-client` — unmaintained; replaced by direct `reqwest` JSON-RPC in testcontainers regtest smoke (Task 15).

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
