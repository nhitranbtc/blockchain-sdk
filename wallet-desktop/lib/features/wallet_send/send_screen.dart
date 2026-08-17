import 'package:flutter/material.dart';

/// Placeholder for Task 21 — real impl is the most security-sensitive:
/// reads OpaqueMnemonic from walletSessionProvider + passes to
/// `withTempSecretFile` → btc wallet send.
class SendScreen extends StatelessWidget {
  const SendScreen({
    super.key,
    required this.network,
    required this.walletId,
  });
  final String network;
  final String walletId;
  @override
  Widget build(BuildContext context) =>
      Scaffold(body: Center(child: Text('Send $network/$walletId')));
}
