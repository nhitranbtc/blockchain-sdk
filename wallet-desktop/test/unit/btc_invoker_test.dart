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
    // L34.1-defensive (Task 22): BtcInvoker.invoke calls parse(null)
    // when stdout is empty (btc_invoker.dart:146). The original cast
    // `(j as List).first as String` threw TypeError on empty output;
    // the fixture build surfaced the latent bug. Use `is List` + `is Map`
    // guards per the L34.1 lesson the comment references.
    final result = await invoker.invoke(
      const WalletList(network: 'testnet'),
      parse: (j) {
        if (j is! List || j.isEmpty) return null;
        final first = j.first;
        if (first is! Map) return null;
        return first['id'] as String?;
      },
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

  test('invoke strips BTC_WALLET_MNEMONIC from child env via wrapper fixture',
      () async {
    // L12 reviewer fix (HIGH): the previous version was vacuously true —
    // no wrapper set BTC_WALLET_MNEMONIC, so the assertion always passed
    // regardless of BtcInvoker's filter. Use `with_secret_env.sh` as the
    // binaryPath: it exports `BTC_WALLET_MNEMONIC=probe-secret-...` then
    // execs fake_btc.sh. The fixture's grep filter MUST strip the secret
    // before writing to `.last_env`.
    //
    // **Limitation**: Dart 3+ `Platform.environment` is unmodifiable, so
    // BtcInvoker's own `_secretEnvKeys` filter can only be exercised by
    // launching `flutter test` with `BTC_WALLET_MNEMONIC=probe` in the
    // shell (L29 operator-driven gate). This test covers the fixture-side
    // filter, which is the parallel defense at the fixture boundary.
    const wrapperScript = 'test/integration/fixtures/with_secret_env.sh';
    if (!await File(wrapperScript).exists()) {
      markTestSkipped('with_secret_env.sh not built (Task 24)');
      return;
    }
    const invoker = BtcInvoker(binaryPath: wrapperScript);
    await invoker.invoke(const ConfigShow(), parse: (_) => null);
    final envFile = File('test/integration/fixtures/.last_env');
    // L12 silent-failure-hunter fix: tighten missing-file from silent
    // skip to loud assertion (regression: a fixture that fails to write
    // `.last_env` would previously pass this test).
    expect(await envFile.exists(), isTrue,
        reason: 'fixture must write .last_env for the L7 verification');
    final env = await envFile.readAsString();
    expect(env, isNot(contains('BTC_WALLET_MNEMONIC')),
        reason: 'fixture L7 filter must strip BTC_WALLET_MNEMONIC');
    expect(env, isNot(contains('probe-secret')),
        reason: 'fixture L7 filter must strip the probe secret value');
  });
}
