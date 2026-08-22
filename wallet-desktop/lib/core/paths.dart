import 'dart:io';

import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

/// On-disk name of the wallet app's data directory. Public so tests can
/// reference one source of truth instead of duplicating the literal.
const appDirName = 'flutter_btc_wallet';

/// Returns (and creates) the per-user data directory for this app.
/// Calls `path_provider.getApplicationSupportDirectory()` and joins
/// [appDirName] under it. Recursive create is idempotent across runs.
Future<Directory> appDataDir() async {
  final base = await getApplicationSupportDirectory();
  return Directory(p.join(base.path, appDirName)).create(recursive: true);
}

/// Returns (and creates) a named subdirectory under [appDataDir].
/// Recursive create is idempotent across runs.
///
/// **Path-traversal defense (Issue #176)**: rejects `name` values that
/// could escape `appDataDir`. Throws [ArgumentError] before any
/// filesystem side effect, so the caller cannot accidentally `create()`
/// outside the app sandbox. Valid names are single-segment only
/// (`btc`, `tmp`, `wallet_data`) — no path separators, no `..`
/// substrings. All 3 production callers pass literal constants so the
/// stricter single-segment contract has zero migration impact.
Future<Directory> subdirFor(String name) async {
  _validateSubdirName(name);
  final base = await appDataDir();
  return Directory(p.join(base.path, name)).create(recursive: true);
}

/// Rejects subdir names that could escape `appDataDir` (CWE-22).
/// Throws [ArgumentError] before any filesystem side effect.
///
/// Validation runs synchronously before any `await` in [subdirFor], so
/// no TOCTOU window exists between the check and `Directory.create()`.
void _validateSubdirName(String name) {
  if (name.isEmpty) {
    throw ArgumentError.value(name, 'name', 'must not be empty');
  }
  if (p.isAbsolute(name)) {
    throw ArgumentError.value(name, 'name', 'must not be absolute');
  }
  if (RegExp(r'[\\/]').hasMatch(name)) {
    throw ArgumentError.value(
      name,
      'name',
      'must not contain path separators (single-segment names only)',
    );
  }
  if (name.contains('..')) {
    throw ArgumentError.value(
      name,
      'name',
      'must not contain ".."',
    );
  }
}
