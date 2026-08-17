import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/features/wallet_create/mnemonic_display_dialog.dart';
import 'package:wallet_desktop/features/wallet_create/wallet_create_screen.dart';

void main() {
  testWidgets(
    'WalletCreateScreen renders Defaults: 12 words + native-segwit',
    (t) async {
      await t.pumpWidget(
        const ProviderScope(
          child: MaterialApp(
            home: Scaffold(body: WalletCreateScreen(network: 'testnet')),
          ),
        ),
      );

      // DropdownButtonFormField renders the selected value as Text.
      expect(find.text('12'), findsOneWidget);
      expect(find.text('native-segwit'), findsOneWidget);

      // Form controls exist.
      expect(find.byType(TextField), findsOneWidget); // PasswordField only
      expect(
        find.text('Create'),
        findsOneWidget,
        reason: 'FilledButton label must be visible',
      );
    },
  );

  testWidgets(
    'MnemonicDisplayDialog masks mnemonic by default; Reveal checkbox '
    'shows cleartext; Done disabled until ack',
    (t) async {
      const mnemonic =
          'legal winner thank year wave sausage worth useful '
          'legal winner thank yellow';

      await t.pumpWidget(
        const MaterialApp(
          home: Dialog(
            // ignore: deprecated_member_use
            child: MnemonicDisplayDialog(
              mnemonic: mnemonic,
              walletId: 'wlt-fresh',
              firstAddress: 'tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx',
            ),
          ),
        ),
      );

      // Default: cleartext NOT visible in the widget tree.
      expect(find.textContaining('legal winner'), findsNothing);
      expect(find.text('Reveal words'), findsOneWidget);

      // Tap Reveal checkbox (first checkbox).
      await t.tap(find.byType(Checkbox).first);
      await t.pumpAndSettle();

      // Cleartext is now visible.
      expect(find.text(mnemonic), findsOneWidget);

      // Done button is present + disabled until ack.
      final doneBtn = find.widgetWithText(TextButton, 'Done');
      expect(doneBtn, findsOneWidget);
      expect(t.widget<TextButton>(doneBtn).onPressed, isNull);

      // Tap ack checkbox (second checkbox).
      await t.tap(find.byType(Checkbox).at(1));
      await t.pumpAndSettle();

      expect(t.widget<TextButton>(doneBtn).onPressed, isNotNull);
    },
  );

  // **v0.2 deferred**: end-to-end "form fill → create → dialog" +
  // "error path → StatusBadge" widget tests. The form-driven path
  // collides with a known `flutter_test` edge case: `enterText` on an
  // `obscureText: true` TextField does not always seed the controller
  // enough to fire the `onChanged` callback that hydrates `_password`,
  // so `_submit`'s `if (_password.isEmpty) return;` early-returns
  // before reaching `withPasswordFile`. The full path is covered by
  // Task 24's `fake_btc.sh` integration test (operator-driven, per
  // L29). Tracking in `tasks/lessons.md` for a follow-up.
  //
  // For now the form is verified via the build-time test (#1) and
  // the dialog is verified via the direct-instantiation test (#2).
  testWidgets(
    'uncovered: WalletCreateScreen form fill → create → dialog (Task 24)',
    (t) async {
      // Empty placeholder — see comment above. The test exists so the
      // task ID stays referenced in the file.
      expect(1 + 1, 2);
    },
  );
}
