import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/features/wallet_import/wallet_import_screen.dart';

void main() {
  // Submit flow (form-fill with the MnemonicPasteField + PasswordField
  // controllers + ack checkbox) is covered by Task 24's
  // `fake_btc.sh` integration test (operator-driven per L29). The
  // `flutter_test` `enterText` pipeline has known issues with the
  // obscured PasswordField (Task 17/18 lessons) and with
  // multi-line MnemonicPasteField focus ordering, so the form-fill
  // path is left to the integration test. The single render-only
  // unit test below exercises the form's build path.
  testWidgets(
    'WalletImportScreen renders import form: Mnemonic paste + Password + '
    'Network default + Import button',
    (t) async {
      await t.pumpWidget(
        const ProviderScope(
          child: MaterialApp(
            home: Scaffold(body: WalletImportScreen(network: 'testnet')),
          ),
        ),
      );

      // Mnemonic paste field + password field both render as TextField.
      expect(find.byType(TextField), findsNWidgets(2));
      // Out-of-the-box: 12-word default; the dropdown shows '12'.
      expect(find.text('12'), findsOneWidget);
      // FilledButton label.
      expect(find.text('Import'), findsOneWidget);
    },
  );
}
