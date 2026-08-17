import 'package:flutter/material.dart';

/// Placeholder for Task 22 — real impl reads `btc tx-list --json`.
class TransactionsScreen extends StatelessWidget {
  const TransactionsScreen({
    super.key,
    required this.network,
    required this.walletId,
  });
  final String network;
  final String walletId;
  @override
  Widget build(BuildContext context) =>
      Scaffold(body: Center(child: Text('Transactions $network/$walletId')));
}
