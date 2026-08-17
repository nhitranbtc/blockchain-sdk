import 'package:flutter/material.dart';
import '../core/btc/btc_error.dart';

/// Maps a [BtcErrorKind] to an icon + theme-driven color + human label.
class StatusBadge extends StatelessWidget {
  const StatusBadge({super.key, required this.kind, this.message});
  final BtcErrorKind kind;
  final String? message;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final (icon, color, label) = switch (kind) {
      BtcErrorKind.wrongPassword => (
          Icons.lock_outline,
          scheme.tertiary,
          'Wrong password'
        ),
      BtcErrorKind.insufficientFunds => (
          Icons.account_balance_wallet_outlined,
          scheme.error,
          'Insufficient funds'
        ),
      BtcErrorKind.unknownWallet => (
          Icons.help_outline,
          scheme.outline,
          'Wallet not found'
        ),
      BtcErrorKind.networkError => (
          Icons.cloud_off,
          scheme.tertiary,
          'Network error'
        ),
      BtcErrorKind.unknownAddressType => (
          Icons.error_outline,
          scheme.error,
          'Wrong network'
        ),
      BtcErrorKind.confirmRequired => (
          Icons.warning_amber,
          scheme.error,
          'Confirm required'
        ),
      BtcErrorKind.other => (Icons.error, scheme.error, 'Error'),
    };
    return Chip(
      avatar: Icon(icon, color: color, size: 18),
      label: Text(message ?? label),
    );
  }
}
