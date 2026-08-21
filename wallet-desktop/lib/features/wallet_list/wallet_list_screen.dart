import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../core/ffi/ffi_exception.dart';
import '../../core/format/wallet_id.dart';
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
                  onPressed: createCb ??
                      () => context.go(WalletRoutes.create(network)),
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
                // Defence-in-depth: `dart:developer.log` lands in
                // DevTools / VM-service / OS syslog and bypasses the
                // `package:logging` pipeline that `BtcLogFilter` sits
                // behind. Pre-redact the exception's `toString()` so a
                // non-FfiException reaching this branch can never echo
                // a mnemonic or password string into an external log
                // surface. (`FfiException.toString()` is safe by
                // construction — see ffi_exception.dart which
                // excludes the `messageForDebug` field. The redact
                // pass is defense-in-depth against future exception
                // types.) The `stackTrace` arg carries file/line info
                // from Dart's catch site — not user input — so it is
                // passed unredacted.
                const filter = BtcLogFilter();
                developer.log(
                  'wallet_list load failed',
                  name: 'WalletListScreen',
                  error: filter.redact(e.toString()),
                  stackTrace: st,
                );
                // L12 review MED #3 (Task 10): use kind-mapped user
                // copy from `ffi_exception.dart` — first real UI
                // consumer of `FfiException`. Every Tasks 11-16
                // screen will follow this pattern.
                final message = e is FfiException
                    ? userMessageForFfiException(e)
                    : 'Failed to load wallets';
                return Center(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Text(message),
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
                  onRefresh: () =>
                      ref.read(walletsListProvider(network).notifier).refresh(),
                  child: ListView.builder(
                    itemCount: list.length,
                    itemBuilder: (_, i) {
                      final id = list[i];
                      final isValid = WalletRoutes.isValidWalletIdSegment(id);
                      return ListTile(
                        key: ValueKey(id),
                        title: Text(
                          formatWalletId(id),
                          style: textTheme.bodyMedium
                              ?.copyWith(fontFamily: 'monospace'),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                        // Plan deviation (Task 10): subtitle dropped —
                        // Rust `wallet_list` returns id only; address
                        // type surfaces in detail screen via
                        // `wallet_peek_addresses` (Tasks 11-16).
                        // Long-press opens a sheet with the full UUID
                        // + Copy affordance (shoulder-surf hygiene:
                        // list shows the short form only).
                        onLongPress: !isValid
                            ? null
                            : () => _showWalletIdSheet(context, id),
                        onTap: !isValid
                            ? null
                            : (openCb != null
                                ? () => openCb(id)
                                : () => context.go(
                                      WalletRoutes.detail(network, id),
                                    )),
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

  /// Bottom sheet showing the full wallet UUID + Copy affordance.
  /// Triggered by long-press on a list row. List row title stays in
  /// the shoulder-surf-safe `first4…last4` short form (see
  /// `core/format/wallet_id.dart`).
  Future<void> _showWalletIdSheet(BuildContext context, String id) async {
    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (sheetCtx) {
        final theme = Theme.of(sheetCtx);
        return Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                'Wallet ID',
                style: theme.textTheme.labelLarge,
              ),
              const SizedBox(height: 8),
              SelectableText(
                id,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(fontFamily: 'monospace'),
              ),
              const SizedBox(height: 16),
              FilledButton.icon(
                icon: const Icon(Icons.copy),
                label: const Text('Copy'),
                onPressed: () async {
                  await Clipboard.setData(ClipboardData(text: id));
                  if (!sheetCtx.mounted) return;
                  ScaffoldMessenger.of(sheetCtx).showSnackBar(
                    const SnackBar(content: Text('Wallet ID copied')),
                  );
                  Navigator.of(sheetCtx).pop();
                },
              ),
            ],
          ),
        );
      },
    );
  }
}
