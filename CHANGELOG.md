# Changelog

All notable changes to `rust-wallet-app` (Bitcoin wallet MVP).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) with two extensions:

- `[user-facing]` / `[internal]` tags distinguish client-visible changes from engineering work
- User Stories section at the bottom shows what's playaround-able today

> **Tag legend:** `[user-facing]` items are visible to clients / external users (new public API, new behavior, new threat-model surface). `[internal]` items are engineering work invisible to users (refactor, audit, defense-in-depth that doesn't change public API).

[Unreleased]: https://github.com/nhitranbtc/blockchain-sdk/compare/v0.1.0...HEAD
[v0.1.0]: https://github.com/nhitranbtc/blockchain-sdk/releases/tag/v0.1.0

## [Unreleased]

### Added

- **`[user-facing]`** `Wallet::sync(esplora_url)` (Task 9b, F12) — partial: URL validation + `coin_type_for` derivation + descriptor path. Full `start_full_scan` deferred (requires xprv expansion). PR #51
- **`[user-facing]`** `Wallet::balance(esplora_url) -> Result<u64>` (Task 9c, F13) — partial: URL validation + `coin_type_for`. Full UTXO aggregation deferred (requires `bdk_wallet::Wallet` construction). PR #52

### Security

- **`[internal]`** F13 / F14 (balance consistency / persistence) — defense-in-depth tripwires in the partial sync/balance impls return honest "partial impl" errors. Full UTXO-sum + `bdk_file_store` deferred.

## [v0.1.0] — 2026-08-10

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
- **`[user-facing]`** `chain::network::coin_type_for` (F37 BIP-44 coin type) — PR #42

### Changed

- **`[internal]`** `Signer::sign_recoverable` signature changed from `&[u8; 32]` to `&MessageHash<Bip137Message>` (F21 type-level defense, U5 phishing mitigation). Caller wrapping in `bip137::sign_message` is internal; public API unchanged — PR #39

### Security

- **`[internal]`** F20 (SPKI pinning for Esplora TLS) — Task 7 — PR #34
- **`[internal]`** F21 (typed Sighash defense for `sign_recoverable`) — U5 mitigation at compile time, sealed trait + phantom-typed wrapper; public API unchanged — PR #39
- **`[internal]`** F37 (BIP-44 coin-type derivation) — defense against caller-supplied-wrong-coin-type footgun; public API addition via `coin_type_for` — PR #42
- **`[internal]`** L20 constant audit — compile-time pinned crypto constants across 13 sites — PR #38
- **`[internal]`** CI advisory RUSTSEC-2025-0134 (rustls-pemfile unmaintained) suppressed in `deny.toml`; migration to `rustls-pki-types` deferred to separate backlog — PR #42

### Test coverage

- **`[internal]`** 156 lib tests (incl. 5 `chain::network::tests::*` for `coin_type_for`)
- **`[internal]`** 1 doc test (`compile_fail` for F21 type-level barrier)
- **`[internal]`** Coverage by module: `keys::mnemonic` (mnemonic gen, phrase roundtrip, zeroize), `keys::derivation` (BIP-32 path derivation, hardened/non-hardened), `keys::signer` (sign/verify roundtrip, deterministic per RFC 6979), `crypto::argon2` (key derivation, salt length validation), `crypto::aes_gcm` (roundtrip, password-based end-to-end), `crypto::bip137` (sign/verify, header byte matrix, base64 edge cases, cross-tool interop), `chain::spki` (pin validation), `chain::esplora` (TLS verifier), `chain::network` (BIP-44 coin types, exhaustive match, default = testnet)

### Threat-model coverage

- **`[internal]`** Defended: F1–F13, F19, F20, F21, F25, F26, F34, F37, F43, F44, F47, F48, F50, F53. Deferred: F25 (PSBT review), F48 (IsTerminal), F53 hardening — see plan §Deferred threats.

---

## User Stories

Each story is a user-facing capability. Once checked, the feature is **playaround-able** via the listed test / library call / CLI command. Per L25: flip the checkbox when the merged PR completes the story.

| # | Story | Status | "Try it" |
|---|---|---|---|
| 1 | [x] Generate BIP-39 mnemonic (12/15/18/21/24 words) | done (PR #25) | `cargo test -p bitcoin-wallet-core keys::mnemonic` |
| 2 | [x] Derive child keys via BIP-32 | done (PR #26) | `cargo test -p bitcoin-wallet-core keys::derivation` |
| 3 | [x] Sign + verify messages (BIP-137, Bitcoin Core + Trezor interop) | done (PR #33) | `cargo test -p bitcoin-wallet-core crypto::bip137` |
| 4 | [x] Encrypt with password (Argon2id KDF + AES-256-GCM) | done (PR #27) | `cargo test -p bitcoin-wallet-core crypto::aes_gcm` |
| 5 | [x] Connect to Esplora server (SPKI-pinned TLS, F20) | done (PR #34) | `cargo test -p bitcoin-wallet-core chain::esplora` |
| 6 | [x] Get BIP-44 coin type per network (F37) | done (PR #42) | `cargo test -p bitcoin-wallet-core chain::network` |
| 7 | [x] Compile-time-pinned crypto constants (L20 audit) | done (PR #38) | see `docs/audit/2026-08-09-l20-constant-audit.md` |
| 8 | [x] Refuse mainnet default (CONTEXT.md hard rule #1) | done (PR #42; `bitcoin::Network` has no `Default` impl, so callers must explicitly choose) | see `rust-wallet-app/CONTEXT.md` hard rule #1 |
| 9 | [x] Refuse transaction sighash as message (F21 type-level) | done (PR #39) | `cargo test -p bitcoin-wallet-core --doc threat::MessageHash` |
| 10 | [x] **Create wallet from mnemonic** | done (PR #48, Task 9a) | `cargo test -p bitcoin-wallet-core wallet` |
| 11 | [ ] **Sync wallet (full chain scan)** | partial (PR #51, Task 9b) | URL validation + `coin_type_for` only; `Wallet::start_full_scan` deferred. Will land fully with #19b.2 follow-up. |
| 12 | [ ] **Get wallet balance** | partial (PR #52, Task 9c) | URL validation + `coin_type_for` only; UTXO aggregation deferred. Will land fully with #19c.2 follow-up. |
| 13 | [ ] **Use btc CLI subcommand** | gated (Task 9 + CLI subcommands) | `cargo run -p btc --help` (currently placeholder) |

**Progress:** 10 of 13 stories playaround-able. 3 still gated (#11 sync, #12 balance partial impl; #13 CLI subcommand full work).

> **L25 maintenance:** After every PR merge, check if the merged PR completes any unchecked story → flip the box to `[x]` + update the "Try it" column if needed. Drift between docs and actual state is the failure mode this rule prevents (per L14).