// Task 10 (#216) test — migrated `WalletsListNotifier` reads from
// `walletCoreProvider` (Task 6+8+9 surface) instead of `btcInvokerProvider`.
// Returns `List<String>` of wallet IDs (Rust `wallet_list` returns the
// id list directly — no `WalletInfo` parsing).
//
// L12 CRITICAL #1 first real consumer: when `walletCore.listWallets`
// throws `FfiException`, the notifier surfaces the typed exception in
// `AsyncError` — UI catch blocks in `WalletListScreen` (Task 10) match
// on `e.kind` to render user-facing copy.
//
// L34.1 guard: empty list for fresh install returns `AsyncData([])`,
// NOT `AsyncError` (the Rust side may legitimately return a 0-length
// array).

import 'dart:ffi';

import 'package:ffi/ffi.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/models/fee_estimate.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';
import 'package:wallet_desktop/core/ffi/ffi_enums.dart';
import 'package:wallet_desktop/core/ffi/ffi_exception.dart';
import 'package:wallet_desktop/core/ffi/secret_buffer.dart';
import 'package:wallet_desktop/core/wallet_core_api.dart';
import 'package:wallet_desktop/providers/wallet_core_provider.dart';
import 'package:wallet_desktop/providers/wallet_providers.dart';

/// Test double — implements the `WalletCoreApi` interface that
/// `walletCoreProvider` exposes (Task 10 mockability seam). Lets
/// tests return canned id lists or throw typed `FfiException`s without
/// requiring the native library to be loaded.
class _FakeWalletCore implements WalletCoreApi {
  _FakeWalletCore({this.fixture = const [], this.throwException});

  /// Default: empty list (fresh-install).
  final List<String> fixture;

  /// When non-null, `listWallets` throws this exception.
  final FfiException? throwException;

  int callCount = 0;

  @override
  String ffiVersion() => 'fake-0.0.0';

  @override
  List<String> listWallets({
    required FfiNetwork network,
    required String baseDir,
  }) {
    callCount += 1;
    final exc = throwException;
    if (exc != null) throw exc;
    return List<String>.from(fixture);
  }

  @override
  void deleteWallet({
    required FfiNetwork network,
    required String walletId,
    required String baseDir,
  }) {
    throw UnimplementedError();
  }

  @override
  WalletCreatedData createWallet({
    required int words,
    required FfiNetwork network,
    required FfiAddressType addressType,
    required SecretBuffer password,
    required String baseDir,
  }) {
    throw UnimplementedError();
  }

  @override
  WalletImportedData importWallet({
    required FfiNetwork network,
    required SecretBuffer phrase,
    required SecretBuffer password,
    required String baseDir,
  }) {
    throw UnimplementedError();
  }

  // Task 13 — required by `WalletCoreApi.showWallet` addition; this
  // fake is only used for list/notifier tests so the body throws.
  @override
  WalletDetail showWallet({
    required FfiNetwork network,
    required String walletId,
    required SecretBuffer password,
    required String baseDir,
  }) {
    throw UnimplementedError();
  }

  // Task 16 — required by `WalletCoreApi.feeEstimate` addition; this
  // fake is only used for list/notifier tests so the body throws.
  @override
  FeeEstimate feeEstimate({required Pointer<Void> esploraHandle}) {
    throw UnimplementedError();
  }

  // Task 14 Sub-split B — handle lifecycle methods; this fake is
  // only used for list/notifier tests so the bodies throw.
  @override
  Pointer<Void> esploraClientNew({
    required Pointer<Utf8> url,
    Pointer<Utf8>? spkiPinB64,
  }) {
    throw UnimplementedError();
  }

  @override
  void esploraClientFree(Pointer<Void> handle) {
    throw UnimplementedError();
  }

  @override
  Pointer<Void> walletLoad({
    required FfiNetwork network,
    required String walletId,
    required SecretBuffer phrase,
    required String baseDir,
  }) {
    throw UnimplementedError();
  }

  @override
  void walletLoadFree(Pointer<Void> handle) {
    throw UnimplementedError();
  }

  // Task 14 Sub-split B-step-2 — walletSend method; this fake is
  // only used for list/notifier tests so the body throws.
  @override
  String walletSend({
    required Pointer<Void> walletHandle,
    required Pointer<Void> esploraHandle,
    required Pointer<Utf8> recipient,
    required int amountSat,
    required int feeRateSatPerVb,
  }) {
    throw UnimplementedError();
  }

  // Task 17 / Issue #223 — walletTxids facade; this fake is only
  // used for list/notifier tests so the body throws.
  @override
  List<String> walletTxids({required Pointer<Void> walletHandle}) {
    throw UnimplementedError();
  }
}

void main() {
  group('WalletsListNotifier (FFI migration, Task 10)', () {
    test('returns AsyncData with id list on success', () async {
      final fake = _FakeWalletCore(
        fixture: const ['wlt-abc', 'wlt-def'],
      );
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(fake),
      ]);
      addTearDown(container.dispose);

      final list = await container.read(walletsListProvider('testnet').future);
      expect(list, equals(['wlt-abc', 'wlt-def']));
      expect(fake.callCount, 1);
    });

    test('returns AsyncData([]) on empty list (L34.1 guard, fresh install)',
        () async {
      final fake = _FakeWalletCore(); // default fixture = []
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(fake),
      ]);
      addTearDown(container.dispose);

      final list = await container.read(walletsListProvider('testnet').future);
      expect(list, isEmpty);
      expect(list, isA<List<String>>());
    });

    test(
        'surfaces FfiException(io) in AsyncError when Rust returns '
        'storage failure', () async {
      final fake = _FakeWalletCore(
        throwException: FfiException.fromCode(code: -42, op: 'wallet_list'),
      );
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(fake),
      ]);
      addTearDown(container.dispose);

      await expectLater(
        container.read(walletsListProvider('testnet').future),
        throwsA(isA<FfiException>()
            .having((e) => e.kind, 'kind', equals(FfiErrorKind.io))),
      );
    });

    test(
        'surfaces FfiException(walletStore) when wallet blob is corrupted '
        '(N2 oracle: indistinguishable from wrong-password)', () async {
      final fake = _FakeWalletCore(
        throwException: FfiException.fromCode(
          code: -34,
          op: 'wallet_list',
        ),
      );
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(fake),
      ]);
      addTearDown(container.dispose);

      await expectLater(
        container.read(walletsListProvider('testnet').future),
        throwsA(isA<FfiException>().having(
          (e) => e.kind,
          'kind',
          equals(FfiErrorKind.walletStore),
        )),
      );
    });

    test('refresh() re-invokes build via invalidateSelf', () async {
      final fake = _FakeWalletCore(fixture: const ['wlt-abc']);
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(fake),
      ]);
      addTearDown(container.dispose);

      final notifier = container.read(walletsListProvider('testnet').notifier);
      final first = await container.read(walletsListProvider('testnet').future);
      expect(first, equals(['wlt-abc']));
      expect(fake.callCount, 1);

      await notifier.refresh();
      expect(fake.callCount, 2);
    });
  });
}
