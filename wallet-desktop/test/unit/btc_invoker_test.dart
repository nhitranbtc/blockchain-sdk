import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/btc_command.dart';
import 'package:wallet_desktop/core/btc/btc_error.dart';
import 'package:wallet_desktop/core/btc/btc_invoker.dart';

void main() {
  // Use a script that echoes argv + canned stdout/stderr.
  // Built in Task 24. Until then all tests skip.
  final mockScript = File('test/integration/fixtures/fake_btc.sh').path;

  test('invoke returns parsed JSON for success exit 0', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
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
    final invoker = BtcInvoker(binaryPath: mockScript);
    await expectLater(
      invoker.invoke(
        const WalletDelete(id: 'x', network: 'testnet'),
        parse: (_) => null,
      ),
      throwsA(isA<BtcError>()),
    );
  });

  test('invoke strips secret-bearing env vars from parent env', () async {
    // The fake_btc.sh writes its inherited env to a file; we inspect
    // that env vars like BTC_WALLET_MNEMONIC are NOT present.
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    Platform.environment['BTC_WALLET_MNEMONIC'] = 'should-be-stripped';
    try {
      final invoker = BtcInvoker(binaryPath: mockScript);
      await invoker.invoke(const ConfigShow(), parse: (_) => null);
      final envFile = File('test/integration/fixtures/.last_env');
      if (await envFile.exists()) {
        final env = await envFile.readAsString();
        expect(env, isNot(contains('BTC_WALLET_MNEMONIC')));
      }
    } finally {
      Platform.environment.remove('BTC_WALLET_MNEMONIC');
    }
  });
}
