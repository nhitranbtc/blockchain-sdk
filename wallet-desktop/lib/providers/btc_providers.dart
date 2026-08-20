// Stub: the btc CLI integration plumbing (binary extractor +
// subprocess invoker) is removed in this commit. `btcInvokerProvider`
// stays as a stub so legacy `SendScreen` + `TransactionsScreen`
// widgets (Tasks 14+15 migration targets) still compile; the
// provider throws on access with a clear "FFI migration in
// progress" message. The FFI surface is now the only wallet-ops
// path; new code uses `walletCoreProvider` (Task 8/10).
//
// **Plan deviation** (Task 13 fold-in): Task 17 (delete btc/ CLI
// plumbing) is partially landed here per user direction. The
// `lib/core/btc/{btc_invoker,btc_command,btc_error,btc_error_messages}.dart`
// files remain on disk for the unmigrated `SendScreen` +
// `TransactionsScreen` to import. Tasks 14 + 15 finish the deletion.

import 'dart:io' show Directory;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/btc/btc_invoker.dart';
import '../core/paths.dart';

/// Resolved `appDataDir` + the three named subdirectories (btc, tmp,
/// wallet_data). Computed once on first read; downstream providers
/// depend on this so they don't redundantly hit the filesystem.
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
/// backward-compat with any path the legacy btc CLI would have
/// written to, but no binary is extracted into it.
final appPathsProvider = FutureProvider<AppPaths>((Ref ref) async {
  return AppPaths(
    dataDir: await appDataDir(),
    btcDir: await subdirFor('btc'),
    tmpDir: await subdirFor('tmp'),
    walletDataDir: await subdirFor('wallet_data'),
  );
});

/// Stub: throws on access. `SendScreen` / `TransactionsScreen`
/// (Tasks 14+15) still import this provider; their `_submit` will
/// surface the error as a SnackBar. The verify gate (analyze + test)
/// passes because the type signature is preserved.
final btcInvokerProvider = FutureProvider<BtcInvoker>((Ref ref) async {
  throw StateError(
    'btcInvokerProvider stub: btc CLI integration is removed '
    '(commit per user direction). FFI surface is the only wallet-ops '
    'path — use walletCoreProvider. SendScreen + TransactionsScreen '
    'are pending Tasks 14+15 migration.',
  );
});
