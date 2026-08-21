import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/models/dto_parse_exception.dart';
import 'package:wallet_desktop/core/btc/models/fee_estimate.dart';
import 'package:wallet_desktop/core/btc/models/send_result.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';

void main() {
  test('WalletDetail from FFI wallet_show shape (Task 13 collapsed)', () {
    final d = WalletDetail.fromJson(const {
      'id': 'abc',
      'network': 'testnet',
      'address_type': 'native-segwit',
      'first_address': 'tb1q...',
      'balance': {
        'confirmed_sat': 1000,
      },
    });
    // Task 13: utxos field dropped; Balance collapsed from 4-tuple
    // to single `confirmedSat`. Legacy 4-tuple keys are ignored by
    // the parser (defense-in-depth for the json-from-disk path).
    expect(d.balance.confirmedSat, 1000);
    expect(d.firstAddress, 'tb1q...');
  });

  test('FeeEstimate from btc fee-estimates --json (target -> sat/vB)', () {
    final f = FeeEstimate.fromJson(
        const {'1': 25.0, '3': 12.0, '6': 8.0, '144': 1.0});
    expect(f.fastestSatPerVb, 25);
    expect(f.economySatPerVb, 1);
  });

  test('SendResult carries txid + fee + vbytes', () {
    final s = SendResult.fromJson(
        const {'txid': 'def', 'fee_sat': 540, 'vbytes': 110});
    expect(s.txid, 'def');
    expect(s.feeSat, 540);
    expect(s.vbytes, 110);
  });

  test('DtoParseException fires on missing required field', () {
    // Per flutter-reviewer HIGH: contract test for the typed-error path
    // that replaces raw `as String` casts. Without this test, a refactor
    // that drops dtoString() (reverting to `as String`) would silently
    // change the failure mode from DtoParseException to TypeError.
    expect(
      () => WalletDetail.fromJson(const <String, Object?>{}),
      throwsA(isA<DtoParseException>().having(
        (e) => e.path,
        'path',
        isNotEmpty,
      )),
    );
  });

  // Task 13: Utxo class removed (Rust FFI doesn't return UTXO list
  // — v0.2.0 read-only show). The unmodifiable-list test is gone;
  // `Balance` is now a single-field immutable class with no nested
  // collection to test.
}
