import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/models/btc_config.dart';
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
}
