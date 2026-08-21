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
      // L12 type-design Task 23 MEDIUM: regtest canonical default.
      // Local regtest Esplora standard is electrs on 127.0.0.1:50002.
      // This is the ONLY hardcoded default — it's a localhost dev
      // escape that bypasses F20 SPKI-pin enforcement (the Rust
      // `EsploraClient::new` accepts null pins for localhost per
      // F36 dev-mode exception). All public-network hosts require
      // an operator-provided config file with a valid SPKI pin —
      // throwing here forces that setup.
      case 'regtest':
        return const EsploraConfig(
          network: 'regtest',
          url: 'http://127.0.0.1:50002/api',
          spkiPin: '',
        );
      // Issue #148 sweep (2026-08-21): removed hardcoded public
      // Esplora URLs (blockstream.info) for bitcoin/testnet/
      // testnet4/signet. The previous defaults would fail F20
      // enforcement at runtime (empty SPKI pin + public host → null
      // pin rejected by `esplora_client_new`). Throwing surfaces
      // the misconfiguration at boot instead of at first Esplora
      // call — better operator UX.
      case 'bitcoin':
      case 'testnet':
      case 'testnet4':
      case 'signet':
      case 'mainnet':
        throw StateError(
          'EsploraConfig.defaults("$network"): public-network hosts '
          'require an operator-provided config file with a valid SPKI '
          'pin (F20 enforcement). No default URL ships with the binary '
          '— write the config to '
          '\$XDG_CONFIG_HOME/flutter_btc_wallet/esplora.json with '
          '{"network":"$network","url":"<your-host>",'
          '"spkiPin":"<base64-pin>"} and retry. See '
          'lib/providers/esplora_config_provider.dart docstring for the '
          'SPKI pin retrieval workflow.',
        );
      default:
        throw ArgumentError(
          'EsploraConfig.defaults: unknown network "$network" — '
          'expected one of: bitcoin, testnet, testnet4, signet, '
          'regtest, mainnet.',
        );
    }
  }

  Map<String, dynamic> toJson() =>
      {'network': network, 'url': url, 'spkiPin': spkiPin};

  /// **Test-only** factory — bypasses the production F20 enforcement
  /// (no operator-provided config required). Exists so tests can
  /// construct arbitrary configs without throwing. **NOT for
  /// production use** — production must go through operator-provided
  /// config file (see [EsploraConfig.defaults] throw path).
  @visibleForTesting
  factory EsploraConfig.forTesting({
    required String network,
    required String url,
    String spkiPin = '',
  }) =>
      EsploraConfig(network: network, url: url, spkiPin: spkiPin);

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
