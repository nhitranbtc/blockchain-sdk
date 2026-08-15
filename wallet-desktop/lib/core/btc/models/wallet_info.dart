import 'package:meta/meta.dart';

import 'dto_parse_exception.dart';

@immutable
class WalletInfo {
  const WalletInfo({
    required this.id,
    required this.network,
    required this.addressType,
  });
  final String id;
  final String network;
  final String addressType;

  factory WalletInfo.fromJson(Map<String, dynamic> j) => WalletInfo(
        id: dtoString(j, 'id'),
        network: dtoString(j, 'network'),
        addressType: dtoString(j, 'address_type'),
      );
}
