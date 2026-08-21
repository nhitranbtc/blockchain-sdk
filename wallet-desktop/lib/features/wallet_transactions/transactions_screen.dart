import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/ffi/ffi_exception.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../providers/wallet_core_provider.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';
import '../../widgets/mnemonic_paste_field.dart';
import '../../widgets/process_progress_overlay.dart';

/// Read-only transaction history (Story 7).
///
/// **FFI migration (Task 17 / Issue #223).** Reads txids via
/// `walletCore.walletTxids(walletHandle)` (Rust `wallet_txids` export
/// in `bdk_extras.rs:748-770`). Migrated off `BtcInvoker.invoke<TxRecord>
/// (BtcCommand.txList(...))` per user directive (2026-08-21): "ensure
/// in wallet-desktop only integrate with btc-wallet-core, don't
/// integrate with btc cli".
///
/// **v0.2.1 limitation**: returns txid hex only — no per-tx fields
/// (direction, amount, confirmations). The richer `wallet_tx_history`
/// FFI export requires Rust-side work to query bdk's transaction
/// metadata and is deferred to v0.3 (see #221 closure). The list view
/// renders one row per txid; tap-to-explorer is a v0.3 follow-up.
///
/// **Three render branches** (decide on `walletSessionProvider` state):
///
/// 1. `state == null` → LockedView (back-to-unlock prompt).
///
/// 2. `state.mnemonic.value.isEmpty` → the Task 20 sentinel: wallet
///    was unlocked by `btc wallet show` (read-only) but the CLI does
///    not return the mnemonic, so the session has no signing key for
///    `btc tx-list --mnemonic`. Render `MnemonicPasteField` (Task 15
///    + Task 21 `onSubmit` hook) so the user re-pastes the phrase.
///    On submit, the screen calls
///    `walletSessionProvider(walletId).notifier.unlock(mnemonic: x,
///    detail: session.detail)` to preserve the parsed `WalletDetail`
///    (Lesson 32.1 sentinel stays in one place).
///
/// 3. `state.mnemonic.value.isNotEmpty` → tx-list view.
///
/// **Identity discipline** (Lesson 32.2): every async-await handler
/// captures `walletId/network` at the top, re-asserts them before
/// mutating provider state, and gates submit on `widget.walletId ==
/// captured` so a mid-load rebuild cannot cross-key a different
/// wallet family.
class TransactionsScreen extends ConsumerStatefulWidget {
  const TransactionsScreen({
    super.key,
    required this.network,
    required this.walletId,
  });
  final String network;
  final String walletId;

  @override
  ConsumerState<TransactionsScreen> createState() => _TransactionsScreenState();
}

class _TransactionsScreenState extends ConsumerState<TransactionsScreen> {
  List<String>? _txids;
  bool _running = false;
  FfiException? _error;

  /// GlobalKey into the `MnemonicPasteField` so the re-entry view's
  /// "Unlock for viewing" button can call
  /// `fieldKey.currentState?.submit()` without the screen holding a
  /// screen-side cache of the typed mnemonic (L33.3 — no unwired
  /// branches on critical surfaces). Typed `MnemonicPasteFieldState`
  /// (L12 type-design Task 22 MEDIUM) so a future rename of the
  /// field's `submit()` method surfaces as a compile error instead
  /// of a runtime `NoSuchMethodError`. The widget had to expose the
  /// State class publicly for this typed-key pattern.
  final GlobalKey<MnemonicPasteFieldState> _mnemonicFieldKey =
      GlobalKey<MnemonicPasteFieldState>();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      final walletId = widget.walletId;
      final network = widget.network;
      // Ensure FFI handles exist before any FFI calls (Task 14 /
      // Issue #220 Sub-split B-step-2 pattern). Idempotent: if
      // handles already exist, no-op.
      try {
        await ref
            .read(walletSessionProvider(walletId).notifier)
            .ensureHandles();
      } catch (e) {
        developer.log(
          'ensureHandles failed',
          name: 'transactions_screen',
          error: e,
        );
      }
      if (!mounted) return;
      _load(walletId, network);
    });
    ref.listenManual<Object?>(
      walletSessionProvider(widget.walletId),
      (prev, next) {
        if (prev != null && next == null && mounted) {
          setState(() {
            _running = false;
            _error = null;
            _txids = null;
          });
        }
      },
    );
  }

  Future<void> _load(String walletId, String network) async {
    if (!mounted) return;
    if (widget.walletId != walletId || widget.network != network) return;
    final session = ref.read(walletSessionProvider(walletId));
    if (session == null) return;
    final mnemonic = session.mnemonic.value;
    if (mnemonic.isEmpty) return;
    setState(() {
      _running = true;
      _error = null;
    });
    try {
      // Re-assert identity before FFI call (Lesson 32.2).
      if (widget.walletId != walletId || widget.network != network) return;
      final walletHandle = session.walletHandle;
      if (walletHandle == null) {
        // `ensureHandles()` failed earlier (e.g., no Esplora connection).
        // Surface as a typed FfiException so the UI renders the
        // kind-mapped copy (FfiErrorKind.notInitialized → "Wallet not
        // initialized — try unlocking again.").
        throw FfiException.fromCode(
          code: -1,
          op: 'wallet_txids',
          messageForDebug: 'ensureHandles did not run or failed',
        );
      }
      if (!mounted) return;
      if (widget.walletId != walletId || widget.network != network) return;
      final core = ref.read(walletCoreProvider);
      final txids = core.walletTxids(walletHandle: walletHandle);
      if (!mounted) return;
      if (widget.walletId != walletId || widget.network != network) return;
      setState(() => _txids = txids);
    } on FfiException catch (e) {
      if (mounted && widget.walletId == walletId && widget.network == network) {
        setState(() => _error = e);
      }
    } catch (e, st) {
      const filter = BtcLogFilter();
      developer.log(
        'transactions_screen load failed',
        name: 'TransactionsScreen',
        error: filter.redact(e.toString()),
        stackTrace: st,
      );
      if (mounted && widget.walletId == walletId && widget.network == network) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Could not load transactions.')),
        );
      }
    } finally {
      if (mounted && widget.walletId == walletId && widget.network == network) {
        setState(() => _running = false);
      }
    }
  }

  /// Sentinel-clear: re-paste the mnemonic on the unlocked session.
  /// Preserves the parsed `WalletDetail` (L12 type-design Task 21
  /// HIGH — `unlock(mnemonic:)` would otherwise drop detail to null).
  void _onReentrySubmit(String mnemonic) {
    final session = ref.read(walletSessionProvider(widget.walletId));
    if (session == null) return;
    final priorDetail = session.detail;
    ref
        .read(walletSessionProvider(widget.walletId).notifier)
        .unlock(mnemonic: mnemonic, detail: priorDetail);
    if (mounted) _load(widget.walletId, widget.network);
  }

  @override
  Widget build(BuildContext context) {
    assert(
      WalletRoutes.isValidWalletIdSegment(widget.walletId),
      'invalid walletId segment: ${widget.walletId}',
    );
    final session = ref.watch(walletSessionProvider(widget.walletId));
    if (session == null) return _buildLockedView();
    if (session.mnemonic.value.isEmpty) {
      return _buildMnemonicReentryView(context);
    }
    return _buildTxListView(context);
  }

  Widget _buildLockedView() {
    return Scaffold(
      appBar: AppBar(title: Text('Transactions (${widget.network})')),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Text('Wallet is locked.'),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: () {
                Navigator.of(context).pushReplacementNamed(
                  WalletRoutes.detail(widget.network, widget.walletId),
                );
              },
              child: const Text('Unlock'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildMnemonicReentryView(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text('Transactions (${widget.network})')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const Text(
              'Re-enter the mnemonic to view the transaction history.',
            ),
            const SizedBox(height: 16),
            MnemonicPasteField(
              key: _mnemonicFieldKey,
              expectedWordCount: 12,
              onChanged: (_) {},
              onSubmit: _onReentrySubmit,
            ),
            const SizedBox(height: 16),
            FilledButton(
              key: const Key('transactions_screen_mnemonic_unlock'),
              onPressed: _running
                  ? null
                  : () {
                      _mnemonicFieldKey.currentState?.submit();
                    },
              child: const Text('Unlock for viewing'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildTxListView(BuildContext context) {
    final error = _error;
    final txids = _txids;
    return Scaffold(
      appBar: AppBar(title: const Text('Transactions')),
      body: Stack(
        children: [
          if (error != null)
            Center(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Text(
                  'Could not load transactions: '
                  '${error.kind.name}',
                ),
              ),
            )
          else if (txids == null)
            const Center(child: CircularProgressIndicator())
          else if (txids.isEmpty)
            const Center(
              child: Padding(
                padding: EdgeInsets.all(16),
                child: Text(
                  'No transactions yet.\n'
                  'Detailed tx history (sender, amount, confirmations) '
                  'lands in v0.3.',
                  textAlign: TextAlign.center,
                ),
              ),
            )
          else
            ListView.builder(
              itemCount: txids.length,
              itemBuilder: (_, i) {
                final txid = txids[i];
                return ListTile(
                  key: ValueKey(txid),
                  title: Text(
                    txid,
                    style: const TextStyle(fontFamily: 'monospace'),
                  ),
                  subtitle: const Text(
                    'Tap-to-explorer in v0.3',
                  ),
                );
              },
            ),
          ProcessProgressOverlay(isRunning: _running),
        ],
      ),
    );
  }
}
