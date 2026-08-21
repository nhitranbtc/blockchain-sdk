import 'dart:io' show Directory;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/paths.dart';

/// Resolved `appDataDir` + the three named subdirectories (btc, tmp,
/// wallet_data). Computed once on first read; downstream providers
/// depend on this so they don't redundantly hit the filesystem.
///
/// **Task 17 / Issue #223** — moved here from the deleted
/// `lib/providers/btc_providers.dart` (which also held the
/// `btcInvokerProvider` stub). Only the path-resolution side stays;
/// the subprocess-invoker side is gone for good.
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

/// Resolves the OS `appDataDir` and creates the three subdirectories
/// `btc/`, `tmp/`, `wallet_data/`. The `btc/` subdir is preserved for
/// any future blob the Rust side might write (e.g., bdk wallet stores);
/// it no longer holds the `btc` CLI binary.
final appPathsProvider = FutureProvider<AppPaths>((Ref ref) async {
  return AppPaths(
    dataDir: await appDataDir(),
    btcDir: await subdirFor('btc'),
    tmpDir: await subdirFor('tmp'),
    walletDataDir: await subdirFor('wallet_data'),
  );
});
