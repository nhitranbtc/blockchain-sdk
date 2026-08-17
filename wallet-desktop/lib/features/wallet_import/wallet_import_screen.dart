import 'package:flutter/material.dart';

/// Placeholder for Task 19 — real impl accepts user-pasted BIP-39
/// mnemonic + BIP-39 checksum validation.
class WalletImportScreen extends StatelessWidget {
  const WalletImportScreen({super.key, required this.network});
  final String network;
  @override
  Widget build(BuildContext context) =>
      Scaffold(body: Center(child: Text('WalletImport $network')));
}
