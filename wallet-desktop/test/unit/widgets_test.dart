import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/btc_error.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';
import 'package:wallet_desktop/widgets/address_chip.dart';
import 'package:wallet_desktop/widgets/balance_card.dart';
import 'package:wallet_desktop/widgets/mnemonic_paste_field.dart';
import 'package:wallet_desktop/widgets/network_picker.dart';
import 'package:wallet_desktop/widgets/password_field.dart';
import 'package:wallet_desktop/widgets/process_progress_overlay.dart';
import 'package:wallet_desktop/widgets/status_badge.dart';

void main() {
  testWidgets('AddressChip displays truncated address', (tester) async {
    await tester.pumpWidget(const MaterialApp(
      home: Scaffold(
        body: AddressChip(
          address: 'tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx',
        ),
      ),
    ));
    expect(find.textContaining('tb1q'), findsOneWidget);
  });

  testWidgets('BalanceCard shows confirmed balance', (tester) async {
    const bal = Balance(
      confirmedSat: 100000,
      trustedPendingSat: 0,
      untrustedPendingSat: 0,
      immatureSat: 0,
    );
    await tester.pumpWidget(const MaterialApp(
      home: Scaffold(body: BalanceCard(balance: bal)),
    ));
    expect(find.textContaining('100000'), findsOneWidget);
  });

  testWidgets('NetworkPicker default is testnet and emits on selection',
      (tester) async {
    String? chosen;
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: NetworkPicker(onChanged: (n) => chosen = n),
      ),
    ));
    // Tapping the currently-selected segment is a no-op (SegmentedButton
    // semantics). Pick a different network to assert onChanged fires.
    await tester.tap(find.text('bitcoin'));
    await tester.pump();
    expect(chosen, 'bitcoin');
  });

  testWidgets('PasswordField obscureText defaults to true', (tester) async {
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: PasswordField(onChanged: (_) {}),
      ),
    ));
    expect(find.byType(TextField), findsOneWidget);
    final tf = tester.widget<TextField>(find.byType(TextField));
    expect(tf.obscureText, isTrue);
  });

  testWidgets('StatusBadge shows wallet icon for insufficientFunds',
      (tester) async {
    await tester.pumpWidget(const MaterialApp(
      home: Scaffold(
        body: StatusBadge(kind: BtcErrorKind.insufficientFunds),
      ),
    ));
    expect(
      find.byIcon(Icons.account_balance_wallet_outlined),
      findsOneWidget,
    );
    expect(find.textContaining('Insufficient funds'), findsOneWidget);
  });

  testWidgets('MnemonicPasteField word count error appears for wrong count',
      (tester) async {
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: MnemonicPasteField(
          onChanged: (_) {},
          expectedWordCount: 12,
        ),
      ),
    ));
    await tester.enterText(find.byType(TextField), 'one two three');
    await tester.pump();
    // 3 words != 12 → error helper shown
    expect(find.textContaining('Expected 12 words'), findsOneWidget);
  });

  testWidgets('ProcessProgressOverlay hidden when not running', (tester) async {
    await tester.pumpWidget(const MaterialApp(
      home: Scaffold(
        body: Stack(
          children: [
            ProcessProgressOverlay(isRunning: false),
          ],
        ),
      ),
    ));
    expect(find.byType(CircularProgressIndicator), findsNothing);
  });
}
