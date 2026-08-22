import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/models/dto_parse_exception.dart';
import 'package:wallet_desktop/core/btc/models/fee_estimate.dart';
import 'package:wallet_desktop/core/btc/models/send_result.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';
import 'package:wallet_desktop/core/ffi/ffi_enums.dart';

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

  // Issue #263 — back-compat contract. `WalletDetail.fromJson`
  // intentionally does NOT read a `sync_status` key from the JSON
  // payload (the FFI never wrote one pre-#263). The factory must
  // default to `FfiSyncStatus.emptyWallet` so legacy JSON-decoded
  // data renders the pre-#263 'no funds yet' hint instead of the
  // red sync-failed banner — preserves pre-#263 UX for any saved
  // JSON, test fixture, or saved response that recorded
  // `balance_sat: 0` from a failed Esplora sync.
  test(
      'WalletDetail.fromJson defaults syncStatus to emptyWallet '
      'for legacy payloads (Issue #263 back-compat)',
      () {
    final d = WalletDetail.fromJson(const {
      'id': 'abc',
      'network': 'testnet',
      'address_type': 'native-segwit',
      'first_address': 'tb1q...',
      'balance': {
        'confirmed_sat': 0, // would have been SyncFailed pre-#263
      },
    });
    expect(d.syncStatus, FfiSyncStatus.emptyWallet);
    expect(d.lastError, isNull);
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

  // Issue #263 — `FfiSyncStatus.fromCode` defensive unknown for
  // ABI drift. Locks the safety net: any unrecognised byte
  // (including the explicit `unknown(255)` self-reference) maps
  // to `FfiSyncStatus.unknown`. Without this assertion a future
  // refactor that drops the defensive arm would silently let an
  // unknown byte flow through as `synced` (the default branch).
  test('FfiSyncStatus.fromCode defensively maps unknown bytes to '
      'FfiSyncStatus.unknown (Issue #263 ABI-drift safety)', () {
    expect(FfiSyncStatus.fromCode(0), FfiSyncStatus.synced);
    expect(FfiSyncStatus.fromCode(1), FfiSyncStatus.emptyWallet);
    expect(FfiSyncStatus.fromCode(2), FfiSyncStatus.syncFailed);
    expect(FfiSyncStatus.fromCode(3), FfiSyncStatus.unknown);
    expect(FfiSyncStatus.fromCode(99), FfiSyncStatus.unknown);
    expect(FfiSyncStatus.fromCode(255), FfiSyncStatus.unknown);
    expect(FfiSyncStatus.fromCode(-1), FfiSyncStatus.unknown);
  });
}
