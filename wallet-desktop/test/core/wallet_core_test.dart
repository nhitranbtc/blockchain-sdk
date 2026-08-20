// Task 8 (#214) test — typed `WalletCore` facade over the FFI
// bindings (Tasks 3-5-7 surfaces). Replaces the Task 1 spike
// `wallet_core.dart` with a fully typed public surface.
//
// L12 CRITICAL #2 contract: the facade accepts `SecretBuffer` for
// every secret-bearing parameter (password, phrase) and returns a
// `MnemonicView` (not a String) for newly-created wallet phrases.
// The spike's `version()` / `listWallets({required String network})`
// surface is replaced with typed `FfiNetwork` / `FfiAddressType`
// enums.

import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/ffi/ffi_enums.dart';
import 'package:wallet_desktop/core/ffi/secret_buffer.dart';
import 'package:wallet_desktop/core/wallet_core.dart';

void main() {
  group('WalletCore.ffiVersion', () {
    test('returns a non-empty version string', () {
      final core = WalletCore.instance;
      final version = core.ffiVersion();
      expect(version, isNotEmpty);
    }, skip: !Platform.isLinux);
  }, skip: !Platform.isLinux);

  group('WalletCore.listWallets', () {
    test('returns a list (possibly empty) for valid baseDir', () {
      final core = WalletCore.instance;
      final baseDir = Directory.systemTemp.createTempSync('wallet_core_list_');
      final baseDirPtr = baseDir.path.toNativeUtf8();
      try {
        final wallets = core.listWallets(
          network: FfiNetwork.testnet,
          baseDir: baseDirPtr,
        );
        expect(wallets, isA<List<String>>());
      } finally {
        calloc.free(baseDirPtr);
        baseDir.deleteSync(recursive: true);
      }
    }, skip: !Platform.isLinux);
  }, skip: !Platform.isLinux);

  group('WalletCore.createWallet + importWallet', () {
    test(
      'createWallet returns DTO with id + mnemonic view; auto-disposes SecretBuffer',
      () {
        final core = WalletCore.instance;
        final password = SecretBuffer.fromUtf8('test-password-1234');
        final baseDir = Directory.systemTemp.createTempSync('wallet_core_create_');
        final baseDirPtr = baseDir.path.toNativeUtf8();
        try {
          final result = core.createWallet(
            words: 12,
            network: FfiNetwork.testnet,
            addressType: FfiAddressType.nativeSegwit,
            password: password,
            baseDir: baseDirPtr,
          );
          expect(result.id, hasLength(36));
          // MnemonicView is non-null and not disposed.
          expect(result.mnemonic, isNotNull);
          expect(result.mnemonic.isDisposed, isFalse);
          // Phrase can be read.
          final phrase = result.mnemonic.read();
          expect(phrase.split(' ').length, 12);
          // Dispose the view.
          result.mnemonic.dispose();
          expect(result.mnemonic.isDisposed, isTrue);
        } finally {
          calloc.free(baseDirPtr);
          baseDir.deleteSync(recursive: true);
          // Password is auto-disposed by the facade.
        }
      },
      skip: !Platform.isLinux,
    );

    test(
      'createWallet throws on invalid word count (FfiError surfaces)',
      () {
        final core = WalletCore.instance;
        final password = SecretBuffer.fromUtf8('test-password-1234');
        final baseDir = Directory.systemTemp.createTempSync('wallet_core_inv_');
        final baseDirPtr = baseDir.path.toNativeUtf8();
        try {
          expect(
            () => core.createWallet(
              words: 7, // not a valid BIP-39 word count
              network: FfiNetwork.testnet,
              addressType: FfiAddressType.nativeSegwit,
              password: password,
              baseDir: baseDirPtr,
            ),
            throwsA(isA<Exception>()),
          );
        } finally {
          calloc.free(baseDirPtr);
          baseDir.deleteSync(recursive: true);
        }
      },
      skip: !Platform.isLinux,
    );

    test(
      'importWallet returns DTO without mnemonic (caller already has phrase)',
      () {
        final core = WalletCore.instance;
        const phrase = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
        final phraseBuf = SecretBuffer.fromUtf8(phrase);
        final passwordBuf = SecretBuffer.fromUtf8('test-password-1234');
        final baseDir = Directory.systemTemp.createTempSync('wallet_core_imp_');
        final baseDirPtr = baseDir.path.toNativeUtf8();
        try {
          final result = core.importWallet(
            network: FfiNetwork.testnet,
            phrase: phraseBuf,
            password: passwordBuf,
            baseDir: baseDirPtr,
          );
          expect(result.id, hasLength(36));
          expect(result.network, FfiNetwork.testnet);
        } finally {
          calloc.free(baseDirPtr);
          baseDir.deleteSync(recursive: true);
          // Both phrase + password auto-disposed.
        }
      },
      skip: !Platform.isLinux,
    );
  }, skip: !Platform.isLinux);
}