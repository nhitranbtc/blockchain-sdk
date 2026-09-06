# Changelog — `tron-wallet-core`

All notable changes to `rust-wallet-app/crates/tron-wallet-core/`. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer 2.0.0](https://semver.org/).

Conventions: `Added` / `Changed` / `Deprecated` / `Removed` / `Fixed` / `Security`. Phase markers per [`docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`](../../../docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md).

---

## [Unreleased]

### Planned (v0.1.0 remaining)
- Phase 4 — TronBox Docker integration tests + Nile remote tests + CI workflows
- Phase 5 — 4-trait PAL + desktop/iOS/Android/test impls + `crypto::encrypt/decrypt` (Argon2id + AES-256-GCM)
- Phase 6 — `tron` CLI binary, 22 commands across 6 top-level trees
- Phase 7 — Spike V1–V11 verification + mainnet `$0.001 USDT` self-send gate (`RUN_TRON_MAINNET=1`)
- v0.1 release cut: branch `rust-tron-core` → `main`

---

## [v0.1.0-pre.3] — 2026-09-06 — Phase 3

Commit: `15f3559 feat(tron): Phase 3 — TRC-20 ABI + bundled token registry + DEM resource model + 5 Phase 2 carry-overs`

### Added
- TRC-20 ABI: `tx::builder::trc20_transfer` + `tx::builder::trc20_approve` wrapping `anychain_tron::trx::build_trc20_transfer_contract` / `build_trc20_approve_contract`. Default `fee_limit = 130_000_000` sun.
- View calls: `chain::TronGridClient::trigger_constant_contract(contract, selector, args)` against `/wallet/triggerconstantcontract`. Wire-format contract (per #410): server prepends 4-byte selector — client sends encoded args only.
- `trc20::balance_of` (selector `0x70a08231`), `trc20::decimals` (`0x313ce567`), `trc20::symbol` (`0x95d89b41`).
- `resource::estimate_energy` via `/wallet/triggerconstantcontract` with `wallet/estimateenergy` fallback. DEM `max_factor = 3.4×` per 6-hour cycle. USDT-TRC20 baseline 65k–130k Energy; default `fee_limit = 100_000_000` sun.
- Bundled token registry: `tokens/{local,nile,mainnet}.json` via `include_str!`. Nile USDT canonical address `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (deep-dive source of truth, overrides legacy user-stories value).
- mainnet tokens: USDT (`TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`, 6), USDC (`TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8`, 6), TUSD (`TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9`, 18), USDD (`TXDk8mbtRbXeYuMNS83CfKPaYYT8Xvi9Hz`, 18), stUSDT (`TThzxNRLrW2Brp9DcTQU8i4Wd9udCWEdZ3`, 6).
- Tests: `tests/v3_trc20_abi.rs` (calldata round-trip, 4-byte selectors), `tests/v5_resource.rs`, `tests/v9_token_registry.rs`.

### Fixed
- Task 2.6 — `TriggerSmartContract.data` proto field index = **4** (NOT 3), confirmed via anychain-tron vendored proto. Regression test in `v3_trc20_abi.rs`.

---

## [v0.1.0-pre.2] — 2026-09-06 — Phase 2

Commit: `63d3e6b feat(tron): Phase 2 — tx builder + sign + broadcast + SPKI pin (#538)`

### Added
- `tx::builder::trx_transfer` wrapping `anychain_tron::trx::build_transfer_contract`. Helpers: `set_ref_block`, `set_fee_limit`, `set_timestamp`, `set_expiration`.
- `tx::sign::sign_tx(sk_z, params) -> SignedTransaction`. Internally `params.to_bytes()` → `anychain_core::sha256` → `secp256k1_sign` (wrapped in `Zeroizing`) → `TronTransaction::sign`. `txid = SHA256(SHA256(raw_bytes))` per Q2.
- `chain::TronGridClient::new(rpc_url, spki_pin)` + `broadcast(&SignedTransaction)` via POST `/wallet/broadcasttransaction` (`{"raw_data_hex", "signature_hex"}`).
- `chain::TronGridClient::get_now_block` (TAPOS via `/walletsolidity/getnowblock`, fullnode NOT solidity) + `get_tx_info(txid)` via `/wallet/gettransactioninfobyid`.
- SPKI pin: `TronConfig::mainnet_default_spki_pin() -> [u8; 32]` = `0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d` (verified 2026-09-05). `TronConfig::for_network(Network::Mainnet)` populates `spki_pin: Some(...)`.
- Reuse `bitcoin_wallet_core::chain::spki::SpkiPinnedVerifier` for `pinned://<spki-hex>@host[:port]` URL scheme.
- Tests: `tests/v2_protobuf_roundtrip.rs`, `tests/v7_spki_pin.rs`.

---

## [v0.1.0-pre.1] — 2026-09-06 — Phase 1

Commit: `47a52d2 feat(tron): Phase 1 foundation — address + keys + signing (#536)`

### Added
- `keys::Mnemonic::generate(MnemonicType, Language) -> Self` (infallible), `from_phrase(&str, Language) -> Result<Self>`, `to_seed(&str) -> Zeroizing<[u8; 64]>` wrapping `anychain_kms::bip39::Mnemonic` + `Seed::new`. Hand-rolled `Debug` redaction.
- `keys::derive_keypair(&Mnemonic, &str passphrase, &DerivationPath) -> Result<KeyPair>` via `XprvSecp256k1::new_from_path`. `KeyPair { secret: Zeroizing<[u8; 32]>, public: TronPublicKey }`.
- `keys::xpub(&Mnemonic, &str passphrase, &DerivationPath) -> Result<String>` (SLIP-0132 xpub export).
- `address::Address::from_public_key`, `to_base58`, `to_hex`, `from_str` (accepts base58check, bare hex, `0x`-prefixed hex), `Address::is_valid(&str)` (associated function).
- `tx::sign::sign_hash(secret, msg32) -> Result<Signed>` wrapping `anychain_kms::secp256k1_sign`. Zeroizing wrap on sk. `RecoveryId` newtype rejects anything outside `0..=1` (NOT Ethereum's `v+27 ∈ {27, 28}`).
- Dual-SHA256 txid: `tx::sign::txid(raw_bytes) -> [u8; 32]` = `Sha256(Sha256(raw))`. Regression test `txid(b"") == 5df6e0e2…4c9456`.
- SLIP-44 vector (coin 195, path `m/44'/195'/0'/0/0`) using mnemonic `abandon ×11 about`. Two independent anchors: (a) `spikes/tron-v1` separate stack produces `TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH`; (b) coin 60 same mnemonic hashes to `0x9858EfFD232B4033E47d90003D41EC34EcaEda94` (repo-wide vector).
- Tests: `tests/v10_slip44.rs`, `tests/v8_sign_only.rs`, `tests/address.rs`.

### Fixed
- Zeroize hazard: `XprvSecp256k1::new_from_path(&*seed, …)` derefs `Zeroizing<[u8; 64]>` and passes by value, leaving unwiped copy. Resolved with `seed.as_slice()` (satisfies `needless_borrows_for_generic_args` without duplicating secret material). Same fix in `xpub.rs`.

---

## [v0.1.0-pre.0] — 2026-09-06 — Phase 0

Commit: `37d499d feat(tron): Phase 0 workspace setup — anychain pins + MSRV 1.98.1 + crate skeleton (#535)`

### Added
- `rust-wallet-app/rust-toolchain.toml` pinned to channel `1.98.1` (MSRV bump 1.85 → 1.98.1 for anychain workspace compatibility).
- Workspace deps at **exact** pins (NOT `^`): `anychain-core = "=0.1.8"`, `anychain-tron = "=0.2.14"`, `anychain-kms = "=0.1.23"`.
- `crates/tron-wallet-core` skeleton: `package.edition = "2021"`, `package.version = "0.1.0"`, `package.license = "MIT"`, `publish = false`, crate-type `["rlib", "cdylib"]` for FFI.
- dev-dependencies: `proptest`, `tempfile`. Optional `testcontainers = { version = "0.23" }` under `[dependencies]` (NOT `[dev-dependencies]` — Cargo rejects optional dev-deps). Feature flag `desktop-tests = ["dep:testcontainers"]`.
- `user-stories.md` regenerated: "Companion to" link fixed (raw-primitives → anychain); Stories 13/14/15/16 marked REMOVED from V0.1; Stories 8/18/23/24 marked deferred V0.2; Nile USDT address corrected; CLI command layout per V0.1 feature map.
- `docs/wallets/CONTEXT.md` extended with anychain-{core,tron,kms} terminology.

---

[Unreleased]: # compare HEAD
[v0.1.0-pre.3]: # 15f3559
[v0.1.0-pre.2]: # 63d3e6b
[v0.1.0-pre.1]: # 47a52d2
[v0.1.0-pre.0]: # 37d499d
