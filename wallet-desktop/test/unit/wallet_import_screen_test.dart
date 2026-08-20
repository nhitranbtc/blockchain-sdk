// Task 12 (#218) test — `WalletImportScreen` migrated to
// `walletCoreProvider`. Rewrite of legacy btc-CLI test suite.
//
// Submit flow (form-fill with the MnemonicPasteField +
// PasswordField controllers) is covered by Task 24's
// `fake_btc.sh` integration test (operator-driven per L29).
// The `flutter_test` `enterText` pipeline has known issues
// with the obscured PasswordField (Task 17/18 lessons) and
// with multi-line MnemonicPasteField focus ordering, so the
// form-fill path is left to the integration test. The
// render-only unit test below exercises the form's build
// path.

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/features/wallet_import/wallet_import_screen.dart';

void main() {
  testWidgets(
    'WalletImportScreen renders import form: Mnemonic paste + Password + '
    '12-word default + Import button',
    (t) async {
      await t.pumpWidget(
        const ProviderScope(
          child: MaterialApp(
            home: Scaffold(body: WalletImportScreen(network: 'testnet')),
          ),
        ),
      );

      // Mnemonic paste field + password field both render as
      // TextField.
      expect(find.byType(TextField), findsNWidgets(2));
      // Out-of-the-box: 12-word default; the dropdown shows '12'.
      expect(find.text('12'), findsOneWidget);
      // FilledButton label.
      expect(find.text('Import'), findsOneWidget);
    },
  );

  // Task 11/12 plan deviation: address type dropdown dropped.
  // Rust `wallet_import` does NOT persist `address_type`. Verify
  // the picker is absent (no 'Address type' label).
  testWidgets(
    'WalletImportScreen does NOT show address type dropdown '
    '(plan deviation: Rust wallet_import does not persist type)',
    (t) async {
      await t.pumpWidget(
        const ProviderScope(
          child: MaterialApp(
            home: Scaffold(body: WalletImportScreen(network: 'testnet')),
          ),
        ),
      );

      expect(find.text('Address type'), findsNothing);
    },
  );
}
