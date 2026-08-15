import 'package:meta/meta.dart';

import 'dto_parse_exception.dart';

@immutable
class SendResult {
  const SendResult({
    required this.txid,
    required this.feeSat,
    required this.vbytes,
  });
  final String txid;
  final int feeSat;
  final int vbytes;

  factory SendResult.fromJson(Map<String, dynamic> j) => SendResult(
        txid: dtoString(j, 'txid'),
        feeSat: dtoInt(j, 'fee_sat'),
        vbytes: dtoInt(j, 'vbytes'),
      );
}
