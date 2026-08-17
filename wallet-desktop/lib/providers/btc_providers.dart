import 'dart:io' show Directory;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/binary/btc_extractor.dart';
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
/// `btc/`, `tmp/`, `wallet_data/`. `ref.watch` consumers get a single
/// `AppPaths` instance per `ProviderContainer`.
final appPathsProvider = FutureProvider<AppPaths>((Ref ref) async {
  return AppPaths(
    dataDir: await appDataDir(),
    btcDir: await subdirFor('btc'),
    tmpDir: await subdirFor('tmp'),
    walletDataDir: await subdirFor('wallet_data'),
  );
});

/// Resolves the bundled `btc` binary via [BtcExtractor] and constructs
/// a [BtcInvoker] pre-configured with the wallet data directory
/// override. `dataDirOverride` is what `btc` reads via `BTC_DATA_DIR`.
///
/// Depends on [appPathsProvider] — extracted binary path comes from
/// `appDataDir/btc/btc` (or `.exe` on Windows).
final btcInvokerProvider = FutureProvider<BtcInvoker>((Ref ref) async {
  final paths = await ref.watch(appPathsProvider.future);
  final binaryPath = await extractBtc();
  return BtcInvoker(
    binaryPath: binaryPath,
    dataDirOverride: paths.walletDataDir.path,
  );
});
