import 'package:flutter/material.dart';
import '../core/btc/models/wallet_detail.dart';

/// Card showing wallet balance breakdown. Shows confirmed + (when
/// non-zero) trusted-pending, untrusted-pending, immature.
class BalanceCard extends StatelessWidget {
  const BalanceCard({super.key, required this.balance});
  final Balance balance;

  String _sats(int v) => '$v sats';

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Confirmed', style: Theme.of(context).textTheme.labelMedium),
            Text(_sats(balance.confirmedSat),
                style: Theme.of(context).textTheme.headlineSmall),
            if (balance.trustedPendingSat > 0)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                    'Pending (trusted): ${_sats(balance.trustedPendingSat)}'),
              ),
            if (balance.untrustedPendingSat > 0)
              Text(
                  'Pending (untrusted): ${_sats(balance.untrustedPendingSat)}'),
            if (balance.immatureSat > 0)
              Text('Immature: ${_sats(balance.immatureSat)}'),
          ],
        ),
      ),
    );
  }
}
