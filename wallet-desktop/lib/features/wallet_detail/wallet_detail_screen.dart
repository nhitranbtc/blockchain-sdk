import 'package:flutter/material.dart';

/// Placeholder for Task 20 — real impl reads `walletSessionProvider`
/// + `walletInfo` + `walletDetail`.
class WalletDetailScreen extends StatelessWidget {
  const WalletDetailScreen({
    super.key,
    required this.network,
    required this.walletId,
  });
  final String network;
  final String walletId;
  @override
  Widget build(BuildContext context) =>
      Scaffold(body: Center(child: Text('WalletDetail $network/$walletId')));
}
