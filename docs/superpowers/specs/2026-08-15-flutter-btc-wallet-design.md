# Flutter Desktop UI for `btc` Bitcoin Wallet — Design

**Date:** 2026-08-15
**Status:** Draft (pending review)
**Scope:** Phase 1.5 of `BlockchainSdk` Rust rewrite. Flutter desktop UI that wraps the existing `btc` CLI binary. Not a new wallet engine; not a new signing model.
**Source plan (Rust):** [`../plans/2026-08-05-rust-bitcoin-wallet.md`](../plans/2026-08-05-rust-bitcoin-wallet.md)
**Source user stories:** [`../../wallets/2026-08-05-btc-wallet-user-stories.md`](../../wallets/2026-08-05-btc-wallet-user-stories.md)
**Source umbrella spec:** [`2026-08-06-rust-wallet-app-architecture.md`](2026-08-06-rust-wallet-app-architecture.md)

---

## 1. Goal

A cross-platform Flutter desktop UI for the `btc` Bitcoin wallet CLI. The UI is a thin skin over the existing CLI binary — it does not reimplement wallet logic, signing, or chain sync. Every user-facing action in the UI corresponds to one `btc` subcommand invocation.

**Out of scope (deferred):**

- Direct FFI / UniFFI integration (Phase 3 per ADR 0001)
- `rust-wallet-server` REST daemon (deferred v2 per Bitcoin spec §4)
- Mobile (iOS / Android) — Phase 2
- Hardware wallet integration (Phase 3)
- Stories not listed in §3 below (multi-recipient, drain, RBF, coin selection, manual UTXO, BIP-137, encrypt, descriptor export)

---

## 2. Architecture

### 2.1 Repository location

New top-level directory `flutter-btc-wallet/`, sibling to `rust-wallet-app/`, `docs/`, `tasks/`. Sibling (not nested in `rust-wallet-app/`) because mixing Cargo + Flutter toolchains in one workspace is awkward for `cargo metadata` resolution. Top-level (not separate repo) because release coupling wants one tag per version.

### 2.2 Project layout

```text
flutter-btc-wallet/                    (new top-level dir)
├── pubspec.yaml
├── lib/
│   ├── main.dart                       entry, ProviderScope, MaterialApp.router
│   ├── core/
│   │   ├── btc/                        btc process layer
│   │   │   ├── btc_invoker.dart        spawn `btc` per command
│   │   │   ├── btc_command.dart        typed command enum
│   │   │   ├── password_supply.dart    --password-file/-stdin logic
│   │   │   └── models/                 DTOs (WalletInfo, Utxo, TxRecord…)
│   │   ├── secrets/
│   │   │   ├── secret_text_field.dart  obscure TextField widget
│   │   │   └── secret_dispose.dart     zero-on-dispose helpers
│   │   ├── binary/
│   │   │   └── btc_extractor.dart      extract bundled `btc` on first run
│   │   ├── paths.dart                  app data dirs (path_provider)
│   │   └── theme.dart                  Material 3 light/dark
│   ├── features/
│   │   ├── wallet_list/                Story 9
│   │   ├── wallet_create/              Story 1 + 20
│   │   ├── wallet_import/              Story 2
│   │   ├── wallet_show/                Stories 3 + 4 + 11 + 12
│   │   ├── wallet_send/                Stories 5 + 6
│   │   └── wallet_transactions/        Story 7
│   ├── routing/
│   │   └── app_router.dart             go_router config
│   └── widgets/                        shared UI (status badges, address chips)
├── assets/
│   └── btc/                            bundled binary per-arch
│       ├── linux-x64/btc
│       ├── linux-arm64/btc
│       ├── macos-x64/btc
│       ├── macos-arm64/btc
│       └── windows-x64/btc.exe
├── test/
│   ├── unit/                           btc_invoker, password_supply
│   ├── widget/                         per-feature
│   └── integration/                    end-to-end against fixture btc mock
└── scripts/
    ├── build_btc.sh                    cross-compile per-arch
    └── bundle_btc.sh                   copy build → assets/btc/<arch>/
```

### 2.3 Boundary rules

- `features/` never imports `core/btc/btc_invoker.dart` directly. Always goes through a Riverpod provider.
- `core/btc/` never imports Flutter widgets. Must remain pure Dart, testable with `package:test` alone.
- `core/secrets/` never logs. Zero-on-dispose is a hard requirement enforced by lint rule.
- `assets/btc/<arch>/` are the single source of truth for the bundled binary. No PATH lookup.

### 2.4 Tooling

- Flutter 3.x stable (exact version pinned in `flutter-btc-wallet/pubspec.yaml` SDK constraint)
- Dart 3.x
- Riverpod 2.x (`flutter_riverpod`, `riverpod_annotation`, `riverpod_generator`)
- go_router 14.x
- Material 3 (built-in, no third-party design system)
- `path_provider` for app data dirs
- `package:logging` with custom `BtcLogFilter`
- `integration_test` for end-to-end

---

## 3. Scope: user stories covered

11 of 20 `btc` user stories are in scope for v1. The other 9 are deferred to follow-up specs.

### 3.1 In scope

| Story | Title |
|---|---|
| 1 | Create a new wallet |
| 2 | Import an existing wallet |
| 3 | Check balance |
| 4 | Sync chain state |
| 5 | Send a payment |
| 6 | Send with custom fee rate |
| 7 | Inspect transaction history |
| 9 | List / show / delete / rename wallets |
| 11 | Show config + debug info |
| 12 | Persist wallet across CLI invocations |
| 20 | Pick a specific address type on creation |

### 3.2 Deferred (separate specs)

- Story 8 (fee estimates as standalone screen) — folded into Send screen as a dropdown instead.
- Story 10 (mainnet explicit) — covered via UI confirm dialog (CLI gate already exists).
- Stories 13, 14, 15, 16, 17 (multi-recipient, drain, coin selection, manual UTXO, RBF) — single-recipient send only for v1.
- Story 18 (BIP-137 sign/verify) — deferred.
- Story 19 (descriptor export) — deferred.
- `btc encrypt` / `btc decrypt` — out of UI scope (CLI only).

---

## 4. Data flow

Every wallet action follows the same shape:

```text
[UI widget]
   │  user taps "Send"
   ▼
[FeatureNotifier] (Riverpod)
   │  build TxRequest (DTO, immutable)
   ▼
[BtcInvoker]
   │  1. write password to temp file mode 0600 (or pipe via stdin)
   │  2. Process.start('btc', ['wallet','send', ...flags, '--password-file', tmpPath])
   │  3. await stdout/stderr/exitCode
   │  4. unlink tmp file
   │  5. parse --json stdout → typed result OR stderr → typed error
   ▼
[AsyncValue<T>] → UI rebuild (loading spinner / data / error chip)
```

### 4.1 Action → invocation map

| UI action | `btc` invocation | Output type |
|---|---|---|
| List wallets | `btc wallet list --network <NET> --json` | `List<WalletInfo>` |
| Show wallet | `btc wallet show <ID> --network <NET> --password-file <tmp>` | `WalletDetail` |
| Sync | reuses `wallet show` cached `ChangeSet`, syncs delta on next show | `WalletDetail` |
| Send | `btc wallet send --to <addr:amt> --fee-rate <n> --password-file <tmp> --esplora-url <URL> --pin-spki <hex>` | `SendResult` (txid, fee, vbytes) |
| Transactions | `btc tx-list --mnemonic <…> --json` (mnemonic from unlocked session) | `List<TxRecord>` |
| Create | `btc wallet create --words <N> --network <NET> --type <T> --password-file <tmp>` | `WalletCreated` (mnemonic + ID + first address) |
| Import | `btc wallet import --mnemonic <…> --network <NET> --password-file <tmp>` | `WalletInfo` |
| Delete | `btc wallet delete <ID> --network <NET>` | unit |
| Rename | `btc wallet rename --id <ID> --to <NEW> --network <NET>` | unit |
| Config | `btc config show --json` | `BtcConfig` |

### 4.2 Unlocked session

`btc tx-list`, `btc wallet balance`, `btc wallet send` are stateless — they require the mnemonic at invocation time. UI keeps the wallet "unlocked" in a `WalletSession` provider that holds `mnemonic: Zeroizing<String>` for the session lifetime. Cleared on lock button + app exit + 15-min idle timeout (configurable).

### 4.3 Process errors

Non-zero exit → typed `BtcError { exitCode, stderr, kind }` where `kind` is parsed from known stderr shapes:

| `kind` | UI mapping |
|---|---|
| `wrongPassword` | "Wrong password — try again" |
| `insufficientFunds` | "Not enough balance for amount + fee" |
| `unknownWallet` | "Wallet not found — may have been deleted" |
| `networkError` | "Esplora unreachable — check Settings → Esplora URL" |
| `unknownAddressType` | "Address does not match the wallet's network" |
| `confirmRequired` (mainnet) | "Type `yes` in the confirmation dialog to proceed" |
| other | raw stderr line in collapsible details |

---

## 5. Components

### 5.1 Riverpod providers

| Provider | Type | Purpose |
|---|---|---|
| `btcInvokerProvider` | `Provider` | Singleton `BtcInvoker` (path to bundled `btc`) |
| `appPathsProvider` | `FutureProvider` | App data dir + bundled binary path |
| `walletsListProvider(network)` | `AsyncNotifierProvider.family` | `List<WalletInfo>` for a network |
| `walletSessionProvider(walletId)` | `NotifierProvider.family` | `WalletSession?` (mnemonic in `Zeroizing<String>` + cached `WalletDetail`) |
| `walletSendProvider(walletId)` | `AsyncNotifierProvider.family` | Pending send tx preview + execution |
| `transactionsProvider(walletId)` | `AsyncNotifierProvider.family` | Tx history |
| `feeEstimateProvider(network)` | `FutureProvider.family` | Live Esplora fee table (folded into Send screen) |
| `esploraConfigProvider` | `Provider` | Esplora URL + SPKI pin per network (Settings-editable) |

### 5.2 Routes → screens

| Route | Screen | Story |
|---|---|---|
| `/` | `HomeShell` (sidebar + content) | shell |
| `/wallets/:network` | `WalletListScreen` | 9 |
| `/wallets/:network/new` | `WalletCreateScreen` | 1 + 20 |
| `/wallets/:network/import` | `WalletImportScreen` | 2 |
| `/wallets/:network/:walletId` | `WalletDetailScreen` | 3 + 4 + 11 + 12 |
| `/wallets/:network/:walletId/send` | `SendScreen` | 5 + 6 |
| `/wallets/:network/:walletId/transactions` | `TransactionsScreen` | 7 |
| `/settings` | `SettingsScreen` | Esplora URL/pin config |

### 5.3 Shared widgets

- `AddressChip` — copy-to-clipboard, truncate mid, network badge
- `BalanceCard` — confirmed / untrusted-pending / immature breakdown
- `NetworkPicker` — segmented control (testnet/mainnet/regtest), default testnet
- `PasswordField` — obscure + reveal toggle + zero-on-dispose callback
- `MnemonicPasteField` — paste-only textarea, word count validator, "I have backed this up" checkbox gate
- `StatusBadge` — success/warn/error chips mapped to typed `BtcError` variants
- `ProcessProgressOverlay` — wraps async actions with cancellable progress UI

### 5.4 Theme

Material 3, system light/dark, monospace font for addresses + txids, accent color Bitcoin orange `#F7931A` (visual identity, not user-configurable).

---

## 6. Bundling + first-run extraction

### 6.1 Per-arch build matrix

| Target arch | Source build | Asset path |
|---|---|---|
| `x86_64-unknown-linux-gnu` | `cargo build --release --target x86_64-unknown-linux-gnu -p btc` | `assets/btc/linux-x64/btc` |
| `aarch64-unknown-linux-gnu` | same with `-aarch64` | `assets/btc/linux-arm64/btc` |
| `x86_64-apple-darwin` | `--target x86_64-apple-darwin` | `assets/btc/macos-x64/btc` |
| `aarch64-apple-darwin` | `--target aarch64-apple-darwin` | `assets/btc/macos-arm64/btc` |
| `x86_64-pc-windows-msvc` | `--target x86_64-pc-windows-msvc` | `assets/btc/windows-x64/btc.exe` |

### 6.2 Asset registration (`pubspec.yaml`)

```yaml
flutter:
  assets:
    - assets/btc/linux-x64/btc
    - assets/btc/linux-arm64/btc
    - assets/btc/macos-x64/btc
    - assets/btc/macos-arm64/btc
    - assets/btc/windows-x64/btc.exe
```

### 6.3 First-run extraction (`core/binary/btc_extractor.dart`)

1. Detect host arch via `Platform.operatingSystem` + `Platform.resolvedExecutable` arch.
2. Pick matching asset path. Missing → error screen "This platform is not bundled — please file an issue at <url>".
3. Compute SHA-256 of asset.
4. Extract to `<appSupportDir>/btc/btc-<version>-<arch>[.exe]` via `writeAsBytes`.
5. Set mode `0o755` (Unix). Windows: no mode change.
6. Persist extraction record (path + sha256) in `<appSupportDir>/btc/manifest.json`. Subsequent runs skip re-extract unless asset version changed.
7. Symlink at extraction path refused (file written directly, not symlinked).
8. Verify by running `btc --version` once. Non-zero exit → quarantine + error screen.

### 6.4 App data dirs (via `path_provider`)

| OS | Path |
|---|---|
| Linux | `~/.local/share/flutter_btc_wallet/` |
| macOS | `~/Library/Application Support/flutter_btc_wallet/` |
| Windows | `%APPDATA%\flutter_btc_wallet\` |

Sub-dirs: `btc/` (extracted binary), `tmp/` (password temp files), `wallet_data/` (passed as `BTC_DATA_DIR` to `btc` so CLI reads/writes the same blob store).

### 6.5 Update flow

When Flutter app updates, asset hash changes → extractor overwrites `btc-<newver>`. Old version kept 1 release for rollback (manual).

---

## 7. Security + secret handling

Reuse `btc`'s existing L12 secret-redaction pipeline. UI never invents a new secret path.

### 7.1 Mnemonic lifecycle

| Phase | Storage | Display |
|---|---|---|
| Input | `MnemonicPasteField` (`String` widget state) | hidden (paste-only, no echo) |
| Submit | Temp file mode 0600 → `btc --password-file <tmp>` (or stdin where supported per Issue #84) | gone from UI immediately after process returns |
| Wallet create success | Shown ONCE in modal with "I have written this down" gate + 5-second display delay + "Hide" button. Clipboard copy optional. After modal closes: `Zeroizing<String>`.zeroize() | one-shot, force acknowledgment |
| Unlocked session | `WalletSession.mnemonic: Zeroizing<String>` in `walletSessionProvider`. Cleared on lock button + app exit + 15-min idle timeout | never displayed again |
| `btc` invocation | Same temp file / stdin pattern as password | never logged |
| Logging | UI logger explicitly redacts BIP-39-shaped strings + password flag values | n/a |

### 7.2 Password lifecycle

| Phase | Storage | Display |
|---|---|---|
| Input | `PasswordField` (`String` widget state, obscure by default) | dots, with eye-icon reveal toggle |
| Submit | Temp file mode 0600 → `btc --password-file <tmp>` | gone |
| Logout | Riverpod provider cleared; cached form state disposed | n/a |
| Idle | 15-min idle (configurable, default 15) → auto-lock all sessions | n/a |

### 7.3 Temp file invariants (`core/secrets/temp_secret_file.dart`)

1. Created in `<appSupportDir>/tmp/<uuid>.pwd` — UUID v4 random filename.
2. `writeAsBytesSync(bytes, flush: true)` — never written via append mode.
3. `chmod 0o600` immediately after creation (Unix). Windows: rely on ACL defaults.
4. Refuse to write if path already exists (paranoid: catches stale file from crash).
5. Refuse if path is a symlink (defense in depth).
6. Consumer MUST `unlink` in `finally` block — Dart `try/finally` enforces.
7. After unlink, file's inode is freed; content not recovered without forensic tools.

### 7.4 Process spawn hardening

- Always `Process.start` (not `Process.run`) to control env + cwd.
- Strip env vars that leak secrets (`BTC_WALLET_MNEMONIC`, `BTC_ENCRYPT_PASSWORD`, `BTC_DECRYPT_PASSWORD`) from inherited parent env.
- Set `BTC_DATA_DIR=<appSupportDir>/wallet_data` explicitly.
- Capture stdout/stderr to temp buffers, never to disk.
- `--confirm-yes yes` gate handled in UI: user must type `yes` in confirmation dialog before send executes on mainnet (Story 10 + 17 — already enforced by `btc` handler).

### 7.5 Debug redaction

`package:logging` configured with custom `BtcLogFilter` that scrubs:

- any string matching `/\b([a-z]+\s){11,23}[a-z]+\b/` (BIP-39 mnemonic shape)
- any password flag value (`--password`, `--password-file` argument content)
- any tx-signing related private keys (none should appear — `btc` keeps them internal)

### 7.6 Threat model (subset)

| Threat | Mitigation |
|---|---|
| Clipboard hijack after mnemonic copy | 5-sec display + force "I've written it down" gate |
| Shoulder-surf on mnemonic paste | `MnemonicPasteField` shows word count only, not words |
| Temp file leak after crash | `unlink` in `finally` + UUID v4 path + 0600 |
| Password field logged | `BtcLogFilter` scrubs; widget-level dispose |
| Long-running unlock session | 15-min idle auto-lock (configurable) |
| Bundle tampering | SHA-256 manifest verification on extract |
| `btc` binary path substitution | Only bundled binary, never PATH lookup |

**Out of scope for v1**: anti-keylogger, screen recording defense, memory encryption (relies on OS swap protection).

---

## 8. Testing + CI

### 8.1 Test layers

| Layer | Tool | Scope | Gate |
|---|---|---|---|
| Unit | `test` (Dart) | `BtcInvoker`, `PasswordSupply`, DTOs, JSON parsers, `BtcLogFilter` | `flutter test` |
| Widget | `flutter_test` | Per-screen render, AsyncValue states | same |
| Integration (mock btc) | `integration_test` | End-to-end happy path against fake `btc` shell script that echoes JSON | manual + CI opt-in |
| Live smoke (real btc) | manual + scripted | Wallet create → fund via faucet → show → send → verify on block explorer | operator-driven, NOT CI (mirrors L29 for `btc`) |

### 8.2 Coverage targets

- `BtcInvoker.invoke()` — every command variant builds correct argv, env vars, exit code handling, JSON parse, error mapping
- `PasswordSupply` — temp file created mode 0600, UUID v4 path, refuses existing path, unlinks on every exit path
- `BtcLogFilter` — scrubs mnemonics (12/15/18/21/24 word matches), password flag values, secret-adjacent paths
- DTOs — round-trip JSON ↔ Dart for every shape `btc --json` produces

### 8.3 Widget test matrix (per screen)

- Initial loading state (`AsyncValue.loading`)
- Data rendered state (mock provider)
- Error state (typed `BtcError` variants → user-visible chip + retry)
- Form validation (e.g. SendScreen rejects negative amount, missing address, mainnet without confirm)
- Secret field dispose: password form widget gone → no password string retained in widget tree

### 8.4 Integration test mock

Bash script `test/integration/fixtures/fake_btc.sh` accepts `--json` flag + command argv, returns canned JSON from a per-test fixture dir. Spawned via `BtcInvoker` with overridden binary path provider in test scope. Asserts that the right `btc` subcommand + flags were passed (capture argv to file).

### 8.5 CI workflows

**`.github/workflows/flutter-btc-wallet-ci.yml`** — triggers on PR + push to main touching `flutter-btc-wallet/**` OR `rust-wallet-app/crates/btc/**`. Steps:

1. `cargo build --release -p btc` (host arch only)
2. Copy build artifact → `flutter-btc-wallet/test/fixtures/btc-host`
3. `flutter pub get`
4. `dart analyze` (must pass with zero warnings — matches `cargo clippy -- -D warnings` bar from CLAUDE.md)
5. `flutter test` (unit + widget)
6. `flutter test integration_test/` (against host-built btc)
7. Cache `flutter-btc-wallet/.dart_tool/`

No live network tests in CI (L29 operator-driven gate).

**`.github/workflows/btc-bundle.yml`** — matrix: 5 target arches. Runs on release tags only (`v*.*.*`). Produces `flutter-btc-wallet-assets-<arch>` artifact attached to GitHub Release. Manual step: download artifacts → `cp` into `assets/btc/<arch>/` → commit → tag UI release.

### 8.6 Coverage gate (matches CLAUDE.md standards)

- `dart analyze --fatal-warnings --fatal-infos`
- `flutter test --coverage` — line coverage ≥ 80% on `lib/core/`
- Widget coverage not enforced by threshold (manual review)

### 8.7 Story → screen → test traceability

| Story | Screen | Primary widget test | E2E flow |
|---|---|---|---|
| 1 Create | `WalletCreateScreen` | form validation, mnemonic display modal | create → mnemonic shown → wallet list refreshes |
| 2 Import | `WalletImportScreen` | mnemonic word count validator | paste → import → wallet list refreshes |
| 3 Balance | `WalletDetailScreen` | balance card breakdown | show wallet → numbers appear |
| 4 Sync | `WalletDetailScreen` | sync progress + post-sync balance change | show wallet → balance updates |
| 5 Send | `SendScreen` | form, fee-rate override, confirm dialog | build → sign → broadcast → txid shown |
| 6 Fee rate | `SendScreen` | fee-rate input + Esplora estimates dropdown | send with rate X → tx includes rate |
| 7 Tx history | `TransactionsScreen` | tx list rendering | show wallet → tx list loads |
| 9 List/show/delete/rename | `WalletListScreen` + `WalletDetailScreen` | list render, delete confirm, rename dialog | list → select → detail → delete → list refreshes |
| 11 Config | `SettingsScreen` | esplora URL + SPKI pin form | change Esplora URL → next sync uses new URL |
| 12 Persist | (cross-cutting) | wallet detail cached across relaunch | create → quit → relaunch → wallet still listed |
| 20 Address type | `WalletCreateScreen` | type picker (4 options) | create with taproot → first address is `tb1p...` |

### 8.8 Manual verification checklist (per release, mirrors L28 three gates)

1. Build release binary.
2. Launch on each target OS (Linux + macOS + Windows VM).
3. Run end-to-end script: create testnet wallet → fund → show → send → delete.
4. Inspect `~/.local/share/flutter_btc_wallet/btc/` — confirm extracted binary matches manifest SHA-256.
5. Confirm no mnemonic or password appears in app logs (`grep -ri 'mnemonic\|password' ~/.local/share/flutter_btc_wallet/logs/` matches only redacted marker strings).

---

## 9. Open questions (carry to planning phase)

1. **macOS code signing + notarization** — required for distribution outside the App Store. Not a v1 blocker (sideload works), but document for v1.1.
2. **Windows installer** — MSIX vs Inno Setup. Default to Flutter's default (`flutter build windows` produces an MSIX-ready bundle).
3. **App icon + branding** — Bitcoin orange + Bitcoin "₿" mark. Designer not in scope; use placeholder.
4. **Auto-update mechanism** — not in v1. User downloads new version from GitHub release.
5. **Localization** — English only for v1. i18n keys structured but no translations.
6. **Crash reporting** — not in v1. Logs to local file only.

---

## 10. Source-of-truth files referenced

- `docs/wallets/2026-08-05-btc-wallet-user-stories.md` — 20 user stories + AC
- `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` — Rust implementation plan
- `docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-design.md` — Rust design spec
- `docs/superpowers/specs/2026-08-06-rust-bitcoin-wallet-architecture.md` — Rust arch spec
- `docs/superpowers/specs/2026-08-06-rust-wallet-app-architecture.md` — Multi-chain umbrella
- `rust-wallet-app/crates/btc/src/cli.rs` — CLI surface (this UI wraps)
- `tasks/lessons.md` — L11, L12 (CRITICAL #2 secret redaction), L13 (per-task pipeline), L28 (three verify gates), L29 (live testnet smoke is operator-driven)
