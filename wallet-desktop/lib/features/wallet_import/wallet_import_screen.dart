// Task 12 (#218) — `WalletImportScreen` migrated from
// `btcInvokerProvider` + `withPasswordFile` (Task 5/6) +
// `BtcCommand.walletImport` to `walletCoreProvider` +
// `walletCore.importWallet(network, phrase: SecretBuffer,
// password: SecretBuffer, baseDir)`. Returns
// `WalletImportedData { id, network, addressType }`.
//
// **Secret handling** (L12 CRITICAL #2 + Task 8/10/11 chain):
// - The mnemonic is held in the `MnemonicPasteField`'s internal
//   `TextEditingController` until the user submits; the field's
//   `dispose()` clears it (Task 15 — `TextEditingController.clear()`
//   is NOT real zeroization; documented F47 sub-gap).
// - The password is held in the `PasswordField`'s controller;
//   `dispose()` clears it (same gap).
// - On submit: the screen copies `_mnemonic` + `_password` into
//   `SecretBuffer.fromUtf8` allocations that the FFI facade
//   auto-disposes in its `finally` block. The screen-side String
//   copies are then force-cleared BEFORE the navigator pop so
//   the route stack doesn't echo them.
// - `FfiException.toString()` excludes `messageForDebug` (Task 9
//   L12 CRITICAL #2 contract); the catch path's
//   `BtcLogFilter.redact(e.toString())` is defense-in-depth.
//
// **BIP-39 checksum**: v0.1 defers client-side validation to the
// Rust `wallet_import` (which rejects bad-checksum phrases with
// `FfiErrorKind.invalidMnemonic` — surfaced as user copy
// 'Invalid recovery phrase — please re-enter.').
//
// **Plan deviation: address type dropdown dropped.** Rust
// `wallet_import` does NOT persist `address_type` (verified at
// Task 8 — `WalletImportedData.addressType = FfiAddressType.unknown`).
// Surfacing it requires an extra `wallet_peek_addresses` call per
// imported wallet. For v0.2.0 the import screen drops the
// dropdown; UI derives the address type on the detail screen
// (Task 20). v0.2 follow-up: surface the picker once Rust
// `wallet_import` is enriched.
//
// **Post-success**: refresh `walletsListProvider` via
// `ref.invalidate` (Task 13 L31 lesson). Navigate to
// `WalletDetailScreen`; Task 20 owns the session-unlock flow.
import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../core/ffi/ffi_enums.dart';
import '../../core/ffi/ffi_exception.dart';
import '../../core/ffi/secret_buffer.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../providers/wallet_core_provider.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';
import '../../widgets/mnemonic_paste_field.dart';
import '../../widgets/password_field.dart';
import '../wallet_create/mnemonic_display_dialog.dart' show MnemonicWordCount;

class WalletImportScreen extends ConsumerStatefulWidget {
  const WalletImportScreen({super.key, required this.network});
  final String network;

  @override
  ConsumerState<WalletImportScreen> createState() => _WalletImportScreenState();
}

class _WalletImportScreenState extends ConsumerState<WalletImportScreen> {
  MnemonicWordCount _wordCount = MnemonicWordCount.twelve;
  String _mnemonic = '';
  String _password = '';
  bool _running = false;
  String? _error;

  Future<void> _submit() async {
    if (_mnemonic.trim().isEmpty || _password.isEmpty) return;
    setState(() {
      _running = true;
      _error = null;
    });
    try {
      final core = ref.read(walletCoreProvider);
      // FFI call: facade owns both SecretBuffer lifetimes
      // (auto-dispose in `finally`). Returns WalletImportedData
      // (no mnemonic returned — caller already has the phrase).
      final imported = core.importWallet(
        network: FfiNetwork.testnet,
        phrase: SecretBuffer.fromUtf8(_mnemonic),
        password: SecretBuffer.fromUtf8(_password),
        baseDir: '', // v0.2.0 stand-in
      );

      if (!mounted) return;
      // Force-clear screen-side password + mnemonic before
      // navigating. The fields' `TextEditingController`s clear
      // on their own dispose (later, on screen unmount).
      // L12 review (Task 11 flutter M2 fix): drop `setState` —
      // neither field is read by `build()` (the paste/password
      // fields own their controllers); no rebuild is needed.
      // Matches the Task 11 `WalletCreateScreen` pattern.
      _mnemonic = '';
      _password = '';
      ref.invalidate(walletsListProvider(widget.network));
      if (mounted) {
        context.go(WalletRoutes.detail(widget.network, imported.id));
      }
    } on FfiException catch (e) {
      // Kind-mapped user copy (Task 10 MED #3 closure). For
      // invalidMnemonic: 'Invalid recovery phrase — please
      // re-enter.' (kind-mapped by `userMessageForFfiException`).
      if (mounted) setState(() => _error = userMessageForFfiException(e));
    } catch (e, st) {
      // Defense-in-depth: dart:developer.log bypasses
      // package:logging. The `FfiException.toString()` contract
      // excludes `messageForDebug`, so the redacted form is
      // safe by construction for typed errors.
      const filter = BtcLogFilter();
      developer.log(
        'wallet_import failed',
        name: 'WalletImportScreen',
        error: filter.redact(e.toString()),
        stackTrace: st,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Could not import wallet.')),
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
      appBar: AppBar(title: Text('Import wallet (${widget.network})')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Task 11/12 pattern: typed enum replaces bare int.
            DropdownButtonFormField<MnemonicWordCount>(
              initialValue: _wordCount,
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
                  setState(() => _wordCount = v ?? MnemonicWordCount.twelve),
            ),
            const SizedBox(height: 16),
            MnemonicPasteField(
              expectedWordCount: _wordCount.value,
              onChanged: (v) => _mnemonic = v,
            ),
            const SizedBox(height: 16),
            PasswordField(onChanged: (v) => _password = v),
            const SizedBox(height: 16),
            if (error != null) ...[
              Text(error),
              const SizedBox(height: 8),
            ],
            FilledButton(
              key: const Key('wallet_import_submit'),
              onPressed: _running ? null : _submit,
              child: _running ? const Text('Importing…') : const Text('Import'),
            ),
          ],
        ),
      ),
    );
  }
}
