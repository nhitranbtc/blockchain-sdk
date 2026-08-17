import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'core/theme.dart';
import 'routing/app_router.dart';

void main() {
  runApp(ProviderScope(child: BtcWalletApp()));
}

class BtcWalletApp extends StatelessWidget {
  BtcWalletApp({super.key});

  // Hoisted: instantiating a new GoRouter per build would lose history
  // and re-fire redirects on every MaterialApp rebuild.
  final _router = appRouter();

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: 'btc wallet',
      theme: buildLightTheme(),
      darkTheme: buildDarkTheme(),
      themeMode: ThemeMode.system,
      routerConfig: _router,
    );
  }
}
