// Task 11 (#217) — `WalletCreateScreen` migrated from
// `btcInvokerProvider` + `withPasswordFile` (Task 5/6) +
// `BtcCommand.walletCreate` to `walletCoreProvider` +
// `walletCore.createWallet(password: SecretBuffer, ...)`. Returns
// `WalletCreatedData { id, mnemonic: MnemonicView, ... }`.
//
// **F47 zeroization gap closure (Task 11 CRITICAL).** The
// plaintext mnemonic NEVER lands as a `String` field anywhere in
// this screen or in `MnemonicDisplayDialog`. The password is
// routed through `SecretBuffer.fromUtf8` (Task 8 RAII newtype —
// zeros calloc buffer on dispose); the returned `MnemonicView`
// wraps the Rust `MnemonicHandle` (which holds a `Secret<Vec<u8>>`
// inside Rust — zeroize-on-drop). The dialog reads the phrase via
// `MnemonicView.read()` only inside the Reveal branch.
//
// **Secret handling** (L12 CRITICAL #2 + Task 5/6/7/8/10 chain):
// - `SecretBuffer.fromUtf8(_password)` → FFI call → facade
//   auto-disposes the buffer in its `finally` block.
// - `_password` (the screen's own copy) is force-cleared BEFORE
//   showing the dialog. The `PasswordField`'s internal
//   `TextEditingController` is cleared in its own `dispose()`
//   (which fires after navigation). v0.2 follow-up: explicit
//   `PasswordField.clear()` seam.
// - Dialog receives `result.mnemonic` (MnemonicView) — NOT a
//   String phrase. The dialog calls `mnemonic.dispose()` in its
//   own `State.dispose()`.
//
// **Error funnel** (Task 10 pattern): `FfiException` →
// `userMessageForFfiException(e)` for kind-mapped user copy;
// everything else renders the redacted `toString()`. Raw FFI
// codes / stderr NEVER surface in the UI.
//
// **Post-success**: refresh `walletsListProvider` via
// `ref.invalidate` (Task 13 L31 lesson). The list screen's
// AsyncValue.error UI + Retry button covers user-visible failure.

import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../core/ffi/ffi_enums.dart';
import '../../core/ffi/ffi_exception.dart';
import '../../core/ffi/secret_buffer.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../providers/app_paths_provider.dart';
import '../../providers/wallet_core_provider.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';
import '../../widgets/password_field.dart';
import 'mnemonic_display_dialog.dart'
    show MnemonicDisplayDialog, MnemonicWordCount;

class WalletCreateScreen extends ConsumerStatefulWidget {
  const WalletCreateScreen({super.key, required this.network});
  final String network;

  @override
  ConsumerState<WalletCreateScreen> createState() => _WalletCreateScreenState();
}

class _WalletCreateScreenState extends ConsumerState<WalletCreateScreen> {
  // L12 review (Task 11) fix: typed enum replaces bare `int _words`
  // (which silently accepted 15/18/21 and coerced to 12).
  MnemonicWordCount _words = MnemonicWordCount.twelve;
  FfiAddressType _addressType = FfiAddressType.nativeSegwit;
  String _password = '';
  bool _running = false;
  String? _error;

  Future<void> _submit() async {
    if (_password.isEmpty) return;
    setState(() {
      _running = true;
      _error = null;
    });
    try {
      final core = ref.read(walletCoreProvider);
      final appPaths = await ref.read(appPathsProvider.future);
      // FFI call: facade owns the SecretBuffer lifetime
      // (auto-disposes in `finally`). Returns WalletCreatedData
      // with a typed MnemonicView handle (Task 8 RAII).
      final result = core.createWallet(
        words: _words.value,
        network: FfiNetwork.testnet,
        addressType: _addressType,
        password: SecretBuffer.fromUtf8(_password),
        baseDir: appPaths.walletDataDir.path,
      );

      if (!mounted) {
        // Defensive: the FFI succeeded but the screen is gone.
        // Dispose the mnemonic handle to free the Rust-side
        // MnemonicHandle + zeroize the cached phrase.
        result.mnemonic.dispose();
        return;
      }
      // Force-clear our password field state before showing mnemonic.
      // L12 review (flutter M2 fix): drop `setState` — `_password`
      // is not read by `build()`, so no rebuild is needed.
      _password = '';
      await showDialog<void>(
        context: context,
        barrierDismissible: false,
        builder: (_) => MnemonicDisplayDialog(
          // H1 fix: word count as typed param (no Key back-channel).
          wordCount: _words,
          mnemonic: result.mnemonic,
          walletId: result.id,
        ),
      );
      // After dialog closes, the dialog's `dispose()` already
      // freed the MnemonicHandle. Belt-and-suspenders: double-free
      // is a no-op (`MnemonicView.dispose` is idempotent).
      result.mnemonic.dispose();
      if (!mounted) return;
      // Deliberately NOT calling `walletSessionProvider(result.id)
      // .unlock(mnemonic: ...)` — the v0.1 design leaves unlock to
      // Task 13 (WalletDetailScreen). v0.2 may move unlock here.
      context.go(WalletRoutes.wallets(widget.network));
      ref.invalidate(walletsListProvider(widget.network));
    } on FfiException catch (e) {
      // Kind-mapped user copy (Task 10 MED #3 closure).
      if (mounted) setState(() => _error = userMessageForFfiException(e));
    } catch (e, st) {
      // Defense-in-depth: dart:developer.log bypasses
      // package:logging, so pipe the redacted form through
      // ourselves (Task 17 lesson). The `FfiException.toString()`
      // contract excludes `messageForDebug`, so this branch is
      // safe by construction for typed errors.
      const filter = BtcLogFilter();
      developer.log(
        'wallet_create failed',
        name: 'WalletCreateScreen',
        error: filter.redact(e.toString()),
        stackTrace: st,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Could not create wallet.')),
        );
      }
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final error = _error;
    return Scaffold(
      appBar: AppBar(title: Text('Create wallet (${widget.network})')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            DropdownButtonFormField<MnemonicWordCount>(
              initialValue: _words,
              decoration: const InputDecoration(labelText: 'Words'),
              items: const [
                DropdownMenuItem(
                  value: MnemonicWordCount.twelve,
                  child: Text('12'),
                ),
                DropdownMenuItem(
                  value: MnemonicWordCount.twentyFour,
                  child: Text('24'),
                ),
              ],
              onChanged: (v) =>
                  setState(() => _words = v ?? MnemonicWordCount.twelve),
            ),
            const SizedBox(height: 16),
            // Task 11 plan deviation: dropped `'legacy'` — Rust
            // `FfiAddressType` only supports nativeSegwit (0),
            // nestedSegwit (1), taproot (2), unknown (255). The
            // legacy dropdown entry would map to `unknown` and
            // surface as `FfiError::Unknown` at the FFI parse step.
            DropdownButtonFormField<FfiAddressType>(
              initialValue: _addressType,
              decoration: const InputDecoration(labelText: 'Address type'),
              items: const [
                DropdownMenuItem(
                  value: FfiAddressType.nestedSegwit,
                  child: Text('nested-segwit'),
                ),
                DropdownMenuItem(
                  value: FfiAddressType.nativeSegwit,
                  child: Text('native-segwit'),
                ),
                DropdownMenuItem(
                  value: FfiAddressType.taproot,
                  child: Text('taproot'),
                ),
              ],
              onChanged: (v) => setState(
                  () => _addressType = v ?? FfiAddressType.nativeSegwit),
            ),
            const SizedBox(height: 16),
            PasswordField(onChanged: (v) => _password = v),
            const SizedBox(height: 16),
            if (error != null) ...[
              Text(error),
              const SizedBox(height: 8),
            ],
            FilledButton(
              key: const Key('wallet_create_submit'),
              onPressed: _running ? null : _submit,
              child: _running ? const Text('Creating…') : const Text('Create'),
            ),
          ],
        ),
      ),
    );
  }
}
