import 'package:flutter/material.dart';
import '../core/btc/models/wallet_detail.dart';

/// Card showing the wallet's confirmed balance.
///
/// **Task 13 collapse** (plan deviation #3): the legacy 4-tuple
/// breakdown (confirmed / trustedPending / untrustedPending /
/// immature) is gone — Rust `wallet_show` returns a single
/// `balance_sat: u64`. v0.2.1 re-introduces the pending/immature
/// breakdown once the Esplora sync is wired into `wallet_show`.
class BalanceCard extends StatelessWidget {
  const BalanceCard({super.key, required this.balance});
  final Balance balance;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Confirmed', style: Theme.of(context).textTheme.labelMedium),
            Text('${balance.confirmedSat} sats',
                style: Theme.of(context).textTheme.headlineSmall),
          ],
        ),
      ),
    );
  }
}
