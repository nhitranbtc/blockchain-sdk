import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/btc/btc_command.dart';
import '../core/btc/models/wallet_info.dart';
import 'btc_providers.dart';

/// Loads the persisted-wallet list for the active network.
///
/// Reads via `btc wallet list --network <NET> --json` and parses the
/// JSON array into [WalletInfo] DTOs. `family<String>` keyed by network
/// so switching networks invalidates only that network's cache.
/// `autoDispose` so the cache drops when the list screen unmounts.
class WalletsListNotifier
    extends AutoDisposeFamilyAsyncNotifier<List<WalletInfo>, String> {
  @override
  Future<List<WalletInfo>> build(String network) async {
    final invoker = await ref.watch(btcInvokerProvider.future);
    return invoker.invoke<List<WalletInfo>>(
      BtcCommand.walletList(network: network),
      // BtcInvoker passes `null` on empty stdout and a string fallback
      // on non-JSON responses (defensive). Treat either as an empty
      // list rather than a parse failure — a fresh install with no
      // wallets should surface `data: []`, not `BtcError(kind: other)`.
      parse: (j) => (j is List)
          ? j
              .map((e) => WalletInfo.fromJson(e as Map<String, dynamic>))
              .toList(growable: false)
          : const <WalletInfo>[],
    );
  }

  /// Force a re-fetch (e.g. after wallet create / import). Used by
  /// Task 17 WalletListScreen's pull-to-refresh + Task 18 / 19
  /// post-action invalidation. Delegates to `ref.invalidateSelf` so
  /// Riverpod's lifecycle (dep tracking, listener coalescing) stays
  /// intact — manually re-invoking `build(arg)` would race with
  /// concurrent invalidations and double-subscribe `btcInvokerProvider`.
  Future<void> refresh() async {
    ref.invalidateSelf();
    await future;
  }
}

final walletsListProvider = AsyncNotifierProvider.autoDispose
    .family<WalletsListNotifier, List<WalletInfo>, String>(
        WalletsListNotifier.new);
