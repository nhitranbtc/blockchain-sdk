import 'package:meta/meta.dart';

import 'dto_parse_exception.dart';

@immutable
class Balance {
  const Balance({
    required this.confirmedSat,
    required this.trustedPendingSat,
    required this.untrustedPendingSat,
    required this.immatureSat,
  });
  final int confirmedSat;
  final int trustedPendingSat;
  final int untrustedPendingSat;
  final int immatureSat;

  factory Balance.fromJson(Map<String, dynamic> j) => Balance(
        confirmedSat: dtoInt(j, 'confirmed_sat'),
        trustedPendingSat: dtoIntOpt(j, 'trusted_pending_sat') ?? 0,
        untrustedPendingSat: dtoIntOpt(j, 'untrusted_pending_sat') ?? 0,
        immatureSat: dtoIntOpt(j, 'immature_sat') ?? 0,
      );
}

@immutable
class Utxo {
  const Utxo({required this.txid, required this.vout, required this.valueSat});
  final String txid;
  final int vout;
  final int valueSat;

  factory Utxo.fromJson(Map<String, dynamic> j) => Utxo(
        txid: dtoString(j, 'txid'),
        vout: dtoInt(j, 'vout'),
        valueSat: dtoInt(j, 'value_sat'),
      );
}

@immutable
class WalletDetail {
  const WalletDetail({
    required this.id,
    required this.network,
    required this.addressType,
    required this.firstAddress,
    required this.balance,
    required this.utxos,
  });
  final String id;
  final String network;
  final String addressType;
  final String firstAddress;
  final Balance balance;
  final List<Utxo> utxos;

  factory WalletDetail.fromJson(Map<String, dynamic> j) {
    final balanceRaw = j['balance'];
    if (balanceRaw is! Map<String, dynamic>) {
      throw DtoParseException('balance', balanceRaw);
    }
    final utxosRaw = j['utxos'];
    if (utxosRaw != null && utxosRaw is! List) {
      throw DtoParseException('utxos', utxosRaw);
    }
    return WalletDetail(
      id: dtoString(j, 'id'),
      network: dtoString(j, 'network'),
      addressType: dtoString(j, 'address_type'),
      firstAddress: dtoString(j, 'first_address'),
      balance: Balance.fromJson(balanceRaw),
      utxos: List<Utxo>.unmodifiable(
        ((utxosRaw ?? const <Object?>[]) as List<Object?>).map((Object? e) {
          if (e is! Map<String, dynamic>) {
            throw DtoParseException('utxos[]', e);
          }
          return Utxo.fromJson(e);
        }),
      ),
    );
  }
}
