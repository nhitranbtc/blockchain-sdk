import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../core/btc/btc_command.dart';
import '../../core/btc/btc_error.dart';
import '../../core/btc/btc_error_messages.dart';
import '../../core/btc/models/wallet_info.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../core/secrets/password_supply.dart';
import '../../providers/btc_providers.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';
import '../../widgets/mnemonic_paste_field.dart';
import '../../widgets/password_field.dart';
import '../../widgets/status_badge.dart';

/// Form for Story 2 — import an existing BIP-39 mnemonic into the
/// `btc` wallet data directory.
///
/// **Secret handling** (L12 CRITICAL #2 + Task 5/6/7 chain + Task 17
/// post-PR learnings):
/// - The mnemonic is held in the `MnemonicPasteField`'s internal
///   `TextEditingController` until the user submits; the field's
///   `dispose()` clears it (Task 15).
/// - The password is routed through `withPasswordFile` (Task 6) →
///   mode-0600 temp file → never enters argv.
/// - The mnemonic DOES enter argv for `BtcCommand.walletImport` in
///   v0.1 (Task 8's `BtcCommand.walletImport` passes `--mnemonic
///   <words>`). `BtcInvoker` does NOT log argv (Task 10); the
///   defense-in-depth `BtcLogFilter` scrubs mnemonic-shape strings
///   if any other code path incidentally echoes it. v0.2 will move
///   to `withMnemonicFile` (Task 8 backlog) for true secret-passing.
/// - `BtcLogFilter.redact` covers the catch path's `developer.log`
///   (Task 17 lesson).
///
/// **BIP-39 checksum**: v0.1 defers client-side validation to `btc`
/// (which rejects bad-checksum phrases with `BtcErrorKind.unknownWallet`
/// arm — Task 8 sealed enum). v0.2 will validate the checksum on the
/// client for fast user feedback.
///
/// **Post-success**: refresh `walletsListProvider` via `ref.invalidate`
/// (NOT `unawaited(notifier.refresh())` — caught uncaught zone
/// errors at Task 18 L12 type-design post-PR HIGH #2). Navigate to
/// the wallet detail screen; Task 20 owns the session-unlock flow.
class WalletImportScreen extends ConsumerStatefulWidget {
  const WalletImportScreen({super.key, required this.network});
  final String network;

  @override
  ConsumerState<WalletImportScreen> createState() => _WalletImportScreenState();
}

class _WalletImportScreenState extends ConsumerState<WalletImportScreen> {
  /// BIP-39 word count. v0.1 defaults to 12; the dropdown lets the
  /// user switch to 24 (matches the Task 18 walletCreate flow).
  int _wordCount = 12;

  String _mnemonic = '';
  String _password = '';
  bool _running = false;
  BtcError? _error;

  Future<void> _submit() async {
    if (_mnemonic.trim().isEmpty || _password.isEmpty) return;
    setState(() {
      _running = true;
      _error = null;
    });
    try {
      final invoker = await ref.read(btcInvokerProvider.future);
      // `withPasswordFile` is `Future<void>` (Task 5/6 thin delegate);
      // capture the parsed DTO via a closure variable. The Task 6.1
      // follow-up (per type-design post-PR feedback) refactors both
      // `withTempSecretFile<T>` + `withPasswordFile<T>` to return `T`.
      WalletInfo? imported;
      await withPasswordFile(_password, (path) async {
        final r = await invoker.invoke<WalletInfo>(
          BtcCommand.walletImport(
            mnemonic: _mnemonic,
            network: widget.network,
            passwordFilePath: path,
          ),
          parse: (j) => WalletInfo.fromJson(j as Map<String, dynamic>),
        );
        imported = r;
      });
      final r = imported;
      if (r == null || !mounted) return;
      // Clear the screen-side password + mnemonic so the navigator
      // pop doesn't echo the secrets.
      setState(() {
        _password = '';
        _mnemonic = '';
      });
      // Refresh the wallet-list family (NOT `unawaited(ref.refresh)`
      // — caught uncaught zone errors at Task 18 L12 type-design
      // post-PR HIGH #2). The list screen's AsyncValue UI surfaces
      // any re-fetch failure.
      ref.invalidate(walletsListProvider(widget.network));
      // Note: deliberately NOT unlocking `walletSessionProvider`
      // here. The import path lands on the WalletDetailScreen
      // (Task 20) which will own the unlock flow + password
      // re-prompt. Documented per Task 18 L12 type-design
      // post-PR MEDIUM #5.
      if (mounted) {
        context.go(WalletRoutes.detail(widget.network, r.id));
      }
    } on BtcError catch (e) {
      if (mounted) setState(() => _error = e);
    } catch (e, st) {
      const filter = BtcLogFilter();
      developer.log(
        'wallet_import failed',
        name: 'WalletImportScreen',
        error: filter.redact(e.toString()),
        stackTrace: st,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Something went wrong.')),
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
            DropdownButtonFormField<int>(
              initialValue: _wordCount,
              decoration: const InputDecoration(labelText: 'Words'),
              items: const [
                DropdownMenuItem(value: 12, child: Text('12')),
                DropdownMenuItem(value: 24, child: Text('24')),
              ],
              onChanged: (v) => setState(() => _wordCount = v ?? 12),
            ),
            const SizedBox(height: 16),
            MnemonicPasteField(
              expectedWordCount: _wordCount,
              onChanged: (v) => _mnemonic = v,
            ),
            const SizedBox(height: 16),
            PasswordField(onChanged: (v) => _password = v),
            const SizedBox(height: 16),
            if (error != null) ...[
              StatusBadge(kind: error.kind),
              const SizedBox(height: 8),
              Text(userMessageForBtcError(error)),
              const SizedBox(height: 8),
            ],
            FilledButton(
              key: const Key('wallet_import_submit'),
              onPressed: _running ? null : _submit,
              child: _running
                  ? const Text('Importing…')
                  : const Text('Import'),
            ),
          ],
        ),
      ),
    );
  }
}
