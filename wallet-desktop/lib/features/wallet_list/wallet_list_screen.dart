import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../core/btc/btc_error.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';

/// Shows the persisted wallets for [network]. Reads the autoDispose
/// family [walletsListProvider], renders a row per wallet, with header
/// buttons to Create / Import a wallet. Tapping a wallet routes to
/// the detail screen.
///
/// **Navigation seams:** the three navigation handlers (`onCreate`,
/// `onImport`, `onOpenWallet`) are nullable. Default behavior uses
/// `go_router` (`context.go`) — wiring the screen into the app shell
/// (Task 16 `HomeShell` -> `GoRoute('/wallets/:network')`) routes
/// there. Tests pass plain `null` to keep the widget free of a
/// `Router` ancestor on the widget tree, so the unit-test pump does
/// not need a real `GoRouter` setup. `context.go` calls are only
/// evaluated when the user taps, so a missing router does not break
/// the build / render path.
///
/// **Error funnel:** the `error` branch never interpolates the raw
/// exception text. [BtcError] funnels through its kind-mapped
/// message; everything else runs through [BtcLogFilter.redact] (Task
/// 7) so a future exception type stays on the sanitised channel, and
/// the underlying error is logged via `dart:developer` for ops triage
/// without leaking to the UI surface.
///
/// **Pull-to-refresh:** wraps the populated list with a
/// [RefreshIndicator] that calls `notifier.refresh()` (Task 13).
/// Retries on the same channel.
///
/// **walletId validation:** every row tap runs [w.id] through
/// [WalletRoutes.isValidWalletIdSegment] before navigation. A CLI
/// returning `id: '../settings'` or `id: 'new'` is silently dropped
/// (the tap is a no-op) — closing the path-injection footgun from
/// security-auditor (Task 17 review) until the v0.2 router-level
/// `redirect:` lands.
///
/// **v0.2 deferred:** `network` should become a `Network` enum and
/// `WalletInfo.id` should become a `WalletId` value type (cross-cutting
/// refactor across Tasks 13/14/15/16/17 — already on the backlog).
class WalletListScreen extends ConsumerWidget {
  const WalletListScreen({
    super.key,
    required this.network,
    this.onCreate,
    this.onImport,
    this.onOpenWallet,
  });

  /// Active network identifier. One of the 5 networks listed in
  /// [NetworkPicker] (Task 15). v0.1 accepts any String; v0.2 will
  /// validate at the route boundary via a `Network` enum.
  final String network;

  /// Override hook for the Create button. Defaults to
  /// `context.go(WalletRoutes.create(network))`.
  final VoidCallback? onCreate;

  /// Override hook for the Import button. Defaults to
  /// `context.go(WalletRoutes.import(network))`.
  final VoidCallback? onImport;

  /// Override hook for wallet-row tap. Receives the wallet id and is
  /// only invoked when [w.id] passes
  /// [WalletRoutes.isValidWalletIdSegment]; otherwise the tap is a
  /// no-op (defence against path-injection).
  final void Function(String walletId)? onOpenWallet;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final asyncList = ref.watch(walletsListProvider(network));
    final createCb = onCreate;
    final importCb = onImport;
    final openCb = onOpenWallet;
    final textTheme = Theme.of(context).textTheme;

    return Scaffold(
      appBar: AppBar(title: Text('Wallets ($network)')),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                FilledButton.icon(
                  onPressed:
                      createCb ?? () => context.go(WalletRoutes.create(network)),
                  icon: const Icon(Icons.add),
                  label: const Text('Create'),
                ),
                const SizedBox(width: 8),
                OutlinedButton.icon(
                  onPressed: importCb ??
                      () => context.go(WalletRoutes.import(network)),
                  icon: const Icon(Icons.file_download),
                  label: const Text('Import'),
                ),
              ],
            ),
          ),
          Expanded(
            child: asyncList.when(
              loading: () => const Center(child: CircularProgressIndicator()),
              error: (e, st) {
                // Log to dart:developer (ops observability) with the
                // raw exception; render via a sanitised channel that
                // BtcError funnels through a kind-mapped message and
                // anything else runs through `BtcLogFilter.redact`.
                developer.log(
                  'wallet_list load failed',
                  name: 'WalletListScreen',
                  error: e,
                  stackTrace: st,
                );
                final message = e is BtcError
                    ? _userMessageForBtcError(e)
                    : 'Failed to load wallets';
                final detail = e is BtcError
                    ? null
                    : const BtcLogFilter().redact(e.toString());
                return Center(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Text(message),
                      if (detail != null) ...[
                        const SizedBox(height: 8),
                        Text(
                          detail,
                          style: textTheme.bodySmall,
                          textAlign: TextAlign.center,
                        ),
                      ],
                      const SizedBox(height: 16),
                      OutlinedButton.icon(
                        onPressed: () => ref
                            .read(walletsListProvider(network).notifier)
                            .refresh(),
                        icon: const Icon(Icons.refresh),
                        label: const Text('Retry'),
                      ),
                    ],
                  ),
                );
              },
              data: (list) {
                if (list.isEmpty) {
                  return const Center(
                    child: Text(
                      'No wallets yet — tap Create or Import to get started.',
                    ),
                  );
                }
                return RefreshIndicator(
                  onRefresh: () => ref
                      .read(walletsListProvider(network).notifier)
                      .refresh(),
                  child: ListView.builder(
                    itemCount: list.length,
                    itemBuilder: (_, i) {
                      final w = list[i];
                      return ListTile(
                        key: ValueKey(w.id),
                        title: Text(
                          _displayWalletId(w.id),
                          style: textTheme.bodyMedium
                              ?.copyWith(fontFamily: 'monospace'),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                        subtitle: Text('${w.network} • ${w.addressType}'),
                        onTap: (openCb != null &&
                                WalletRoutes.isValidWalletIdSegment(w.id))
                            ? () => openCb(w.id)
                            : (WalletRoutes.isValidWalletIdSegment(w.id)
                                ? () => context.go(
                                      WalletRoutes.detail(network, w.id),
                                    )
                                : null),
                      );
                    },
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }

  /// Maps a [BtcError] into a UI-facing message based on its kind.
  /// Does not expose the redacted stderr surface — that lives in the
  /// log layer for ops triage. v0.2 backlog: per-kind actionable copy
  /// + i18n key per kind.
  String _userMessageForBtcError(BtcError err) {
    switch (err.kind) {
      case BtcErrorKind.wrongPassword:
        return 'Wrong password.';
      case BtcErrorKind.insufficientFunds:
        return 'Not enough funds.';
      case BtcErrorKind.unknownWallet:
        return 'Wallet not found.';
      case BtcErrorKind.networkError:
        return 'Network error. Check your connection.';
      case BtcErrorKind.unknownAddressType:
        return 'Unsupported address type.';
      case BtcErrorKind.confirmRequired:
        return 'Confirmation required.';
      case BtcErrorKind.other:
        return 'Could not load wallets.';
    }
  }

  /// Renders the wallet id as a legibly-truncated monospace string.
  /// Wallet ids from `btc` are 32+ char hex/sha256 fingerprints; the
  /// security-auditor (Task 17 review) flagged full-fingerprint display
  /// as a shoulder-surfing / screen-recording hygiene concern. Show
  /// `first4…last4` for ids > 12 chars, full string otherwise.
  String _displayWalletId(String id) {
    if (id.length <= 12) return id;
    return '${id.substring(0, 4)}…${id.substring(id.length - 4)}';
  }
}
