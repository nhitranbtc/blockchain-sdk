import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/providers/esplora_config_provider.dart';

void main() {
  test('default EsploraConfig returns testnet + blockstream URL', () {
    final config = EsploraConfig.defaults('testnet');
    expect(config.network, 'testnet');
    expect(config.url, 'https://blockstream.info/testnet/api');
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
