import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/providers/esplora_config_provider.dart';

void main() {
  group('EsploraConfig.defaults', () {
    test('returns regtest localhost default (the only hardcoded default)',
        () {
      // Per Issue #148: only the localhost regtest default ships
      // with the binary (F20 SPKI-pin enforcement bypass for dev).
      final config = EsploraConfig.defaults('regtest');
      expect(config.network, 'regtest');
      expect(config.url, 'http://127.0.0.1:50002/api');
      expect(config.spkiPin, '');
    });

    test(
        'throws StateError for public-network hosts (F20 enforcement — '
        'operator must provide config file)', () {
      for (final network in [
        'bitcoin',
        'mainnet',
        'testnet',
        'testnet4',
        'signet'
      ]) {
        expect(
          () => EsploraConfig.defaults(network),
          throwsA(isA<StateError>().having(
            (e) => e.message,
            'message',
            contains('SPKI'),
          )),
          reason: 'network=$network should require operator config',
        );
      }
    });

    test('throws ArgumentError for unknown network', () {
      expect(() => EsploraConfig.defaults('mainchain'),
          throwsA(isA<ArgumentError>()));
    });
  });

  group('EsploraConfig.forTesting', () {
    test('bypasses F20 enforcement (test-only)', () {
      // Production NEVER calls this — only tests. Documents the
      // dev escape so future tests don't accidentally rely on
      // the production factory.
      final c = EsploraConfig.forTesting(
        network: 'testnet',
        url: 'https://blockstream.info/testnet/api',
        spkiPin: '',
      );
      expect(c.network, 'testnet');
      expect(c.url, 'https://blockstream.info/testnet/api');
    });
  });

  test('EsploraConfig JSON round-trip', () {
    const c = EsploraConfig(
      network: 'mainnet',
      url: 'https://blockstream.info/api',
      spkiPin: 'abc',
    );
    final j = c.toJson();
    expect(EsploraConfig.fromJson(j).network, 'mainnet');
    expect(EsploraConfig.fromJson(j).spkiPin, 'abc');
  });

  test('notifier persists update to disk', () async {
    final tmp = Directory.systemTemp.createTempSync('esplora_cfg_test');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final container = ProviderContainer(overrides: [
      esploraConfigFilePathProvider
          .overrideWithValue(File('${tmp.path}/cfg.json')),
    ]);
    addTearDown(container.dispose);

    final notifier = container.read(esploraConfigProvider.notifier);
    await notifier.save(
      const EsploraConfig(network: 'mainnet', url: 'https://x', spkiPin: ''),
    );

    final onDisk =
        await container.read(esploraConfigFilePathProvider).readAsString();
    expect(onDisk, contains('mainnet'));
  });
}
