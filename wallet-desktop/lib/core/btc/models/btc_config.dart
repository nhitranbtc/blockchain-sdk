import 'package:meta/meta.dart';

import 'dto_parse_exception.dart';

@immutable
class BtcConfig {
  const BtcConfig({
    required this.dataDir,
    required this.network,
    required this.esploraUrl,
    required this.wallets,
  });
  final String dataDir;
  final String network;
  final String esploraUrl;
  final List<String> wallets;

  factory BtcConfig.fromJson(Map<String, dynamic> j) => BtcConfig(
        dataDir: dtoString(j, 'data_dir'),
        network: dtoString(j, 'network'),
        esploraUrl: dtoString(j, 'esplora_url'),
        wallets: List<String>.unmodifiable(
          ((j['wallets'] as List?) ?? const <Object>[])
              .map((e) {
                if (e is! String) {
                  throw DtoParseException('wallets[]', e);
                }
                return e;
              }),
        ),
      );
}
