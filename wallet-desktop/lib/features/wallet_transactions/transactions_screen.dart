import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/btc/btc_command.dart';
import '../../core/btc/btc_error.dart';
import '../../core/btc/btc_error_messages.dart';
import '../../core/btc/models/tx_record.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../providers/btc_providers.dart';
import '../../providers/esplora_config_provider.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';
import '../../widgets/mnemonic_paste_field.dart';
import '../../widgets/process_progress_overlay.dart';
import '../../widgets/status_badge.dart';

/// Read-only transaction history (Story 7). Calls `btc tx-list
/// --mnemonic <words> --json` via `BtcInvoker` and renders one row
/// per tx.
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
/// 3. `state.mnemonic.value.isNotEmpty` → tx-list view. The mnemonic
///    lives ONLY in `session.mnemonic.value` — never copied into a
///    screen-side field (L33.1 pure-build + L12 security-auditor
///    Task 21 cleartext-lifetime discipline).
///
/// **v0.1 mnemonic-in-argv gap** (mirrors Task 18/19): `BtcCommand
/// .txList` passes `--mnemonic <words>` in argv; `BtcInvoker` does
/// NOT log argv (Task 10); the `BtcLogFilter` regex covers 12-24
/// word-run shape for the catch path's `developer.log`. v0.2 will
/// introduce a stdin-mode for mnemonic-only CLI calls (Task 8 backlog)
/// for true secret-passing parity.
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
  List<TxRecord>? _txs;
  bool _running = false;
  BtcError? _error;

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
    // Capture identity at top of postFrame callback (L32.2).
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final walletId = widget.walletId;
      final network = widget.network;
      _load(walletId, network);
    });
    // External lock from another screen — clear in-flight state so
    // a stale `_running` doesn't survive across sessions.
    ref.listenManual<Object?>(
      walletSessionProvider(widget.walletId),
      (prev, next) {
        if (prev != null && next == null && mounted) {
          setState(() {
            _running = false;
            _error = null;
            _txs = null;
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
      final invoker = await ref.read(btcInvokerProvider.future);
      final esploraCfg = await ref.read(esploraConfigProvider.future);
      if (!mounted) return;
      if (widget.walletId != walletId || widget.network != network) return;
      final txs = await invoker.invoke<List<TxRecord>>(
        BtcCommand.txList(
          mnemonic: mnemonic,
          network: network,
          esploraUrl: esploraCfg.url,
          esploraSpkiPin: esploraCfg.spkiPin,
          limit: 100,
        ),
        // L12 flutter-reviewer Task 22 HIGH: defensive `is List` check
        // (matches Task 17 `WalletsListNotifier` pattern). When the
        // CLI returns empty stdout, `BtcInvoker` invokes `parse(null)`
        // (line 146 of btc_invoker.dart) — the prior `(j as List)`
        // cast threw TypeError, wrapped as `BtcError(kind: other)`,
        // and surfaced as "Something went wrong." for empty wallets.
        // Test stub `_FakeBtcInvoker` calls `parse(<Map>[])` directly
        // and masked this bug; the real CLI path returns `null`.
        parse: (j) => j is List
            ? j
                .map((e) => TxRecord.fromJson(e as Map<String, dynamic>))
                .toList(growable: false)
            : const <TxRecord>[],
      );
      if (!mounted) return;
      if (widget.walletId != walletId || widget.network != network) return;
      setState(() => _txs = txs);
    } on BtcError catch (e) {
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
    // Trigger the load after the session re-keys.
    if (mounted) _load(widget.walletId, widget.network);
  }

  @override
  Widget build(BuildContext context) {
    // L32.3: defence-in-depth — catch parent bypasses of router-level
    // redirect (deep link, programmatic nav, test harness).
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
                // Use `pushReplacement` so deep-link entry to
                // /transactions does not stack a redundant detail
                // screen on top of the current one (L33.2 pattern
                // from Task 21).
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
              // v0.2 follow-up (L12 flutter-reviewer Task 22 MEDIUM):
              // hardcoded `12` silently rejects valid mnemonics for
              // 15/18/21/24-word wallets with "Expected 12 words;
              // got N" — the user can't unlock and the screen
              // dead-ends. Until the wallet-detail exposes a
              // `words` field, document the user-impact severity
              // here rather than the v0.2 dropdown comment that
              // understates it.
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
                      // Typed `State<MnemonicPasteField>` (no cast)
                      // — the compiler will flag any future rename
                      // of the field's `submit()` method (L12
                      // type-design Task 22 MEDIUM).
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
    final txs = _txs;
    return Scaffold(
      appBar: AppBar(title: const Text('Transactions')),
      body: Stack(
        children: [
          if (error != null)
            Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  StatusBadge(kind: error.kind),
                  const SizedBox(height: 8),
                  Text(userMessageForBtcError(error)),
                ],
              ),
            )
          else if (txs == null)
            const Center(child: CircularProgressIndicator())
          else if (txs.isEmpty)
            const Center(child: Text('No transactions yet'))
          else
            ListView.builder(
              itemCount: txs.length,
              itemBuilder: (_, i) {
                final t = txs[i];
                return ListTile(
                  title: Text(
                    t.txid,
                    style: const TextStyle(fontFamily: 'monospace'),
                  ),
                  subtitle: Text(
                    '${t.direction.name} • ${t.amountSat} sats • '
                    '${t.confirmations} conf',
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
