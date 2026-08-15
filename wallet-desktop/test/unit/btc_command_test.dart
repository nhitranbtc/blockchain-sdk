import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/btc_command.dart';

void main() {
  test('WalletList builds argv with --network + --json', () {
    final cmd = BtcCommand.walletList(network: 'testnet');
    expect(cmd.argv, ['wallet', 'list', '--network', 'testnet', '--json']);
  });

  test('WalletShow includes password-file and esplora-url', () {
    final cmd = BtcCommand.walletShow(
      id: 'abc-uuid',
      network: 'testnet',
      passwordFilePath: '/tmp/abc.pwd',
      esploraUrl: 'https://blockstream.info/testnet/api',
      esploraSpkiPin: '0'.padLeft(64, '0'),
    );
    expect(cmd.argv, [
      'wallet', 'show', 'abc-uuid',
      '--network', 'testnet',
      '--password-file', '/tmp/abc.pwd',
      '--esplora-url', 'https://blockstream.info/testnet/api',
      '--pin-spki', '0'.padLeft(64, '0'),
    ]);
  });

  test('WalletSend single-recipient builds --to flag', () {
    final cmd = BtcCommand.walletSend(
      mnemonic: 'abandon x11 about',
      network: 'testnet',
      to: 'tb1qaddr:10000',
      feeRateSatPerVb: 5,
      passwordFilePath: '/tmp/pwd',
      esploraUrl: 'https://blockstream.info/testnet/api',
      esploraSpkiPin: '0'.padLeft(64, '0'),
    );
    expect(cmd.argv, containsAllInOrder([
      'wallet', 'send',
      '--mnemonic', 'abandon x11 about',
      '--network', 'testnet',
      '--to', 'tb1qaddr:10000',
      '--fee-rate', '5',
      '--password-file', '/tmp/pwd',
      '--esplora-url', 'https://blockstream.info/testnet/api',
      '--pin-spki', '0'.padLeft(64, '0'),
    ]));
  });

  test('WalletCreate includes words + type + password-file', () {
    final cmd = BtcCommand.walletCreate(
      words: 12,
      network: 'testnet',
      addressType: 'native-segwit',
      passwordFilePath: '/tmp/pwd',
    );
    expect(cmd.argv, containsAllInOrder([
      'wallet', 'create',
      '--words', '12',
      '--network', 'testnet',
      '--type', 'native-segwit',
      '--password-file', '/tmp/pwd',
    ]));
  });

  test('TxList includes mnemonic + limit', () {
    final cmd = BtcCommand.txList(
      mnemonic: 'abandon x11 about',
      network: 'testnet',
      esploraUrl: 'https://blockstream.info/testnet/api',
      esploraSpkiPin: '0'.padLeft(64, '0'),
      limit: 10,
    );
    expect(cmd.argv, containsAllInOrder([
      'tx-list', '--mnemonic', 'abandon x11 about',
      '--network', 'testnet',
      '--esplora-url', 'https://blockstream.info/testnet/api',
      '--pin-spki', '0'.padLeft(64, '0'),
      '--limit', '10',
    ]));
  });
}
