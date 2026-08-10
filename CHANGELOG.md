# Changelog

All notable changes to `rust-wallet-app` (Bitcoin wallet MVP).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- `chain::network::coin_type_for(Network) -> u32` — BIP-44 coin-type lookup per plan §Task 8 (F37). Hard rule #1 (no mainnet default) enforced at compile time via exhaustive match (PR #42).

### Changed

- `Signer::sign_recoverable` signature changed from `&[u8; 32]` to `&MessageHash<Bip137Message>` (F21 type-level defense, U5 phishing mitigation). Caller wrapping in `bip137::sign_message` is internal; public API unchanged (PR #39).

### Fixed

- CI advisory RUSTSEC-2025-0134 (rustls-pemfile unmaintained) suppressed in `deny.toml`; migration to `rustls-pki-types` deferred to separate backlog (PR #42).

### Security

- F20 (SPKI pinning for Esplora TLS) — Task 7 (PR #34).
- F21 (typed Sighash defense for `sign_recoverable`) — U5 mitigation at compile time (PR #39).
- F37 (BIP-44 coin-type derivation) — defense against caller-supplied-wrong-coin-type footgun (PR #42).

## [v0.1] — 2026-08-10

### Added

- Workspace scaffold (`bitcoin-wallet-core` library + `btc` CLI) — PR #11
- Threat model spec (F1–F53, A1–A8, U1–U7, T1–T4) — PR #10
- `Secret<T>` newtype (F47 zeroize wrapper) — PR #23
- `atomic_write` + 0o600 permissions (defends U6, U7) — PR #23
- 17-variant `Error` enum — PR #13
- `keys::mnemonic` BIP-39 (`Secret<bip39::Mnemonic>`, zeroize-on-drop) — PR #25
- `keys::derivation` + `keys::signer` (BIP-32 + secp256k1) — PR #26
- `crypto::argon2` KDF + `crypto::aes_gcm` encryption (F5, F6) — PR #27
- `crypto::bip137` message signing + verification (F7, F9, F50) — PR #33
- `chain::spki` SPKI pin primitives + `chain::esplora` SPKI-pinned TLS (F20) — PR #34
- L20 constant audit — compile-time pinned crypto constants across 13 sites — PR #38
- F21 typed Sighash defense (`MessageHash<C>` sealed trait + phantom-typed wrapper) — PR #39
- `chain::network::coin_type_for` (F37 BIP-44 coin type) — PR #42

### Test coverage

- 156 lib tests (incl. 5 `chain::network::tests::*` for `coin_type_for`)
- 1 doc test (`compile_fail` for F21 type-level barrier)

### Threat-model coverage

Defended: F1–F13, F19, F20, F21, F25, F26, F34, F37, F43, F44, F47, F48, F50, F53. Deferred: F25 (PSBT review), F48 (IsTerminal), F53 hardening — see plan §Deferred threats.