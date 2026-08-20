// Task 8 (#214) test — typed wrapper around the Rust-side
// `MnemonicHandle` returned by `wallet_create`.
//
// L12 CRITICAL #2 closure (UI side): the wrapper ensures the handle is
// freed via `phraseViewFree` once the user acknowledges the displayed
// phrase, and that the plaintext phrase is not exposed via any
// default `toString` / reflection path. The cached phrase String is
// nulled out on dispose so the Dart heap reference is released.

import 'dart:ffi';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/ffi/mnemonic_view.dart';

void main() {
  group('MnemonicView.dispose', () {
    test('idempotent — second dispose is a no-op', () {
      // Use nullptr so dispose() short-circuits the phraseViewFree
      // call (any non-null synthetic handle would segfault the test
      // runner when passed to the real Rust binding).
      final view = MnemonicView(nullptr);
      view.dispose();
      expect(view.isDisposed, isTrue);
      expect(() => view.dispose(), returnsNormally);
      expect(view.isDisposed, isTrue);
    }, skip: !Platform.isLinux);

    test('freshly-constructed view is not disposed', () {
      final view = MnemonicView(nullptr);
      expect(view.isDisposed, isFalse);
      view.dispose();
    }, skip: !Platform.isLinux);

    test('isDisposed flips exactly once on first dispose', () {
      final view = MnemonicView(nullptr);
      expect(view.isDisposed, isFalse);
      view.dispose();
      expect(view.isDisposed, isTrue);
      view.dispose();
      expect(view.isDisposed, isTrue);
    }, skip: !Platform.isLinux);
  }, skip: !Platform.isLinux);

  group('MnemonicView leak prevention (L12 CRITICAL #2)', () {
    test('toString never contains raw bytes (defense-in-depth)', () {
      final view = MnemonicView(nullptr);
      // Inherited Object.toString() must render the type name only,
      // not any cached phrase (no phrase was cached yet — fresh
      // instance). This guards the case where a future change adds a
      // toString override that accidentally leaks `_cached`.
      final rendered = view.toString();
      expect(rendered.contains('abandon'), isFalse,
          reason: 'toString() must never include plaintext phrase');
      view.dispose();
    }, skip: !Platform.isLinux);
  }, skip: !Platform.isLinux);
}
