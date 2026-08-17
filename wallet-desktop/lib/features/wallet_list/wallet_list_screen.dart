import 'package:flutter/material.dart';

/// Placeholder for Task 17 — real impl wires `walletsListProvider`.
class WalletListScreen extends StatelessWidget {
  const WalletListScreen({super.key, required this.network});
  final String network;
  @override
  Widget build(BuildContext context) =>
      Scaffold(body: Center(child: Text('WalletList $network')));
}
