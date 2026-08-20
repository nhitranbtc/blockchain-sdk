import 'package:meta/meta.dart';

import 'dto_parse_exception.dart';

@immutable
class WalletCreated {
  const WalletCreated({
    required this.id,
    required this.mnemonic,
    required this.firstAddress,
    required this.network,
    required this.addressType,
  });
  final String id;
  final String mnemonic;
  final String firstAddress;
  final String network;
  final String addressType;

  factory WalletCreated.fromJson(Map<String, dynamic> j) => WalletCreated(
        id: dtoString(j, 'id'),
        mnemonic: dtoString(j, 'mnemonic'),
        firstAddress: dtoString(j, 'first_address'),
        network: dtoString(j, 'network'),
        addressType: dtoString(j, 'address_type'),
      );

  /// SECURITY: `mnemonic` is a plaintext String (Deferred to v0.2
  /// `Secret<String>` zeroize wrapper). Override `toString` to mask the
  /// mnemonic so accidental `print(walletCreated)` / Sentry breadcrumbs /
  /// Flutter error handler can't leak it. Callers needing the value MUST
  /// access `walletCreated.mnemonic` directly.
  @override
  String toString() => 'WalletCreated(id: $id, firstAddress: $firstAddress, '
      'network: $network, addressType: $addressType, '
      'mnemonic: <masked>)';
}
