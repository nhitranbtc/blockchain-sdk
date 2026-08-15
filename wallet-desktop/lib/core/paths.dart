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
Future<Directory> subdirFor(String name) async {
  final base = await appDataDir();
  return Directory(p.join(base.path, name)).create(recursive: true);
}
