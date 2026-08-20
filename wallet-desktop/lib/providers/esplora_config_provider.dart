import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:meta/meta.dart';

@immutable
class EsploraConfig {
  const EsploraConfig({
    required this.network,
    required this.url,
    required this.spkiPin,
  });

  final String network;
  final String url;
  final String spkiPin;

  factory EsploraConfig.defaults(String network) {
    switch (network) {
      case 'bitcoin':
        return const EsploraConfig(
          network: 'bitcoin',
          url: 'https://blockstream.info/api',
          spkiPin: '',
        );
      case 'testnet':
        return const EsploraConfig(
          network: 'testnet',
          url: 'https://blockstream.info/testnet/api',
          spkiPin: '',
        );
      case 'testnet4':
        return const EsploraConfig(
          network: 'testnet4',
          url: 'https://blockstream.info/testnet4/api',
          spkiPin: '',
        );
      case 'signet':
        return const EsploraConfig(
          network: 'signet',
          url: 'https://blockstream.info/signet/api',
          spkiPin: '',
        );
      // L12 type-design Task 23 MEDIUM: add regtest canonical default.
      // The dropdown in SettingsScreen includes 'regtest'; without
      // this case, selecting it falls through to `default:` which
      // returns an empty URL — an invalid business state (wallet
      // can never reach the chain). Local regtest Esplora standard
      // is electrs on 127.0.0.1:50002.
      case 'regtest':
        return const EsploraConfig(
          network: 'regtest',
          url: 'http://127.0.0.1:50002/api',
          spkiPin: '',
        );
      default:
        return EsploraConfig(network: network, url: '', spkiPin: '');
    }
  }

  Map<String, dynamic> toJson() =>
      {'network': network, 'url': url, 'spkiPin': spkiPin};

  factory EsploraConfig.fromJson(Map<String, dynamic> j) => EsploraConfig(
        network: j['network'] as String,
        url: j['url'] as String,
        spkiPin: j['spkiPin'] as String? ?? '',
      );

  EsploraConfig copyWith({String? network, String? url, String? spkiPin}) =>
      EsploraConfig(
        network: network ?? this.network,
        url: url ?? this.url,
        spkiPin: spkiPin ?? this.spkiPin,
      );
}

/// Override this provider in a [ProviderScope] to point at the
/// on-disk config file. Defaults to throwing so production wiring
/// must inject the file path explicitly (no surprise global state).
final esploraConfigFilePathProvider = Provider<File>((ref) {
  throw UnimplementedError('Override in ProviderScope');
});

class EsploraConfigNotifier extends AsyncNotifier<EsploraConfig> {
  @override
  Future<EsploraConfig> build() async {
    final file = ref.read(esploraConfigFilePathProvider);
    if (await file.exists()) {
      final raw = await file.readAsString();
      return EsploraConfig.fromJson(jsonDecode(raw) as Map<String, dynamic>);
    }
    return EsploraConfig.defaults('testnet');
  }

  Future<void> save(EsploraConfig cb) async {
    state = AsyncData(cb);
    final file = ref.read(esploraConfigFilePathProvider);
    await file.parent.create(recursive: true);
    await file.writeAsString(jsonEncode(cb.toJson()));
  }
}

final esploraConfigProvider =
    AsyncNotifierProvider<EsploraConfigNotifier, EsploraConfig>(
        EsploraConfigNotifier.new);
