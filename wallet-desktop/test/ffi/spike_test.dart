// Spike test — proves the dart:ffi path works end-to-end.
//
// Run from the wallet-desktop/ directory after building the native lib:
//   cargo build --release -p bitcoin-wallet-core
//   cp target/release/librust_wallet_core.so native/linux-x64/
//   flutter test test/ffi/spike_test.dart

import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/ffi/wallet_core.dart';

void main() {
  test('spike: ffi loads + ffi_version returns 0.2.0', () {
    final core = WalletCore.open();
    final v = core.version();
    expect(v, '0.2.0');
  });

  test('spike: wallet_list(testnet) returns existing wallets', () {
    final core = WalletCore.open();
    final ids = core.listWallets(network: 'testnet');
    expect(ids, isA<List<String>>());
    // Don't assert specific count — operator's data dir state varies.
    // Just confirm the call works end-to-end.
  });
}
