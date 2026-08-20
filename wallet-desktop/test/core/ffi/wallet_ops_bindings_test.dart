// Task 7 (#213) test — typed FFI wrappers for wallet ops (Task 4 surface).
//
// Verifies:
//   1. Each expected symbol resolves to a non-null function pointer
//      against the loaded native lib.
//   2. The version-helper smoke path round-trips (calls `ffi_version`,
//      verifies a SemVer-shaped string, frees it).
//
// Tests are gated on Linux (the dev box that hosts the built
// `librust_wallet_core.so`). Task 18 (build-native) will mirror the
// binary to other platforms; the bindings themselves are platform-
// agnostic — only the symbol-resolution test depends on the loader.

import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/ffi/wallet_ops_bindings.dart';

void main() {
  group('WalletOpsBindings symbol resolution', () {
    test('ffiVersion resolves to a non-null function pointer', () {
      expect(WalletOpsBindings.ffiVersion, isNotNull);
    });

    test('ffiVersionFree resolves to a non-null function pointer', () {
      expect(WalletOpsBindings.ffiVersionFree, isNotNull);
    });

    test('walletCreate resolves to a non-null function pointer', () {
      expect(WalletOpsBindings.walletCreate, isNotNull);
    });

    test('phraseViewCopy resolves to a non-null function pointer', () {
      expect(WalletOpsBindings.phraseViewCopy, isNotNull);
    });

    test('phraseViewFree resolves to a non-null function pointer', () {
      expect(WalletOpsBindings.phraseViewFree, isNotNull);
    });

    test('walletList resolves to a non-null function pointer', () {
      expect(WalletOpsBindings.walletList, isNotNull);
    });

    test('walletListArrayFree resolves to a non-null function pointer', () {
      expect(WalletOpsBindings.walletListArrayFree, isNotNull);
    });

    test('walletDelete resolves to a non-null function pointer', () {
      expect(WalletOpsBindings.walletDelete, isNotNull);
    });

    test('walletImport resolves to a non-null function pointer', () {
      expect(WalletOpsBindings.walletImport, isNotNull);
    });
  }, skip: !Platform.isLinux);

  group('WalletOpsBindings smoke', () {
    test(
      'ffi_version returns a SemVer-shaped string and free is symmetric',
      () {
        final ptr = WalletOpsBindings.ffiVersion();
        expect(ptr, isNotNull);
        try {
          final s = ptr.toDartString();
          expect(s, matches(RegExp(r'^\d+\.\d+\.\d+')));
        } finally {
          WalletOpsBindings.ffiVersionFree(ptr);
        }
      },
      skip: !Platform.isLinux,
    );
  });
}
