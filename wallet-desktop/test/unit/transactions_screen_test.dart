import 'dart:ffi';

import 'package:ffi/ffi.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/models/fee_estimate.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';
import 'package:wallet_desktop/core/ffi/ffi_enums.dart';
import 'package:wallet_desktop/core/ffi/ffi_exception.dart';
import 'package:wallet_desktop/core/ffi/secret_buffer.dart';
import 'package:wallet_desktop/core/wallet_core_api.dart';
import 'package:wallet_desktop/features/wallet_transactions/transactions_screen.dart';
import 'package:wallet_desktop/providers/wallet_core_provider.dart';
import 'package:wallet_desktop/providers/wallet_providers.dart';

const _kTestnet = 'testnet';
const _kWalletId = 'wlt-abc';
const _kMnemonic = 'legal winner thank year wave sausage worth useful legal '
    'winner thank yellow';

/// Test double — implements [WalletCoreApi]. Returns canned txid list
/// (or empty) without loading the native library. Mirrors the seam
/// established in Tasks 13/17/20/21.
class _FakeWalletCore implements WalletCoreApi {
  _FakeWalletCore({this.txids = const [], this.exceptionToThrow});

  /// Default: empty list (fresh wallet).
  final List<String> txids;

  /// When non-null, `walletTxids` throws this exception.
  final FfiException? exceptionToThrow;

  @override
  String ffiVersion() => 'fake-0.0.0';

  @override
  List<String> listWallets({
    required FfiNetwork network,
    required String baseDir,
  }) {
    throw UnimplementedError();
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

  @override
  WalletDetail showWallet({
    required FfiNetwork network,
    required String walletId,
    required SecretBuffer password,
    required String baseDir,
  }) {
    throw UnimplementedError();
  }

  @override
  FeeEstimate feeEstimate({required Pointer<Void> esploraHandle}) {
    throw UnimplementedError();
  }

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

  @override
  List<String> walletTxids({required Pointer<Void> walletHandle}) {
    final exc = exceptionToThrow;
    if (exc != null) throw exc;
    return List<String>.from(txids);
  }
}

/// Stub [WalletSessionNotifier] that pre-populates the session with
/// fake FFI handles — avoids needing `ensureHandles()` to actually
/// invoke `walletLoad` + `esploraClientNew`. The screen's ensureHandles
/// call is a no-op (handles already in state). Tests inject this via
/// `walletSessionProvider.overrideWith(_FakeSessionNotifier.new)`.
class _FakeSessionNotifier extends WalletSessionNotifier {
  @override
  WalletSession? build(String walletId) {
    ref.onDispose(() {});
    return WalletSession(
      walletId: walletId,
      mnemonic: OpaqueMnemonic(_kMnemonic),
      detail: _seedDetail(),
      walletHandle: Pointer<Void>.fromAddress(0xDEAD),
      esploraHandle: Pointer<Void>.fromAddress(0xBEEF),
    );
  }

  @override
  Future<void> ensureHandles() async {
    // No-op: handles pre-populated in build().
  }
}

/// Stub [WalletSessionNotifier] that returns a session with the
/// empty-string mnemonic sentinel (Task 20 carry-over). Used by the
/// re-entry view test — ensures `_load` is short-circuited.
class _FakeSessionSentinelNotifier extends WalletSessionNotifier {
  @override
  WalletSession? build(String walletId) {
    ref.onDispose(() {});
    return WalletSession(
      walletId: walletId,
      mnemonic: OpaqueMnemonic(''),
      detail: _seedDetail(),
      walletHandle: Pointer<Void>.fromAddress(0xDEAD),
      esploraHandle: Pointer<Void>.fromAddress(0xBEEF),
    );
  }

  @override
  Future<void> ensureHandles() async {
    // No-op.
  }
}

WalletDetail _seedDetail() => const WalletDetail(
      id: _kWalletId,
      network: _kTestnet,
      addressType: 'native-segwit',
      firstAddress: 'tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx',
      balance: Balance(
        confirmedSat: 0,
      ),
    );

void main() {
  testWidgets(
    'TransactionsScreen renders the Transactions header '
    'when the wallet session has a mnemonic',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(_FakeWalletCore()),
        walletSessionProvider
            .overrideWith(_FakeSessionNotifier.new),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(
              body: TransactionsScreen(
                network: _kTestnet,
                walletId: _kWalletId,
              ),
            ),
          ),
        ),
      );
      await t.pump();
      await t.pump(const Duration(milliseconds: 50));

      expect(find.text('Transactions'), findsOneWidget);
    },
  );

  testWidgets(
    'TransactionsScreen shows the re-enter-mnemonic form when the '
    'session has the empty-string sentinel (Task 20 carry-over)',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(_FakeWalletCore()),
        walletSessionProvider.overrideWith(_FakeSessionSentinelNotifier.new),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(
              body: TransactionsScreen(
                network: _kTestnet,
                walletId: _kWalletId,
              ),
            ),
          ),
        ),
      );
      await t.pump();

      // Tx list MUST NOT render — the user must first provide the
      // mnemonic before we call `wallet_txids`.
      expect(find.textContaining('txid'), findsNothing);
      // Re-prompt surface renders.
      expect(find.textContaining('mnemonic'), findsAtLeastNWidgets(1));
    },
  );

  testWidgets(
    'TransactionsScreen shows the LockedView when the wallet session '
    'is null (deep-link entry without prior unlock)',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(_FakeWalletCore()),
        // No session override — walletSessionProvider(_kWalletId)
        // resolves to null. Screen renders LockedView.
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(
              body: TransactionsScreen(
                network: _kTestnet,
                walletId: _kWalletId,
              ),
            ),
          ),
        ),
      );
      await t.pump();

      expect(find.text('Wallet is locked.'), findsOneWidget);
      expect(find.text('Unlock'), findsOneWidget);
      expect(find.textContaining('txid'), findsNothing);
    },
  );

  testWidgets(
    'TransactionsScreen renders the FfiException kind surface when '
    'walletTxids throws',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(
          _FakeWalletCore(
            exceptionToThrow: FfiException.fromCode(
              code: -34, // walletStore
              op: 'wallet_txids',
            ),
          ),
        ),
        walletSessionProvider
            .overrideWith(_FakeSessionNotifier.new),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(
              body: TransactionsScreen(
                network: _kTestnet,
                walletId: _kWalletId,
              ),
            ),
          ),
        ),
      );
      for (var i = 0; i < 10; i++) {
        await t.pump(const Duration(milliseconds: 50));
      }

      // Kind-mapped user message surfaces (FfiErrorKind.walletStore).
      // The screen renders `error.kind.name` for now (v0.2.1 — see
      // docstring). Future v0.3: kind-mapped copy via
      // `userMessageForFfiException`.
      expect(find.textContaining('walletStore'), findsOneWidget);
    },
  );

  testWidgets(
    'TransactionsScreen renders one row per tx returned by the FFI',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(
          _FakeWalletCore(txids: const ['a1b2c3d4e5f6', 'f6e5d4c3b2a1']),
        ),
        walletSessionProvider
            .overrideWith(_FakeSessionNotifier.new),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(
              body: TransactionsScreen(
                network: _kTestnet,
                walletId: _kWalletId,
              ),
            ),
          ),
        ),
      );
      await t.pump();
      for (var i = 0; i < 10; i++) {
        await t.pump(const Duration(milliseconds: 50));
      }

      // Both txids surface (txids are public on the blockchain so
      // safe to render).
      expect(find.text('a1b2c3d4e5f6'), findsOneWidget);
      expect(find.text('f6e5d4c3b2a1'), findsOneWidget);
    },
  );

  // v0.2 deferred: end-to-end submit via Task 24 `fake_btc.sh`
  // integration test (operator-driven per L29).
  test('placeholder — tx-list submit coverage deferred to Task 24', () {
    // empty body — defer per Task 17/18 lesson.
  }, skip: 'Task 24 fake_btc.sh integration');
}
