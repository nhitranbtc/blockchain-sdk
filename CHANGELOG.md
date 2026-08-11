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

- **`[user-facing]`** `btc wallet create --words <N> --network <NET> [--password <PWD>]` (Task 54d / Issue #64) — generate BIP-39 mnemonic + persist encrypted wallet to `$XDG_DATA_HOME/btc/wallets/<network>/<wallet_id>.enc` per ADR 0001. Prints `wallet_id` to STDOUT, mnemonic to STDERR (with banner). Windows in v0.1 surfaces a clear error rather than falling back to `~/btc/wallets`.
- **`[user-facing]`** `btc wallet show <ID> --network <NET> [--password <PWD>] [--esplora-url <URL>]` (Issue #64) — load encrypted wallet, decrypt with password + network AAD, sync from Esplora, print `{receive_addresses, change_addresses, balance_sat}` JSON to STDOUT.
- **`[user-facing]`** `WalletId(Uuid)` newtype (Issue #64, ADR 0001) — v4-only random UUIDs at the type boundary; `WalletId::new()` is the only public constructor (no `From<Uuid>`); `FromStr` rejects nil + non-v4 + non-RFC-4122 inputs. Compile-time v4 witness (L20 pattern) fails the build if `Uuid::new_v4`'s version discriminant ever drifts.
- **`[user-facing]`** `wallet::create_wallet` / `wallet::show_wallet` / `WalletInfo` library API (Issue #64) — testable from `bitcoin-wallet-core` independent of the CLI binary. Functions take an explicit `base: &Path` so unit tests use `tempfile::tempdir()`; production callers resolve `base` via `wallet::store::data_dir()`.
- **`[user-facing]`** `Wallet::sync(&EsploraClient)` (Task 9 #19b.2, F12) — full chain scan via Esplora `/address/{addr}/utxo` + `bdk_wallet::Wallet::insert_txout`. Caller builds `EsploraClient` with explicit `TlsPolicy` (F20 SPKI pinning). PR #55
- **`[user-facing]`** `Wallet::balance(&EsploraClient) -> Result<u64>` (Task 9 #19b.2, F13) — confirmed-only UTXO aggregation. Lazily syncs on first call; reuses cached `bdk_wallet::Wallet` thereafter. PR #55
- **`[user-facing]`** `Wallet::sync` / `Wallet::balance` API breaking change: now take `&EsploraClient` (was `&str esplora_url`). Caller must build `EsploraClient::from_config(&WalletConfig)` (which carries network + optional SPKI pin). PR #55
- **`[user-facing]`** `EsploraClient::address_utxos(&Address) -> Result<Vec<EsploraUtxo>>` + `EsploraClient::get_tx(&Txid) -> Result<bitcoin::Transaction>` — additive API used by `Wallet::sync` for F12 chain scan. PR #55

### Security

- **`[internal]`** F19 (`atomic_write`) defense for wallet blob persistence (Issue #64 / ADR 0001) — `tempfile::NamedTempFile` in same dir → `rename` over target + `fsync`; explicit `set_permissions(0o600)` on the final blob file; `ensure_secure_dir` walks the parent chain, refuses symlinks (TOCTOU window check), and sets `0o700` on every newly-created dir (closes umask-leak per F19).
- **`[internal]`** N2 file-existence oracle collapsed (Issue #64 / ADR 0001): `Error::WalletStore("wallet not accessible (wrong password, wrong network, or corrupt blob)")` is the single indistinguishable error for 4 distinct failure modes (missing file, wrong password, wrong network AAD, corrupt blob). The wallet-store layer owns the collapse; the underlying primitives surface technical detail and this layer translates.
- **`[internal]`** N8 timing oracle on missing-file path closed (Issue #64 / ADR 0001): `wallet::store::constant_time_padding()` runs a dummy Argon2id derive (~500ms) on the missing-file path so its wall-clock matches the wrong-password path.
- **`[internal]`** Symlink-DoS defense on read path (Issue #64 / ADR 0001): `read_wallet_at` calls `symlink_metadata` before `read` and refuses if the blob resolves to a symlink. Symlinks in the parent-dir chain are also refused at `create_dir_all` time (A2 mitigation).
- **`[internal]`** Cross-network footgun (N5) defended at the type layer + filesystem layer (Issue #64 / ADR 0001): `Aad::network(network)` binds the `bitcoin::Network` discriminant to the ciphertext; the `<network>/` directory layout provides defense-in-depth (CLI bug yields "no wallet found" not silent cross-network load).
- **`[internal]`** L28 fix (Issue #64 acceptance): mnemonic NEVER appears on STDOUT in the `btc wallet create` flow — routes to STDERR with a banner (`WARNING: This mnemonic is shown ONCE ...`). Regression test enforces this: `wallet::tests::create_writes_mnemonic_to_stderr_not_stdout`.
- **`[internal]`** `Error::WalletStore(String)` per F43 pattern (Issue #64 / ADR 0001) — distinct from generic `Storage` so callers can match wallet-persistence failures separately. The Display message is intentionally generic for failures that would otherwise leak whether a wallet exists (N2 oracle mitigation).
- **`[internal]`** `Wallet::peek_addresses(kind, count)` — `pub` accessor for the first `count` external/internal addresses; panics if the bdk wallet is not synced (caller contract). Used by `wallet::ops::show_wallet` to render the wallet's addresses after sync.
- **`[internal]`** F12 / F13: full implementation in `Wallet::sync` / `Wallet::balance`. F19 (`atomic_write`-backed persistence) deferred for UTXO state; encrypted mnemonic blob persistence lands in v0.1 via #54d per ADR 0001 (in-memory UTXO state until next `sync`).
- **`[internal]`** `XPrvHolder::to_xprv_secret() -> Secret<String>` (replaces `to_xprv_string`; `pub(crate)`; zeroize-on-drop) — closes xprv zeroize window in descriptor construction. PR #55
- **`[internal]`** `Error::Bdk` carries fixed message; raw bdk error dropped (avoids xprv leak via descriptor echo). PR #55
- **`[internal]`** `Wallet::sync` UTXO value capped against `Amount::MAX_MONEY`; reject on overflow (DoS mitigation against malicious Esplora response). PR #55
- **`[internal]`** `Wallet::sync` / `Wallet::balance` take `&EsploraClient` (no internal `TlsPolicy::SystemRoots` default); caller is responsible for `TlsPolicy::Pinned` for production endpoints. PR #55
- **`[internal]`** `crypto::aad::Aad<'a>` newtype + `MAX_AAD_LEN` (64-byte DoS cap) + exhaustive `Aad::network(Network)` encoding (Issue #66 precursor to ADR 0001) — typed AAD closes plaintext/AAD positional swap at call site; exhaustive match prevents silent on-disk blob remapping when `bitcoin::Network` gains a new variant (caught `Network::Testnet4` on first build). `# Errors` doc blocks on `aes_gcm::encrypt`/`decrypt`; error wraps drop `aes-gcm` internal format (oracle hygiene).
- **`[internal]`** `MnemonicCipherBlob` API: `encrypt_mnemonic` / `decrypt_mnemonic` gain required `aad: Aad<'_>` parameter (breaking); `new_checked` private constructor + `MAX_LEN` upper bound (DoS mitigation); rejects empty phrases; manual `Debug` using `finish_non_exhaustive()` (closes length-leak via `tracing::debug!(?blob)`); `from_bytes` constructor for `Aad` enforces length cap.
- **`[internal]`** ADR 0001 (`docs/superpowers/adrs/2026-08-11-adr-0001-btc-wallet-store.md`) — `btc` wallet-store layout decision: keep F19-deferred UTXO snapshot, persist only `MnemonicCipherBlob` at `$XDG_DATA_HOME/btc/wallets/<network>/<wallet_id>.enc` (XDG on Linux/macOS, Windows deferred). Network discriminant bound via AES-GCM AAD (closes cross-network footgun). Symlink-defense on read path; constant-time padding on missing-file path (closes file-existence + timing oracles). Unblocks #64 (Task 54d).
- **`[cleanup]`** `CONTEXT.md` deleted per audit 2026-08-10. Type-system invariants (`Secret<T>`, `bip39` `zeroize` feature, `finish_non_exhaustive()` for mnemonic types) carry the security load. PR #55

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
| 11 | [x] **Sync wallet (full chain scan)** | done (Task 9 #19b.2) | Fresh wallet: `cargo test -p bitcoin-wallet-core wallet::tests::sync_completes_against_testnet_for_fresh_wallet -- --ignored --test-threads=1` (requires live testnet Esplora). |
| 12 | [ ] **Get wallet balance** | partial (PR #52, Task 9c) | URL validation + `coin_type_for` only; UTXO aggregation deferred. Will land fully with #19c.2 follow-up. |
| 13 | [x] **Use btc CLI subcommand** | done (PR #68, Issue #64 / Task 54d) | `cargo run -p btc -- wallet create --words 12 --network testnet --password <pwd>` prints wallet_id to STDOUT + mnemonic to STDERR; `cargo run -p btc -- wallet show <id> --network testnet --password <pwd>` prints `{receive_addresses, change_addresses, balance_sat}` JSON |

**Progress:** 11 of 13 stories playaround-able. 2 still gated (#11 sync requires live testnet Esplora; #12 balance partial impl).

> **L25 maintenance:** After every PR merge, check if the merged PR completes any unchecked story → flip the box to `[x]` + update the "Try it" column if needed. Drift between docs and actual state is the failure mode this rule prevents (per L14).