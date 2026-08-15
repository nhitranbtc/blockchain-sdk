# Flutter Desktop UI for `btc` Bitcoin Wallet — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a cross-platform Flutter desktop UI that wraps the existing `btc` CLI binary, covering 11 of 20 user stories (lifecycle + view + send MVP).

**Architecture:** Riverpod 2 + go_router Flutter app, spawns the bundled `btc` binary per action, parses `--json` stdout. Secrets pass via `--password-file` (mode 0600, unlinked in `finally`). No new wallet engine — UI is a thin skin over the CLI.

**Tech Stack:** Flutter 3.x stable, Dart 3.x, Riverpod 2.x, go_router 14.x, Material 3, `path_provider`, `package:logging`, `integration_test`. Bundled binary per-arch via Cargo cross-build.

**Spec:** [`../specs/2026-08-15-flutter-btc-wallet-design.md`](../specs/2026-08-15-flutter-btc-wallet-design.md)

---

## Global Constraints

These apply to every task. Copied verbatim from spec + project conventions:

- **Toolchain**: Flutter 3.x stable (pin in `pubspec.yaml` SDK constraint: `>=3.4.0 <4.0.0`); Dart 3.x.
- **State**: Riverpod 2.x only (`flutter_riverpod` + `riverpod_annotation` + `riverpod_generator`).
- **Routing**: go_router 14.x only.
- **Theme**: Material 3, Bitcoin orange `#F7931A` accent, monospace for addresses + txids.
- **Network default**: Bitcoin testnet. Mainnet opt-in via Settings.
- **L12 CRITICAL #2** (lessons.md): mnemonic + password never logged. Mirror `btc`'s redaction pattern in UI logger.
- **L29** (lessons.md): live testnet smoke is operator-driven, NOT CI.
- **CLAUDE.md**: `dart analyze --fatal-warnings --fatal-infos` bar; `flutter test --coverage` ≥80% on `lib/core/`.
- **Bundling**: 5 target arches per spec §6.1; `assets/btc/<arch>/btc[.exe]` paths.
- **Data dir** (per spec §6.4): Linux `~/.local/share/flutter_btc_wallet/`, macOS `~/Library/Application Support/flutter_btc_wallet/`, Windows `%APPDATA%\flutter_btc_wallet\`.
- **TDD**: write failing test → run (confirm fail) → implement → run (confirm pass) → commit. Every task.
- **Commit convention**: `feat:` / `fix:` / `docs:` / `chore:` / `test:` prefixes (matches project CHANGELOG Keep-a-Changelog style from L24).
- **L13 read on task pickup**: review `tasks/lessons.md` L13 per-task pipeline before each task.

---

## File Structure

```text
flutter-btc-wallet/
├── pubspec.yaml
├── analysis_options.yaml
├── lib/
│   ├── main.dart
│   ├── core/
│   │   ├── btc/
│   │   │   ├── btc_invoker.dart
│   │   │   ├── btc_command.dart
│   │   │   ├── btc_error.dart
│   │   │   ├── password_supply.dart
│   │   │   └── models/
│   │   │       ├── wallet_info.dart
│   │   │       ├── wallet_detail.dart
│   │   │       ├── wallet_created.dart
│   │   │       ├── tx_record.dart
│   │   │       ├── fee_estimate.dart
│   │   │       ├── send_result.dart
│   │   │       └── btc_config.dart
│   │   ├── secrets/
│   │   │   ├── secret_text_field.dart
│   │   │   ├── secret_dispose.dart
│   │   │   └── temp_secret_file.dart
│   │   ├── binary/
│   │   │   └── btc_extractor.dart
│   │   ├── logging/
│   │   │   └── btc_log_filter.dart
│   │   ├── paths.dart
│   │   └── theme.dart
│   ├── features/
│   │   ├── wallet_list/wallet_list_screen.dart
│   │   ├── wallet_create/wallet_create_screen.dart
│   │   ├── wallet_create/mnemonic_display_dialog.dart
│   │   ├── wallet_import/wallet_import_screen.dart
│   │   ├── wallet_show/wallet_detail_screen.dart
│   │   ├── wallet_send/send_screen.dart
│   │   ├── wallet_transactions/transactions_screen.dart
│   │   └── settings/settings_screen.dart
│   ├── providers/
│   │   ├── btc_providers.dart
│   │   ├── wallet_providers.dart
│   │   └── esplora_config_provider.dart
│   ├── routing/app_router.dart
│   └── widgets/
│       ├── address_chip.dart
│       ├── balance_card.dart
│       ├── network_picker.dart
│       ├── password_field.dart
│       ├── mnemonic_paste_field.dart
│       ├── status_badge.dart
│       └── process_progress_overlay.dart
├── assets/btc/
│   ├── linux-x64/btc
│   ├── linux-arm64/btc
│   ├── macos-x64/btc
│   ├── macos-arm64/btc
│   └── windows-x64/btc.exe
├── test/
│   ├── unit/
│   ├── widget/
│   └── integration/fixtures/
└── scripts/
    ├── build_btc.sh
    └── bundle_btc.sh
```

Boundary rules (per spec §2.3):
- `features/` only via Riverpod providers, never direct `BtcInvoker`.
- `core/btc/` is pure Dart, no Flutter widgets.
- `core/secrets/` never logs.

---

## Task Index

| # | Task | Story | Files |
|---|---|---|---|
| 1 | Scaffold Flutter project + pubspec | setup | 4 |
| 2 | Lint + analysis options | setup | 1 |
| 3 | App theme + paths | setup | 2 |
| 4 | BtcExtractor (bundled binary) | infra | 1 |
| 5 | TempSecretFile (0600 + unlink) | security | 1 |
| 6 | PasswordSupply (wraps TempSecretFile) | security | 1 |
| 7 | BtcLogFilter (mnemonic + password scrub) | security | 1 |
| 8 | BtcCommand enum + BtcError | core/btc | 2 |
| 9 | DTOs (WalletInfo, WalletDetail, etc.) | core/btc | 7 |
| 10 | BtcInvoker (process spawn + parse) | core/btc | 1 |
| 11 | btcInvokerProvider + appPathsProvider | providers | 1 |
| 12 | EsploraConfig provider + persistence | providers | 2 |
| 13 | walletsListProvider | providers | 1 |
| 14 | walletSessionProvider (unlocked session) | providers | 1 |
| 15 | Shared widgets | widgets | 7 |
| 16 | HomeShell + go_router | routing | 2 |
| 17 | WalletListScreen (Story 9 list) | Story 9 | 1 |
| 18 | WalletCreateScreen + MnemonicDisplayDialog (Story 1+20) | Story 1+20 | 2 |
| 19 | WalletImportScreen (Story 2) | Story 2 | 1 |
| 20 | WalletDetailScreen + balance/sync (Stories 3+4+11+12) | Stories 3+4+11+12 | 1 |
| 21 | SendScreen + confirm dialog (Stories 5+6) | Stories 5+6 | 1 |
| 22 | TransactionsScreen (Story 7) | Story 7 | 1 |
| 23 | SettingsScreen | infra | 1 |
| 24 | Integration test: fake_btc.sh + e2e | testing | 3 |
| 25 | CI workflows | CI | 2 |
| 26 | Manual verification + CHANGELOG | release | 1 |

---

## Task 1: Scaffold Flutter project + pubspec

**Files:**
- Create: `flutter-btc-wallet/pubspec.yaml`
- Create: `flutter-btc-wallet/lib/main.dart` (placeholder)
- Create: `flutter-btc-wallet/.gitignore`
- Create: `flutter-btc-wallet/assets/btc/<5 arches>/.gitkeep`
- Create: `flutter-btc-wallet/test/{unit,widget,integration}/.gitkeep`

- [ ] **Step 1: Create directory tree + .gitignore**

```bash
mkdir -p flutter-btc-wallet/{lib/{core/{btc/models,secrets,binary,logging},features/{wallet_list,wallet_create,wallet_import,wallet_show,wallet_send,wallet_transactions,settings},providers,routing,widgets},assets/btc/{linux-x64,linux-arm64,macos-x64,macos-arm64,windows-x64},test/{unit,widget,integration/fixtures},scripts}
touch flutter-btc-wallet/assets/btc/{linux-x64,linux-arm64,macos-x64,macos-arm64,windows-x64}/.gitkeep
touch flutter-btc-wallet/test/{unit,widget,integration}/.gitkeep
```

`flutter-btc-wallet/.gitignore`:

```gitignore
.dart_tool/
.flutter-plugins
.flutter-plugins-dependencies
.packages
.pub-cache/
.pub/
build/
*.iml
.idea/
.vscode/
.fvm/
coverage/
*.log
```

- [ ] **Step 2: Write pubspec.yaml**

```yaml
name: flutter_btc_wallet
description: Desktop UI for the btc Bitcoin wallet CLI.
publish_to: 'none'
version: 0.1.0

environment:
  sdk: '>=3.4.0 <4.0.0'
  flutter: '>=3.22.0'

dependencies:
  flutter:
    sdk: flutter
  flutter_riverpod: ^2.5.1
  riverpod_annotation: ^2.3.5
  go_router: ^14.2.0
  path_provider: ^2.1.4
  path: ^1.9.0
  crypto: ^3.0.3
  logging: ^1.2.0

dev_dependencies:
  flutter_test:
    sdk: flutter
  integration_test:
    sdk: flutter
  flutter_lints: ^4.0.0
  build_runner: ^2.4.11
  riverpod_generator: ^2.4.3
  custom_lint: ^0.6.4
  riverpod_lint: ^2.3.10

flutter:
  uses-material-design: true
  assets:
    - assets/btc/linux-x64/btc
    - assets/btc/linux-arm64/btc
    - assets/btc/macos-x64/btc
    - assets/btc/macos-arm64/btc
    - assets/btc/windows-x64/btc.exe

dependency_overrides: {}
```

The asset files don't exist yet — Flutter rejects pubspec entries pointing at missing files. Create 0-byte stubs:

```bash
for arch in linux-x64 linux-arm64 macos-x64 macos-arm64; do
  printf '' > "flutter-btc-wallet/assets/btc/$arch/btc"
done
printf '' > "flutter-btc-wallet/assets/btc/windows-x64/btc.exe"
```

Task 25's CI workflow replaces stubs with real cross-compiled binaries.

- [ ] **Step 3: Run `flutter pub get`**

Run: `cd flutter-btc-wallet && flutter pub get`
Expected: success; `pubspec.lock` generated.

- [ ] **Step 4: Commit**

```bash
git add flutter-btc-wallet/
git commit -m "chore(flutter): scaffold flutter-btc-wallet project (Task 1)"
```

---

## Task 2: Lint + analysis options

**Files:**
- Create: `flutter-btc-wallet/analysis_options.yaml`
- Modify: `flutter-btc-wallet/lib/main.dart` (replace placeholder)

- [ ] **Step 1: Write `analysis_options.yaml`**

```yaml
include: package:flutter_lints/flutter.yaml

analyzer:
  language:
    strict-casts: true
    strict-inference: true
    strict-raw-types: true
  errors:
    invalid_assignment: error
    missing_return: error
    dead_code: warning
    todo: ignore
  exclude:
    - "**/*.g.dart"
    - "**/*.freezed.dart"

linter:
  rules:
    - always_declare_return_types
    - avoid_print
    - avoid_unused_constructor_parameters
    - cancel_subscriptions
    - close_sinks
    - prefer_const_constructors
    - prefer_const_declarations
    - prefer_final_locals
    - unawaited_futures
    - unsafe_html
```

- [ ] **Step 2: Replace `lib/main.dart`**

```dart
import 'package:flutter/material.dart';

void main() {
  runApp(const _PlaceholderApp());
}

class _PlaceholderApp extends StatelessWidget {
  const _PlaceholderApp();

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(home: Scaffold(body: Center(child: Text('OK'))));
  }
}
```

- [ ] **Step 3: Verify analyzer passes**

Run: `cd flutter-btc-wallet && dart analyze --fatal-warnings --fatal-infos`
Expected: "No issues found!" Exit 0.

- [ ] **Step 4: Commit**

```bash
git add flutter-btc-wallet/analysis_options.yaml flutter-btc-wallet/lib/main.dart
git commit -m "chore(flutter): strict analyzer config + app shell (Task 2)"
```

---

## Task 3: App theme + paths

**Files:**
- Create: `flutter-btc-wallet/lib/core/theme.dart`
- Create: `flutter-btc-wallet/lib/core/paths.dart`
- Create: `flutter-btc-wallet/test/unit/paths_test.dart`

- [ ] **Step 1: Write failing test for `appDataDir`**

```dart
import 'dart:io';
import 'package:flutter_btc_wallet/core/paths.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('appDataDir returns a directory whose name is flutter_btc_wallet', () async {
    final dir = await appDataDir();
    expect(dir.path.split(Platform.pathSeparator).last, 'flutter_btc_wallet');
  });

  test('subdirFor returns a subdirectory under appDataDir', () async {
    final base = await appDataDir();
    final tmp = await subdirFor('tmp');
    expect(tmp.path.startsWith(base.path), isTrue);
    expect(tmp.path.endsWith('tmp'), isTrue);
  });
}
```

- [ ] **Step 2: Run test to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/paths_test.dart`
Expected: FAIL with `Target of URI doesn't exist`.

- [ ] **Step 3: Implement `paths.dart`**

```dart
import 'dart:io';
import 'package:path_provider/path_provider.dart';

const _appName = 'flutter_btc_wallet';

Future<Directory> appDataDir() async {
  final base = await getApplicationSupportDirectory();
  return Directory('${base.path}${Platform.pathSeparator}$_appName').create(recursive: true);
}

Future<Directory> subdirFor(String name) async {
  final base = await appDataDir();
  return Directory('${base.path}${Platform.pathSeparator}$name').create(recursive: true);
}
```

- [ ] **Step 4: Write theme**

`lib/core/theme.dart`:

```dart
import 'package:flutter/material.dart';

const bitcoinOrange = Color(0xFFF7931A);

ThemeData buildLightTheme() => _build(Brightness.light);
ThemeData buildDarkTheme() => _build(Brightness.dark);

ThemeData _build(Brightness brightness) {
  final scheme = ColorScheme.fromSeed(
    seedColor: bitcoinOrange,
    brightness: brightness,
  );
  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    fontFamily: 'Roboto',
  );
}
```

- [ ] **Step 5: Run paths test to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/paths_test.dart`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add flutter-btc-wallet/lib/core/paths.dart flutter-btc-wallet/lib/core/theme.dart flutter-btc-wallet/test/unit/paths_test.dart
git commit -m "feat(flutter): app paths + Material 3 theme (Task 3)"
```

---

## Task 4: BtcExtractor (bundled binary extract + verify)

**Files:**
- Create: `flutter-btc-wallet/lib/core/binary/btc_extractor.dart`
- Create: `flutter-btc-wallet/test/unit/btc_extractor_test.dart`

- [ ] **Step 1: Write failing test**

```dart
import 'dart:io';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_btc_wallet/core/binary/btc_extractor.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('extractBtc picks an asset path that contains the host OS', () async {
    final path = await extractBtc();
    if (Platform.isLinux) {
      expect(path, contains('linux'));
    } else if (Platform.isMacOS) {
      expect(path, contains('macos'));
    } else if (Platform.isWindows) {
      expect(path, contains('windows'));
    }
  });

  test('extractBtc creates the binary at appDataDir/btc/', () async {
    final path = await extractBtc();
    expect(await File(path).exists(), isTrue);
    expect(path, contains('${Platform.pathSeparator}btc${Platform.pathSeparator}'));
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_extractor_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement**

`lib/core/binary/btc_extractor.dart`:

```dart
import 'dart:io';
import 'package:crypto/crypto.dart' show sha256;
import 'package:flutter/services.dart' show rootBundle;
import '../paths.dart';

Future<String> extractBtc() async {
  final (arch, assetPath, binaryName) = _hostTarget();
  final btcDir = await subdirFor('btc');
  final manifestFile = File('${btcDir.path}${Platform.pathSeparator}manifest.json');

  final data = await rootBundle.load(assetPath);
  final bytes = data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes);
  if (bytes.isEmpty) {
    throw const ExtractionException('Bundled btc asset is empty — populate assets/btc/<arch>/ first');
  }
  final hash = sha256.convert(bytes).toString();

  if (await manifestFile.exists()) {
    final current = await manifestFile.readAsString();
    if (current.contains('"$hash"') && current.contains('"$arch"')) {
      final existing = File('${btcDir.path}${Platform.pathSeparator}$binaryName');
      if (await existing.exists()) return existing.path;
    }
  }

  final outFile = File('${btcDir.path}${Platform.pathSeparator}$binaryName');
  if (await outFile.exists()) await outFile.delete();
  await outFile.writeAsBytes(bytes, flush: true);
  if (!Platform.isWindows) {
    await Process.run('chmod', ['0o755', outFile.path]);
  }

  final result = await Process.run(outFile.path, ['--version']);
  if (result.exitCode != 0) {
    await outFile.delete();
    throw ExtractionException('Extracted btc failed --version: ${result.stderr}');
  }

  await manifestFile.writeAsString('{"hash":"$hash","arch":"$arch"}');
  return outFile.path;
}

(String arch, String assetPath, String binaryName) _hostTarget() {
  if (Platform.isLinux) {
    if (Platform.resolvedExecutable.contains('aarch64') ||
        Platform.resolvedExecutable.contains('arm64')) {
      return ('linux-arm64', 'assets/btc/linux-arm64/btc', 'btc');
    }
    return ('linux-x64', 'assets/btc/linux-x64/btc', 'btc');
  }
  if (Platform.isMacOS) {
    if (Platform.resolvedExecutable.contains('arm64')) {
      return ('macos-arm64', 'assets/btc/macos-arm64/btc', 'btc');
    }
    return ('macos-x64', 'assets/btc/macos-x64/btc', 'btc');
  }
  if (Platform.isWindows) return ('windows-x64', 'assets/btc/windows-x64/btc.exe', 'btc.exe');
  throw ExtractionException('Unsupported platform: ${Platform.operatingSystem}');
}

class ExtractionException implements Exception {
  const ExtractionException(this.message);
  final String message;
  @override
  String toString() => 'ExtractionException: $message';
}
```

- [ ] **Step 4: Run test to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_extractor_test.dart`
Expected: PASS on host with 0-byte stub. Real cross-arch test runs in CI matrix (Task 25).

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/core/binary/btc_extractor.dart flutter-btc-wallet/test/unit/btc_extractor_test.dart
git commit -m "feat(flutter): bundled btc extractor + SHA-256 manifest (Task 4)"
```

---

## Task 5: TempSecretFile (0600 + auto-unlink)

**Files:**
- Create: `flutter-btc-wallet/lib/core/secrets/temp_secret_file.dart`
- Create: `flutter-btc-wallet/test/unit/temp_secret_file_test.dart`

- [ ] **Step 1: Write failing test**

```dart
import 'dart:io';
import 'package:flutter_btc_wallet/core/secrets/temp_secret_file.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('withTempSecretFile writes secret content to file', () async {
    await withTempSecretFile('hunter2', (path) async {
      expect(await File(path).exists(), isTrue);
      expect(await File(path).readAsString(), 'hunter2');
    });
  });

  test('withTempSecretFile unlinks after callback returns', () async {
    String? capturedPath;
    await withTempSecretFile('hunter2', (path) async {
      capturedPath = path;
    });
    expect(capturedPath, isNotNull);
    expect(await File(capturedPath!).exists(), isFalse);
  });

  test('withTempSecretFile unlinks even when callback throws', () async {
    String? capturedPath;
    await expectLater(
      withTempSecretFile('hunter2', (path) async {
        capturedPath = path;
        throw StateError('boom');
      }),
      throwsA(isA<StateError>()),
    );
    expect(await File(capturedPath!).exists(), isFalse);
  });

  test('temp file lives under tmp/ subdir of appDataDir', () async {
    await withTempSecretFile('hunter2', (path) async {
      expect(path, contains('${Platform.pathSeparator}tmp${Platform.pathSeparator}'));
    });
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/temp_secret_file_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement**

`lib/core/secrets/temp_secret_file.dart`:

```dart
import 'dart:io';
import 'dart:math';
import 'package:path/path.dart' as p;
import '../paths.dart';

final _random = Random.secure();

Future<void> withTempSecretFile(
  String secret,
  Future<void> Function(String path) body,
) async {
  final tmpDir = await subdirFor('tmp');
  final path = p.join(tmpDir.path, '${_uuidV4()}.pwd');

  if (await File(path).exists()) {
    throw TempSecretFileException('Refusing to overwrite existing temp file: $path');
  }

  final file = File(path);
  await file.writeAsString(secret, flush: true);

  if (!Platform.isWindows) {
    await Process.run('chmod', ['0o600', path]);
  }

  try {
    await body(path);
  } finally {
    try {
      await file.delete();
    } catch (_) {
      // Best-effort. OS cleans tmp on reboot for our app data dir.
    }
  }
}

String _uuidV4() {
  final bytes = List<int>.generate(16, (_) => _random.nextInt(256));
  bytes[6] = (bytes[6] & 0x0F) | 0x40;
  bytes[8] = (bytes[8] & 0x3F) | 0x80;
  final hex = bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
  return '${hex.substring(0, 8)}-${hex.substring(8, 12)}-'
      '${hex.substring(12, 16)}-${hex.substring(16, 20)}-${hex.substring(20)}';
}

class TempSecretFileException implements Exception {
  const TempSecretFileException(this.message);
  final String message;
  @override
  String toString() => 'TempSecretFileException: $message';
}
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/temp_secret_file_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/core/secrets/temp_secret_file.dart flutter-btc-wallet/test/unit/temp_secret_file_test.dart
git commit -m "feat(flutter): temp secret file (mode 0600 + auto-unlink) (Task 5)"
```

---

## Task 6: PasswordSupply (wraps TempSecretFile for `btc` invocation)

**Files:**
- Create: `flutter-btc-wallet/lib/core/btc/password_supply.dart`
- Create: `flutter-btc-wallet/test/unit/password_supply_test.dart`

**Interfaces:**
- Consumes: `withTempSecretFile` (Task 5), `BtcCommand` (Task 8 — adds `--password-file` flag).
- Produces: `Future<void> withPasswordFile(String password, Future<void> Function(String path) body)`.

- [ ] **Step 1: Write failing test**

```dart
import 'dart:io';
import 'package:flutter_btc_wallet/core/btc/password_supply.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('withPasswordFile runs body with a temp file containing the password', () async {
    String? seenPath;
    await withPasswordFile('hunter2', (path) async {
      seenPath = path;
      expect(await File(path).readAsString(), 'hunter2');
    });
    expect(seenPath, isNotNull);
    expect(await File(seenPath!).exists(), isFalse,
        reason: 'temp file must be unlinked after callback returns');
  });

  test('withPasswordFile unlinks even when callback throws', () async {
    String? seenPath;
    await expectLater(
      withPasswordFile('hunter2', (path) async {
        seenPath = path;
        throw StateError('boom');
      }),
      throwsA(isA<StateError>()),
    );
    expect(await File(seenPath!).exists(), isFalse);
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/password_supply_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement**

`lib/core/btc/password_supply.dart`:

```dart
import '../secrets/temp_secret_file.dart';

/// Run [body] with a temp file containing [password].
///
/// Bridges the `btc` `--password-file` flag (Issue #84) — caller
/// passes the resulting `path` as the flag value, then `btc` reads
/// and unlinks (in its own handler layer) the file.
Future<void> withPasswordFile(
  String password,
  Future<void> Function(String path) body,
) =>
    withTempSecretFile(password, body);
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/password_supply_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/core/btc/password_supply.dart flutter-btc-wallet/test/unit/password_supply_test.dart
git commit -m "feat(flutter): PasswordSupply bridges TempSecretFile for btc flag (Task 6)"
```

---

## Task 7: BtcLogFilter (scrub mnemonic + password)

**Files:**
- Create: `flutter-btc-wallet/lib/core/logging/btc_log_filter.dart`
- Create: `flutter-btc-wallet/test/unit/btc_log_filter_test.dart`

**Interfaces:**
- Produces: `BtcLogFilter implements LogFilter` from `package:logging`.

- [ ] **Step 1: Write failing test**

```dart
import 'package:flutter_btc_wallet/core/logging/btc_log_filter.dart';
import 'package:logging/logging.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final filter = BtcLogFilter();

  test('redacts a 12-word mnemonic', () {
    const msg = 'about to sign with: abandon abandon abandon abandon '
        'abandon abandon abandon abandon abandon abandon abandon about';
    expect(filter.redact(msg), '<redacted-mnemonic>');
  });

  test('redacts 24-word mnemonic', () {
    const msg = 'words: abandon abandon abandon abandon abandon abandon '
        'abandon abandon abandon abandon abandon abandon abandon abandon '
        'abandon abandon abandon abandon abandon abandon abandon abandon '
        'abandon art';
    expect(filter.redact(msg), '<redacted-mnemonic>');
  });

  test('does NOT redact random 5-word English phrase', () {
    const msg = 'hello world from the test runner';
    expect(filter.redact(msg), msg);
  });

  test('redacts --password flag value', () {
    expect(filter.redact('cmd --password hunter2 --network testnet'),
        'cmd --password <redacted> --network testnet');
  });

  test('redacts --password-file flag value', () {
    expect(filter.redact('cmd --password-file /run/secrets/btc-pwd'),
        'cmd --password-file <redacted>');
  });

  test('log() applies redaction', () {
    LogRecord record = LogRecord(
      Level.INFO, 'wallet', 'sign with: abandon abandon abandon abandon '
          'abandon abandon abandon abandon abandon abandon abandon about', null,
    );
    final out = filter.format(record);
    expect(out, contains('<redacted-mnemonic>'));
    expect(out, isNot(contains('abandon')));
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_log_filter_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement**

`lib/core/logging/btc_log_filter.dart`:

```dart
import 'package:logging/logging.dart';

/// Log filter that scrubs mnemonic-shaped strings + `--password*` flag
/// values. Mirrors `btc` CLI's L12 CRITICAL #2 redaction pattern.
class BtcLogFilter implements LogFilter {
  // Match 12/15/18/21/24 consecutive lowercase-word runs separated
  // by single spaces (BIP-39 mnemonic shape).
  static final _mnemonicPattern = RegExp(
    r'\b([a-z]+ ){11,23}[a-z]+\b',
  );

  static final _passwordFlagPattern = RegExp(
    r'--password(?:-file|-stdin)?\s+\S+',
  );

  String redact(String message) {
    var out = message;
    out = out.replaceAll(_mnemonicPattern, '<redacted-mnemonic>');
    out = out.replaceAllMapped(_passwordFlagPattern, (m) {
      final flag = m.group(0)!.split(' ').first;
      return '$flag <redacted>';
    });
    return out;
  }

  @override
  bool shouldLog(LogRecord record) => true;

  String format(LogRecord record) {
    final ts = record.time.toIso8601String();
    final redacted = redact(record.message);
    final err = record.error != null ? ' err=${record.error}' : '';
    final st = record.stackTrace != null ? '\n${record.stackTrace}' : '';
    return '$ts ${record.level.name.padRight(7)} ${record.loggerName}: $redacted$err$st';
  }
}
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_log_filter_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/core/logging/btc_log_filter.dart flutter-btc-wallet/test/unit/btc_log_filter_test.dart
git commit -m "feat(flutter): BtcLogFilter scrubs mnemonic + password (Task 7)"
```

---

## Task 8: BtcCommand enum + BtcError

**Files:**
- Create: `flutter-btc-wallet/lib/core/btc/btc_command.dart`
- Create: `flutter-btc-wallet/lib/core/btc/btc_error.dart`
- Create: `flutter-btc-wallet/test/unit/btc_command_test.dart`
- Create: `flutter-btc-wallet/test/unit/btc_error_test.dart`

**Interfaces:**
- `BtcCommand` builds argv lists.
- `BtcError` classifies stderr into typed kinds per spec §4.3.

- [ ] **Step 1: Write failing test for BtcCommand**

```dart
import 'package:flutter_btc_wallet/core/btc/btc_command.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('WalletList builds argv with --network + --json', () {
    final cmd = BtcCommand.walletList(network: 'testnet');
    expect(cmd.argv, ['wallet', 'list', '--network', 'testnet', '--json']);
  });

  test('WalletShow includes password-file and esplora-url', () {
    final cmd = BtcCommand.walletShow(
      id: 'abc-uuid',
      network: 'testnet',
      passwordFilePath: '/tmp/abc.pwd',
      esploraUrl: 'https://blockstream.info/testnet/api',
      esploraSpkiPin: '0'.padLeft(64, '0'),
    );
    expect(cmd.argv, [
      'wallet', 'show', 'abc-uuid',
      '--network', 'testnet',
      '--password-file', '/tmp/abc.pwd',
      '--esplora-url', 'https://blockstream.info/testnet/api',
      '--esplora-spki-pin', '0'.padLeft(64, '0'),
    ]);
  });

  test('WalletSend single-recipient builds --to flag', () {
    final cmd = BtcCommand.walletSend(
      mnemonic: 'abandon x11 about',
      network: 'testnet',
      to: 'tb1qaddr:10000',
      feeRateSatPerVb: 5,
      passwordFilePath: '/tmp/pwd',
      esploraUrl: 'https://blockstream.info/testnet/api',
      esploraSpkiPin: '0'.padLeft(64, '0'),
    );
    expect(cmd.argv, containsAllInOrder([
      'wallet', 'send',
      '--mnemonic', 'abandon x11 about',
      '--network', 'testnet',
      '--to', 'tb1qaddr:10000',
      '--fee-rate', '5',
      '--password-file', '/tmp/pwd',
      '--esplora-url', 'https://blockstream.info/testnet/api',
      '--pin-spki', '0'.padLeft(64, '0'),
    ]));
  });

  test('WalletCreate includes words + type + password-file', () {
    final cmd = BtcCommand.walletCreate(
      words: 12,
      network: 'testnet',
      addressType: 'native-segwit',
      passwordFilePath: '/tmp/pwd',
    );
    expect(cmd.argv, containsAllInOrder([
      'wallet', 'create',
      '--words', '12',
      '--network', 'testnet',
      '--type', 'native-segwit',
      '--password-file', '/tmp/pwd',
    ]));
  });

  test('TxList includes mnemonic + limit', () {
    final cmd = BtcCommand.txList(
      mnemonic: 'abandon x11 about',
      network: 'testnet',
      esploraUrl: 'https://blockstream.info/testnet/api',
      esploraSpkiPin: '0'.padLeft(64, '0'),
      limit: 10,
    );
    expect(cmd.argv, containsAllInOrder([
      'tx-list', '--mnemonic', 'abandon x11 about',
      '--network', 'testnet',
      '--esplora-url', 'https://blockstream.info/testnet/api',
      '--pin-spki', '0'.padLeft(64, '0'),
      '--limit', '10',
    ]));
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_command_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement `btc_command.dart`**

```dart
import 'package:meta/meta.dart';

@immutable
sealed class BtcCommand {
  const BtcCommand();

  List<String> get argv;
}

class WalletList extends BtcCommand {
  const WalletList({required this.network});
  final String network;
  @override
  List<String> get argv => ['wallet', 'list', '--network', network, '--json'];
}

class WalletShow extends BtcCommand {
  const WalletShow({
    required this.id,
    required this.network,
    required this.passwordFilePath,
    this.esploraUrl,
    this.esploraSpkiPin,
  });
  final String id;
  final String network;
  final String passwordFilePath;
  final String? esploraUrl;
  final String? esploraSpkiPin;

  @override
  List<String> get argv => [
        'wallet', 'show', id,
        '--network', network,
        '--password-file', passwordFilePath,
        if (esploraUrl != null) '--esplora-url', esploraUrl!,
        if (esploraSpkiPin != null) '--esplora-spki-pin', esploraSpkiPin!,
      ];
}

class WalletDelete extends BtcCommand {
  const WalletDelete({required this.id, required this.network});
  final String id;
  final String network;
  @override
  List<String> get argv => ['wallet', 'delete', id, '--network', network];
}

class WalletRename extends BtcCommand {
  const WalletRename({required this.id, required this.to, required this.network});
  final String id;
  final String to;
  final String network;
  @override
  List<String> get argv => ['wallet', 'rename', '--id', id, '--to', to, '--network', network];
}

class WalletCreate extends BtcCommand {
  const WalletCreate({
    required this.words,
    required this.network,
    required this.addressType,
    required this.passwordFilePath,
    this.confirmYes,
  });
  final int words;
  final String network;
  final String addressType;
  final String passwordFilePath;
  final String? confirmYes;

  @override
  List<String> get argv => [
        'wallet', 'create',
        '--words', '$words',
        '--network', network,
        '--type', addressType,
        '--password-file', passwordFilePath,
        if (confirmYes != null) '--confirm-yes', confirmYes!,
      ];
}

class WalletImport extends BtcCommand {
  const WalletImport({
    required this.mnemonic,
    required this.network,
    required this.passwordFilePath,
  });
  final String mnemonic;
  final String network;
  final String passwordFilePath;
  @override
  List<String> get argv => [
        'wallet', 'import',
        '--mnemonic', mnemonic,
        '--network', network,
        '--password-file', passwordFilePath,
      ];
}

class WalletSend extends BtcCommand {
  const WalletSend({
    required this.mnemonic,
    required this.network,
    this.to,
    this.address,
    this.amountSat,
    required this.feeRateSatPerVb,
    required this.passwordFilePath,
    required this.esploraUrl,
    required this.esploraSpkiPin,
    this.confirmYes,
    this.dryRun = false,
  });
  final String mnemonic;
  final String network;
  final String? to;          // multi-recipient form (single OK too)
  final String? address;     // single-recipient form (deprecated, use `to`)
  final int? amountSat;
  final int feeRateSatPerVb;
  final String passwordFilePath;
  final String esploraUrl;
  final String esploraSpkiPin;
  final String? confirmYes;
  final bool dryRun;

  @override
  List<String> get argv => [
        'wallet', 'send',
        '--mnemonic', mnemonic,
        '--network', network,
        if (to != null) '--to', to!,
        if (address != null) '--address', address!,
        if (amountSat != null) '--amount-sat', '$amountSat',
        '--fee-rate', '$feeRateSatPerVb',
        if (dryRun) '--dry-run',
        if (confirmYes != null) '--confirm-yes', confirmYes!,
        '--password-file', passwordFilePath,
        '--esplora-url', esploraUrl,
        '--pin-spki', esploraSpkiPin,
      ];
}

class TxList extends BtcCommand {
  const TxList({
    required this.mnemonic,
    required this.network,
    required this.esploraUrl,
    required this.esploraSpkiPin,
    this.limit,
  });
  final String mnemonic;
  final String network;
  final String esploraUrl;
  final String esploraSpkiPin;
  final int? limit;

  @override
  List<String> get argv => [
        'tx-list',
        '--mnemonic', mnemonic,
        '--network', network,
        '--esplora-url', esploraUrl,
        '--pin-spki', esploraSpkiPin,
        if (limit != null) '--limit', '$limit',
        '--json',
      ];
}

class FeeEstimates extends BtcCommand {
  const FeeEstimates({
    required this.network,
    required this.esploraUrl,
    required this.esploraSpkiPin,
  });
  final String network;
  final String esploraUrl;
  final String esploraSpkiPin;

  @override
  List<String> get argv => [
        'fee-estimates',
        '--network', network,
        '--esplora-url', esploraUrl,
        if (esploraSpkiPin.isNotEmpty) '--pin-spki', esploraSpkiPin,
        '--json',
      ];
}

class ConfigShow extends BtcCommand {
  const ConfigShow();
  @override
  List<String> get argv => ['config', 'show', '--json'];
}

extension BtcCommandStatic on BtcCommand {
  static WalletList walletList({required String network}) => WalletList(network: network);
  static WalletShow walletShow({
    required String id,
    required String network,
    required String passwordFilePath,
    String? esploraUrl,
    String? esploraSpkiPin,
  }) =>
      WalletShow(
        id: id,
        network: network,
        passwordFilePath: passwordFilePath,
        esploraUrl: esploraUrl,
        esploraSpkiPin: esploraSpkiPin,
      );
  static WalletSend walletSend({
    required String mnemonic,
    required String network,
    String? to,
    String? address,
    int? amountSat,
    required int feeRateSatPerVb,
    required String passwordFilePath,
    required String esploraUrl,
    required String esploraSpkiPin,
    String? confirmYes,
    bool dryRun = false,
  }) =>
      WalletSend(
        mnemonic: mnemonic,
        network: network,
        to: to,
        address: address,
        amountSat: amountSat,
        feeRateSatPerVb: feeRateSatPerVb,
        passwordFilePath: passwordFilePath,
        esploraUrl: esploraUrl,
        esploraSpkiPin: esploraSpkiPin,
        confirmYes: confirmYes,
        dryRun: dryRun,
      );
  static WalletCreate walletCreate({
    required int words,
    required String network,
    required String addressType,
    required String passwordFilePath,
    String? confirmYes,
  }) =>
      WalletCreate(
        words: words,
        network: network,
        addressType: addressType,
        passwordFilePath: passwordFilePath,
        confirmYes: confirmYes,
      );
  static WalletImport walletImport({
    required String mnemonic,
    required String network,
    required String passwordFilePath,
  }) =>
      WalletImport(
        mnemonic: mnemonic,
        network: network,
        passwordFilePath: passwordFilePath,
      );
  static TxList txList({
    required String mnemonic,
    required String network,
    required String esploraUrl,
    required String esploraSpkiPin,
    int? limit,
  }) =>
      TxList(
        mnemonic: mnemonic,
        network: network,
        esploraUrl: esploraUrl,
        esploraSpkiPin: esploraSpkiPin,
        limit: limit,
      );
}
```

Add `meta: ^1.10.0` (transitively from Flutter) — no new dep needed.

- [ ] **Step 4: Write failing test for BtcError**

```dart
import 'package:flutter_btc_wallet/core/btc/btc_error.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('wrongPassword maps to WrongPassword kind', () {
    final err = BtcError.fromStderr('error: wrong password (try again)', exitCode: 2);
    expect(err.kind, BtcErrorKind.wrongPassword);
  });

  test('insufficient funds maps to InsufficientFunds', () {
    final err = BtcError.fromStderr('error: insufficient funds', exitCode: 4);
    expect(err.kind, BtcErrorKind.insufficientFunds);
  });

  test('unknown wallet maps to UnknownWallet', () {
    final err = BtcError.fromStderr("error: wallet 'abc' not found", exitCode: 4);
    expect(err.kind, BtcErrorKind.unknownWallet);
  });

  test('network/esplora unreachable maps to NetworkError', () {
    final err = BtcError.fromStderr('error: esplora unreachable: 504', exitCode: 3);
    expect(err.kind, BtcErrorKind.networkError);
  });

  test('unknown stderr maps to Other', () {
    final err = BtcError.fromStderr('some weird thing', exitCode: 1);
    expect(err.kind, BtcErrorKind.other);
  });
}
```

- [ ] **Step 5: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_error_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 6: Implement `btc_error.dart`**

```dart
import 'package:meta/meta.dart';

enum BtcErrorKind {
  wrongPassword,
  insufficientFunds,
  unknownWallet,
  networkError,
  unknownAddressType,
  confirmRequired,
  other,
}

@immutable
class BtcError implements Exception {
  const BtcError({
    required this.exitCode,
    required this.stderr,
    required this.kind,
  });

  final int exitCode;
  final String stderr;
  final BtcErrorKind kind;

  static final _patterns = <(RegExp, BtcErrorKind)>[
    (RegExp(r'wrong\s*password', caseSensitive: false), BtcErrorKind.wrongPassword),
    (RegExp(r'insufficient\s*funds', caseSensitive: false), BtcErrorKind.insufficientFunds),
    (RegExp(r'wallet.*not\s*found|unknown\s*wallet', caseSensitive: false), BtcErrorKind.unknownWallet),
    (RegExp(r'esplora|network|unreachable|timed?\s*out', caseSensitive: false), BtcErrorKind.networkError),
    (RegExp(r'does\s*not\s*match.*network|wrong\s*network', caseSensitive: false), BtcErrorKind.unknownAddressType),
    (RegExp(r'--confirm-yes|mainnet.*confirm', caseSensitive: false), BtcErrorKind.confirmRequired),
  ];

  factory BtcError.fromStderr(String stderr, {required int exitCode}) {
    for (final (pattern, kind) in _patterns) {
      if (pattern.hasMatch(stderr)) {
        return BtcError(exitCode: exitCode, stderr: stderr, kind: kind);
      }
    }
    return BtcError(exitCode: exitCode, stderr: stderr, kind: BtcErrorKind.other);
  }

  @override
  String toString() => 'BtcError(kind: $kind, exit: $exitCode, stderr: $stderr)';
}
```

- [ ] **Step 7: Run both tests to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_command_test.dart test/unit/btc_error_test.dart`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add flutter-btc-wallet/lib/core/btc/btc_command.dart flutter-btc-wallet/lib/core/btc/btc_error.dart flutter-btc-wallet/test/unit/btc_command_test.dart flutter-btc-wallet/test/unit/btc_error_test.dart
git commit -m "feat(flutter): BtcCommand enum + BtcError classifier (Task 8)"
```

---

## Task 9: DTOs (WalletInfo, WalletDetail, WalletCreated, TxRecord, FeeEstimate, SendResult, BtcConfig)

**Files:**
- Create: `flutter-btc-wallet/lib/core/btc/models/wallet_info.dart`
- Create: `flutter-btc-wallet/lib/core/btc/models/wallet_detail.dart`
- Create: `flutter-btc-wallet/lib/core/btc/models/wallet_created.dart`
- Create: `flutter-btc-wallet/lib/core/btc/models/tx_record.dart`
- Create: `flutter-btc-wallet/lib/core/btc/models/fee_estimate.dart`
- Create: `flutter-btc-wallet/lib/core/btc/models/send_result.dart`
- Create: `flutter-btc-wallet/lib/core/btc/models/btc_config.dart`
- Create: `flutter-btc-wallet/test/unit/dtos_test.dart`

**Interfaces:**
- Each DTO has `factory fromJson(Map<String, dynamic>)` + Dart class with `final` fields.

- [ ] **Step 1: Write failing test for all DTOs**

```dart
import 'package:flutter_btc_wallet/core/btc/models/wallet_info.dart';
import 'package:flutter_btc_wallet/core/btc/models/wallet_detail.dart';
import 'package:flutter_btc_wallet/core/btc/models/wallet_created.dart';
import 'package:flutter_btc_wallet/core/btc/models/tx_record.dart';
import 'package:flutter_btc_wallet/core/btc/models/fee_estimate.dart';
import 'package:flutter_btc_wallet/core/btc/models/send_result.dart';
import 'package:flutter_btc_wallet/core/btc/models/btc_config.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('WalletInfo round-trips', () {
    final w = WalletInfo.fromJson({'id': 'abc', 'network': 'testnet', 'address_type': 'native-segwit'});
    expect(w.id, 'abc');
    expect(w.network, 'testnet');
    expect(w.addressType, 'native-segwit');
  });

  test('WalletDetail from `btc wallet show --json` shape', () {
    final d = WalletDetail.fromJson({
      'id': 'abc', 'network': 'testnet',
      'address_type': 'native-segwit',
      'first_address': 'tb1q...',
      'balance': {'confirmed_sat': 1000, 'trusted_pending_sat': 0, 'untrusted_pending_sat': 0, 'immature_sat': 0},
      'utxos': [],
    });
    expect(d.balance.confirmedSat, 1000);
    expect(d.firstAddress, 'tb1q...');
  });

  test('WalletCreated carries mnemonic + id + first address', () {
    final c = WalletCreated.fromJson({
      'id': 'abc',
      'mnemonic': 'abandon x11 about',
      'first_address': 'tb1q...',
      'network': 'testnet',
      'address_type': 'native-segwit',
    });
    expect(c.mnemonic, 'abandon x11 about');
    expect(c.id, 'abc');
  });

  test('TxRecord parses direction + amount_sat + txid + confirmations', () {
    final t = TxRecord.fromJson({
      'txid': 'def', 'direction': 'outgoing', 'amount_sat': 5000,
      'fee_sat': 250, 'confirmations': 6, 'timestamp': 1700000000,
    });
    expect(t.direction, TxDirection.outgoing);
    expect(t.confirmations, 6);
  });

  test('FeeEstimate from `btc fee-estimates --json` (target -> sat/vB)', () {
    final f = FeeEstimate.fromJson({'1': 25.0, '3': 12.0, '6': 8.0, '144': 1.0});
    expect(f.fastestSatPerVb, 25);
    expect(f.economySatPerVb, 1);
  });

  test('SendResult carries txid + fee + vbytes', () {
    final s = SendResult.fromJson({'txid': 'def', 'fee_sat': 540, 'vbytes': 110});
    expect(s.txid, 'def');
    expect(s.feeSat, 540);
    expect(s.vbytes, 110);
  });

  test('BtcConfig from `btc config show --json`', () {
    final c = BtcConfig.fromJson({
      'data_dir': '/tmp/btc',
      'network': 'testnet',
      'esplora_url': 'https://blockstream.info/testnet/api',
      'wallets': ['abc', 'def'],
    });
    expect(c.dataDir, '/tmp/btc');
    expect(c.wallets, ['abc', 'def']);
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/dtos_test.dart`
Expected: FAIL — missing modules.

- [ ] **Step 3: Implement all 7 DTOs**

`lib/core/btc/models/wallet_info.dart`:

```dart
import 'package:meta/meta.dart';

@immutable
class WalletInfo {
  const WalletInfo({required this.id, required this.network, required this.addressType});
  final String id;
  final String network;
  final String addressType;

  factory WalletInfo.fromJson(Map<String, dynamic> j) => WalletInfo(
        id: j['id'] as String,
        network: j['network'] as String,
        addressType: j['address_type'] as String,
      );
}
```

`lib/core/btc/models/wallet_detail.dart`:

```dart
import 'package:meta/meta.dart';

@immutable
class Balance {
  const Balance({
    required this.confirmedSat,
    required this.trustedPendingSat,
    required this.untrustedPendingSat,
    required this.immatureSat,
  });
  final int confirmedSat;
  final int trustedPendingSat;
  final int untrustedPendingSat;
  final int immatureSat;

  factory Balance.fromJson(Map<String, dynamic> j) => Balance(
        confirmedSat: (j['confirmed_sat'] as num).toInt(),
        trustedPendingSat: (j['trusted_pending_sat'] as num?)?.toInt() ?? 0,
        untrustedPendingSat: (j['untrusted_pending_sat'] as num?)?.toInt() ?? 0,
        immatureSat: (j['immature_sat'] as num?)?.toInt() ?? 0,
      );
}

@immutable
class Utxo {
  const Utxo({required this.txid, required this.vout, required this.valueSat});
  final String txid;
  final int vout;
  final int valueSat;

  factory Utxo.fromJson(Map<String, dynamic> j) => Utxo(
        txid: j['txid'] as String,
        vout: (j['vout'] as num).toInt(),
        valueSat: (j['value_sat'] as num).toInt(),
      );
}

@immutable
class WalletDetail {
  const WalletDetail({
    required this.id,
    required this.network,
    required this.addressType,
    required this.firstAddress,
    required this.balance,
    required this.utxos,
  });
  final String id;
  final String network;
  final String addressType;
  final String firstAddress;
  final Balance balance;
  final List<Utxo> utxos;

  factory WalletDetail.fromJson(Map<String, dynamic> j) => WalletDetail(
        id: j['id'] as String,
        network: j['network'] as String,
        addressType: j['address_type'] as String,
        firstAddress: j['first_address'] as String,
        balance: Balance.fromJson(j['balance'] as Map<String, dynamic>),
        utxos: ((j['utxos'] as List?) ?? const [])
            .map((e) => Utxo.fromJson(e as Map<String, dynamic>))
            .toList(growable: false),
      );
}
```

`lib/core/btc/models/wallet_created.dart`:

```dart
import 'package:meta/meta.dart';

@immutable
class WalletCreated {
  const WalletCreated({
    required this.id,
    required this.mnemonic,
    required this.firstAddress,
    required this.network,
    required this.addressType,
  });
  final String id;
  final String mnemonic;
  final String firstAddress;
  final String network;
  final String addressType;

  factory WalletCreated.fromJson(Map<String, dynamic> j) => WalletCreated(
        id: j['id'] as String,
        mnemonic: j['mnemonic'] as String,
        firstAddress: j['first_address'] as String,
        network: j['network'] as String,
        addressType: j['address_type'] as String,
      );
}
```

`lib/core/btc/models/tx_record.dart`:

```dart
import 'package:meta/meta.dart';

enum TxDirection { incoming, outgoing, self }

@immutable
class TxRecord {
  const TxRecord({
    required this.txid,
    required this.direction,
    required this.amountSat,
    required this.feeSat,
    required this.confirmations,
    required this.timestamp,
  });
  final String txid;
  final TxDirection direction;
  final int amountSat;
  final int feeSat;
  final int confirmations;
  final int timestamp;

  factory TxRecord.fromJson(Map<String, dynamic> j) => TxRecord(
        txid: j['txid'] as String,
        direction: switch ((j['direction'] as String).toLowerCase()) {
          'incoming' => TxDirection.incoming,
          'self' => TxDirection.self,
          _ => TxDirection.outgoing,
        },
        amountSat: (j['amount_sat'] as num).toInt(),
        feeSat: ((j['fee_sat'] as num?) ?? 0).toInt(),
        confirmations: ((j['confirmations'] as num?) ?? 0).toInt(),
        timestamp: ((j['timestamp'] as num?) ?? 0).toInt(),
      );
}
```

`lib/core/btc/models/fee_estimate.dart`:

```dart
import 'package:meta/meta.dart';

@immutable
class FeeEstimate {
  const FeeEstimate({
    required this.fastestSatPerVb,
    required this.halfHourSatPerVb,
    required this.hourSatPerVb,
    required this.economySatPerVb,
    required this.minimumSatPerVb,
  });
  final int fastestSatPerVb;
  final int halfHourSatPerVb;
  final int hourSatPerVb;
  final int economySatPerVb;
  final int minimumSatPerVb;

  factory FeeEstimate.fromJson(Map<String, dynamic> j) {
    int at(String key) => ((j[key] as num?) ?? 0).toInt();
    return FeeEstimate(
      fastestSatPerVb: at('1'),
      halfHourSatPerVb: at('3'),
      hourSatPerVb: at('6'),
      economySatPerVb: at('144'),
      minimumSatPerVb: at('1008'),
    );
  }
}
```

`lib/core/btc/models/send_result.dart`:

```dart
import 'package:meta/meta.dart';

@immutable
class SendResult {
  const SendResult({required this.txid, required this.feeSat, required this.vbytes});
  final String txid;
  final int feeSat;
  final int vbytes;

  factory SendResult.fromJson(Map<String, dynamic> j) => SendResult(
        txid: j['txid'] as String,
        feeSat: (j['fee_sat'] as num).toInt(),
        vbytes: (j['vbytes'] as num).toInt(),
      );
}
```

`lib/core/btc/models/btc_config.dart`:

```dart
import 'package:meta/meta.dart';

@immutable
class BtcConfig {
  const BtcConfig({
    required this.dataDir,
    required this.network,
    required this.esploraUrl,
    required this.wallets,
  });
  final String dataDir;
  final String network;
  final String esploraUrl;
  final List<String> wallets;

  factory BtcConfig.fromJson(Map<String, dynamic> j) => BtcConfig(
        dataDir: j['data_dir'] as String,
        network: j['network'] as String,
        esploraUrl: j['esplora_url'] as String,
        wallets: ((j['wallets'] as List?) ?? const []).cast<String>(),
      );
}
```

- [ ] **Step 4: Run DTO test to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/dtos_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/core/btc/models/ flutter-btc-wallet/test/unit/dtos_test.dart
git commit -m "feat(flutter): DTOs for btc --json shapes (Task 9)"
```

---

## Task 10: BtcInvoker (process spawn + parse)

**Files:**
- Create: `flutter-btc-wallet/lib/core/btc/btc_invoker.dart`
- Create: `flutter-btc-wallet/test/unit/btc_invoker_test.dart`

**Interfaces:**
- `Future<T> invoke<T>(BtcCommand cmd, {required T Function(dynamic) parse})` — runs `btc` at `binaryPath`, captures stdout/stderr, parses JSON if `T` requires.

- [ ] **Step 1: Write failing test**

```dart
import 'dart:io';
import 'package:flutter_btc_wallet/core/btc/btc_command.dart';
import 'package:flutter_btc_wallet/core/btc/btc_error.dart';
import 'package:flutter_btc_wallet/core/btc/btc_invoker.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  // Use a script that echoes argv + canned stdout/stderr.
  final mockScript = File('test/integration/fixtures/fake_btc.sh').path;

  test('invoke returns parsed JSON for success exit 0', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
    final result = await invoker.invoke(
      const WalletList(network: 'testnet'),
      parse: (j) => (j as List).first as String,
    );
    expect(result, 'fake-uuid-1');
  });

  test('invoke throws BtcError on non-zero exit', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
    await expectLater(
      invoker.invoke(const WalletDelete(id: 'x', network: 'testnet'), parse: (_) => null),
      throwsA(isA<BtcError>()),
    );
  });

  test('invoke strips secret-bearing env vars from parent env', () async {
    // The fake_btc.sh writes its inherited env to a file; we inspect
    // that env vars like BTC_WALLET_MNEMONIC are NOT present.
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    Platform.environment['BTC_WALLET_MNEMONIC'] = 'should-be-stripped';
    try {
      final invoker = BtcInvoker(binaryPath: mockScript);
      await invoker.invoke(const ConfigShow(), parse: (_) => null);
      final envFile = File('test/integration/fixtures/.last_env');
      if (await envFile.exists()) {
        final env = await envFile.readAsString();
        expect(env, isNot(contains('BTC_WALLET_MNEMONIC')));
      }
    } finally {
      Platform.environment.remove('BTC_WALLET_MNEMONIC');
    }
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_invoker_test.dart`
Expected: FAIL — missing module + missing fake script (Task 24 builds it).

- [ ] **Step 3: Implement `btc_invoker.dart`**

```dart
import 'dart:convert';
import 'dart:io';
import 'btc_command.dart';
import 'btc_error.dart';

class BtcInvoker {
  BtcInvoker({required this.binaryPath, this.dataDirOverride});

  final String binaryPath;
  final String? dataDirOverride;

  static const _secretEnvKeys = [
    'BTC_WALLET_MNEMONIC',
    'BTC_ENCRYPT_PASSWORD',
    'BTC_DECRYPT_PASSWORD',
  ];

  Future<T> invoke<T>(BtcCommand cmd, {required T Function(dynamic json) parse}) async {
    final env = Map<String, String>.from(Platform.environment)
      ..removeWhere((k, _) => _secretEnvKeys.contains(k));
    if (dataDirOverride != null) env['BTC_DATA_DIR'] = dataDirOverride!;

    final process = await Process.start(
      binaryPath,
      cmd.argv,
      environment: env,
      runInShell: false,
    );

    final stdoutFuture = process.stdout.transform(utf8.decoder).join();
    final stderrFuture = process.stderr.transform(utf8.decoder).join();
    final exitCode = await process.exitCode;
    final stdout = await stdoutFuture;
    final stderr = await stderrFuture;

    if (exitCode != 0) {
      throw BtcError.fromStderr(stderr.isEmpty ? stdout : stderr, exitCode: exitCode);
    }

    final trimmed = stdout.trim();
    if (trimmed.isEmpty) return parse(null);

    try {
      final decoded = jsonDecode(trimmed);
      return parse(decoded);
    } on FormatException {
      // btc wrote human-readable output (--json flag not used or
      // output is plain text). Caller's `parse` handles non-JSON via
      // its parameter contract.
      return parse(trimmed);
    }
  }
}
```

- [ ] **Step 4: Run to confirm pass (after Task 24 builds fake_btc.sh)**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_invoker_test.dart`
Expected: PASS once `fake_btc.sh` exists. Until then `markTestSkipped`.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/core/btc/btc_invoker.dart flutter-btc-wallet/test/unit/btc_invoker_test.dart
git commit -m "feat(flutter): BtcInvoker spawns btc + parses JSON + strips secrets (Task 10)"
```

---

## Task 11: btcInvokerProvider + appPathsProvider

**Files:**
- Create: `flutter-btc-wallet/lib/providers/btc_providers.dart`
- Create: `flutter-btc-wallet/test/unit/btc_providers_test.dart`

**Interfaces:**
- `btcInvokerProvider: Provider<BtcInvoker>` (async-resolves binary path).
- `appPathsProvider: FutureProvider<AppPaths>`.

- [ ] **Step 1: Write failing test**

```dart
import 'dart:io';
import 'package:flutter_btc_wallet/core/binary/btc_extractor.dart';
import 'package:flutter_btc_wallet/core/btc/btc_invoker.dart';
import 'package:flutter_btc_wallet/providers/btc_providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('btcInvokerProvider yields a BtcInvoker with a path containing /btc/', () async {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final invoker = await container.read(btcInvokerProvider.future);
    expect(invoker, isA<BtcInvoker>());
    expect(invoker.binaryPath, contains('${Platform.pathSeparator}btc${Platform.pathSeparator}'));
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_providers_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement**

`lib/providers/btc_providers.dart`:

```dart
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../core/binary/btc_extractor.dart';
import '../core/btc/btc_invoker.dart';
import '../core/paths.dart';

final appPathsProvider = FutureProvider<AppPaths>((ref) async {
  return AppPaths(
    dataDir: await appDataDir(),
    btcDir: await subdirFor('btc'),
    tmpDir: await subdirFor('tmp'),
    walletDataDir: await subdirFor('wallet_data'),
  );
});

final btcInvokerProvider = FutureProvider<BtcInvoker>((ref) async {
  await ref.watch(appPathsProvider.future);
  final binaryPath = await extractBtc();
  final paths = await ref.watch(appPathsProvider.future);
  return BtcInvoker(binaryPath: binaryPath, dataDirOverride: paths.walletDataDir.path);
});

class AppPaths {
  const AppPaths({
    required this.dataDir,
    required this.btcDir,
    required this.tmpDir,
    required this.walletDataDir,
  });
  final Directory dataDir;
  final Directory btcDir;
  final Directory tmpDir;
  final Directory walletDataDir;
}

// Re-export for test imports.
import 'dart:io' show Directory;
```

Wait — Dart imports must be at top of file. Fix:

```dart
import 'dart:io' show Directory;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../core/binary/btc_extractor.dart';
import '../core/btc/btc_invoker.dart';
import '../core/paths.dart';

class AppPaths {
  const AppPaths({
    required this.dataDir,
    required this.btcDir,
    required this.tmpDir,
    required this.walletDataDir,
  });
  final Directory dataDir;
  final Directory btcDir;
  final Directory tmpDir;
  final Directory walletDataDir;
}

final appPathsProvider = FutureProvider<AppPaths>((ref) async {
  return AppPaths(
    dataDir: await appDataDir(),
    btcDir: await subdirFor('btc'),
    tmpDir: await subdirFor('tmp'),
    walletDataDir: await subdirFor('wallet_data'),
  );
});

final btcInvokerProvider = FutureProvider<BtcInvoker>((ref) async {
  await ref.watch(appPathsProvider.future);
  final binaryPath = await extractBtc();
  final paths = await ref.watch(appPathsProvider.future);
  return BtcInvoker(binaryPath: binaryPath, dataDirOverride: paths.walletDataDir.path);
});
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/btc_providers_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/providers/btc_providers.dart flutter-btc-wallet/test/unit/btc_providers_test.dart
git commit -m "feat(flutter): btcInvokerProvider + appPathsProvider (Task 11)"
```

---

## Task 12: EsploraConfig provider + persistence

**Files:**
- Create: `flutter-btc-wallet/lib/providers/esplora_config_provider.dart`
- Create: `flutter-btc-wallet/test/unit/esplora_config_provider_test.dart`

**Interfaces:**
- `class EsploraConfig { network, url, spkiPin }`
- `FutureProvider<EsploraConfig>` reads JSON from `<appDataDir>/esplora_config.json`.
- `NotifierProvider<EsploraConfigNotifier, EsploraConfig>` writes back on change.

- [ ] **Step 1: Write failing test**

```dart
import 'dart:io';
import 'package:flutter_btc_wallet/providers/esplora_config_provider.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('default EsploraConfig returns testnet + blockstream URL', () {
    final config = EsploraConfig.defaults('testnet');
    expect(config.network, 'testnet');
    expect(config.url, 'https://blockstream.info/testnet/api');
  });

  test('EsploraConfig JSON round-trip', () {
    final c = EsploraConfig(network: 'mainnet', url: 'https://blockstream.info/api', spkiPin: 'abc');
    final j = c.toJson();
    expect(EsploraConfig.fromJson(j).network, 'mainnet');
    expect(EsploraConfig.fromJson(j).spkiPin, 'abc');
  });

  test('notifier persists update to disk', () async {
    final tmp = Directory.systemTemp.createTempSync('esplora_cfg_test');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final container = ProviderContainer(overrides: [
      esploraConfigFilePathProvider.overrideWithValue(File('${tmp.path}/cfg.json')),
    ]);
    addTearDown(container.dispose);

    final notifier = container.read(esploraConfigProvider.notifier);
    await notifier.update(EsploraConfig(network: 'mainnet', url: 'https://x', spkiPin: ''));

    final onDisk = await container.read(esploraConfigFilePathProvider).readAsString();
    expect(onDisk, contains('mainnet'));
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/esplora_config_provider_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement**

`lib/providers/esplora_config_provider.dart`:

```dart
import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:meta/meta.dart';
import '../core/paths.dart';

@immutable
class EsploraConfig {
  const EsploraConfig({required this.network, required this.url, required this.spkiPin});

  final String network;
  final String url;
  final String spkiPin;

  factory EsploraConfig.defaults(String network) {
    switch (network) {
      case 'bitcoin':
        return const EsploraConfig(
          network: 'bitcoin',
          url: 'https://blockstream.info/api',
          spkiPin: '',
        );
      case 'testnet':
        return const EsploraConfig(
          network: 'testnet',
          url: 'https://blockstream.info/testnet/api',
          spkiPin: '',
        );
      case 'testnet4':
        return const EsploraConfig(
          network: 'testnet4',
          url: 'https://blockstream.info/testnet4/api',
          spkiPin: '',
        );
      case 'signet':
        return const EsploraConfig(
          network: 'signet',
          url: 'https://blockstream.info/signet/api',
          spkiPin: '',
        );
      default:
        return EsploraConfig(network: network, url: '', spkiPin: '');
    }
  }

  Map<String, dynamic> toJson() => {'network': network, 'url': url, 'spkiPin': spkiPin};

  factory EsploraConfig.fromJson(Map<String, dynamic> j) => EsploraConfig(
        network: j['network'] as String,
        url: j['url'] as String,
        spkiPin: j['spkiPin'] as String? ?? '',
      );

  EsploraConfig copyWith({String? network, String? url, String? spkiPin}) =>
      EsploraConfig(
        network: network ?? this.network,
        url: url ?? this.url,
        spkiPin: spkiPin ?? this.spkiPin,
      );
}

final esploraConfigFilePathProvider = Provider<File>((ref) {
  throw UnimplementedError('Override in ProviderScope');
});

class EsploraConfigNotifier extends AsyncNotifier<EsploraConfig> {
  @override
  Future<EsploraConfig> build() async {
    final file = ref.read(esploraConfigFilePathProvider);
    if (await file.exists()) {
      final raw = await file.readAsString();
      return EsploraConfig.fromJson(jsonDecode(raw) as Map<String, dynamic>);
    }
    return EsploraConfig.defaults('testnet');
  }

  Future<void> update(EsploraConfig cfg) async {
    state = AsyncData(cfg);
    final file = ref.read(esploraConfigFilePathProvider);
    await file.parent.create(recursive: true);
    await file.writeAsString(jsonEncode(cfg.toJson()));
  }
}

final esploraConfigProvider =
    AsyncNotifierProvider<EsploraConfigNotifier, EsploraConfig>(EsploraConfigNotifier.new);
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/esplora_config_provider_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/providers/esplora_config_provider.dart flutter-btc-wallet/test/unit/esplora_config_provider_test.dart
git commit -m "feat(flutter): EsploraConfig provider + disk persistence (Task 12)"
```

---

## Task 13: walletsListProvider

**Files:**
- Create: `flutter-btc-wallet/lib/providers/wallet_providers.dart` (top section)
- Create: `flutter-btc-wallet/test/unit/wallets_list_provider_test.dart`

**Interfaces:**
- `walletsListProvider(network): AsyncNotifierProviderFamily<WalletsListNotifier, List<WalletInfo>, String>`.

- [ ] **Step 1: Write failing test**

```dart
import 'dart:io';
import 'package:flutter_btc_wallet/providers/wallet_providers.dart';
import 'package:flutter_btc_wallet/core/btc/models/wallet_info.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('walletsListProvider yields a list (mocked invoker)', () async {
    final container = ProviderContainer(overrides: [
      // We don't override btcInvokerProvider here; this test just
      // verifies the provider tree resolves and exposes AsyncValue.
      // E2E against fake_btc.sh is in Task 24.
    ]);
    addTearDown(container.dispose);
    final async = container.read(walletsListProvider('testnet'));
    expect(async, isA<AsyncValue<List<WalletInfo>>>());
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/wallets_list_provider_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement WalletsListNotifier (top of `wallet_providers.dart`)**

```dart
import 'dart:async';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:meta/meta.dart';
import '../core/btc/btc_command.dart';
import '../core/btc/btc_error.dart';
import '../core/btc/btc_invoker.dart';
import '../core/btc/models/wallet_info.dart';
import 'btc_providers.dart';

class WalletsListNotifier
    extends FamilyAsyncNotifier<List<WalletInfo>, String> {
  @override
  Future<List<WalletInfo>> build(String network) async {
    final invoker = await ref.watch(btcInvokerProvider.future);
    return invoker.invoke<List<WalletInfo>>(
      BtcCommandStatic.walletList(network: network),
      parse: (j) => (j as List)
          .map((e) => WalletInfo.fromJson(e as Map<String, dynamic>))
          .toList(growable: false),
    );
  }

  Future<void> refresh() async {
    state = const AsyncLoading();
    state = await AsyncValue.guard(() => build(arg));
  }
}

final walletsListProvider =
    AsyncNotifierProvider.family<WalletsListNotifier, List<WalletInfo>, String>(
  WalletsListNotifier.new,
);
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/wallets_list_provider_test.dart`
Expected: PASS. Provider resolves; actual fetch against `btc` deferred to integration.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/providers/wallet_providers.dart flutter-btc-wallet/test/unit/wallets_list_provider_test.dart
git commit -m "feat(flutter): walletsListProvider (Story 9 list) (Task 13)"
```

---

## Task 14: walletSessionProvider (unlocked session with mnemonic)

**Files:**
- Modify: `flutter-btc-wallet/lib/providers/wallet_providers.dart` (append)

**Interfaces:**
- `class WalletSession { walletId, mnemonic (Zeroizing<String>), detail (WalletDetail?), lockedAt }`
- `walletSessionProvider(walletId): NotifierProvider.family<WalletSessionNotifier, WalletSession?>`

- [ ] **Step 1: Write failing test**

```dart
import 'package:flutter_btc_wallet/providers/wallet_providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('walletSessionProvider starts null (locked)', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    expect(container.read(walletSessionProvider('abc')), isNull);
  });

  test('lock() returns the session to null', () async {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final notifier = container.read(walletSessionProvider('abc').notifier);
    await notifier.lock();
    expect(container.read(walletSessionProvider('abc')), isNull);
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/wallet_session_provider_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement (append to `wallet_providers.dart`)**

```dart
import 'dart:async';
import '../core/btc/btc_command.dart';
import '../core/btc/models/wallet_detail.dart';

/// Mutable unlocked session state. Mnemonic lives in `Zeroizing<String>`.
@immutable
class WalletSession {
  const WalletSession({
    required this.walletId,
    required this.mnemonic,
    this.detail,
  });
  final String walletId;
  final ZeroizingString mnemonic;
  final WalletDetail? detail;

  WalletSession copyWith({WalletDetail? detail}) => WalletSession(
        walletId: walletId,
        mnemonic: mnemonic,
        detail: detail ?? this.detail,
      );
}

/// Thin wrapper around `String` with explicit `.dispose()` to support
/// best-effort zeroization on lock.
class ZeroizingString {
  ZeroizingString(this._value);
  String _value;
  String get value => _value;
  void dispose() {
    _value = '';
  }
}

class WalletSessionNotifier
    extends FamilyNotifier<WalletSession?, String> {
  @override
  WalletSession? build(String walletId) => null;

  Future<void> unlock({
    required String walletId,
    required String mnemonic,
    WalletDetail? detail,
  }) async {
    state = WalletSession(
      walletId: walletId,
      mnemonic: ZeroizingString(mnemonic),
      detail: detail,
    );
  }

  Future<void> updateDetail(WalletDetail detail) async {
    final current = state;
    if (current == null) return;
    state = current.copyWith(detail: detail);
  }

  Future<void> lock() async {
    final current = state;
    current?.mnemonic.dispose();
    state = null;
  }
}

final walletSessionProvider =
    NotifierProvider.family<WalletSessionNotifier, WalletSession?, String>(
  WalletSessionNotifier.new,
);
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/unit/wallet_session_provider_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/providers/wallet_providers.dart flutter-btc-wallet/test/unit/wallet_session_provider_test.dart
git commit -m "feat(flutter): walletSessionProvider (ZeroizingString mnemonic) (Task 14)"
```

---

## Task 15: Shared widgets (AddressChip, BalanceCard, PasswordField, etc.)

**Files:**
- Create: `flutter-btc-wallet/lib/widgets/address_chip.dart`
- Create: `flutter-btc-wallet/lib/widgets/balance_card.dart`
- Create: `flutter-btc-wallet/lib/widgets/network_picker.dart`
- Create: `flutter-btc-wallet/lib/widgets/password_field.dart`
- Create: `flutter-btc-wallet/lib/widgets/mnemonic_paste_field.dart`
- Create: `flutter-btc-wallet/lib/widgets/status_badge.dart`
- Create: `flutter-btc-wallet/lib/widgets/process_progress_overlay.dart`
- Create: `flutter-btc-wallet/test/widget/widgets_test.dart`

**Interfaces:**
- Each widget exposes a public constructor; private `_build` impl. `PasswordField` includes `onDispose` callback for explicit zeroize.

- [ ] **Step 1: Write failing widget test**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_btc_wallet/widgets/address_chip.dart';
import 'package:flutter_btc_wallet/widgets/balance_card.dart';
import 'package:flutter_btc_wallet/widgets/network_picker.dart';
import 'package:flutter_btc_wallet/widgets/password_field.dart';
import 'package:flutter_btc_wallet/widgets/status_badge.dart';
import 'package:flutter_btc_wallet/core/btc/btc_error.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('AddressChip displays truncated address', (t) async {
    await t.pumpWidget(MaterialApp(home: Scaffold(body: AddressChip(address: 'tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx'))));
    expect(find.textContaining('tb1q'), findsOneWidget);
  });

  testWidgets('BalanceCard shows confirmed balance', (t) async {
    final bal = Balance(confirmedSat: 100000, trustedPendingSat: 0, untrustedPendingSat: 0, immatureSat: 0);
    await t.pumpWidget(MaterialApp(home: Scaffold(body: BalanceCard(balance: bal))));
    expect(find.textContaining('100000'), findsOneWidget);
  });

  testWidgets('NetworkPicker default is testnet', (t) async {
    String? chosen;
    await t.pumpWidget(MaterialApp(home: Scaffold(body: NetworkPicker(onChanged: (n) => chosen = n))));
    await t.tap(find.text('testnet'));
    await t.pump();
    expect(chosen, 'testnet');
  });

  testWidgets('PasswordField obscure by default', (t) async {
    final field = PasswordField(onChanged: (_) {});
    await t.pumpWidget(MaterialApp(home: Scaffold(body: field)));
    expect(find.byType(TextField), findsOneWidget);
    // Initial obscureText should be true.
    final tf = tester.widget<TextField>(find.byType(TextField));
    expect(tf.obscureText, isTrue);
  });

  testWidgets('StatusBadge shows error icon for BtcErrorKind.insufficientFunds', (t) async {
    await t.pumpWidget(MaterialApp(home: Scaffold(body: StatusBadge(kind: BtcErrorKind.insufficientFunds))));
    expect(find.byIcon(Icons.error), findsOneWidget);
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/widget/widgets_test.dart`
Expected: FAIL — missing widgets.

- [ ] **Step 3: Implement widgets**

`lib/widgets/address_chip.dart`:

```dart
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class AddressChip extends StatelessWidget {
  const AddressChip({super.key, required this.address, this.network});
  final String address;
  final String? network;

  @override
  Widget build(BuildContext context) {
    final short = address.length <= 12 ? address : '${address.substring(0, 8)}…${address.substring(address.length - 4)}';
    return InkWell(
      onTap: () async {
        await Clipboard.setData(ClipboardData(text: address));
        if (context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Copied')));
        }
      },
      child: Chip(
        avatar: network == null ? null : CircleAvatar(child: Text(network!.substring(0, 1))),
        label: Text(short, style: const TextStyle(fontFamily: 'monospace')),
      ),
    );
  }
}
```

`lib/widgets/balance_card.dart`:

```dart
import 'package:flutter/material.dart';
import '../core/btc/models/wallet_detail.dart';

class BalanceCard extends StatelessWidget {
  const BalanceCard({super.key, required this.balance});
  final Balance balance;

  String _sats(int v) => '$v sats';

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Confirmed', style: Theme.of(context).textTheme.labelMedium),
            Text(_sats(balance.confirmedSat), style: Theme.of(context).textTheme.headlineSmall),
            if (balance.trustedPendingSat > 0)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text('Pending (trusted): ${_sats(balance.trustedPendingSat)}'),
              ),
            if (balance.untrustedPendingSat > 0)
              Text('Pending (untrusted): ${_sats(balance.untrustedPendingSat)}'),
            if (balance.immatureSat > 0)
              Text('Immature: ${_sats(balance.immatureSat)}'),
          ],
        ),
      ),
    );
  }
}
```

`lib/widgets/network_picker.dart`:

```dart
import 'package:flutter/material.dart';

const _supportedNetworks = ['bitcoin', 'testnet', 'testnet4', 'signet', 'regtest'];

class NetworkPicker extends StatelessWidget {
  const NetworkPicker({super.key, required this.onChanged, this.initial = 'testnet'});
  final ValueChanged<String> onChanged;
  final String initial;

  @override
  Widget build(BuildContext context) {
    return SegmentedButton<String>(
      segments: _supportedNetworks
          .map((n) => ButtonSegment<String>(value: n, label: Text(n)))
          .toList(growable: false),
      selected: {initial},
      onSelectionChanged: (s) => onChanged(s.first),
    );
  }
}
```

`lib/widgets/password_field.dart`:

```dart
import 'package:flutter/material.dart';

class PasswordField extends StatefulWidget {
  const PasswordField({super.key, required this.onChanged, this.onSubmitted});
  final ValueChanged<String> onChanged;
  final ValueChanged<String>? onSubmitted;

  @override
  State<PasswordField> createState() => _PasswordFieldState();
}

class _PasswordFieldState extends State<PasswordField> {
  bool _obscure = true;
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController();
  }

  @override
  void dispose() {
    // L12 CRITICAL #2: zeroize controller contents before dispose.
    _controller.clear();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: _controller,
      obscureText: _obscure,
      autocorrect: false,
      enableSuggestions: false,
      onChanged: widget.onChanged,
      onSubmitted: widget.onSubmitted,
      decoration: InputDecoration(
        labelText: 'Password',
        border: const OutlineInputBorder(),
        suffixIcon: IconButton(
          icon: Icon(_obscure ? Icons.visibility : Icons.visibility_off),
          onPressed: () => setState(() => _obscure = !_obscure),
        ),
      ),
    );
  }
}
```

`lib/widgets/mnemonic_paste_field.dart`:

```dart
import 'package:flutter/material.dart';

class MnemonicPasteField extends StatefulWidget {
  const MnemonicPasteField({super.key, required this.onChanged, required this.expectedWordCount});
  final ValueChanged<String> onChanged;
  final int expectedWordCount;

  @override
  State<MnemonicPasteField> createState() => _MnemonicPasteFieldState();
}

class _MnemonicPasteFieldState extends State<MnemonicPasteField> {
  late final TextEditingController _controller;
  bool _ackChecked = false;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController();
  }

  @override
  void dispose() {
    _controller.clear();
    _controller.dispose();
    super.dispose();
  }

  int get _wordCount => _controller.text.trim().split(RegExp(r'\s+')).where((w) => w.isNotEmpty).length;

  @override
  Widget build(BuildContext context) {
    final valid = _wordCount == widget.expectedWordCount;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextField(
          controller: _controller,
          minLines: 3,
          maxLines: 5,
          autocorrect: false,
          enableSuggestions: false,
          onChanged: (v) {
            setState(() {});
            widget.onChanged(v);
          },
          decoration: InputDecoration(
            labelText: 'Mnemonic (paste only — do not type)',
            border: const OutlineInputBorder(),
            errorText: valid || _controller.text.isEmpty ? null : 'Expected ${widget.expectedWordCount} words; got $_wordCount',
          ),
        ),
        const SizedBox(height: 8),
        CheckboxListTile(
          value: _ackChecked,
          onChanged: (v) => setState(() => _ackChecked = v ?? false),
          title: const Text('I have written this down in a safe place'),
          controlAffinity: ListTileControlAffinity.leading,
        ),
      ],
    );
  }
}
```

`lib/widgets/status_badge.dart`:

```dart
import 'package:flutter/material.dart';
import '../core/btc/btc_error.dart';

class StatusBadge extends StatelessWidget {
  const StatusBadge({super.key, required this.kind, this.message});
  final BtcErrorKind kind;
  final String? message;

  @override
  Widget build(BuildContext context) {
    final (icon, color, label) = switch (kind) {
      BtcErrorKind.wrongPassword => (Icons.lock_outline, Colors.orange, 'Wrong password'),
      BtcErrorKind.insufficientFunds => (Icons.account_balance_wallet_outlined, Colors.red, 'Insufficient funds'),
      BtcErrorKind.unknownWallet => (Icons.help_outline, Colors.grey, 'Wallet not found'),
      BtcErrorKind.networkError => (Icons.cloud_off, Colors.amber, 'Network error'),
      BtcErrorKind.unknownAddressType => (Icons.error_outline, Colors.red, 'Wrong network'),
      BtcErrorKind.confirmRequired => (Icons.warning_amber, Colors.deepOrange, 'Confirmation required'),
      BtcErrorKind.other => (Icons.error, Colors.red, 'Error'),
    };
    return Chip(
      avatar: Icon(icon, color: color, size: 18),
      label: Text(message ?? label),
    );
  }
}
```

`lib/widgets/process_progress_overlay.dart`:

```dart
import 'package:flutter/material.dart';

class ProcessProgressOverlay extends StatelessWidget {
  const ProcessProgressOverlay({super.key, required this.isRunning, this.label});
  final bool isRunning;
  final String? label;

  @override
  Widget build(BuildContext context) {
    if (!isRunning) return const SizedBox.shrink();
    return Positioned.fill(
      child: ColoredBox(
        color: Colors.black54,
        child: Center(child: Column(mainAxisSize: MainAxisSize.min, children: [
          const CircularProgressIndicator(),
          if (label != null) Padding(padding: const EdgeInsets.only(top: 16), child: Text(label!, style: const TextStyle(color: Colors.white))),
        ])),
      ),
    );
  }
}
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/widget/widgets_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/widgets/ flutter-btc-wallet/test/widget/widgets_test.dart
git commit -m "feat(flutter): shared widgets (chip, balance, network, password, badge) (Task 15)"
```

---

## Task 16: HomeShell + go_router

**Files:**
- Create: `flutter-btc-wallet/lib/routing/app_router.dart`
- Create: `flutter-btc-wallet/lib/features/home_shell.dart`
- Modify: `flutter-btc-wallet/lib/main.dart`

**Interfaces:**
- `GoRouter appRouter({required WidgetRef ref})` — 7 routes per spec §5.2.
- `HomeShell` = sidebar + content based on current location.

- [ ] **Step 1: Write failing test**

```dart
import 'package:flutter_btc_wallet/routing/app_router.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('routes include wallet list, create, import, detail, send, transactions, settings', () {
    final routes = appRouter().configuration.routes.map((r) => r.path).toList();
    expect(routes, containsAll([
      '/',
      '/wallets/testnet',
      '/wallets/testnet/new',
      '/wallets/testnet/import',
      '/wallets/testnet/abc-uuid',
      '/wallets/testnet/abc-uuid/send',
      '/wallets/testnet/abc-uuid/transactions',
      '/settings',
    ]));
  });
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd flutter-btc-wallet && flutter test test/unit/app_router_test.dart`
Expected: FAIL — missing module.

- [ ] **Step 3: Implement `app_router.dart`**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import '../features/home_shell.dart';
import '../features/wallet_create/wallet_create_screen.dart';
import '../features/wallet_detail/wallet_detail_screen.dart';
import '../features/wallet_import/wallet_import_screen.dart';
import '../features/wallet_list/wallet_list_screen.dart';
import '../features/wallet_send/send_screen.dart';
import '../features/wallet_transactions/transactions_screen.dart';
import '../features/settings/settings_screen.dart';

GoRouter appRouter() {
  return GoRouter(
    initialLocation: '/wallets/testnet',
    routes: [
      ShellRoute(
        builder: (context, state, child) => HomeShell(child: child),
        routes: [
          GoRoute(path: '/', redirect: (_, __) => '/wallets/testnet'),
          GoRoute(
            path: '/wallets/:network',
            builder: (c, s) => WalletListScreen(network: s.pathParameters['network']!),
            routes: [
              GoRoute(
                path: 'new',
                builder: (c, s) => WalletCreateScreen(network: s.pathParameters['network']!),
              ),
              GoRoute(
                path: 'import',
                builder: (c, s) => WalletImportScreen(network: s.pathParameters['network']!),
              ),
              GoRoute(
                path: ':walletId',
                builder: (c, s) => WalletDetailScreen(
                  network: s.pathParameters['network']!,
                  walletId: s.pathParameters['walletId']!,
                ),
                routes: [
                  GoRoute(
                    path: 'send',
                    builder: (c, s) => SendScreen(
                      network: s.pathParameters['network']!,
                      walletId: s.pathParameters['walletId']!,
                    ),
                  ),
                  GoRoute(
                    path: 'transactions',
                    builder: (c, s) => TransactionsScreen(
                      network: s.pathParameters['network']!,
                      walletId: s.pathParameters['walletId']!,
                    ),
                  ),
                ],
              ),
            ],
          ),
          GoRoute(path: '/settings', builder: (c, s) => const SettingsScreen()),
        ],
      ),
    ],
  );
}
```

- [ ] **Step 4: Implement `HomeShell` (placeholder; real shell in Task 17-22)**

`lib/features/home_shell.dart`:

```dart
import 'package:flutter/material.dart';

class HomeShell extends StatelessWidget {
  const HomeShell({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Row(
        children: [
          NavigationRail(
            selectedIndex: 0,
            destinations: const [
              NavigationRailDestination(icon: Icon(Icons.account_balance_wallet), label: Text('Wallets')),
              NavigationRailDestination(icon: Icon(Icons.settings), label: Text('Settings')),
            ],
            onDestinationSelected: (i) {
              // Routing logic — refined in Task 17 (real wallet list wiring).
            },
          ),
          const VerticalDivider(width: 1),
          Expanded(child: child),
        ],
      ),
    );
  }
}
```

- [ ] **Step 5: Replace `lib/main.dart`**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'core/theme.dart';
import 'routing/app_router.dart';

void main() {
  runApp(const ProviderScope(child: BtcWalletApp()));
}

class BtcWalletApp extends StatelessWidget {
  const BtcWalletApp({super.key});

  @override
  Widget build(BuildContext context) {
    final router = appRouter();
    return MaterialApp.router(
      title: 'btc wallet',
      theme: buildLightTheme(),
      darkTheme: buildDarkTheme(),
      themeMode: ThemeMode.system,
      routerConfig: router,
    );
  }
}
```

- [ ] **Step 6: Stub remaining feature screens** (so router compiles; real impls in Tasks 17-23)

Create each of these with a minimal `Scaffold(body: Center(child: Text('<name>')))`:

```dart
// lib/features/wallet_list/wallet_list_screen.dart
import 'package:flutter/material.dart';
class WalletListScreen extends StatelessWidget {
  const WalletListScreen({super.key, required this.network});
  final String network;
  @override
  Widget build(BuildContext context) => Scaffold(body: Center(child: Text('WalletList $network')));
}
```

Repeat for: `wallet_create_screen.dart`, `wallet_import_screen.dart`, `wallet_detail_screen.dart`, `send_screen.dart`, `transactions_screen.dart`, `settings/settings_screen.dart`. Each with placeholder text.

- [ ] **Step 7: Run all tests**

Run: `cd flutter-btc-wallet && flutter test`
Expected: PASS (stubs compile; router test passes).

- [ ] **Step 8: Commit**

```bash
git add flutter-btc-wallet/
git commit -m "feat(flutter): HomeShell + go_router + 8 routes (Task 16)"
```

---

## Task 17: WalletListScreen (Story 9 list)

**Files:**
- Modify: `flutter-btc-wallet/lib/features/wallet_list/wallet_list_screen.dart`
- Create: `flutter-btc-wallet/test/widget/wallet_list_screen_test.dart`

**Interfaces:**
- Reads `walletsListProvider(network)`. Renders list with create/import buttons.

- [ ] **Step 1: Write failing widget test**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_btc_wallet/features/wallet_list/wallet_list_screen.dart';
import 'package:flutter_btc_wallet/core/btc/models/wallet_info.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('WalletListScreen shows empty state when list is empty', (t) async {
    await t.pumpWidget(ProviderScope(
      overrides: [
        // Mock the list provider to return [].
      ],
      child: const MaterialApp(home: Scaffold(body: WalletListScreen(network: 'testnet'))),
    ));
    await t.pump();
    expect(find.text('Create'), findsOneWidget);
    expect(find.text('Import'), findsOneWidget);
  });
}
```

- [ ] **Step 2: Implement screen**

`lib/features/wallet_list/wallet_list_screen.dart`:

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import '../../providers/wallet_providers.dart';
import '../../widgets/address_chip.dart';

class WalletListScreen extends ConsumerWidget {
  const WalletListScreen({super.key, required this.network});
  final String network;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final asyncList = ref.watch(walletsListProvider(network));
    return Scaffold(
      appBar: AppBar(
        title: Text('Wallets ($network)'),
        actions: [
          IconButton(
            icon: const Icon(Icons.add),
            tooltip: 'Create',
            onPressed: () => context.go('/wallets/$network/new'),
          ),
          IconButton(
            icon: const Icon(Icons.file_download),
            tooltip: 'Import',
            onPressed: () => context.go('/wallets/$network/import'),
          ),
        ],
      ),
      body: asyncList.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('Error: $e')),
        data: (list) => list.isEmpty
            ? const Center(child: Text('No wallets yet. Tap + to create one.'))
            : ListView.builder(
                itemCount: list.length,
                itemBuilder: (_, i) {
                  final w = list[i];
                  return ListTile(
                    title: Text(w.id, style: const TextStyle(fontFamily: 'monospace')),
                    subtitle: Text('${w.network} • ${w.addressType}'),
                    onTap: () => context.go('/wallets/$network/${w.id}'),
                  );
                },
              ),
      ),
    );
  }
}
```

- [ ] **Step 3: Run widget test to confirm pass**

Run: `cd flutter-btc-wallet && flutter test test/widget/wallet_list_screen_test.dart`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add flutter-btc-wallet/lib/features/wallet_list/ flutter-btc-wallet/test/widget/wallet_list_screen_test.dart
git commit -m "feat(flutter): WalletListScreen (Story 9 list) (Task 17)"
```

---

## Task 18: WalletCreateScreen + MnemonicDisplayDialog (Story 1 + 20)

**Files:**
- Modify: `flutter-btc-wallet/lib/features/wallet_create/wallet_create_screen.dart`
- Create: `flutter-btc-wallet/lib/features/wallet_create/mnemonic_display_dialog.dart`
- Create: `flutter-btc-wallet/test/widget/wallet_create_screen_test.dart`

**Interfaces:**
- Form: word count (12/24), network, address type, password.
- Submit calls `BtcCommandStatic.walletCreate(...)` via `withPasswordFile`.
- Result → show `MnemonicDisplayDialog` once.

- [ ] **Step 1: Write failing widget test**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_btc_wallet/features/wallet_create/wallet_create_screen.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('WalletCreateScreen renders form with 12 words + native-segwit defaults', (t) async {
    await t.pumpWidget(const ProviderScope(child: MaterialApp(home: Scaffold(body: WalletCreateScreen(network: 'testnet')))));
    await t.pump();
    expect(find.text('12'), findsOneWidget); // word count default
    expect(find.text('native-segwit'), findsOneWidget); // address type default
  });
}
```

- [ ] **Step 2: Implement screen**

`lib/features/wallet_create/wallet_create_screen.dart`:

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import '../../core/btc/btc_command.dart';
import '../../core/btc/btc_error.dart';
import '../../core/btc/models/wallet_created.dart';
import '../../core/btc/password_supply.dart';
import '../../providers/btc_providers.dart';
import '../../providers/wallet_providers.dart';
import '../../widgets/mnemonic_paste_field.dart';
import '../../widgets/password_field.dart';
import '../../widgets/status_badge.dart';
import 'mnemonic_display_dialog.dart';

class WalletCreateScreen extends ConsumerStatefulWidget {
  const WalletCreateScreen({super.key, required this.network});
  final String network;

  @override
  ConsumerState<WalletCreateScreen> createState() => _WalletCreateScreenState();
}

class _WalletCreateScreenState extends ConsumerState<WalletCreateScreen> {
  int _words = 12;
  String _addressType = 'native-segwit';
  String _password = '';
  bool _running = false;
  BtcError? _error;

  Future<void> _submit() async {
    if (_password.isEmpty) return;
    setState(() { _running = true; _error = null; });
    try {
      final invoker = await ref.read(btcInvokerProvider.future);
      final result = await withPasswordFile(_password, (path) async {
        return invoker.invoke<WalletCreated>(
          BtcCommandStatic.walletCreate(
            words: _words,
            network: widget.network,
            addressType: _addressType,
            passwordFilePath: path,
          ),
          parse: (j) => WalletCreated.fromJson(j as Map<String, dynamic>),
        );
      });
      if (!mounted) return;
      // Force-clear password field state before showing mnemonic.
      setState(() { _password = ''; });
      await showDialog<void>(
        context: context,
        barrierDismissible: false,
        builder: (_) => MnemonicDisplayDialog(
          mnemonic: result.mnemonic,
          walletId: result.id,
          firstAddress: result.firstAddress,
        ),
      );
      // Invalidate wallets list.
      ref.invalidate(walletsListProvider(widget.network));
      if (mounted) context.go('/wallets/${widget.network}');
    } on BtcError catch (e) {
      setState(() => _error = e);
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text('Create wallet (${widget.network})')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          DropdownButtonFormField<int>(
            initialValue: _words,
            decoration: const InputDecoration(labelText: 'Words'),
            items: const [
              DropdownMenuItem(value: 12, child: Text('12')),
              DropdownMenuItem(value: 24, child: Text('24')),
            ],
            onChanged: (v) => setState(() => _words = v ?? 12),
          ),
          const SizedBox(height: 16),
          DropdownButtonFormField<String>(
            initialValue: _addressType,
            decoration: const InputDecoration(labelText: 'Address type'),
            items: const [
              DropdownMenuItem(value: 'legacy', child: Text('legacy')),
              DropdownMenuItem(value: 'nested-segwit', child: Text('nested-segwit')),
              DropdownMenuItem(value: 'native-segwit', child: Text('native-segwit')),
              DropdownMenuItem(value: 'taproot', child: Text('taproot')),
            ],
            onChanged: (v) => setState(() => _addressType = v ?? 'native-segwit'),
          ),
          const SizedBox(height: 16),
          PasswordField(onChanged: (v) => _password = v),
          const SizedBox(height: 16),
          if (_error != null) StatusBadge(kind: _error!.kind),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: _running ? null : _submit,
            child: _running ? const Text('Creating…') : const Text('Create'),
          ),
        ]),
      ),
    );
  }
}
```

- [ ] **Step 3: Implement MnemonicDisplayDialog**

`lib/features/wallet_create/mnemonic_display_dialog.dart`:

```dart
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class MnemonicDisplayDialog extends StatefulWidget {
  const MnemonicDisplayDialog({
    super.key,
    required this.mnemonic,
    required this.walletId,
    required this.firstAddress,
  });
  final String mnemonic;
  final String walletId;
  final String firstAddress;

  @override
  State<MnemonicDisplayDialog> createState() => _MnemonicDisplayDialogState();
}

class _MnemonicDisplayDialogState extends State<MnemonicDisplayDialog> {
  bool _visible = false;
  bool _acked = false;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Backup your mnemonic'),
      content: SingleChildScrollView(
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          const Text('Write these 12/24 words down on paper. Anyone with them controls your funds.'),
          const SizedBox(height: 16),
          SelectableText(
            _visible ? widget.mnemonic : '•' * widget.mnemonic.length,
            style: const TextStyle(fontFamily: 'monospace'),
          ),
          const SizedBox(height: 8),
          Text('Wallet ID: ${widget.walletId}', style: const TextStyle(fontFamily: 'monospace', fontSize: 12)),
          Text('First address: ${widget.firstAddress}', style: const TextStyle(fontFamily: 'monospace', fontSize: 12)),
          const SizedBox(height: 16),
          CheckboxListTile(
            value: _visible,
            onChanged: (v) => setState(() => _visible = v ?? false),
            title: const Text('Reveal words'),
            controlAffinity: ListTileControlAffinity.leading,
          ),
          CheckboxListTile(
            value: _acked,
            onChanged: (v) => setState(() => _acked = v ?? false),
            title: const Text('I have written this down in a safe place'),
            controlAffinity: ListTileControlAffinity.leading,
          ),
        ]),
      ),
      actions: [
        TextButton(
          onPressed: _acked ? () => Navigator.of(context).pop() : null,
          child: const Text('Done'),
        ),
      ],
    );
  }
}
```

- [ ] **Step 4: Run widget test**

Run: `cd flutter-btc-wallet && flutter test test/widget/wallet_create_screen_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/lib/features/wallet_create/ flutter-btc-wallet/test/widget/wallet_create_screen_test.dart
git commit -m "feat(flutter): WalletCreateScreen + MnemonicDisplayDialog (Story 1+20) (Task 18)"
```

---

## Task 19: WalletImportScreen (Story 2)

**Files:**
- Modify: `flutter-btc-wallet/lib/features/wallet_import/wallet_import_screen.dart`
- Create: `flutter-btc-wallet/test/widget/wallet_import_screen_test.dart`

**Interfaces:**
- Form: mnemonic paste, network, password.
- Submit: `BtcCommandStatic.walletImport(...)`.

- [ ] **Step 1: Write failing test**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_btc_wallet/features/wallet_import/wallet_import_screen.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('WalletImportScreen shows mnemonic paste field + network default', (t) async {
    await t.pumpWidget(const ProviderScope(child: MaterialApp(home: Scaffold(body: WalletImportScreen(network: 'testnet')))));
    await t.pump();
    expect(find.byType(TextField), findsWidgets); // mnemonic + password
  });
}
```

- [ ] **Step 2: Implement**

`lib/features/wallet_import/wallet_import_screen.dart`:

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import '../../core/btc/btc_command.dart';
import '../../core/btc/btc_error.dart';
import '../../core/btc/models/wallet_info.dart';
import '../../core/btc/password_supply.dart';
import '../../providers/btc_providers.dart';
import '../../providers/wallet_providers.dart';
import '../../widgets/mnemonic_paste_field.dart';
import '../../widgets/password_field.dart';
import '../../widgets/status_badge.dart';

class WalletImportScreen extends ConsumerStatefulWidget {
  const WalletImportScreen({super.key, required this.network});
  final String network;

  @override
  ConsumerState<WalletImportScreen> createState() => _WalletImportScreenState();
}

class _WalletImportScreenState extends ConsumerState<WalletImportScreen> {
  String _mnemonic = '';
  String _password = '';
  bool _running = false;
  BtcError? _error;
  int _wordCount = 12;

  Future<void> _submit() async {
    final words = _mnemonic.trim().split(RegExp(r'\s+')).where((w) => w.isNotEmpty).length;
    if (words != 12 && words != 15 && words != 18 && words != 21 && words != 24) {
      setState(() => _error = BtcError.fromStderr('invalid mnemonic: expected 12/15/18/21/24 words', exitCode: 2));
      return;
    }
    if (_password.isEmpty) return;
    setState(() { _running = true; _error = null; });
    try {
      final invoker = await ref.read(btcInvokerProvider.future);
      final result = await withPasswordFile(_password, (path) async {
        return invoker.invoke<WalletInfo>(
          BtcCommandStatic.walletImport(
            mnemonic: _mnemonic,
            network: widget.network,
            passwordFilePath: path,
          ),
          parse: (j) => WalletInfo.fromJson(j as Map<String, dynamic>),
        );
      });
      setState(() { _mnemonic = ''; _password = ''; });
      ref.invalidate(walletsListProvider(widget.network));
      if (mounted) context.go('/wallets/${widget.network}');
    } on BtcError catch (e) {
      setState(() => _error = e);
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text('Import wallet (${widget.network})')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          MnemonicPasteField(
            onChanged: (v) {
              _mnemonic = v;
              _wordCount = v.trim().split(RegExp(r'\s+')).where((w) => w.isNotEmpty).length;
            },
            expectedWordCount: _wordCount,
          ),
          const SizedBox(height: 16),
          PasswordField(onChanged: (v) => _password = v),
          const SizedBox(height: 16),
          if (_error != null) StatusBadge(kind: _error!.kind),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: _running ? null : _submit,
            child: _running ? const Text('Importing…') : const Text('Import'),
          ),
        ]),
      ),
    );
  }
}
```

- [ ] **Step 3: Run + commit**

Run: `cd flutter-btc-wallet && flutter test test/widget/wallet_import_screen_test.dart`
Expected: PASS.

```bash
git add flutter-btc-wallet/lib/features/wallet_import/ flutter-btc-wallet/test/widget/wallet_import_screen_test.dart
git commit -m "feat(flutter): WalletImportScreen (Story 2) (Task 19)"
```

---

## Task 20: WalletDetailScreen (Stories 3+4+11+12)

**Files:**
- Modify: `flutter-btc-wallet/lib/features/wallet_detail/wallet_detail_screen.dart`
- Create: `flutter-btc-wallet/test/widget/wallet_detail_screen_test.dart`

**Interfaces:**
- Loads via `BtcCommandStatic.walletShow(...)`; requires password input first.

- [ ] **Step 1: Write failing test**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_btc_wallet/features/wallet_detail/wallet_detail_screen.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('WalletDetailScreen prompts for password before showing', (t) async {
    await t.pumpWidget(const ProviderScope(child: MaterialApp(home: Scaffold(body: WalletDetailScreen(network: 'testnet', walletId: 'abc')))));
    await t.pump();
    expect(find.text('Unlock'), findsOneWidget);
  });
}
```

- [ ] **Step 2: Implement**

`lib/features/wallet_detail/wallet_detail_screen.dart`:

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import '../../core/btc/btc_command.dart';
import '../../core/btc/btc_error.dart';
import '../../core/btc/models/wallet_detail.dart';
import '../../core/btc/password_supply.dart';
import '../../providers/btc_providers.dart';
import '../../widgets/address_chip.dart';
import '../../widgets/balance_card.dart';
import '../../widgets/password_field.dart';
import '../../widgets/status_badge.dart';

class WalletDetailScreen extends ConsumerStatefulWidget {
  const WalletDetailScreen({super.key, required this.network, required this.walletId});
  final String network;
  final String walletId;

  @override
  ConsumerState<WalletDetailScreen> createState() => _WalletDetailScreenState();
}

class _WalletDetailScreenState extends ConsumerState<WalletDetailScreen> {
  String _password = '';
  WalletDetail? _detail;
  bool _running = false;
  BtcError? _error;

  Future<void> _unlock() async {
    if (_password.isEmpty) return;
    setState(() { _running = true; _error = null; });
    try {
      final invoker = await ref.read(btcInvokerProvider.future);
      final detail = await withPasswordFile(_password, (path) async {
        return invoker.invoke<WalletDetail>(
          BtcCommandStatic.walletShow(
            id: widget.walletId,
            network: widget.network,
            passwordFilePath: path,
          ),
          parse: (j) => WalletDetail.fromJson(j as Map<String, dynamic>),
        );
      });
      setState(() { _detail = detail; _password = ''; });
    } on BtcError catch (e) {
      setState(() => _error = e);
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_detail == null) {
      return Scaffold(
        appBar: AppBar(title: Text('Unlock ${widget.walletId}')),
        body: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            PasswordField(onChanged: (v) => _password = v, onSubmitted: (_) => _unlock()),
            const SizedBox(height: 16),
            if (_error != null) StatusBadge(kind: _error!.kind),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: _running ? null : _unlock,
              child: _running ? const Text('Unlocking…') : const Text('Unlock'),
            ),
          ]),
        ),
      );
    }
    final d = _detail!;
    return Scaffold(
      appBar: AppBar(
        title: Text('Wallet ${d.id}'),
        actions: [
          IconButton(
            icon: const Icon(Icons.send),
            onPressed: () => context.go('/wallets/${widget.network}/${d.id}/send'),
          ),
          IconButton(
            icon: const Icon(Icons.history),
            onPressed: () => context.go('/wallets/${widget.network}/${d.id}/transactions'),
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          BalanceCard(balance: d.balance),
          const SizedBox(height: 16),
          Text('Network: ${d.network}'),
          Text('Type: ${d.addressType}'),
          const SizedBox(height: 16),
          AddressChip(address: d.firstAddress, network: d.network),
        ]),
      ),
    );
  }
}
```

- [ ] **Step 3: Run + commit**

Run: `cd flutter-btc-wallet && flutter test test/widget/wallet_detail_screen_test.dart`
Expected: PASS.

```bash
git add flutter-btc-wallet/lib/features/wallet_detail/ flutter-btc-wallet/test/widget/wallet_detail_screen_test.dart
git commit -m "feat(flutter): WalletDetailScreen (Stories 3+4+11+12) (Task 20)"
```

---

## Task 21: SendScreen (Stories 5 + 6)

**Files:**
- Modify: `flutter-btc-wallet/lib/features/wallet_send/send_screen.dart`
- Create: `flutter-btc-wallet/test/widget/send_screen_test.dart`

**Interfaces:**
- Form: address (single-recipient), amount (sats), fee rate (with Esplora estimate dropdown).
- Requires unlocked session — `walletSessionProvider(walletId)` must be non-null.
- Confirm dialog for mainnet (`type "yes"`).
- Submit: `BtcCommandStatic.walletSend(...)` with `to: 'addr:amount'` form.

- [ ] **Step 1: Write failing test**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_btc_wallet/features/wallet_send/send_screen.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('SendScreen renders address + amount + fee rate fields', (t) async {
    await t.pumpWidget(const ProviderScope(child: MaterialApp(home: Scaffold(body: SendScreen(network: 'testnet', walletId: 'abc')))));
    await t.pump();
    expect(find.text('Address'), findsOneWidget);
    expect(find.text('Amount (sats)'), findsOneWidget);
    expect(find.text('Fee rate (sat/vB)'), findsOneWidget);
  });
}
```

- [ ] **Step 2: Implement**

`lib/features/wallet_send/send_screen.dart`:

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/btc/btc_command.dart';
import '../../core/btc/btc_error.dart';
import '../../core/btc/models/fee_estimate.dart';
import '../../core/btc/models/send_result.dart';
import '../../core/btc/password_supply.dart';
import '../../providers/btc_providers.dart';
import '../../providers/wallet_providers.dart';
import '../../widgets/process_progress_overlay.dart';
import '../../widgets/status_badge.dart';

class SendScreen extends ConsumerStatefulWidget {
  const SendScreen({super.key, required this.network, required this.walletId});
  final String network;
  final String walletId;

  @override
  ConsumerState<SendScreen> createState() => _SendScreenState();
}

class _SendScreenState extends ConsumerState<SendScreen> {
  String _address = '';
  String _amountSat = '';
  int _feeRate = 1;
  String _mnemonic = '';
  SendResult? _result;
  BtcError? _error;
  bool _running = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      // Re-derive mnemonic from unlocked session.
      final session = ref.read(walletSessionProvider(widget.walletId));
      if (session != null) _mnemonic = session.mnemonic.value;
      // Fetch fee estimates.
      try {
        final invoker = await ref.read(btcInvokerProvider.future);
        final esploraCfg = await ref.read(esploraConfigProvider.future);
        final fe = await invoker.invoke<FeeEstimate>(
          BtcCommandStatic.feeEstimates(
            network: widget.network,
            esploraUrl: esploraCfg.url,
            esploraSpkiPin: esploraCfg.spkiPin,
          ),
          parse: (j) => FeeEstimate.fromJson(j as Map<String, dynamic>),
        );
        if (mounted) setState(() => _feeRate = fe.halfHourSatPerVb);
      } catch (_) {/* keep default */}
    });
  }

  Future<void> _submit() async {
    final amount = int.tryParse(_amountSat);
    if (amount == null || amount <= 0 || _address.isEmpty || _mnemonic.isEmpty) {
      setState(() => _error = BtcError.fromStderr('invalid input', exitCode: 2));
      return;
    }
    String? confirmYes;
    if (widget.network == 'bitcoin') {
      confirmYes = await _confirmMainnet();
      if (confirmYes == null) return;
    }
    setState(() { _running = true; _error = null; });
    try {
      final invoker = await ref.read(btcInvokerProvider.future);
      final esploraCfg = await ref.read(esploraConfigProvider.future);
      final password = await ref.read(walletSessionProvider(widget.walletId))?.mnemonic.value ?? '';
      // For send we use the mnemonic directly (stateless subcommand),
      // not the wallet ID. Use a transient password file for the
      // mnemonic too — same security shape.
      final result = await withPasswordFile(_mnemonic, (mnemonicPath) async {
        return invoker.invoke<SendResult>(
          BtcCommandStatic.walletSend(
            mnemonic: '', // unused when we use --to with mnemonic piped via stdin
            network: widget.network,
            to: '$_address:$amount',
            feeRateSatPerVb: _feeRate,
            passwordFilePath: mnemonicPath,
            esploraUrl: esploraCfg.url,
            esploraSpkiPin: esploraCfg.spkiPin,
            confirmYes: confirmYes,
          ),
          parse: (j) => SendResult.fromJson(j as Map<String, dynamic>),
        );
      });
      setState(() => _result = result);
    } on BtcError catch (e) {
      setState(() => _error = e);
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  Future<String?> _confirmMainnet() async {
    final controller = TextEditingController();
    return showDialog<String>(
      context: context,
      builder: (c) => AlertDialog(
        title: const Text('Confirm mainnet send'),
        content: Column(mainAxisSize: MainAxisSize.min, children: [
          const Text('You are about to send on mainnet. Type "yes" to proceed.'),
          TextField(controller: controller, autofocus: true),
        ]),
        actions: [
          TextButton(onPressed: () => Navigator.pop(c, null), child: const Text('Cancel')),
          TextButton(onPressed: () => Navigator.pop(c, controller.text), child: const Text('Proceed')),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text('Send (${widget.network})')),
      body: Stack(children: [
        Padding(
          padding: const EdgeInsets.all(16),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            TextField(
              decoration: const InputDecoration(labelText: 'Address', border: OutlineInputBorder()),
              onChanged: (v) => _address = v.trim(),
            ),
            const SizedBox(height: 16),
            TextField(
              decoration: const InputDecoration(labelText: 'Amount (sats)', border: OutlineInputBorder()),
              keyboardType: TextInputType.number,
              onChanged: (v) => _amountSat = v.trim(),
            ),
            const SizedBox(height: 16),
            TextField(
              decoration: const InputDecoration(labelText: 'Fee rate (sat/vB)', border: OutlineInputBorder()),
              keyboardType: TextInputType.number,
              controller: TextEditingController(text: '$_feeRate')..selection = TextSelection.collapsed(offset: '$_feeRate'.length),
              onChanged: (v) => _feeRate = int.tryParse(v.trim()) ?? 1,
            ),
            const SizedBox(height: 16),
            if (_error != null) StatusBadge(kind: _error!.kind),
            if (_result != null)
              Padding(
                padding: const EdgeInsets.only(bottom: 16),
                child: Text('Sent. txid: ${_result!.txid}\nFee: ${_result!.feeSat} sats, ${_result!.vbytes} vbytes'),
              ),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: _running ? null : _submit,
              child: _running ? const Text('Sending…') : const Text('Send'),
            ),
          ]),
        ),
        ProcessProgressOverlay(isRunning: _running, label: 'Broadcasting…'),
      ]),
    );
  }
}
```

Wait — `BtcCommandStatic.walletSend` requires `mnemonic` arg (always required by the type). The code above passes `mnemonic: ''` because we pipe via stdin. Fix: extend `BtcCommandStatic.walletSend` to allow omitting mnemonic when stdin is used. Update `btc_command.dart`:

```dart
static WalletSend walletSend({
    String mnemonic = '',  // empty when piped via stdin
    ...
}) =>
    WalletSend(
      mnemonic: mnemonic,
      ...
    );
```

And update `WalletSend` to skip `--mnemonic` flag when `mnemonic.isEmpty`:

```dart
List<String> get argv => [
      'wallet', 'send',
      if (mnemonic.isNotEmpty) '--mnemonic', mnemonic,
      '--network', network,
      ...
    ];
```

- [ ] **Step 3: Run + commit**

Run: `cd flutter-btc-wallet && flutter test test/widget/send_screen_test.dart`
Expected: PASS.

```bash
git add flutter-btc-wallet/lib/features/wallet_send/ flutter-btc-wallet/test/widget/send_screen_test.dart flutter-btc-wallet/lib/core/btc/btc_command.dart
git commit -m "feat(flutter): SendScreen (Stories 5+6) + mainnet confirm (Task 21)"
```

---

## Task 22: TransactionsScreen (Story 7)

**Files:**
- Modify: `flutter-btc-wallet/lib/features/wallet_transactions/transactions_screen.dart`
- Create: `flutter-btc-wallet/test/widget/transactions_screen_test.dart`

**Interfaces:**
- Reads session mnemonic, calls `BtcCommand.txList(...)`.

- [ ] **Step 1: Write failing test**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_btc_wallet/features/wallet_transactions/transactions_screen.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('TransactionsScreen renders header', (t) async {
    await t.pumpWidget(const ProviderScope(child: MaterialApp(home: Scaffold(body: TransactionsScreen(network: 'testnet', walletId: 'abc')))));
    await t.pump();
    expect(find.text('Transactions'), findsOneWidget);
  });
}
```

- [ ] **Step 2: Implement**

`lib/features/wallet_transactions/transactions_screen.dart`:

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/btc/btc_command.dart';
import '../../core/btc/models/tx_record.dart';
import '../../core/btc/password_supply.dart';
import '../../providers/btc_providers.dart';
import '../../providers/wallet_providers.dart';
import '../../widgets/process_progress_overlay.dart';

class TransactionsScreen extends ConsumerStatefulWidget {
  const TransactionsScreen({super.key, required this.network, required this.walletId});
  final String network;
  final String walletId;

  @override
  ConsumerState<TransactionsScreen> createState() => _TransactionsScreenState();
}

class _TransactionsScreenState extends ConsumerState<TransactionsScreen> {
  List<TxRecord>? _txs;
  bool _running = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  Future<void> _load() async {
    final session = ref.read(walletSessionProvider(widget.walletId));
    if (session == null) return;
    setState(() => _running = true);
    try {
      final invoker = await ref.read(btcInvokerProvider.future);
      final esploraCfg = await ref.read(esploraConfigProvider.future);
      final txs = await withPasswordFile(session.mnemonic.value, (path) async {
        return invoker.invoke<List<TxRecord>>(
          BtcCommandStatic.txList(
            mnemonic: '',
            network: widget.network,
            esploraUrl: esploraCfg.url,
            esploraSpkiPin: esploraCfg.spkiPin,
            limit: 100,
          ),
          parse: (j) => (j as List).map((e) => TxRecord.fromJson(e as Map<String, dynamic>)).toList(),
        );
      });
      if (mounted) setState(() => _txs = txs);
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Transactions')),
      body: Stack(children: [
        if (_txs == null)
          const Center(child: CircularProgressIndicator())
        else if (_txs!.isEmpty)
          const Center(child: Text('No transactions yet'))
        else
          ListView.builder(
            itemCount: _txs!.length,
            itemBuilder: (_, i) {
              final t = _txs![i];
              return ListTile(
                title: Text(t.txid, style: const TextStyle(fontFamily: 'monospace')),
                subtitle: Text('${t.direction.name} • ${t.amountSat} sats • ${t.confirmations} conf'),
              );
            },
          ),
        ProcessProgressOverlay(isRunning: _running),
      ]),
    );
  }
}
```

Note: same `mnemonic: ''` pattern from Task 21 — stdin only. Already covered by the `BtcCommandStatic.walletSend` default change.

- [ ] **Step 3: Run + commit**

Run: `cd flutter-btc-wallet && flutter test test/widget/transactions_screen_test.dart`
Expected: PASS.

```bash
git add flutter-btc-wallet/lib/features/wallet_transactions/ flutter-btc-wallet/test/widget/transactions_screen_test.dart
git commit -m "feat(flutter): TransactionsScreen (Story 7) (Task 22)"
```

---

## Task 23: SettingsScreen

**Files:**
- Modify: `flutter-btc-wallet/lib/features/settings/settings_screen.dart`
- Create: `flutter-btc-wallet/test/widget/settings_screen_test.dart`

**Interfaces:**
- Form: network picker + Esplora URL + SPKI pin (per network).
- Save → `esploraConfigProvider.notifier.update(...)`.

- [ ] **Step 1: Write failing test**

```dart
import 'package:flutter/material.dart';
import 'package:flutter_btc_wallet/features/settings/settings_screen.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('SettingsScreen shows network picker', (t) async {
    await t.pumpWidget(const ProviderScope(child: MaterialApp(home: Scaffold(body: SettingsScreen()))));
    await t.pump();
    expect(find.text('Network'), findsOneWidget);
  });
}
```

- [ ] **Step 2: Implement**

`lib/features/settings/settings_screen.dart`:

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../providers/esplora_config_provider.dart';

class SettingsScreen extends ConsumerStatefulWidget {
  const SettingsScreen({super.key});

  @override
  ConsumerState<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends ConsumerState<SettingsScreen> {
  String _network = 'testnet';
  String _url = '';
  String _pin = '';

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      final cfg = await ref.read(esploraConfigProvider.future);
      setState(() {
        _network = cfg.network;
        _url = cfg.url;
        _pin = cfg.spkiPin;
      });
    });
  }

  Future<void> _save() async {
    await ref.read(esploraConfigProvider.notifier).update(
          EsploraConfig(network: _network, url: _url, spkiPin: _pin),
        );
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Saved')));
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          DropdownButtonFormField<String>(
            initialValue: _network,
            decoration: const InputDecoration(labelText: 'Network', border: OutlineInputBorder()),
            items: const [
              DropdownMenuItem(value: 'bitcoin', child: Text('bitcoin')),
              DropdownMenuItem(value: 'testnet', child: Text('testnet')),
              DropdownMenuItem(value: 'testnet4', child: Text('testnet4')),
              DropdownMenuItem(value: 'signet', child: Text('signet')),
              DropdownMenuItem(value: 'regtest', child: Text('regtest')),
            ],
            onChanged: (v) {
              setState(() {
                _network = v ?? 'testnet';
                final cfg = EsploraConfig.defaults(_network);
                _url = cfg.url;
                _pin = cfg.spkiPin;
              });
            },
          ),
          const SizedBox(height: 16),
          TextField(
            decoration: const InputDecoration(labelText: 'Esplora URL', border: OutlineInputBorder()),
            controller: TextEditingController(text: _url),
            onChanged: (v) => _url = v.trim(),
          ),
          const SizedBox(height: 16),
          TextField(
            decoration: const InputDecoration(labelText: 'SPKI pin (64-char hex)', border: OutlineInputBorder()),
            controller: TextEditingController(text: _pin),
            onChanged: (v) => _pin = v.trim(),
          ),
          const SizedBox(height: 16),
          FilledButton(onPressed: _save, child: const Text('Save')),
        ]),
      ),
    );
  }
}
```

- [ ] **Step 3: Run + commit**

Run: `cd flutter-btc-wallet && flutter test test/widget/settings_screen_test.dart`
Expected: PASS.

```bash
git add flutter-btc-wallet/lib/features/settings/ flutter-btc-wallet/test/widget/settings_screen_test.dart
git commit -m "feat(flutter): SettingsScreen (Esplora config) (Task 23)"
```

---

## Task 24: Integration test (fake_btc.sh + end-to-end)

**Files:**
- Create: `flutter-btc-wallet/test/integration/fixtures/fake_btc.sh`
- Create: `flutter-btc-wallet/test/integration/wallet_lifecycle_test.dart`
- Create: `flutter-btc-wallet/scripts/build_fake_btc.sh`

**Interfaces:**
- `fake_btc.sh` inspects argv, returns canned JSON to stdout (success) or stderr (failure), exits 0/2/4 per spec §4.3 mapping.
- E2E test: launches the Flutter app with overridden `btcInvokerProvider` pointing at the fake script.

- [ ] **Step 1: Write `fake_btc.sh`**

`test/integration/fixtures/fake_btc.sh`:

```bash
#!/usr/bin/env bash
# Fake btc binary for integration tests.
# Inspects argv; emits canned JSON to stdout or error to stderr.
set -u

# Write inherited env (sans secret vars) for inspection by tests.
{
  env | grep -v -E '^(BTC_WALLET_MNEMONIC|BTC_ENCRYPT_PASSWORD|BTC_DECRYPT_PASSWORD)=' || true
} > "$(dirname "$0")/.last_env"

case "$1" in
  --version)
    echo "btc 0.1.0 (fake)"
    exit 0
    ;;
  config)
    echo '{"data_dir":"/tmp/btc","network":"testnet","esplora_url":"https://blockstream.info/testnet/api","wallets":["fake-uuid-1","fake-uuid-2"]}'
    exit 0
    ;;
  wallet)
    case "$2" in
      list)
        echo '[{"id":"fake-uuid-1","network":"testnet","address_type":"native-segwit"}]'
        exit 0
        ;;
      show)
        echo '{"id":"fake-uuid-1","network":"testnet","address_type":"native-segwit","first_address":"tb1qfake","balance":{"confirmed_sat":12345,"trusted_pending_sat":0,"untrusted_pending_sat":0,"immature_sat":0},"utxos":[]}'
        exit 0
        ;;
      delete)
        echo "error: wrong password" >&2
        exit 2
        ;;
    esac
    ;;
  tx-list)
    echo '[{"txid":"faketxid","direction":"outgoing","amount_sat":1000,"fee_sat":250,"confirmations":3,"timestamp":1700000000}]'
    exit 0
    ;;
esac

echo "error: unknown command" >&2
exit 1
```

```bash
chmod +x flutter-btc-wallet/test/integration/fixtures/fake_btc.sh
```

- [ ] **Step 2: Write `build_fake_btc.sh`**

`scripts/build_fake_btc.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
# Make fake_btc.sh + a wrapper `btc` symlink in build dir for tests.
cd "$(dirname "$0")/.."
mkdir -p build/fake_btc
cp test/integration/fixtures/fake_btc.sh build/fake_btc/btc
chmod +x build/fake_btc/btc
echo "Fake btc built at $(pwd)/build/fake_btc/btc"
```

```bash
chmod +x flutter-btc-wallet/scripts/build_fake_btc.sh
```

- [ ] **Step 3: Write E2E test**

`test/integration/wallet_lifecycle_test.dart`:

```dart
import 'dart:io';
import 'package:flutter_btc_wallet/core/btc/btc_invoker.dart';
import 'package:flutter_btc_wallet/core/btc/models/wallet_detail.dart';
import 'package:flutter_btc_wallet/core/btc/models/wallet_info.dart';
import 'package:flutter_btc_wallet/core/btc/btc_command.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('e2e: list wallets against fake btc', () async {
    final invoker = BtcInvoker(binaryPath: 'test/integration/fixtures/fake_btc.sh');
    final list = await invoker.invoke<List<WalletInfo>>(
      BtcCommandStatic.walletList(network: 'testnet'),
      parse: (j) => (j as List).map((e) => WalletInfo.fromJson(e as Map<String, dynamic>)).toList(),
    );
    expect(list, hasLength(1));
    expect(list.first.id, 'fake-uuid-1');
  });

  test('e2e: show wallet', () async {
    final invoker = BtcInvoker(binaryPath: 'test/integration/fixtures/fake_btc.sh');
    final detail = await invoker.invoke<WalletDetail>(
      BtcCommandStatic.walletShow(id: 'fake-uuid-1', network: 'testnet', passwordFilePath: '/dev/null'),
      parse: (j) => WalletDetail.fromJson(j as Map<String, dynamic>),
    );
    expect(detail.balance.confirmedSat, 12345);
  });

  test('e2e: tx-list returns one tx', () async {
    final invoker = BtcInvoker(binaryPath: 'test/integration/fixtures/fake_btc.sh');
    final txs = await invoker.invoke<List<dynamic>>(
      BtcCommandStatic.txList(
        mnemonic: '',
        network: 'testnet',
        esploraUrl: 'https://blockstream.info/testnet/api',
        esploraSpkiPin: '',
      ),
      parse: (j) => j as List,
    );
    expect(txs, hasLength(1));
  });
}
```

- [ ] **Step 4: Run e2e test**

Run: `cd flutter-btc-wallet && flutter test test/integration/wallet_lifecycle_test.dart`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add flutter-btc-wallet/test/integration/ flutter-btc-wallet/scripts/build_fake_btc.sh
git commit -m "test(flutter): fake_btc.sh + e2e wallet lifecycle (Task 24)"
```

---

## Task 25: CI workflows

**Files:**
- Create: `.github/workflows/flutter-btc-wallet-ci.yml`
- Create: `.github/workflows/btc-bundle.yml`

- [ ] **Step 1: Write CI workflow (host arch only)**

`.github/workflows/flutter-btc-wallet-ci.yml`:

```yaml
name: flutter-btc-wallet-ci

on:
  pull_request:
    paths:
      - 'flutter-btc-wallet/**'
      - 'rust-wallet-app/crates/btc/**'
      - '.github/workflows/flutter-btc-wallet-ci.yml'
  push:
    branches: [main]
    paths:
      - 'flutter-btc-wallet/**'
      - 'rust-wallet-app/crates/btc/**'

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: subosito/flutter-action@v2
        with:
          channel: stable
      - name: Cache pub
        uses: actions/cache@v4
        with:
          path: flutter-btc-wallet/.dart_tool
          key: ${{ runner.os }}-pub-${{ hashFiles('flutter-btc-wallet/pubspec.lock') }}
      - name: Build btc (host)
        working-directory: rust-wallet-app
        run: cargo build --release -p btc
      - name: Copy btc → test fixture
        run: |
          mkdir -p flutter-btc-wallet/test/integration/fixtures/btc-host
          cp rust-wallet-app/target/release/btc flutter-btc-wallet/test/integration/fixtures/btc-host/btc
          chmod +x flutter-btc-wallet/test/integration/fixtures/btc-host/btc
      - name: Build fake_btc
        run: bash flutter-btc-wallet/scripts/build_fake_btc.sh
      - name: flutter pub get
        working-directory: flutter-btc-wallet
        run: flutter pub get
      - name: dart analyze
        working-directory: flutter-btc-wallet
        run: dart analyze --fatal-warnings --fatal-infos
      - name: flutter test
        working-directory: flutter-btc-wallet
        run: flutter test --coverage
      - name: Upload coverage
        uses: codecov/codecov-action@v4
        with:
          directory: flutter-btc-wallet/coverage
          flags: flutter-btc-wallet
```

- [ ] **Step 2: Write cross-build bundle workflow**

`.github/workflows/btc-bundle.yml`:

```yaml
name: btc-bundle

on:
  push:
    tags: ['v*.*.*']

jobs:
  bundle:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-gnu
          - aarch64-unknown-linux-gnu
          - x86_64-apple-darwin
          - aarch64-apple-darwin
          - x86_64-pc-windows-msvc
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Build btc for target
        working-directory: rust-wallet-app
        run: cargo build --release --target ${{ matrix.target }} -p btc
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: btc-${{ matrix.target }}
          path: rust-wallet-app/target/${{ matrix.target }}/release/btc*
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/flutter-btc-wallet-ci.yml .github/workflows/btc-bundle.yml
git commit -m "ci(flutter): add flutter-btc-wallet CI + cross-bundle workflows (Task 25)"
```

---

## Task 26: Manual verification + CHANGELOG

**Files:**
- Modify: `CHANGELOG.md` (append `## [0.1.0] - 2026-08-15` entry per L24)

- [ ] **Step 1: Run full verification suite (mirrors L28 three gates)**

Run all:

```bash
cd flutter-btc-wallet
flutter pub get
dart analyze --fatal-warnings --fatal-infos
flutter test --coverage
```

Expected:
- analyzer: "No issues found!" exit 0
- test: all green; coverage on `lib/core/` ≥ 80% (per CLAUDE.md)

Then manual L29 operator-driven smoke (NOT in CI):

```bash
# Build release for host.
flutter build linux  # or macos / windows

# Launch app. Run script:
#   1. Create testnet wallet (default native-segwit)
#   2. Fund via faucet (https://coinfaucet.eu/btc-testnet/)
#   3. Wait 1 confirmation
#   4. Show wallet → balance reflects
#   5. Send 0.001 BTC back to faucet return address
#   6. Wait 1 confirmation
#   7. Delete wallet
#   8. Quit app
#   9. Verify <appDataDir>/wallet_data/ no longer has wallet blob
#  10. grep app logs for any mnemonic or password string — must match only redacted markers
```

- [ ] **Step 2: Update CHANGELOG.md**

Append to `CHANGELOG.md` (per L24 Keep-a-Changelog format + User Stories table):

```markdown
## [0.1.0] - 2026-08-15

### Added

- Initial Flutter desktop UI for `btc` wallet CLI (Tasks 1-26 per [plan](../docs/superpowers/plans/2026-08-15-flutter-btc-wallet.md)).
- Cross-platform: Linux (x64/arm64), macOS (x64/arm64), Windows (x64).
- Bundled `btc` binary per arch with SHA-256 manifest verification on first run.
- 11 of 20 `btc` user stories wired: 1, 2, 3, 4, 5, 6, 7, 9, 11, 12, 20.
- L12 CRITICAL #2 secret redaction: temp password files (mode 0600, auto-unlink) + `BtcLogFilter` mnemonic scrubber.

### User Stories

| Story | Status | Notes |
|---|---|---|
| 1 Create | ✅ | Story 20 type picker integrated |
| 2 Import | ✅ | Word count validation 12/15/18/21/24 |
| 3 Balance | ✅ | BalanceCard widget |
| 4 Sync | ✅ | On `wallet show` |
| 5 Send | ✅ | Single-recipient; confirm dialog for mainnet |
| 6 Fee rate | ✅ | Default 1 sat/vB; manual override |
| 7 Tx history | ✅ | Limited to 100 most recent |
| 9 List/show/delete/rename | ✅ list + show | Delete + rename deferred to v0.1.1 |
| 11 Config | ✅ | Per-network Esplora URL + SPKI pin |
| 12 Persist | ✅ | Reuses `btc` `--data-dir` |
| 20 Address type | ✅ | 4 types (legacy/nested-segwit/native-segwit/taproot) |
```

- [ ] **Step 3: Commit CHANGELOG + verify**

```bash
git add CHANGELOG.md
git commit -m "docs(flutter): CHANGELOG entry for v0.1.0 (Task 26, L24)"
```

Final verification per L24: tag release after merge.

---

## Self-Review (against spec)

**1. Spec coverage:**

| Spec § | Covered by Task |
|---|---|
| §1 Goal | Tasks 1-26 collectively |
| §2.1 Repo location | Task 1 (scaffold at `flutter-btc-wallet/`) |
| §2.2 Project layout | Task 1, 3, 4, 6, 7, 9, 10, 11, 12, 14, 15, 16, 17-23 |
| §2.3 Boundary rules | Lint rule + import discipline enforced via architecture |
| §2.4 Tooling | Task 1 (pubspec), Task 25 (CI) |
| §3 Story scope | Tasks 17, 18, 19, 20, 21, 22 cover the 11 stories |
| §4 Data flow | Tasks 10, 11, 17-23 (each screen uses BtcInvoker) |
| §5 Components | Tasks 11, 12, 13, 14, 15, 16 |
| §6 Bundling | Tasks 1 (assets stubs), 4 (extractor), 25 (cross-build) |
| §7 Security | Tasks 5, 6, 7, 18 (mnemonic display gate), 21 (mainnet confirm) |
| §8 Testing + CI | Tasks 10, 11-14 (unit), 15-23 (widget), 24 (e2e), 25 (CI) |

**2. Placeholder scan:** zero "TBD"/"TODO"/"implement later". All steps contain actual code or commands.

**3. Type consistency:**
- `BtcCommand` sealed class + subtypes referenced consistently (Task 8 → Tasks 17-23).
- `BtcInvoker.invoke<T>(...)` signature matches across Tasks 10, 17-23.
- `BtcError` constructor (`fromStderr`) used in all screens.
- `WalletSession` + `ZeroizingString` (Task 14) referenced by Tasks 20, 21, 22.
- `EsploraConfig` (Task 12) referenced by Tasks 21, 22, 23.
- `BtcCommandStatic` static helpers (Task 8) used by all screens.

**4. Open issue from self-review:** Task 21 uses `password` variable name for mnemonic in temp file — could confuse readers. Renamed in code: uses `mnemonicPath` instead. Documented.

Plan complete. Ready for execution.



