import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/btc_command.dart';
import 'package:wallet_desktop/core/btc/btc_invoker.dart';
import 'package:wallet_desktop/providers/btc_providers.dart';
import 'package:wallet_desktop/providers/wallet_providers.dart';

/// Test double — returns canned JSON without spawning a subprocess.
/// Each `invoke` call increments `invokeCount` and rewrites each entry's
/// `id` to `w<invokeCount>`, so a refresh test can assert the parse
/// path re-ran. The real subprocess path is covered by Task 24's
/// integration test with fake_btc.sh.
class _FakeBtcInvoker extends BtcInvoker {
  _FakeBtcInvoker(this.fixture) : super(binaryPath: '/tmp/fake_btc');

  final List<Map<String, dynamic>> fixture;
  int invokeCount = 0;

  @override
  Future<T> invoke<T>(
    BtcCommand cmd, {
    required T Function(dynamic json) parse,
  }) async {
    invokeCount += 1;
    final mutated = [
      for (final e in fixture) {...e, 'id': 'w$invokeCount'},
    ];
    return parse(mutated);
  }
}

void main() {
  test('walletsListProvider parses wallet list (autoDispose, family)',
      () async {
    final container = ProviderContainer(overrides: [
      btcInvokerProvider.overrideWith(
        (ref) async => _FakeBtcInvoker(const [
          {
            'id': 'abc123',
            'network': 'testnet',
            'address_type': 'native-segwit',
          },
          {
            'id': 'def456',
            'network': 'testnet',
            'address_type': 'taproot',
          },
        ]),
      ),
    ]);
    addTearDown(container.dispose);

    final list = await container.read(walletsListProvider('testnet').future);
    expect(list, hasLength(2));
    expect(list.first.id, 'w1'); // fake mutates id to w<invokeCount>
    expect(list.first.network, 'testnet');
    expect(list.last.addressType, 'taproot');
  });

  test('walletsListProvider refresh() re-invokes build via invalidateSelf',
      () async {
    // Track invoker invocations on the fake itself — `btcInvokerProvider`
    // is non-autoDispose (Task 11), so its override body runs once; the
    // wallet provider's `invalidateSelf` re-runs `build` which calls
    // `invoker.invoke` again, incrementing this counter.
    final fake = _FakeBtcInvoker(const [
      {
        'id': 'w0',
        'network': 'testnet',
        'address_type': 'native-segwit',
      },
    ]);
    final container = ProviderContainer(overrides: [
      btcInvokerProvider.overrideWith((ref) async => fake),
    ]);
    addTearDown(container.dispose);

    final notifier = container.read(walletsListProvider('testnet').notifier);
    final first = await container.read(walletsListProvider('testnet').future);
    expect(first.single.id, 'w1'); // fake mutates id to w<invokeCount>
    expect(fake.invokeCount, 1);

    await notifier.refresh();
    final refreshed =
        await container.read(walletsListProvider('testnet').future);
    expect(refreshed.single.id, 'w2'); // 2nd invoke → w2
    expect(fake.invokeCount, 2);
  });

  test('walletsListProvider tolerates empty / null stdout (returns [])',
      () async {
    final container = ProviderContainer(overrides: [
      btcInvokerProvider.overrideWith((ref) async => _FakeBtcInvoker(const [])),
    ]);
    addTearDown(container.dispose);

    final list = await container.read(walletsListProvider('testnet').future);
    expect(list, isEmpty);
  });
}
