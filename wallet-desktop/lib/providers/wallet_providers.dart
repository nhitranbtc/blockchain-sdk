import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/btc/btc_command.dart';
import '../core/btc/models/wallet_info.dart';
import 'btc_providers.dart';

/// Loads the persisted-wallet list for the active network.
///
/// Reads via `btc wallet list --network <NET> --json` (handled by
/// [BtcCommandStatic.walletList]) and parses the JSON array into
/// [WalletInfo] DTOs. `family<String>` keyed by network so switching
/// networks invalidates only that network's cache.
class WalletsListNotifier
    extends FamilyAsyncNotifier<List<WalletInfo>, String> {
  @override
  Future<List<WalletInfo>> build(String network) async {
    final invoker = await ref.watch(btcInvokerProvider.future);
    return invoker.invoke<List<WalletInfo>>(
      BtcCommand.walletList(network: network),
      parse: (j) => (j as List)
          .map((e) => WalletInfo.fromJson(e as Map<String, dynamic>))
          .toList(growable: false),
    );
  }

  /// Force a re-fetch (e.g. after wallet create / import). Used by
  /// Task 17 WalletListScreen's pull-to-refresh + Task 18 / 19
  /// post-action invalidation.
  Future<void> refresh() async {
    state = const AsyncLoading();
    state = await AsyncValue.guard(() => build(arg));
  }
}

final walletsListProvider =
    AsyncNotifierProvider.family<WalletsListNotifier, List<WalletInfo>, String>(
        WalletsListNotifier.new);
