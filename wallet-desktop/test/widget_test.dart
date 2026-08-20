// Placeholder for the `flutter create` boilerplate widget test.
//
// The original test referenced `MyApp` (the default counter app scaffold)
// and the `+` icon. The current app's root widget is `BtcWalletApp`
// (riverpod + go_router) and has no counter. The real widget tests live
// in `test/unit/widgets_test.dart` (BtcLogFilter + PasswordField +
// MnemonicPasteField + StatusBadge + ProcessProgressOverlay per the
// wallet-desktop design §8.3 widget test matrix).
//
// This placeholder must remain in `test/widget_test.dart` because
// `flutter test` walks the `test/` tree by default. An empty test body
// passes trivially. The boilerplate was failing CI (PR #239 pre-merge
// cleanup) because the app's root widget changed.

import 'package:flutter_test/flutter_test.dart';

void main() {
  // Trivial pass — no assertions to violate.
  test('flutter create boilerplate replaced', () {
    expect(1, 1);
  });
}
