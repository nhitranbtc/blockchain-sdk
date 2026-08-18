import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../core/btc/btc_command.dart';
import '../../core/btc/btc_error.dart';
import '../../core/btc/btc_error_messages.dart';
import '../../core/btc/models/wallet_detail.dart';
import '../../core/format/wallet_id.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../core/secrets/password_supply.dart';
import '../../providers/btc_providers.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';
import '../../widgets/address_chip.dart';
import '../../widgets/balance_card.dart';
import '../../widgets/password_field.dart';
import '../../widgets/status_badge.dart';

/// Detail view for a single wallet — Stories 3 (balance), 4 (receiving
/// address), 11 (unlock), 12 (lock). Owns the read-only unlock flow
/// that the create + import screens deliberately deferred (Task 18 L12
/// type-design post-PR MEDIUM #5).
///
/// **Unlock flow** (Story 11):
/// - User types password + taps Unlock → `BtcCommand.walletShow` runs
///   with `--password-file` pointing at a mode-0600 temp file (Task 5/6
///   `withPasswordFile`). Password NEVER enters argv.
/// - On success: detail parsed → `walletSessionProvider(walletId)
///   .notifier.unlock(mnemonic: '', detail: parsed)`. The empty-string
///   mnemonic is the v0.1 sentinel for "wallet is unlocked in this
///   session but no mnemonic is cached" — `btc wallet show` does not
///   return the mnemonic (the CLI re-decrypts per call), so this is
///   the cleanest read-only representation. Task 21 SendScreen will
///   detect `state.mnemonic.value.isEmpty` and prompt the user for the
///   mnemonic + password before signing — documented carry-over.
///
/// **Lock flow** (Story 12):
/// - `walletSessionProvider(walletId).notifier.lock()` clears state +
///   disposes the (empty-string) mnemonic handle. Detail screen falls
///   back to the Unlock form on next build.
///
/// **Secret handling** (L12 CRITICAL #2 + Task 5/6/7 chain):
/// - Password via `withPasswordFile` only (mode 0600 + auto-unlink).
/// - `_password` is force-cleared after submit (screen-side state).
/// - `BtcError` funnels through `userMessageForBtcError` (Task 17
///   helper) — raw `e.stderr` is never displayed; non-`BtcError`
///   catches run `toString()` through `BtcLogFilter.redact` and emit
///   via `dart:developer.log` (Task 17 lesson — bypasses package:logging
///   so we pre-redact ourselves).
///
/// **v0.2 deferred**: app-lifecycle auto-lock on backgrounding +
/// `Network` enum + `WalletId` value class (cross-cutting refactor).
class WalletDetailScreen extends ConsumerStatefulWidget {
  const WalletDetailScreen({
    super.key,
    required this.network,
    required this.walletId,
  });
  final String network;
  final String walletId;

  @override
  ConsumerState<WalletDetailScreen> createState() => _WalletDetailScreenState();
}

class _WalletDetailScreenState extends ConsumerState<WalletDetailScreen> {
  String _password = '';
  bool _running = false;
  BtcError? _error;

  @override
  void dispose() {
    // Defense-in-depth (L12 type-design Task 20 LOW): if the user
    // navigates away mid-`withPasswordFile` await, the State object
    // is torn down before the post-await `_password = ''` clear runs.
    // Zero the local handle here too. Same Dart-string-zeroization gap
    // as `OpaqueMnemonic` — documented at the type level; the real
    // fix is FFI Uint8List + Finalizable (v0.2).
    _password = '';
    super.dispose();
  }

  Future<void> _unlock() async {
    if (_password.isEmpty) return;
    // Capture the routing identity BEFORE the async suspension
    // (L12 type-design Task 20 MEDIUM). If the parent rebuilds the
    // screen with a different `walletId` mid-await, the CLI
    // invocation runs against wallet A but
    // `walletSessionProvider(widget.walletId)` lands in wallet B's
    // family — the detail would be mis-attributed AND the mode-0600
    // password file's contents consumed by the wrong-wallet CLI
    // process. Re-assert identity before populating the session.
    final walletId = widget.walletId;
    final network = widget.network;
    setState(() {
      _running = true;
      _error = null;
    });
    try {
      final invoker = await ref.read(btcInvokerProvider.future);
      // `withPasswordFile` is typed `Future<void>` (Task 5/6 thin
      // delegate); capture the parsed DTO via a closure variable. The
      // Task 6.1 follow-up re-parameterises both helpers to return `T`.
      WalletDetail? detail;
      await withPasswordFile(_password, (path) async {
        final r = await invoker.invoke<WalletDetail>(
          BtcCommand.walletShow(
            id: walletId,
            network: network,
            passwordFilePath: path,
          ),
          parse: (j) => WalletDetail.fromJson(j as Map<String, dynamic>),
        );
        detail = r;
      });
      final d = detail;
      if (d == null || !mounted) return;
      // Re-assert identity: if widget was rebuilt with a different
      // walletId mid-await, do NOT populate the wrong family.
      if (widget.walletId != walletId || widget.network != network) return;
      // Force-clear our screen-side password copy before unlocking the
      // session. The `PasswordField`'s controller is cleared in its own
      // dispose (Task 15); see v0.2 backlog for an explicit `clear()`
      // seam.
      setState(() {
        _password = '';
      });
      // Populate the session via the dedicated read-only factory
      // (Task 20 L12 type-design HIGH). The empty-string mnemonic
      // sentinel is constructed inside `unlockWithDetail` so the
      // convention doesn't leak to callers.
      ref
          .read(walletSessionProvider(walletId).notifier)
          .unlockWithDetail(d);
    } on BtcError catch (e) {
      if (mounted) setState(() => _error = e);
    } catch (e, st) {
      const filter = BtcLogFilter();
      developer.log(
        'wallet_detail unlock failed',
        name: 'WalletDetailScreen',
        error: filter.redact(e.toString()),
        stackTrace: st,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Could not unlock wallet.')),
        );
      }
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  void _lock() {
    ref.read(walletSessionProvider(widget.walletId).notifier).lock();
    setState(() {
      _password = '';
      _error = null;
    });
  }

  @override
  Widget build(BuildContext context) {
    // Defense-in-depth (L12 type-design Task 20 LOW): router-level
    // `redirect:` is the canonical walletId validator, but a parent
    // that bypasses the router (or a future refactor that swaps the
    // redirect for one that doesn't run on initial-load) could surface
    // a path-injection-shaped id. Assert here so the failure mode is a
    // visible runtime error rather than a silently-truncated format.
    assert(
      WalletRoutes.isValidWalletIdSegment(widget.walletId),
      'invalid walletId segment: ${widget.walletId}',
    );
    final session = ref.watch(walletSessionProvider(widget.walletId));
    if (session == null) return _buildUnlockForm(context);
    final detail = session.detail;
    if (detail == null) return _buildUnlockForm(context);
    return _buildUnlockedView(context, detail);
  }

  Widget _buildUnlockForm(BuildContext context) {
    final error = _error;
    return Scaffold(
      appBar: AppBar(
        title: Text('Unlock ${formatWalletId(widget.walletId)}'),
      ),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            PasswordField(
              onChanged: (v) => _password = v,
              onSubmitted: (_) => _unlock(),
            ),
            const SizedBox(height: 16),
            if (error != null) ...[
              StatusBadge(kind: error.kind),
              const SizedBox(height: 8),
              Text(userMessageForBtcError(error)),
              const SizedBox(height: 8),
            ],
            FilledButton(
              key: const Key('wallet_detail_unlock'),
              onPressed: _running ? null : _unlock,
              child: _running
                  ? const Text('Unlocking…')
                  : const Text('Unlock'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildUnlockedView(BuildContext context, WalletDetail d) {
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      appBar: AppBar(
        title: Text('Wallet ${formatWalletId(d.id)}'),
        actions: [
          IconButton(
            key: const Key('wallet_detail_send'),
            icon: const Icon(Icons.send),
            tooltip: 'Send',
            onPressed: () => context.go(
              WalletRoutes.send(widget.network, widget.walletId),
            ),
          ),
          IconButton(
            key: const Key('wallet_detail_history'),
            icon: const Icon(Icons.history),
            tooltip: 'Transactions',
            onPressed: () => context.go(
              WalletRoutes.transactions(widget.network, widget.walletId),
            ),
          ),
          IconButton(
            key: const Key('wallet_detail_lock'),
            icon: const Icon(Icons.lock),
            tooltip: 'Lock',
            onPressed: _lock,
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            BalanceCard(balance: d.balance),
            const SizedBox(height: 16),
            Text(
              'Network: ${d.network}',
              style: textTheme.bodyMedium,
            ),
            Text(
              'Type: ${d.addressType}',
              style: textTheme.bodyMedium,
            ),
            const SizedBox(height: 16),
            AddressChip(address: d.firstAddress, network: d.network),
          ],
        ),
      ),
    );
  }
}
