import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart' as p;

import 'core/paths.dart';
import 'core/theme.dart';
import 'providers/esplora_config_provider.dart';
import 'routing/app_router.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final dir = await appDataDir();
  final esploraConfigFile = File(p.join(dir.path, 'esplora.json'));
  runApp(ProviderScope(
    overrides: [
      esploraConfigFilePathProvider.overrideWithValue(esploraConfigFile),
    ],
    child: BtcWalletApp(),
  ));
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
