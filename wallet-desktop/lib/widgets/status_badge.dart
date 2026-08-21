import 'package:flutter/material.dart';
import '../core/ffi/ffi_exception.dart';

/// Maps an [FfiErrorKind] to an icon + theme-driven color + human label.
///
/// **Task 17 / Issue #223** — replaced `BtcErrorKind` with `FfiErrorKind`
/// during the subprocess teardown. `BtcErrorKind` was tied to the
/// subprocess `btc` CLI path (exit code + stderr shape); `FfiErrorKind`
/// mirrors the stable C ABI codes returned by `bitcoin-wallet-core`'s
/// `FfiError` enum.
///
/// Mapping notes:
/// - `insufficientFunds` → wallet icon (most common send-screen error)
/// - `network` / `esplora` / `electrum` → cloud_off (network)
/// - `invalidMnemonic` / `encryption` / `walletStore` → lock (auth)
/// - `notInitialized` → help (missing setup)
/// - everything else → error_outline (generic)
class StatusBadge extends StatelessWidget {
  const StatusBadge({super.key, required this.kind, this.message});
  final FfiErrorKind kind;
  final String? message;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final (icon, color, label) = switch (kind) {
      FfiErrorKind.insufficientFunds => (
          Icons.account_balance_wallet_outlined,
          scheme.error,
          'Insufficient funds'
        ),
      FfiErrorKind.network ||
      FfiErrorKind.esplora ||
      FfiErrorKind.electrum => (
          Icons.cloud_off,
          scheme.tertiary,
          'Network error'
        ),
      FfiErrorKind.invalidMnemonic ||
      FfiErrorKind.encryption ||
      FfiErrorKind.walletStore => (
          Icons.lock_outline,
          scheme.tertiary,
          'Auth error'
        ),
      FfiErrorKind.notInitialized => (
          Icons.help_outline,
          scheme.outline,
          'Wallet not loaded'
        ),
      _ => (Icons.error_outline, scheme.error, 'Error'),
    };
    return Chip(
      avatar: Icon(icon, color: color, size: 18),
      label: Text(message ?? label),
    );
  }
}
