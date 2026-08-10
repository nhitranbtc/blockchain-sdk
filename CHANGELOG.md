# Changelog

All notable changes to `rust-wallet-app` (Bitcoin wallet MVP).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **`[user-facing]`** `chain::network::coin_type_for(Network) -> u32` — BIP-44 coin-type lookup per plan §Task 8 (F37). Hard rule #1 (no mainnet default) enforced at compile time via exhaustive match (PR #42).

### Changed

- **`[internal]`** `Signer::sign_recoverable` signature changed from `&[u8; 32]` to `&MessageHash<Bip137Message>` (F21 type-level defense, U5 phishing mitigation). Caller wrapping in `bip137::sign_message` is internal; public API unchanged (PR #39).

### Fixed

- **`[internal]`** CI advisory RUSTSEC-2025-0134 (rustls-pemfile unmaintained) suppressed in `deny.toml`; migration to `rustls-pki-types` deferred to separate backlog (PR #42).

### Security

- **`[user-facing]`** F20 (SPKI pinning for Esplora TLS) — Task 7 (PR #34).
- **`[user-facing]`** F21 (typed Sighash defense for `sign_recoverable`) — U5 mitigation at compile time (PR #39).
- **`[user-facing]`** F37 (BIP-44 coin-type derivation) — defense against caller-supplied-wrong-coin-type footgun (PR #42).

## [v0.1] — 2026-08-10

### Added

- **`[user-facing]`** Workspace scaffold (`bitcoin-wallet-core` library + `btc` CLI) — PR #11
- **`[internal]`** Threat model spec (F1–F53, A1–A8, U1–U7, T1–T4) — PR #10
- **`[internal]`** `Secret<T>` newtype (F47 zeroize wrapper) — PR #23
- **`[internal]`** `atomic_write` + 0o600 permissions (defends U6, U7) — PR #23
- **`[internal]`** 17-variant `Error` enum — PR #13
- **`[user-facing]`** `keys::mnemonic` BIP-39 (`Secret<bip39::Mnemonic>`, zeroize-on-drop) — PR #25
- **`[user-facing]`** `keys::derivation` + `keys::signer` (BIP-32 + secp256k1) — PR #26
- **`[user-facing]`** `crypto::argon2` KDF + `crypto::aes_gcm` encryption (F5, F6) — PR #27
- **`[user-facing]`** `crypto::bip137` message signing + verification (F7, F9, F50) — PR #33
- **`[user-facing]`** `chain::spki` SPKI pin primitives + `chain::esplora` SPKI-pinned TLS (F20) — PR #34
- **`[internal]`** L20 constant audit — compile-time pinned crypto constants across 13 sites — PR #38
- **`[internal]`** F21 typed Sighash defense (`MessageHash<C>` sealed trait + phantom-typed wrapper) — PR #39
- **`[user-facing]`** `chain::network::coin_type_for` (F37 BIP-44 coin type) — PR #42

### Test coverage

- **`[internal]`** 156 lib tests (incl. 5 `chain::network::tests::*` for `coin_type_for`)
- **`[internal]`** 1 doc test (`compile_fail` for F21 type-level barrier)

### Threat-model coverage

- **`[internal]`** Defended: F1–F13, F19, F20, F21, F25, F26, F34, F37, F43, F44, F47, F48, F50, F53. Deferred: F25 (PSBT review), F48 (IsTerminal), F53 hardening — see plan §Deferred threats.

> **Tag legend:** `[user-facing]` items are visible to clients / external users; `[internal]` items are engineering work invisible to users but important for audit trail. Per L24.