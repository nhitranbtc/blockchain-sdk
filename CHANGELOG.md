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

- **`[user-facing]`** `btc fee-estimates [--network N] [--esplora-url URL] [--pin-spki HEX64] [--json]` CLI subcommand (Story 8 / Issue #121 / Task 14 `get_fee_estimates` portion) — read-only Esplora fee estimator. No wallet required (uses the `bitcoin_wallet_core::chain::esplora::EsploraClient::fee_estimate()` HTTP client directly). Pretty table by default (`target_blocks | sat/vB`, sorted ascending); `--json` for machine-readable output. Reuses `default_url_for` + `parse_spki_pin_hex` + `build_esplora_client_for`-style client construction (F20 SPKI pin + F36 HTTPS-only enforcement). Output includes `network` label; empty maps surface `(no estimates)` (defensive against Esplora incidents). 3 CLI parse tests (accept + `--json` flag + reject missing network). Closes v0.1.1 Story 8.
- **`[user-facing]`** `btc wallet send --fee-rate <SAT_PER_VB>` CLI flag (Story 6 / Issue #119 / Task 14) — user-specified sat/vB fee rate. Validates `>= 1` (0 = no fee, txs don't relay). Default (`None`): 1 sat/vB (conservative; Story 8 #121 will fetch Esplora estimates when omitted). `Wallet::send` signature gains `fee_rate: FeeRate` parameter — callers must construct explicitly (no internal default). Threat-model: invalid fee rate → `anyhow!` error before any network IO. 2 new CLI parse tests + 1 debug-redaction test.
- **`[user-facing]`** `btc wallet send --mnemonic <words> --network <NET> --address <ADDR> --amount-sat <SAT> --esplora-url <URL> [--pin-spki <hex64>]` CLI subcommand (Issue #118 / Story 5 / Task 11+13) — full tx lifecycle: sync → build → sign → broadcast → return txid. Default fee rate 1 sat/vB (Story 6 #119 added `--fee-rate` override). Threat-model: cross-network rejection enforced via `Address::require_network` (defends against "send to wrong chain" operator error); F36 HTTPS-only Esplora URL reuses `EsploraUrl::new` validator (Issue #63); F20 SPKI pin enforcement reuses `build_esplora_client_for` (non-regtest requires pin); L12 CRITICAL #2 mnemonic redaction in Debug (mirrors `Sync`/`Balance` pattern). 4 lib unit tests + 3 CLI integration tests + 4 demo-script gates cover accept/reject/cross-network/invalid-address/F36. Closes Phase 1 §MVP follow-up gap.
- **`[internal]`** `bitcoin-wallet-core::tx` module — three single-purpose units wrapping `bdk_wallet::TxBuilder` + `wallet.sign` + Esplora broadcast: `tx::builder::build_send_tx(&mut BdkWallet, &Address, Amount, FeeRate) -> Result<Psbt>`, `tx::sign::sign_psbt(&BdkWallet, &mut Psbt) + extract_tx(&Psbt) -> Transaction`, `tx::broadcast::broadcast(&EsploraClient, &Transaction) -> Result<Txid>`. Sanitized error mapping (bdk `CreateTxError` variant names preserved without descriptor echo per F25 / U1). 3 lib unit tests pin the public API.
- **`[internal]`** `EsploraClient::broadcast_tx(&str) -> Result<Txid>` — POST raw tx hex to Esplora `/tx` endpoint. F20 enforcement carried via the underlying `reqwest::Client` (no separate TLS check). Sanitized error: surfaces HTTP status + first 200 chars of body on failure.
- **`[internal]`** `Wallet::send(&EsploraClient, &Address, Amount, FeeRate) -> Result<Txid>` (Story 6 / Issue #119 signature update) — composes `tx::{builder,sign,broadcast}` after lazy sync. `MutexGuard` discipline: bdk wallet is `take()`-en out for the build step (which needs `&mut`), put back BEFORE the async broadcast (no guard crosses `.await`). Story 6 replaced the internal `DEFAULT_FEE_RATE_SAT_PER_VB` constant with caller-provided `FeeRate` — CLI default lives in `handle_wallet_send`.
- **`[internal]`** `scripts/btc-send-demo.sh` — operator-friendly demo for `btc wallet send` covering 4 gates (CLI flag surface, cross-network rejection, invalid-address rejection, F36 HTTPS-only). Mirrors the gate pattern of `btc-balance-demo.sh` / `btc-import-demo.sh`.

- **`[user-facing]`** `btc wallet import --mnemonic "..." --network <NET> --password <PW>` CLI subcommand (Issue #99 / Story 2) — Phase 1 closure gap closure. Imports an existing BIP-39 mnemonic (12/15/18/21/24 words), encrypts via F5/F6 (Argon2id + AES-256-GCM), persists to `$XDG_DATA_HOME/btc/wallets/<network>/<id>.enc` per ADR 0001, prints `wallet_id` on STDOUT. Optional `--passphrase` accepted (BIP-39 derivation-time only — NOT persisted, re-supply at `wallet show` time). Threat-model: invalid checksum / unsupported word count → exit non-zero with `invalid mnemonic` error; mnemonic never echoed on STDOUT or STDERR (L28 regression coverage); Manual `Debug` redacts `mnemonic` + `passphrase` + `password` (L12 CRITICAL #2); encryption AAD bound to network discriminant (cross-network decrypt fails). 5 lib unit tests + 6 CLI integration tests cover accept/reject/determinism/non-echo/distinct-IDs/persistence. Closes Phase 1 §MVP scope Story 2 gap.
- **`[user-facing]`** `btc encrypt` / `btc decrypt` accept `--password-file <PATH>` and `--password-stdin` (Issue #84) — closes the scripted-automation gap from PR #62 backlog. Use cases: k8s secrets mounted as files, systemd `LoadCredential=`, vault-agent sidecar pipes, CI pipelines. Priority order: `--password` flag > `--password-file` > `--password-stdin` > `BTC_ENCRYPT_PASSWORD` / `BTC_DECRYPT_PASSWORD` env > `/dev/tty` prompt. Mutually exclusive at parse layer via clap `conflicts_with_all`. Threat-model: `--password-file` rejects symlinks (F19 pattern) and world/group-readable files (`mode & 0o077 != 0`) — defense-in-depth so a misconfigured k8s mount can't leak the secret via `/proc/<pid>/cmdline` chain. Manual `Debug` redacts both `password` and `password_file` paths (L12 CRITICAL #2). 19 new unit tests cover parse + F20-style mode check + symlink check + priority order + end-to-end roundtrip via `--password-file`.

- **`[user-facing]`** `btc wallet create` + `btc wallet show` CLI subcommands (Task 54d, PR-2 of #64) — clap 4 subcommands wired on top of the PR-1 lib (`create_wallet` + `show_wallet`). `create` persists an encrypted wallet to `$XDG_DATA_HOME/btc/wallets/<network>/<id>.enc` and prints `wallet_id` to STDOUT + mnemonic to STDERR (L28/F49 closure — mnemonic never on STDOUT). `show` loads + decrypts + syncs from Esplora + prints `{receive_addresses, change_addresses, balance_sat}` JSON. Manual `Debug` impls on `Cli`/`Commands`/`WalletAction` redact password (L12 CRITICAL #2). Threat-model coverage reuses PR-1 defenses: F19, U6, U7, A1, A2, N2, N5, N8 — all carried through. PR #70
- **`[user-facing]`** `btc wallet show --esplora-spki-pin <HEX64>` (Issue #73, F20 enforcement) — new flag + `BTC_ESPLORA_SPKI_PIN` env to expose SPKI pin configuration for production Esplora endpoints. When set, CLI builds `EsploraClient` via `from_config` with `TlsPolicy::Pinned` (closes F20 gap on mainnet/signet/regtest). When unset, PR-2 `SystemRoots` default preserved (testnet-suitable). PR #73
- **`[user-facing]`** `btc message sign` + `btc message verify` subcommands (Issue #61 / Task 54a) — stateless BIP-137 wrapper around `crypto::bip137`. `sign` derives the first external address (`m/44'/coin'/0'/0/0`) from a BIP-39 mnemonic + network, requires the operator's `--address` to match (closes F47 mnemonic-mismatch), prints base64 signature. `verify` prints `true`/`false` for a base64 signature against an address + message. Manual `Debug` redacts mnemonic + signature (L12 CRITICAL #2 pattern). v0.1 supports legacy P2PKH only (BIP-322 / Taproot / P2WPKH deferred to v0.1.1). PR #61
- **`[user-facing]`** `btc wallet show --network <NET>` defaults to network-appropriate Esplora URL (Issue #74) — `bitcoin`→`blockstream.info/api`, `testnet`→`blockstream.info/testnet/api`, `signet`→`blockstream.info/signet/api`, `testnet4`→`mempool.space/testnet4/api`. `regtest` has no default (HTTPS-only per F20; operator must pass `--esplora-url`). PR #74
- **`[user-facing]`** `btc encrypt` + `btc decrypt` subcommands (Issue #62 / Task 54b) — stateless file encryption wrapping `crypto::mnemonic_cipher` (F5 Argon2id KDF m=256 MiB/t=10/p=4 + F6 AES-256-GCM AEAD). Reads UTF-8 `--in`, writes `MnemonicCipherBlob` (salt(16) || nonce(12) || ct || tag(16)) to `--out`; decrypt reverses. `--password` optional (env `BTC_ENCRYPT_PASSWORD`/`BTC_DECRYPT_PASSWORD` or `/dev/tty` prompt via `rpassword`). Threat-model: F47 zeroize preserved via `Secret<String>` borrow (no clone into non-zeroizing String); F19 atomic_write (`util::atomic_write`) for `--out` — 0o600 perms, write-to-temp + fsync + parent fsync + rename, no partial ciphertext/plaintext on crash; N2 oracle collapsed — wrong-password / tampered / truncated / non-UTF8 all surface as uniform `decrypt failed`; `--in == --out` refused; `--in` symlinks + oversize (`> 1 MiB` encrypt, `> MAX_LEN` decrypt) rejected pre-read. PR #62
- **`[user-facing]`** `Wallet::sync(&EsploraClient)` (Task 9 #19b.2, F12) — full chain scan via Esplora `/address/{addr}/utxo` + `bdk_wallet::Wallet::insert_txout`. Caller builds `EsploraClient` with explicit `TlsPolicy` (F20 SPKI pinning). PR #55
- **`[user-facing]`** `Wallet::balance(&EsploraClient) -> Result<u64>` (Task 9 #19b.2, F13) — confirmed-only UTXO aggregation. Lazily syncs on first call; reuses cached `bdk_wallet::Wallet` thereafter. PR #55
- **`[user-facing]`** `Wallet::sync` / `Wallet::balance` API breaking change: now take `&EsploraClient` (was `&str esplora_url`). Caller must build `EsploraClient::from_config(&WalletConfig)` (which carries network + optional SPKI pin). PR #55
- **`[user-facing]`** `EsploraClient::address_utxos(&Address) -> Result<Vec<EsploraUtxo>>` + `EsploraClient::get_tx(&Txid) -> Result<bitcoin::Transaction>` — additive API used by `Wallet::sync` for F12 chain scan. PR #55

### Security

- **`[internal]`** F20 enforcement tightened in `btc wallet show` (push-sweep #2): non-regtest networks (Bitcoin, Testnet, Testnet4, Signet) now refuse to construct an EsploraClient without an explicit `--pin-spki` (or `BTC_ESPLORA_SPKI_PIN` env). Regtest retains the operator-opt-in exemption (localhost development via stunnel + SystemRoots). Closes active mainnet attack surface where `default_url_for(Network::Bitcoin) → blockstream.info/api` would route without a pin. Mirrors the F20 gate already shipped in `btc wallet sync` / `btc wallet balance` (PR #81). 5 new unit tests pin the contract across all 4 non-regtest networks + regtest exemption.
- **`[internal]`** GitHub Actions SHA-pinning (push-sweep #1): `actions/checkout`, `dtolnay/rust-toolchain`, and `Swatinem/rust-cache` are now pinned to full commit SHAs in both `ci.yml` and `btc-cli-demo.yml`. Dependabot (already configured at `.github/dependabot.yml` for `github-actions` ecosystem, weekly cadence) will auto-bump SHAs via future PRs. Closes HIGH supply-chain-pin-mismatch finding (tag rewrite attack vector).
- **`[internal]`** F12 / F13: full implementation in `Wallet::sync` / `Wallet::balance`. F19 (`atomic_write`-backed persistence) deferred for UTXO state; encrypted mnemonic blob persistence lands in v0.1 via #54d per ADR 0001 (in-memory UTXO state until next `sync`).
- **`[internal]`** `XPrvHolder::to_xprv_secret() -> Secret<String>` (replaces `to_xprv_string`; `pub(crate)`; zeroize-on-drop) — closes xprv zeroize window in descriptor construction. PR #55
- **`[internal]`** `Error::Bdk` carries fixed message; raw bdk error dropped (avoids xprv leak via descriptor echo). PR #55
- **`[internal]`** `Wallet::sync` UTXO value capped against `Amount::MAX_MONEY`; reject on overflow (DoS mitigation against malicious Esplora response). PR #55
- **`[internal]`** `Wallet::sync` / `Wallet::balance` take `&EsploraClient` (no internal `TlsPolicy::SystemRoots` default); caller is responsible for `TlsPolicy::Pinned` for production endpoints. PR #55
- **`[internal]`** `crypto::aad::Aad<'a>` newtype + `MAX_AAD_LEN` (64-byte DoS cap) + exhaustive `Aad::network(Network)` encoding (Issue #66 precursor to ADR 0001) — typed AAD closes plaintext/AAD positional swap at call site; exhaustive match prevents silent on-disk blob remapping when `bitcoin::Network` gains a new variant (caught `Network::Testnet4` on first build). `# Errors` doc blocks on `aes_gcm::encrypt`/`decrypt`; error wraps drop `aes-gcm` internal format (oracle hygiene).
- **`[internal]`** `MnemonicCipherBlob` API: `encrypt_mnemonic` / `decrypt_mnemonic` gain required `aad: Aad<'_>` parameter (breaking); `new_checked` private constructor + `MAX_LEN` upper bound (DoS mitigation); rejects empty phrases; manual `Debug` using `finish_non_exhaustive()` (closes length-leak via `tracing::debug!(?blob)`); `from_bytes` constructor for `Aad` enforces length cap. PR #28 + #66
- **`[internal]`** `btc encrypt`/`btc decrypt` hardened per L12 critical-tier review (PR #62 fix commit): F47 zeroize regression closed (`Secret<String>` borrow, no `.clone()`); `std::fs::write` → `util::atomic_write` (F19, closes non-atomic + 0o600 umask + symlink-following-write); N2 oracle collapsed (uniform `decrypt failed` for wrong-password / tampered / truncated / non-UTF8); `--in == --out` guard added; `--in` size cap (`1 MiB` encrypt, `MAX_LEN` decrypt) + symlink rejection pre-read. PR #62
- **`[internal]`** ADR 0001 (`docs/superpowers/adrs/2026-08-11-adr-0001-btc-wallet-store.md`) — `btc` wallet-store layout decision: keep F19-deferred UTXO snapshot, persist only `MnemonicCipherBlob` at `$XDG_DATA_HOME/btc/wallets/<network>/<wallet_id>.enc` (XDG on Linux/macOS, Windows deferred). Network discriminant bound via AES-GCM AAD (closes cross-network footgun). Symlink-defense on read path; constant-time padding on missing-file path (closes file-existence + timing oracles). Unblocks #64 (Task 54d).
- **`[cleanup]`** `CONTEXT.md` deleted per audit 2026-08-10. Type-system invariants (`Secret<T>`, `bip39` `zeroize` feature, `finish_non_exhaustive()` for mnemonic types) carry the security load. PR #55

### Infrastructure (Phase 1 closure)

- **`[internal]`** GitHub Actions CI workflow (`rust-wallet-app/.github/workflows/ci.yml`) — 4-job gate mirroring L13 step 11 verify: `fmt` (rustfmt check), `clippy` (`-D warnings`), `test` (`cargo test --workspace`, L29 live-testnet `#[ignore]` preserved), `geiger` (unsafe audit). SHA-pinned `actions/checkout@v4`; cargo target dir at `rust-wallet-app/target`. Triggers on push to `main` + PR to `main`. Closes Task 1 Step 10 (final unchecked plan deliverable).
- **`[internal]`** Plan-file drift sync (`docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`) — all 51 plan checkboxes flipped from `[ ]` to `[x]` per L30 drift fix, except Task 1 Step 10 (closes in this PR). Workspace version drift documented inline: workspace resolved to `0.2.0` after `chain-traits` umbrella scaffold was added as a workspace member predating the F33/F35 deferral decision. Resolution: accept `0.2.0`, Phase 2 plan formally adopts `chain-traits`.
- **`[internal]`** Verification gate cleared (L28): `cargo fmt --check` exit 0, `cargo clippy --workspace --all-targets -- -D warnings` exit 0, `cargo test --workspace` 89 passed / 0 failed / 1 ignored (L29 live testnet), `cargo geiger` 0 unsafe fn/impl/trait in `bitcoin-wallet-core` (6 unsafe expressions, all `Secret::into_inner` pattern per F53).

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
| 7 | [x] Compile-time-pinned crypto constants (L20 audit) | done (PR #38) | `cargo test -p bitcoin-wallet-core crypto` (compile-time `const _: ()` blocks fail the build on out-of-range literals) |
| 8 | [x] Refuse mainnet default (CONTEXT.md hard rule #1) | done (PR #42; `bitcoin::Network` has no `Default` impl, so callers must explicitly choose) | see `rust-wallet-app/CONTEXT.md` hard rule #1 |
| 9 | [x] Refuse transaction sighash as message (F21 type-level) | done (PR #39) | `cargo test -p bitcoin-wallet-core --doc threat::MessageHash` |
| 10 | [x] **Create wallet from mnemonic** | done (PR #48, Task 9a) | `cargo test -p bitcoin-wallet-core wallet` |
| 11 | [x] **Sync wallet (full chain scan)** | done (Task 9 #19b.2 + CLI wrapper PR for #63) | Fresh wallet: `cargo test -p bitcoin-wallet-core wallet::tests::sync_completes_against_testnet_for_fresh_wallet -- --ignored --test-threads=1` (requires live testnet Esplora). CLI: `cargo run -p btc -- wallet sync --mnemonic "<12-word phrase>" --network testnet --esplora-url https://blockstream.info/testnet/api` (add `--pin-spki <hex64>` for F20 enforcement). |
| 12 | [x] **Get wallet balance** | done (Task 9 #19c + CLI wrapper PR for #63) | `cargo test -p bitcoin-wallet-core wallet::tests::balance_returns_zero_for_fresh_wallet -- --ignored --test-threads=1` (live testnet). CLI: `cargo run -p btc -- wallet balance --mnemonic "<12-word phrase>" --network testnet --esplora-url https://blockstream.info/testnet/api`. |
| 13 | [x] **Use btc CLI subcommand** | done (PR #70, Task 54d; extended by PR #61, PR #62, PR #73, PR #74, PR for #63) | `cargo run -p btc -- wallet create --words 12 --network testnet` then `cargo run -p btc -- wallet show <wallet_id> --network testnet`; `cargo run -p btc -- message sign --mnemonic "<12-word phrase>" --network testnet --address <ADDR> "x"`; `cargo run -p btc -- encrypt --password hunter2 --in /tmp/plain.txt --out /tmp/cipher.enc` then `btc decrypt --password hunter2 --in /tmp/cipher.enc --out /tmp/recovered.txt`; `cargo run -p btc -- wallet balance --mnemonic "<12-word phrase>" --network testnet --esplora-url https://blockstream.info/testnet/api` |

**Progress:** 13 of 13 stories playaround-able.

> **L25 maintenance:** After every PR merge, check if the merged PR completes any unchecked story → flip the box to `[x]` + update the "Try it" column if needed. Drift between docs and actual state is the failure mode this rule prevents (per L14).