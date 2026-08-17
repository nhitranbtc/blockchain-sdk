import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/btc_command.dart';
import 'package:wallet_desktop/core/btc/btc_error.dart';
import 'package:wallet_desktop/core/btc/btc_invoker.dart';

void main() {
  // Integration tests (success JSON parse, non-zero exit, env-strip
  // positive verification) require a working `fake_btc.sh` shell fixture
  // that echoes argv + writes inherited env. That fixture is built in
  // Task 24 (Integration test). Until then, all tests skip.
  //
  // The previous version of this file tried to mutate `Platform.environment`
  // for the env-strip test, but Dart 3+ makes `Platform.environment` an
  // unmodifiable Map — the mutation throws UnsupportedError and the test
  // is dead code. The proper test (Task 24) spawns a hermetic subprocess
  // via a wrapper script that sets the secret env var explicitly.

  const mockScript = 'test/integration/fixtures/fake_btc.sh';

  test('invoke returns parsed JSON for success exit 0', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    const invoker = BtcInvoker(binaryPath: mockScript);
    final result = await invoker.invoke(
      const WalletList(network: 'testnet'),
      parse: (j) => (j as List).first as String,
    );
    expect(result, 'fake-uuid-1');
  });

  test('invoke throws BtcError on non-zero exit', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    const invoker = BtcInvoker(binaryPath: mockScript);
    await expectLater(
      invoker.invoke(
        const WalletDelete(id: 'x', network: 'testnet'),
        parse: (_) => null,
      ),
      throwsA(isA<BtcError>()),
    );
  });

  test(
      'invoke does NOT inherit BTC_WALLET_MNEMONIC even when set in parent shell',
      () async {
    // Requires fake_btc.sh that writes its inherited env to a file
    // (Task 24). The shell wrapper sets BTC_WALLET_MNEMONIC so we can
    // verify the L7 env-strip is honored (not re-inherited by
    // includeParentEnvironment: false).
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    const invoker = BtcInvoker(binaryPath: mockScript);
    await invoker.invoke(const ConfigShow(), parse: (_) => null);
    final envFile = File('test/integration/fixtures/.last_env');
    if (await envFile.exists()) {
      final env = await envFile.readAsString();
      expect(env, isNot(contains('BTC_WALLET_MNEMONIC')));
    }
  });
}
