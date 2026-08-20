// Task 7 (#213) test — typed FFI wrappers for Esplora + Wallet async
// ops (Task 5 surface).
//
// Verifies each expected symbol resolves to a non-null function pointer
// against the loaded native lib. 16 exports total (5 Esplora + 1
// wallet-construction + 1 wallet-free + 5 wallet-async + 2 wallet-array
// + 2 free). Async-network tests require live network and are
// operator-driven per L29.

import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/ffi/esplora_bindings.dart';

void main() {
  group('EsploraBindings symbol resolution', () {
    test('esploraClientNew resolves', () {
      expect(EsploraBindings.esploraClientNew, isNotNull);
    });

    test('esploraClientFree resolves', () {
      expect(EsploraBindings.esploraClientFree, isNotNull);
    });

    test('esploraFeeEstimate resolves', () {
      expect(EsploraBindings.esploraFeeEstimate, isNotNull);
    });

    test('esploraFeeEstimateFree resolves', () {
      expect(EsploraBindings.esploraFeeEstimateFree, isNotNull);
    });

    test('esploraBroadcastTx resolves', () {
      expect(EsploraBindings.esploraBroadcastTx, isNotNull);
    });

    test('esploraBroadcastTxFree resolves', () {
      expect(EsploraBindings.esploraBroadcastTxFree, isNotNull);
    });

    test('walletFromMnemonic resolves', () {
      expect(EsploraBindings.walletFromMnemonic, isNotNull);
    });

    test('walletFree resolves', () {
      expect(EsploraBindings.walletFree, isNotNull);
    });

    test('walletSync resolves', () {
      expect(EsploraBindings.walletSync, isNotNull);
    });

    test('walletBalance resolves', () {
      expect(EsploraBindings.walletBalance, isNotNull);
    });

    test('walletSend resolves', () {
      expect(EsploraBindings.walletSend, isNotNull);
    });

    test('walletSendFree resolves', () {
      expect(EsploraBindings.walletSendFree, isNotNull);
    });

    test('walletTxids resolves', () {
      expect(EsploraBindings.walletTxids, isNotNull);
    });

    test('walletTxidsArrayFree resolves', () {
      expect(EsploraBindings.walletTxidsArrayFree, isNotNull);
    });

    test('walletPeekAddresses resolves', () {
      expect(EsploraBindings.walletPeekAddresses, isNotNull);
    });

    test('walletPeekAddressesArrayFree resolves', () {
      expect(EsploraBindings.walletPeekAddressesArrayFree, isNotNull);
    });
  }, skip: !Platform.isLinux);

  group('EsploraBindings sync smoke', () {
    test(
      'esploraClientNew on localhost + null pin + free round-trip',
      () {
        // F20 dev escape: localhost allows null pin. Mirrors the Rust
        // unit test `esplora_client_new_localhost_with_null_pin_succeeds`.
        final url = 'http://127.0.0.1:50001/api'.toNativeUtf8();
        try {
          final handle = EsploraBindings.esploraClientNew(url, nullptr);
          expect(handle, isNotNull);
          EsploraBindings.esploraClientFree(handle);
        } finally {
          calloc.free(url);
        }
      },
      skip: !Platform.isLinux,
    );

    test(
      'walletFromMnemonic with valid testnet phrase + free round-trip',
      () {
        // Known-good BIP-39 test vector (abandon×11 + about). Mirrors
        // the Rust unit test `wallet_from_mnemonic_valid_phrase_returns_nonnull`.
        // The phrase is NUL-terminated by `toNativeUtf8`; the Rust
        // side reads until NUL.
        final phrase =
            'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about'
                .toNativeUtf8();
        try {
          final handle = EsploraBindings.walletFromMnemonic(
            phrase,
            1, // FfiNetwork.testnet.code
            0, // FfiAddressType.nativeSegwit.code
          );
          expect(handle, isNotNull);
          EsploraBindings.walletFree(handle);
        } finally {
          calloc.free(phrase);
        }
      },
      skip: !Platform.isLinux,
    );
  });
}
