// Integration tests for the wallet lifecycle exercised via BtcInvoker
// against the `fake_btc.sh` shell fixture. The fixture is built in
// Task 24 (Integration test). Until it exists, every test calls
// `markTestSkipped` so the suite is dead — once `fake_btc.sh` is in
// place, all tests run.
//
// **Operator-driven scope** (L29): these exercise the Dart↔CLI surface only.
// Live Testnet smoke is NOT covered here — that's the L29 operator-run
// gate. The fake returns canned JSON that matches the DTO schemas the
// real `btc` binary emits, so any DTO regression in the parser surfaces
// here too.
//
// **SECURITY**: the fixture is hermetic — it MUST NOT log or echo
// the mnemonic (L12 CRITICAL #2). The tests verify shape only; no
// mnemonic material is asserted on stdout/stderr.
// ignore_for_file: prefer_const_constructors
// (BtcCommand's static factory methods are not const-constructors;
//  the prefer_const_constructors hint cannot be honored here.)
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:path/path.dart' as p;
import 'package:wallet_desktop/core/btc/btc_command.dart';
import 'package:wallet_desktop/core/btc/btc_invoker.dart';
import 'package:wallet_desktop/core/btc/models/fee_estimate.dart';
import 'package:wallet_desktop/core/btc/models/send_result.dart';
import 'package:wallet_desktop/core/btc/models/tx_record.dart';
import 'package:wallet_desktop/core/btc/models/wallet_created.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';
import 'package:wallet_desktop/core/btc/models/wallet_info.dart';

void main() {
  // Task 24 contract: same fixture path the existing
  // btc_invoker_test.dart uses. Resolve to absolute path via
  // `p.canonicalize(Directory.current.path + ...)` so the test is
  // CWD-independent (per L12 reviewer LOW finding — flutter test
  // typically runs from the package root, but CI runners may differ).
  final mockScript =
      p.canonicalize(p.join('test', 'integration', 'fixtures', 'fake_btc.sh'));

  // Canonical BIP-39 12-word test phrase used as the fixture's
  // canned mnemonic output. PINNED explicitly per L12 CRITICAL #2: a
  // future fixture regression that emits a different phrase (or any
  // real-looking mnemonic) will fail this test loudly.
  const canonicalMnemonic =
      'abandon abandon abandon abandon abandon abandon '
      'abandon abandon abandon abandon abandon about';

  test('e2e: list wallets against fake btc', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
    final list = await invoker.invoke<List<WalletInfo>>(
      BtcCommand.walletList(network: 'testnet'),
      parse: (j) {
        // L34.1: defensive parse-null. BtcInvoker may call `parse(null)`
        // when stdout is empty; the real list is a JSON array.
        if (j is! List) return const <WalletInfo>[];
        return j
            .map((e) => WalletInfo.fromJson(e as Map<String, dynamic>))
            .toList(growable: false);
      },
    );
    expect(list, hasLength(1));
    expect(list.first.id, 'fake-uuid-1');
    expect(list.first.network, 'testnet');
  });

  test('e2e: show wallet returns balance + first address', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
    final detail = await invoker.invoke<WalletDetail>(
      BtcCommand.walletShow(
        id: 'fake-uuid-1',
        network: 'testnet',
        // Fixture ignores the password file contents — it MUST NEVER
        // echo them back. Pass `/dev/null` to avoid leaking the real
        // password-file path through the test logs.
        passwordFilePath: '/dev/null',
      ),
      // L34.1: defensive `is Map` guard. Empty stdout would otherwise
        // surface as a swallowed BtcError(other) at the UI layer.
      parse: (j) => j is Map<String, dynamic>
          ? WalletDetail.fromJson(j)
          : throw FormatException('empty wallet detail'),
    );
    expect(detail.id, 'fake-uuid-1');
    expect(detail.firstAddress, 'tb1qfake');
    expect(detail.balance.confirmedSat, 12345);
  });

  test('e2e: tx-list returns one tx', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
    final txs = await invoker.invoke<List<TxRecord>>(
      BtcCommand.txList(
        // L33.4 fix (per L12 reviewer HIGH finding): the previous
        // version passed `mnemonic: ''` which perpetuates the empty-
        // mnemonic argv violation L33.4 was written to prevent. The
        // fixture never echoes `--mnemonic`, so a non-empty fake value
        // models correct API usage without leaking secrets.
        mnemonic: 'fake-mnemonic-not-echoed-by-fixture',
        network: 'testnet',
        esploraUrl: 'https://blockstream.info/testnet/api',
        esploraSpkiPin: '',
      ),
      parse: (j) {
        if (j is! List) return const <TxRecord>[];
        return j
            .map((e) => TxRecord.fromJson(e as Map<String, dynamic>))
            .toList(growable: false);
      },
    );
    expect(txs, hasLength(1));
    expect(txs.first.txid, 'faketxid');
    expect(txs.first.amountSat, 1000);
  });

  test('e2e: wallet create returns id + first address + canned mnemonic',
      () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
    final created = await invoker.invoke<WalletCreated>(
      BtcCommand.walletCreate(
        words: 12,
        network: 'testnet',
        addressType: 'native-segwit',
        passwordFilePath: '/dev/null',
      ),
      // L34.1: defensive `is Map` guard.
      parse: (j) => j is Map<String, dynamic>
          ? WalletCreated.fromJson(j)
          : throw FormatException('empty wallet created'),
    );
    expect(created.id, 'fake-uuid-1');
    expect(created.firstAddress, 'tb1qfake');
    expect(created.network, 'testnet');
    // L12 CRITICAL #2: the fixture's stdout MUST contain the canonical
    // BIP-39 phrase. A regression that emits any other mnemonic (real
    // or fake) will fail this assertion loudly — the fixture must NEVER
    // accept a user-supplied mnemonic into its canned JSON output.
    expect(created.mnemonic, canonicalMnemonic);
  });

  test('e2e: wallet import returns id + first address + canned mnemonic',
      () async {
    // Plan §Task 24 covers import alongside create/show/list. The
    // fixture (`fake_btc.sh`) emits a `WalletCreated`-shaped response;
    // this test pins the DTO parse contract the same way the create
    // test does.
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
    final imported = await invoker.invoke<WalletCreated>(
      BtcCommand.walletImport(
        // L33.4: non-empty mnemonic models correct API usage. The
        // fixture never echoes this value.
        mnemonic: 'fake-mnemonic-not-echoed-by-fixture',
        network: 'testnet',
        passwordFilePath: '/dev/null',
      ),
      parse: (j) => j is Map<String, dynamic>
          ? WalletCreated.fromJson(j)
          : throw FormatException('empty wallet imported'),
    );
    expect(imported.id, 'fake-uuid-2');
    expect(imported.firstAddress, 'tb1qfake');
    // L12 CRITICAL #2: import must NOT echo the caller-supplied
    // mnemonic. The fixture returns the canonical BIP-39 phrase instead.
    expect(imported.mnemonic, canonicalMnemonic);
    expect(imported.mnemonic, isNot(contains('fake-mnemonic')));
  });

  test('e2e: fee-estimates returns parseable FeeEstimate', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
    final fe = await invoker.invoke<FeeEstimate>(
      BtcCommand.feeEstimates(
        network: 'testnet',
        esploraUrl: 'https://blockstream.info/testnet/api',
        esploraSpkiPin: '',
      ),
      // L34.1: defensive `is Map` guard.
      parse: (j) => j is Map<String, dynamic>
          ? FeeEstimate.fromJson(j)
          : throw FormatException('empty fee-estimates'),
    );
    expect(fe.fastestSatPerVb, greaterThan(0));
  });

  test('e2e: wallet send dry-run returns SendResult', () async {
    if (!await File(mockScript).exists()) {
      markTestSkipped('fake_btc.sh not built (Task 24)');
      return;
    }
    final invoker = BtcInvoker(binaryPath: mockScript);
    final result = await invoker.invoke<SendResult>(
      BtcCommand.walletSend(
        mnemonic: 'fake-mnemonic-not-echoed-by-fixture',
        network: 'testnet',
        address: 'tb1qfake',
        amountSat: 1000,
        feeRateSatPerVb: 1,
        passwordFilePath: '/dev/null',
        esploraUrl: 'https://blockstream.info/testnet/api',
        esploraSpkiPin: '',
        dryRun: true,
      ),
      // L34.1: defensive `is Map` guard.
      parse: (j) => j is Map<String, dynamic>
          ? SendResult.fromJson(j)
          : throw FormatException('empty send result'),
    );
    expect(result.txid, startsWith('faketxid'));
  });
}