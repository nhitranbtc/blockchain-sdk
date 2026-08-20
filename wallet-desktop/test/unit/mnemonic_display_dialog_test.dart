// Task 11 (#217) test — `MnemonicDisplayDialog` accepts a typed
// `MnemonicView` (not a String) + typed `MnemonicWordCount`
// enum. Closes the F47 zeroization gap.
//
// L12 review (Task 11) fixes applied:
// - H1: word count is a typed constructor param, not a
//   `ValueKey<String>` back-channel.
// - H2: `read()` happens in the Reveal onChanged handler, not
//   in `build()` (which must be infallible).
// - M1 (flutter-reviewer): `ExcludeSemantics` only wraps the
//   cleartext subtree.
// - M4 (type-design): the dispose test now has a real
//   assertion (`expect(view.isDisposed, isTrue)` after unmount).

import 'dart:ffi';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/ffi/mnemonic_view.dart';
import 'package:wallet_desktop/features/wallet_create/mnemonic_display_dialog.dart';

void main() {
  group('MnemonicDisplayDialog (Task 11, MnemonicView typed)', () {
    testWidgets(
      'masks mnemonic by default — fixed-width placeholder, no phrase leak',
      (t) async {
        await t.pumpWidget(
          MaterialApp(
            home: Dialog(
              // ignore: deprecated_member_use // Dialog direct-wrap is the standard showDialog test seam
              child: MnemonicDisplayDialog(
                mnemonic: MnemonicView(nullptr),
                walletId: 'wlt-fresh',
                wordCount: MnemonicWordCount.twelve,
              ),
            ),
          ),
        );

        // The masked placeholder renders exactly `wordCount.value`
        // bullets. No phrase bytes reach the widget tree.
        expect(find.text('•' * 12), findsOneWidget);
        expect(find.text('Reveal words'), findsOneWidget);
        expect(find.textContaining('abandon'), findsNothing);
        expect(find.textContaining('legal'), findsNothing);
      },
    );

    testWidgets(
      'Reveal onChanged eagerly reads via widget.mnemonic.read()',
      (t) async {
        await t.pumpWidget(
          MaterialApp(
            home: Dialog(
              // ignore: deprecated_member_use // Dialog direct-wrap is the standard showDialog test seam
              child: MnemonicDisplayDialog(
                mnemonic: MnemonicView(nullptr),
                walletId: 'wlt-fresh',
                wordCount: MnemonicWordCount.twentyFour,
              ),
            ),
          ),
        );

        // H2: build() never invokes read(). The placeholder is
        // 24 bullets for twentyFour.
        expect(find.text('•' * 24), findsOneWidget);

        // Tap Reveal — onChanged catches StateError from the
        // null handle and surfaces an error String instead of
        // letting it propagate as an ErrorWidget.
        await t.tap(find.byType(Checkbox).first);
        await t.pumpAndSettle();

        // The error message appears (no Flutter ErrorWidget).
        expect(find.textContaining('Cannot reveal phrase'), findsOneWidget);
        // Phrase placeholder still masked (the read() threw).
        expect(find.text('•' * 24), findsOneWidget);
      },
    );

    testWidgets(
      'dialog unmount calls widget.mnemonic.dispose() — isDisposed flips',
      (t) async {
        final view = MnemonicView(nullptr);
        expect(view.isDisposed, isFalse);

        await t.pumpWidget(
          MaterialApp(
            home: Dialog(
              // ignore: deprecated_member_use // Dialog direct-wrap is the standard showDialog test seam
              child: MnemonicDisplayDialog(
                mnemonic: view,
                walletId: 'wlt-fresh',
                wordCount: MnemonicWordCount.twelve,
              ),
            ),
          ),
        );

        // Unmount the dialog.
        await t.pumpWidget(const MaterialApp(home: SizedBox()));

        // M4 fix: real assertion. F47 zeroization contract
        // verified at the framework boundary.
        expect(view.isDisposed, isTrue);
      },
    );
  });
}
