import 'package:meta/meta.dart';

import 'dto_parse_exception.dart';

/// Transaction direction. `unknown` is the safe default — surfaces UI to
/// "unrecognized tx direction" instead of silently mislabeling as
/// `outgoing` (per flutter-reviewer HIGH: a CLI rename to `'received'`
/// would silently mislabel all incoming txs as outgoing in the UI).
enum TxDirection { incoming, outgoing, self, unknown }

@immutable
class TxRecord {
  const TxRecord({
    required this.txid,
    required this.direction,
    required this.amountSat,
    required this.feeSat,
    required this.confirmations,
    required this.timestamp,
  });
  final String txid;
  final TxDirection direction;
  final int amountSat;
  final int feeSat;
  final int confirmations;
  final int timestamp;

  factory TxRecord.fromJson(Map<String, dynamic> j) => TxRecord(
        txid: dtoString(j, 'txid'),
        direction: switch ((dtoString(j, 'direction')).toLowerCase()) {
          'incoming' => TxDirection.incoming,
          'outgoing' => TxDirection.outgoing,
          'self' => TxDirection.self,
          _ => TxDirection.unknown,
        },
        amountSat: dtoInt(j, 'amount_sat'),
        feeSat: dtoIntOpt(j, 'fee_sat') ?? 0,
        confirmations: dtoIntOpt(j, 'confirmations') ?? 0,
        timestamp: dtoIntOpt(j, 'timestamp') ?? 0,
      );
}
