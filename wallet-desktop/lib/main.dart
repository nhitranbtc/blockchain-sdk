// App entry point. Real shell (ProviderScope + MaterialApp.router) lands in Task 16.
import 'package:flutter/material.dart';

void main() {
  runApp(const _PlaceholderApp());
}

class _PlaceholderApp extends StatelessWidget {
  const _PlaceholderApp();

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(home: Scaffold(body: Center(child: Text('wallet-desktop scaffold'))));
  }
}
