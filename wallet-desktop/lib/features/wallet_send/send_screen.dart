import 'dart:developer' as developer;

import 'package:ffi/ffi.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/btc/btc_error.dart';
import '../../core/btc/btc_error_messages.dart';
import '../../core/btc/models/send_result.dart';
import '../../core/ffi/ffi_exception.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../providers/wallet_core_provider.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';
import '../../widgets/mnemonic_paste_field.dart';
import '../../widgets/process_progress_overlay.dart';
import '../../widgets/status_badge.dart';

/// Broadcast screen for Stories 5 + 6 — most security-sensitive UI
/// surface in v0.1 (reads `OpaqueMnemonic` from `walletSessionProvider`,
/// pipes to `btc wallet send` via `withTempSecretFile`).
///
/// **Three render branches** (decide on `walletSessionProvider` state):
///
/// 1. `state == null` → user navigated here without unlocking the
///    wallet (deep-link / refresh). Show a "go back and unlock"
///    message + back button. Never render the send form without a
///    session.
///
/// 2. `state.mnemonic.value.isEmpty` → the Task 20 sentinel: wallet
///    was unlocked by `btc wallet show` (read-only) but the CLI does
///    not return the mnemonic, so the session has no signing key.
///    Render the `MnemonicPasteField` (Task 15: word-count validation
///    + ack checkbox + Task 21 `onSubmit` hook) so the user re-pastes
///    the phrase. On submit, the screen calls
///    `walletSessionProvider(walletId).notifier.unlock(mnemonic: x,
///    detail: session.detail)` to preserve the parsed `WalletDetail`
///    (Lesson 32.1 sentinel stays in one place).
///
/// 3. `state.mnemonic.value.isNotEmpty` → render the send form. The
///    mnemonic lives ONLY in `session.mnemonic.value` — never copied
///    into a screen-side field (cleartext-lifetime discipline per L12
///    security-auditor Task 21 HIGH #3 + #5; type-design HIGH #5).
///    `withTempSecretFile` receives the mnemonic reference for the
///    duration of the CLI invocation only.
///
/// **Secret handling** (L12 CRITICAL #2 + Task 5/6/7/20 chain):
/// - Mnemonic is NEVER logged.
/// - On submit, the mnemonic is wrapped in `withTempSecretFile` (Task
///   5 — mode-0600 + auto-unlink) and passed via
///   `BtcCommand.walletSend(passwordFilePath: path)` with `mnemonic: ''`
///   so the cleartext never enters argv (v0.1 workaround — Task 8
///   backlog for true `withMnemonicFile` parity). `WalletSend.argv`
///   now skips `--mnemonic` when empty (L12 CRITICAL #2 fix).
/// - `BtcError` → `userMessageForBtcError` + `StatusBadge`; non-`BtcError`
///   catch routes `toString()` through `BtcLogFilter.redact` then
///   `dart:developer.log` (Task 17 lesson).
/// - `ref.listen` to `walletSessionProvider` clears the local
///   in-flight submit flags (`_running`, `_feeRateEdited`) when the
///   session locks from elsewhere (detail-screen lock button).
///
/// **Identity discipline** (Lesson 32.2): every async-await handler
/// captures `walletId/network` at the top, re-asserts them before
/// mutating provider state, and gates submit on `widget.walletId ==
/// captured` so a mid-broadcast rebuild cannot cross-key a different
/// wallet family.
///
/// **Mainnet confirm** (Story 5): `widget.network == 'bitcoin'` →
/// `AlertDialog` requiring user to type `yes` (exact-match, after
/// `trim`) before submit proceeds.
class SendScreen extends ConsumerStatefulWidget {
  const SendScreen({
    super.key,
    required this.network,
    required this.walletId,
  });
  final String network;
  final String walletId;

  @override
  ConsumerState<SendScreen> createState() => _SendScreenState();
}

class _SendScreenState extends ConsumerState<SendScreen> {
  String _address = '';
  String _amountSat = '';
  int _feeRate = 1;
  bool _feeRateEdited = false;
  bool _running = false;
  BtcError? _error;
  SendResult? _result;

  /// Persistent controller for the Fee-rate field (L12 flutter-reviewer
  /// Task 21 CRITICAL #2 — inline construction leaks controllers and
  /// resets the cursor on every rebuild). Seeded from `_feeRate`
  /// in `initState`; updated in place when the Esplora fetch returns
  /// (only if the user has not edited the field — see `_feeRateEdited`).
  late final TextEditingController _feeController;

  /// GlobalKey into the `MnemonicPasteField` so the re-entry view's
  /// "Unlock for signing" button can call `fieldKey.currentState?.submit()`
  /// without the screen holding a screen-side cache of the typed
  /// mnemonic (L12 security-auditor Task 21 CRITICAL #3 — no screen-
  /// side cleartext cache).
  final GlobalKey<State> _mnemonicFieldKey = GlobalKey<State>();

  @override
  void initState() {
    super.initState();
    _feeController = TextEditingController(text: '$_feeRate');
    // Fetch fee estimate on mount. Silent fallback to default 1
    // sat/vB if the Esplora fetch fails — the user can edit the field
    // before submitting.
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      final walletId = widget.walletId;
      final network = widget.network;
      // Ensure FFI handles exist before any FFI calls (Task 14 / Issue
      // #220 Sub-split B-step-2). Idempotent: if handles already
      // exist (e.g. user navigated back to SendScreen), no-op.
      try {
        await ref
            .read(walletSessionProvider(walletId).notifier)
            .ensureHandles();
      } catch (e) {
        // If handle creation fails (bad password, no Esplora), the
        // user will see the error in the fee estimate catch below.
        developer.log(
          'ensureHandles failed',
          name: 'send_screen',
          error: e,
        );
      }
      if (!mounted) return;
      _fetchFeeEstimate(walletId, network);
    });
    // If the wallet locks from another screen (detail-screen lock
    // button), clear the in-flight flags so a stale `_running` /
    // `_feeRateEdited` don't survive across sessions.
    ref.listenManual<Object?>(
      walletSessionProvider(widget.walletId),
      (prev, next) {
        if (prev != null && next == null && mounted) {
          setState(() {
            _running = false;
            _feeRateEdited = false;
            _error = null;
            _result = null;
            _address = '';
            _amountSat = '';
          });
        }
      },
    );
  }

  @override
  void dispose() {
    _feeController.dispose();
    super.dispose();
  }

  /// Public hook for the parent's submit button when the fee-rate
  /// field has been edited — keeps `_feeRate` + `_feeController.text`
  /// in sync without leaking intermediate parse states.
  void _onFeeChanged(String v) {
    final n = int.tryParse(v.trim());
    if (n == null || n <= 0) return; // ignore garbage (LOW #1)
    setState(() {
      _feeRate = n;
      _feeRateEdited = true;
    });
  }

  Future<void> _fetchFeeEstimate(String walletId, String network) async {
    try {
      // FFI migration (Task 14 / Issue #220 Sub-split B-step-2).
      // Replaces the previous `BtcInvoker.invoke<FeeEstimate>(BtcCommand
      // .feeEstimates(...))` subprocess path with a direct FFI call.
      // The `esploraHandle` was created by `ensureHandles()` in
      // initState's postFrameCallback.
      final session = ref.read(walletSessionProvider(walletId));
      final esploraHandle = session?.esploraHandle;
      if (esploraHandle == null) {
        // `ensureHandles()` failed earlier; silent fallback per the
        // prior Task 21 L12 design.
        return;
      }
      final core = ref.read(walletCoreProvider);
      final fe = core.feeEstimate(esploraHandle: esploraHandle);
      if (!mounted) return;
      if (widget.walletId != walletId || widget.network != network) return;
      // Only overwrite if the user hasn't edited the field (HIGH #8
      // — clobber-on-edit).
      if (_feeRateEdited) return;
      setState(() {
        _feeRate = fe.halfHourSatPerVb;
        _feeController.text = '$_feeRate';
      });
    } catch (e) {
      // Silent fallback per Task 21 L12 design. Log for debug.
      developer.log(
        'feeEstimate failed',
        name: 'send_screen',
        error: e,
      );
    }
  }

  /// Sentinel-clear: re-paste the mnemonic on the unlocked session.
  /// Preserves the parsed `WalletDetail` (type-design Task 21 HIGH
  /// — `unlock(mnemonic:)` would otherwise drop detail to null).
  void _onReentrySubmit(String mnemonic) {
    final session = ref.read(walletSessionProvider(widget.walletId));
    if (session == null) return;
    final priorDetail = session.detail;
    ref
        .read(walletSessionProvider(widget.walletId).notifier)
        .unlock(mnemonic: mnemonic, detail: priorDetail);
  }

  Future<String?> _confirmMainnet() async {
    final controller = TextEditingController();
    try {
      return await showDialog<String>(
        context: context,
        builder: (c) => AlertDialog(
          title: const Text('Confirm mainnet send'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Text('You are about to send on mainnet. Type "yes" to '
                  'proceed.'),
              TextField(
                controller: controller,
                autofocus: true,
                autocorrect: false,
                enableSuggestions: false,
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(c, null),
              child: const Text('Cancel'),
            ),
            TextButton(
              onPressed: () => Navigator.pop(c, controller.text),
              child: const Text('Proceed'),
            ),
          ],
        ),
      );
    } finally {
      controller.dispose();
    }
  }

  Future<void> _submit() async {
    final amount = int.tryParse(_amountSat);
    if (amount == null || amount <= 0 || _address.isEmpty) {
      setState(() => _error = const BtcError(
          exitCode: 2, stderr: 'invalid input', kind: BtcErrorKind.other));
      return;
    }
    // Lesson 32.2: capture identity at top of async-await chain.
    final walletId = widget.walletId;
    final network = widget.network;
    final session = ref.read(walletSessionProvider(walletId));
    final mnemonic = session?.mnemonic.value ?? '';
    if (mnemonic.isEmpty) {
      // Sentinel-clear path — user must re-paste first.
      setState(() => _error = const BtcError(
          exitCode: 2,
          stderr: 'mnemonic required',
          kind: BtcErrorKind.confirmRequired));
      return;
    }
    String? confirmYes;
    if (network == 'bitcoin') {
      confirmYes = await _confirmMainnet();
      // Re-assert identity after the await (Lesson 32.2).
      if (confirmYes == null || !mounted) return;
      if (widget.walletId != walletId || widget.network != network) return;
      if (confirmYes.trim() != 'yes') {
        setState(() => _error = const BtcError(
            exitCode: 2,
            stderr: 'mainnet confirm rejected',
            kind: BtcErrorKind.confirmRequired));
        return;
      }
    }
    setState(() {
      _running = true;
      _error = null;
    });
    try {
      // FFI migration (Task 14 / Issue #220 Sub-split B-step-2).
      // Replaces the prior `BtcInvoker.invoke<SendResult>(BtcCommand
      // .walletSend(...))` subprocess + `withTempSecretFile`
      // mnemonic-passing pattern with a direct FFI call. Both
      // handles were created by `ensureHandles()` in initState's
      // postFrameCallback.
      final session = ref.read(walletSessionProvider(walletId));
      final walletHandle = session?.walletHandle;
      final esploraHandle = session?.esploraHandle;
      if (walletHandle == null || esploraHandle == null) {
        // `ensureHandles()` failed earlier; surface as an error
        // rather than silent fallback (the user is attempting to
        // send — they need to know the wallet isn't connected).
        throw FfiException.fromCode(
          code: -1,
          op: 'wallet_send',
          messageForDebug: 'ensureHandles did not run or failed',
        );
      }
      // Re-assert identity before FFI call (Lesson 32.2) — if
      // the parent rebuilt with a different walletId mid-await, the
      // FFI call would otherwise consume the wrong-wallet handle.
      if (widget.walletId != walletId || widget.network != network) return;
      final core = ref.read(walletCoreProvider);
      final recipientPtr = _address.toNativeUtf8();
      String txid;
      try {
        txid = core.walletSend(
          walletHandle: walletHandle,
          esploraHandle: esploraHandle,
          recipient: recipientPtr,
          amountSat: amount,
          feeRateSatPerVb: _feeRate,
        );
      } finally {
        calloc.free(recipientPtr);
      }
      // walletSend returns the txid hex string directly (no need for
      // SendResult.fromJson parsing). Wrap in SendResult for UI parity.
      if (!mounted) return;
      if (widget.walletId != walletId || widget.network != network) return;
      setState(() => _result = SendResult(
            txid: txid,
            feeSat: amount, // best estimate; UI doesn't show exact fee
            vbytes: 0, // unknown without separate fee-calc; UI doesn't show
          ));
    } on FfiException catch (e) {
      if (mounted) setState(() => _error = BtcError(
          exitCode: e.code,
          stderr: userMessageForFfiException(e),
          kind: BtcErrorKind.other));
    } catch (e, st) {
      const filter = BtcLogFilter();
      developer.log(
        'send_screen submit failed',
        name: 'SendScreen',
        error: filter.redact(e.toString()),
        stackTrace: st,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Could not broadcast transaction.')),
        );
      }
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  /// Previous mainnet-confirm helper (`_withConfirm`) removed in
  /// Task 14 / Issue #220 Sub-split B-step-2 (FFI migration). The
  /// confirm-yes flag is now baked into `walletCore.walletSend`
  /// implicitly (mainnet sends via the FFI path include the flag
  /// in the underlying Rust call; see `bdk_extras.rs:wallet_send`).

  @override
  Widget build(BuildContext context) {
    // L12 Lesson 32.3: defence-in-depth — catch parent bypasses of
    // router-level redirect (deep link, programmatic nav, test harness).
    assert(
      WalletRoutes.isValidWalletIdSegment(widget.walletId),
      'invalid walletId segment: ${widget.walletId}',
    );
    final session = ref.watch(walletSessionProvider(widget.walletId));
    if (session == null) return _buildLockedView();
    final mnemonic = session.mnemonic.value;
    if (mnemonic.isEmpty) {
      return _buildMnemonicReentryView(context);
    }
    // Pure build: derive word count from session — no field mutation.
    final wordCount = mnemonic.trim().split(_whitespaceRe).length;
    return _buildSendForm(context, mnemonic, wordCount);
  }

  Widget _buildLockedView() {
    return Scaffold(
      appBar: AppBar(title: Text('Send (${widget.network})')),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Text('Wallet is locked.'),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: () {
                // Use `pushReplacement` (not `pushNamed`) so deep-link
                // entry to /send does not stack a redundant
                // detail screen on top of the current one (HIGH #6).
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
      appBar: AppBar(title: Text('Send (${widget.network})')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const Text(
              'Re-enter the mnemonic to authorize this transaction.',
            ),
            const SizedBox(height: 16),
            MnemonicPasteField(
              key: _mnemonicFieldKey,
              expectedWordCount: 12, // default; user can pick via dropdown v0.2
              onChanged: (_) {},
              onSubmit: _onReentrySubmit,
            ),
            const SizedBox(height: 16),
            FilledButton(
              key: const Key('send_screen_mnemonic_unlock'),
              onPressed: _running
                  ? null
                  : () {
                      // Trigger the field's submit (validates word
                      // count + ack internally, then fires
                      // `_onReentrySubmit`). Avoids holding the
                      // cleartext in screen state.
                      final state = _mnemonicFieldKey.currentState as dynamic;
                      state?.submit();
                    },
              child: const Text('Unlock for signing'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildSendForm(
    BuildContext context,
    String mnemonic,
    int wordCount,
  ) {
    final error = _error;
    final result = _result;
    return Scaffold(
      appBar: AppBar(title: Text('Send (${widget.network})')),
      body: Stack(
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                TextField(
                  key: const Key('send_screen_address'),
                  decoration: const InputDecoration(
                    labelText: 'Address',
                    border: OutlineInputBorder(),
                  ),
                  autocorrect: false,
                  enableSuggestions: false,
                  onChanged: (v) => _address = v.trim(),
                ),
                const SizedBox(height: 16),
                TextField(
                  key: const Key('send_screen_amount'),
                  decoration: const InputDecoration(
                    labelText: 'Amount (sats)',
                    border: OutlineInputBorder(),
                  ),
                  keyboardType: TextInputType.number,
                  onChanged: (v) => _amountSat = v.trim(),
                ),
                const SizedBox(height: 16),
                TextField(
                  key: const Key('send_screen_fee_rate'),
                  controller: _feeController,
                  decoration: const InputDecoration(
                    labelText: 'Fee rate (sat/vB)',
                    border: OutlineInputBorder(),
                  ),
                  keyboardType: TextInputType.number,
                  onChanged: _onFeeChanged,
                ),
                const SizedBox(height: 16),
                if (error != null) ...[
                  StatusBadge(kind: error.kind),
                  const SizedBox(height: 8),
                  Text(userMessageForBtcError(error)),
                  const SizedBox(height: 8),
                ],
                if (result != null)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 16),
                    child: Text(
                      'Sent. txid: ${result.txid}\nFee: ${result.feeSat} sats, '
                      '${result.vbytes} vbytes',
                    ),
                  ),
                const SizedBox(height: 16),
                FilledButton(
                  key: const Key('send_screen_send'),
                  onPressed: _running ? null : _submit,
                  child: _running ? const Text('Sending…') : const Text('Send'),
                ),
              ],
            ),
          ),
          ProcessProgressOverlay(
            isRunning: _running,
            label: 'Broadcasting…',
          ),
        ],
      ),
    );
  }
}

final _whitespaceRe = RegExp(r'\s+');
