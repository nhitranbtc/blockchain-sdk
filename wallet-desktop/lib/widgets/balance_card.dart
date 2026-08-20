import 'package:flutter/material.dart';
import '../core/btc/models/wallet_detail.dart';

/// Card showing the wallet's confirmed balance.
///
/// **Task 13 collapse** (plan deviation #3): the legacy 4-tuple
/// breakdown (confirmed / trustedPending / untrustedPending /
/// immature) is gone — Rust `wallet_show` returns a single
/// `balance_sat: u64`. v0.2.1 re-introduces the pending/immature
/// breakdown once the Esplora sync is wired into `wallet_show`.
///
/// **L12 flutter HIGH** (Task 13): v0.2.0 always shows "0 sats" —
/// the FFI defers sync. The user sees a hint below the headline
/// explaining the "0 sats" is a placeholder, not the wallet's real
/// state. v0.2.1 removes the hint when sync populates real values.
class BalanceCard extends StatelessWidget {
  const BalanceCard({super.key, required this.balance});
  final Balance balance;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final isPlaceholder = balance.confirmedSat == 0;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Confirmed', style: theme.textTheme.labelMedium),
            Text('${balance.confirmedSat} sats',
                style: theme.textTheme.headlineSmall),
            if (isPlaceholder) ...[
              const SizedBox(height: 4),
              Text(
                'Balance syncs on send — opens in v0.2.1',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
