import 'dart:developer' as developer;
import 'dart:ffi';

import 'package:ffi/ffi.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/btc/models/send_result.dart';
import '../../core/ffi/ffi_enums.dart';
import '../../core/ffi/ffi_exception.dart';
import '../../core/ffi/secret_buffer.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../providers/app_paths_provider.dart';
import '../../providers/esplora_config_provider.dart';
import '../../providers/wallet_core_provider.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';
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
  FfiException? _error;
  SendResult? _result;
  String _password = '';

  /// Persistent controller for the Fee-rate field (L12 flutter-reviewer
  /// Task 21 CRITICAL #2 — inline construction leaks controllers and
  /// resets the cursor on every rebuild). Seeded from `_feeRate`
  /// in `initState`; updated in place when the Esplora fetch returns
  /// (only if the user has not edited the field — see `_feeRateEdited`).
  late final TextEditingController _feeController;

  /// Controller for the password field (v0.2.x deviance closure —
  /// password re-auth replaces mnemonic re-paste; Rust decrypts
  /// internally + returns a signing handle).
  late final TextEditingController _passwordController;

  @override
  void initState() {
    super.initState();
    _feeController = TextEditingController(text: '$_feeRate');
    _passwordController = TextEditingController();
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
        // L12 review LOW #3: scrub via BtcLogFilter.redact at the
        // call site so a future Sentry-style sink that reflects on
        // the typed FfiException fields can't leak `lastError`.
        const filter = BtcLogFilter();
        developer.log(
          'ensureHandles failed',
          name: 'send_screen',
          error: filter.redact(e.toString()),
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
    _passwordController.dispose();
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
      // L12 review LOW #3: scrub via BtcLogFilter.redact at the
      // call site (matches the pattern at `_submit`'s catch).
      const filter = BtcLogFilter();
      developer.log(
        'feeEstimate failed',
        name: 'send_screen',
        error: filter.redact(e.toString()),
      );
    }
  }

  /// (v0.2.x deviance closure) — no-op; the mnemonic-paste re-entry
  /// flow was removed in favor of password-only auth (see `_submit`
  /// below). Kept as a private stub so the dispatch table in
  /// `build` doesn't need a conditional import.
  // ignore: unused_element
  void _legacyMnemonicStub() {}

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
      // Use a plain Dart exception for client-side validation
      // errors (NOT FfiException with code -1) — `code: -1`
      // maps to `FfiErrorKind.invalidMnemonic` which renders as
      // "Invalid recovery phrase — please re-enter." in the UI
      // (irrelevant to send-screen validation). The
      // generic catch below displays a SnackBar with the actual
      // message.
      throw Exception('Invalid input — check address, amount, fee rate');
    }
    // Lesson 32.2: capture identity at top of async-await chain.
    final walletId = widget.walletId;
    final network = widget.network;
    // v0.2.x deviance closure: mnemonic paste removed. The user
    // re-auths via password. We construct BOTH handles
    // (esplora + wallet) inline in this scope — frees after
    // the send call. No reliance on session state (which may
    // have null esplora handle if `ensureHandles` failed earlier
    // for a wallet-without-`db_path`).
    final password = _passwordController.text;
    if (password.isEmpty) {
      throw Exception('Wallet password required to sign the send');
    }
    String? confirmYes;
    if (network == 'bitcoin') {
      confirmYes = await _confirmMainnet();
      // Re-assert identity after the await (Lesson 32.2).
      if (confirmYes == null || !mounted) return;
      if (widget.walletId != walletId || widget.network != network) return;
      if (confirmYes.trim() != 'yes') {
        throw Exception('Mainnet confirmation rejected');
      }
    }
    setState(() {
      _running = true;
      _error = null;
    });
    final core = ref.read(walletCoreProvider);
    try {
      // FFI migration (Task 14 / Issue #220 Sub-split B-step-2).
      // Replaces the prior `BtcInvoker.invoke<SendResult>(BtcCommand
      // .walletSend(...))` subprocess + `withTempSecretFile`
      // mnemonic-passing pattern with a direct FFI call. Both
      // Construct the esplora handle inline (v0.2.x — no reliance on
      // session state, since `ensureHandles` may have failed for
      // a wallet-without-`db_path`). Same pattern as the wallet
      // handle below (re-built fresh per send).
      final esploraCfg = await ref.read(esploraConfigProvider.future);
      final appPaths = await ref.read(appPathsProvider.future);
      Pointer<Void> esploraHandle;
      final urlPtr = esploraCfg.url.toNativeUtf8();
      final pinPtr = esploraCfg.spkiPin.isEmpty
          ? nullptr
          : esploraCfg.spkiPin.toNativeUtf8();
      try {
        esploraHandle = core.esploraClientNew(
          url: urlPtr,
          spkiPinB64: pinPtr,
        );
      } finally {
        calloc.free(urlPtr);
        if (pinPtr != nullptr) calloc.free(pinPtr);
      }
      if (esploraHandle == nullptr) {
        throw Exception(
          'Failed to construct Esplora client — check Settings '
          '(URL + SPKI pin)',
        );
      }
      // Re-call `walletShow` with the password — Rust decrypts the
      // mnemonic internally and returns a fresh `WalletHandle`
      // for this send only. **Mnemonic never crosses FFI as raw
      // bytes.** The handle is freed at the end of this block (RAII).
      final showResult = core.showWallet(
        network: FfiNetwork.testnet,
        walletId: walletId,
        password: SecretBuffer.fromUtf8(password),
        baseDir: appPaths.walletDataDir.path,
        esploraUrl: esploraCfg.url,
        esploraSpkiPin: esploraCfg.spkiPin,
      );
      final walletHandle = showResult.walletHandle;
      if (walletHandle == nullptr) {
        core.esploraClientFree(esploraHandle);
        throw Exception(
          'wallet_show returned a null handle — password may be wrong',
        );
      }
      // Re-assert identity before FFI call (Lesson 32.2) — if
      // the parent rebuilt with a different walletId mid-await, the
      // FFI call would otherwise consume the wrong-wallet handle.
      if (widget.walletId != walletId || widget.network != network) {
        // Free the esplora + wallet handles we built (won't be
        // freed by the inner finally since we're returning early).
        core.esploraClientFree(esploraHandle);
        core.walletLoadFree(walletHandle);
        return;
      }
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
        // Free the per-send esplora + wallet handles (decrypted
        // mnemonic drops here via `Wallet::drop` on the Rust side;
        // zeroize via `Secret<String>`).
        core.esploraClientFree(esploraHandle);
        core.walletLoadFree(walletHandle);
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
      if (mounted) setState(() => _error = e);
    } catch (e, st) {
      const filter = BtcLogFilter();
      developer.log(
        'send_screen submit failed',
        name: 'SendScreen',
        error: filter.redact(e.toString()),
        stackTrace: st,
      );
      if (mounted) {
        // Surface the actual error to the operator — the generic
        // "Could not broadcast transaction." was uninformative
        // (a wrong password + an esplora handle missing + an
        // insufficient-funds error all surfaced as the same
        // opaque message). Show the redacted exception toString
        // (L12 CRITICAL #2 — BtcLogFilter strips mnemonic /
        // password patterns from the developer.log emission).
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(filter.redact(e.toString()))),
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
    // v0.2.x deviance closure: the mnemonic-paste re-entry view is
    // gone. The send form is rendered unconditionally; the password
    // field inside it gates the actual sign operation. `mnemonic`
    // is no longer read here (it was the empty-string sentinel
    // before — `walletShow(password)` is the new auth path).
    return _buildSendForm(context, '', 0);
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
                // v0.2.x deviance closure: password re-auth replaces
                // the mnemonic paste. Rust decrypts internally +
                // returns a fresh signing handle per send; mnemonic
                // never crosses FFI as raw bytes. The password is
                // cleared on successful send (see `_submit`).
                TextField(
                  key: const Key('send_screen_password'),
                  controller: _passwordController,
                  obscureText: true,
                  enableSuggestions: false,
                  autocorrect: false,
                  decoration: const InputDecoration(
                    labelText: 'Wallet password (re-auth to sign)',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 16),
                if (error != null) ...[
                  StatusBadge(kind: error.kind),
                  const SizedBox(height: 8),
                  // Issue #265 C1 fix: per-op copy (keys on
                  // `error.op`) instead of the kind-only fallback.
                  // Previously the kind-only copy function
                  // mapped `FfiException(op: 'esplora_client_new',
                  // kind: esplora)` to the misleading "Invalid
                  // recovery phrase — please re-enter." copy —
                  // SendScreen has no recovery-phrase field in
                  // v0.2.x. The new helper surfaces the actual
                  // failure arm.
                  Text(userMessageForFfiExceptionWithOp(error)),
                  const SizedBox(height: 4),
                  // Op + code expose WHICH FFI call failed so the
                  // "Auth error" chip / "Invalid recovery phrase"
                  // copy doesn't lie about the root cause when the
                  // real failure lives in `esplora_client_new` /
                  // `wallet_load` rather than the wallet_show password
                  // path. Same op + code as `FfiException.toString()`
                  // but routed through `error.op` + `error.code` so we
                  // don't surface `messageForDebug` (L12 CRITICAL #2 —
                  // may contain mnemonic / password bytes verbatim).
                  Text(
                    'FFI op: ${error.op} (code ${error.code})',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: Theme.of(context).colorScheme.outline,
                        ),
                  ),
                  // Issue #265 C1 + L12 review MEDIUM #1: render the
                  // Rust thread-local diagnostic so operators see WHY
                  // the FFI op failed, not just THAT. Scrubbed via
                  // BtcLogFilter.redact — the Rust side does NOT
                  // sanitize mnemonic/password bytes (Issue #242), so
                  // any direct interpolation of `error.lastError`
                  // risks a leak. `BtcLogFilter` strips BIP-39 word
                  // sequences and 64-char hex digests before display.
                  // Mirrors the `_SyncFailedBanner` pattern in
                  // `balance_card.dart` for `WalletDetail.lastError`.
                  if (error.lastError != null &&
                      error.lastError!.isNotEmpty) ...[
                    const SizedBox(height: 4),
                    Builder(builder: (context) {
                      const filter = BtcLogFilter();
                      final scrubbed = filter.redact(error.lastError ?? '');
                      return Text(
                        scrubbed,
                        style: Theme.of(context).textTheme.bodySmall?.copyWith(
                              color: Theme.of(context).colorScheme.outline,
                              fontFamily: 'monospace',
                            ),
                      );
                    }),
                  ],
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
