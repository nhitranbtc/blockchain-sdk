import 'package:flutter/material.dart';

/// Brand seed for `ColorScheme.fromSeed`. Bitcoin's brand orange.
const bitcoinOrange = Color(0xFFF7931A);

/// Material 3 light theme derived from [bitcoinOrange].
ThemeData buildLightTheme() => _build(Brightness.light);

/// Material 3 dark theme derived from [bitcoinOrange].
ThemeData buildDarkTheme() => _build(Brightness.dark);

ThemeData _build(Brightness brightness) {
  final scheme = ColorScheme.fromSeed(
    seedColor: bitcoinOrange,
    brightness: brightness,
  );
  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
  );
}
