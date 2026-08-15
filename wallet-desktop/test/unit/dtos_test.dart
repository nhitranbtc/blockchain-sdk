import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/models/btc_config.dart';
import 'package:wallet_desktop/core/btc/models/dto_parse_exception.dart';
import 'package:wallet_desktop/core/btc/models/fee_estimate.dart';
import 'package:wallet_desktop/core/btc/models/send_result.dart';
import 'package:wallet_desktop/core/btc/models/tx_record.dart';
import 'package:wallet_desktop/core/btc/models/wallet_created.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';
import 'package:wallet_desktop/core/btc/models/wallet_info.dart';

void main() {
  test('WalletInfo round-trips', () {
    final w = WalletInfo.fromJson(const {
      'id': 'abc',
      'network': 'testnet',
      'address_type': 'native-segwit',
    });
    expect(w.id, 'abc');
    expect(w.network, 'testnet');
    expect(w.addressType, 'native-segwit');
  });

  test('WalletDetail from btc wallet show --json shape', () {
    final d = WalletDetail.fromJson(const {
      'id': 'abc',
      'network': 'testnet',
      'address_type': 'native-segwit',
      'first_address': 'tb1q...',
      'balance': {
        'confirmed_sat': 1000,
        'trusted_pending_sat': 0,
        'untrusted_pending_sat': 0,
        'immature_sat': 0,
      },
      'utxos': <Map<String, dynamic>>[],
    });
    expect(d.balance.confirmedSat, 1000);
    expect(d.firstAddress, 'tb1q...');
  });

  test('WalletCreated carries mnemonic + id + first address', () {
    final c = WalletCreated.fromJson(const {
      'id': 'abc',
      'mnemonic': 'abandon x11 about',
      'first_address': 'tb1q...',
      'network': 'testnet',
      'address_type': 'native-segwit',
    });
    expect(c.mnemonic, 'abandon x11 about');
    expect(c.id, 'abc');
    // SECURITY: toString must mask mnemonic (defense vs accidental logging).
    expect(c.toString(), isNot(contains('abandon')));
    expect(c.toString(), contains('<masked>'));
  });

  test('TxRecord parses direction + amount_sat + txid + confirmations', () {
    final t = TxRecord.fromJson(const {
      'txid': 'def',
      'direction': 'outgoing',
      'amount_sat': 5000,
      'fee_sat': 250,
      'confirmations': 6,
      'timestamp': 1700000000,
    });
    expect(t.direction, TxDirection.outgoing);
    expect(t.confirmations, 6);
  });

  test('TxRecord unknown direction maps to TxDirection.unknown (not silent)',
      () {
    // Per flutter-reviewer HIGH: a CLI rename to 'received' must NOT
    // silently coerce to 'outgoing'. Pin the load-bearing unknown arm.
    final t = TxRecord.fromJson(const {
      'txid': 'def',
      'direction': 'received',
      'amount_sat': 5000,
    });
    expect(t.direction, TxDirection.unknown);
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

  test('BtcConfig from btc config show --json', () {
    final c = BtcConfig.fromJson(const {
      'data_dir': '/tmp/btc',
      'network': 'testnet',
      'esplora_url': 'https://blockstream.info/testnet/api',
      'wallets': ['abc', 'def'],
    });
    expect(c.dataDir, '/tmp/btc');
    expect(c.wallets, ['abc', 'def']);
  });

  test('DtoParseException fires on missing required field', () {
    // Per flutter-reviewer HIGH: contract test for the typed-error path
    // that replaces raw `as String` casts. Without this test, a refactor
    // that drops dtoString() (reverting to `as String`) would silently
    // change the failure mode from DtoParseException to TypeError.
    expect(
      () => WalletInfo.fromJson(const <String, Object?>{}),
      throwsA(isA<DtoParseException>().having(
        (e) => e.path,
        'path',
        isNotEmpty,
      )),
    );
  });

  test('Utxo list is unmodifiable (immutability contract)', () {
    final d = WalletDetail.fromJson(const {
      'id': 'abc',
      'network': 'testnet',
      'address_type': 'native-segwit',
      'first_address': 'tb1q...',
      'balance': {
        'confirmed_sat': 1000,
        'trusted_pending_sat': 0,
        'untrusted_pending_sat': 0,
        'immature_sat': 0,
      },
      'utxos': <Map<String, dynamic>>[],
    });
    expect(() => d.utxos.clear(), throwsUnsupportedError);
  });
}
