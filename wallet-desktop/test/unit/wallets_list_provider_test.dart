import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/models/wallet_info.dart';
import 'package:wallet_desktop/providers/wallet_providers.dart';

void main() {
  test('walletsListProvider exposes AsyncValue<List<WalletInfo>>', () {
    // We do not override btcInvokerProvider here; this test just verifies
    // the provider tree resolves and exposes AsyncValue. End-to-end fetch
    // against a fake_btc binary is in Task 24.
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final async = container.read(walletsListProvider('testnet'));
    expect(async, isA<AsyncValue<List<WalletInfo>>>());
  });
}
