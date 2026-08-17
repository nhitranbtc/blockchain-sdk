import 'package:flutter/material.dart';

/// Placeholder for Task 18 — real impl displays freshly-generated
/// BIP-39 mnemonic (L12 CRITICAL #2: never logged).
class WalletCreateScreen extends StatelessWidget {
  const WalletCreateScreen({super.key, required this.network});
  final String network;
  @override
  Widget build(BuildContext context) =>
      Scaffold(body: Center(child: Text('WalletCreate $network')));
}
