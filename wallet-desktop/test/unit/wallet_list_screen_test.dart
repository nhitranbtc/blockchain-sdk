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
import 'package:wallet_desktop/features/wallet_list/wallet_list_screen.dart';
import 'package:wallet_desktop/providers/wallet_core_provider.dart';
import 'package:wallet_desktop/routing/wallet_routes.dart';

/// Test double — implements [WalletCoreApi]. Returns canned id lists
/// or throws typed [FfiException]s without loading the native library.
class _FakeWalletCore implements WalletCoreApi {
  _FakeWalletCore({this.fixture = const [], this.throwException});

  final List<String> fixture;
  final FfiException? throwException;

  @override
  String ffiVersion() => 'fake-0.0.0';

  @override
  List<String> listWallets({
    required FfiNetwork network,
    required String baseDir,
  }) {
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
  // fake is only used for list-screen tests so the body throws.
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
  // fake is only used for list-screen tests so the body throws.
  @override
  FeeEstimate feeEstimate({required Pointer<Void> esploraHandle}) {
    throw UnimplementedError();
  }

  // Task 14 Sub-split B — handle lifecycle methods; this fake is
  // only used for list-screen tests so the bodies throw.
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
  // only used for list-screen tests so the body throws.
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
}

/// Single source of truth for the testnet identifier used by every Task 17
/// fixture + screen. Matches `appRouter().initialLocation = '/wallets/testnet'`
/// (Task 16) and `NetworkPicker.default = 'testnet'` (Task 15).
const _kTestnet = 'testnet';

void main() {
  testWidgets(
    'WalletListScreen shows empty state + Create/Import labels '
    'when the wallet list is empty',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(_FakeWalletCore()),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: WalletListScreen(network: _kTestnet)),
          ),
        ),
      );
      await t.pumpAndSettle();

      expect(find.text('Create'), findsOneWidget);
      expect(find.text('Import'), findsOneWidget);
      expect(find.textContaining('No wallets'), findsOneWidget);
    },
  );

  testWidgets(
    'WalletListScreen renders a row per wallet when the list is non-empty',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(
          _FakeWalletCore(fixture: const ['wlt-abc', 'wlt-def']),
        ),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: WalletListScreen(network: _kTestnet)),
          ),
        ),
      );
      await t.pumpAndSettle();

      // ids <=12 chars render in full (security-auditor Task 17 review).
      expect(find.text('wlt-abc'), findsOneWidget);
      expect(find.text('wlt-def'), findsOneWidget);
    },
  );

  testWidgets(
    'WalletListScreen truncates long wallet ids to first4...last4',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(
          _FakeWalletCore(fixture: const [
            // 32-char hex (matches btc's fingerprint shape).
            'abcdef0123456789abcdef0123456789',
          ]),
        ),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: WalletListScreen(network: _kTestnet)),
          ),
        ),
      );
      await t.pumpAndSettle();

      expect(find.text('abcd…6789'), findsOneWidget);
      // Full id never surfaces in the visible widget tree.
      expect(find.text('abcdef0123456789abcdef0123456789'), findsNothing);
    },
  );

  testWidgets(
    'WalletListScreen shows a friendly error + Retry button '
    'when walletCore throws FfiException(io)',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(
          _FakeWalletCore(
            throwException: FfiException.fromCode(
              code: -42, // Io
              op: 'wallet_list',
            ),
          ),
        ),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: WalletListScreen(network: _kTestnet)),
          ),
        ),
      );
      await t.pumpAndSettle();

      // L12 review MED #3 fix: kind-mapped copy from
      // `userMessageForFfiException` (FfiErrorKind.io -> 'I/O error.').
      expect(find.text('I/O error.'), findsOneWidget);
      expect(find.text('Retry'), findsOneWidget);
      // The raw `code` MUST NOT surface (L12 CRITICAL #2 contract).
      expect(find.textContaining('-42'), findsNothing);
    },
  );

  testWidgets(
    'WalletListScreen shows kind-mapped copy for FfiException(walletStore)',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(
          _FakeWalletCore(
            throwException: FfiException.fromCode(
              code: -34, // walletStore
              op: 'wallet_list',
            ),
          ),
        ),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: WalletListScreen(network: _kTestnet)),
          ),
        ),
      );
      await t.pumpAndSettle();

      // walletStore maps to the N2 oracle-mitigation message — same
      // copy for "wrong password" / "wrong blob" / "wrong network".
      expect(
          find.text('Cannot unlock wallet — check password.'), findsOneWidget);
    },
  );

  testWidgets(
    'WalletListScreen invokes onCreate / onImport / onOpenWallet '
    'callbacks when the user taps them',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(
          _FakeWalletCore(fixture: const ['wlt-abc']),
        ),
      ]);
      addTearDown(container.dispose);

      final captured = <String>[];
      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: WalletListScreen(
                network: _kTestnet,
                onCreate: () => captured.add('create'),
                onImport: () => captured.add('import'),
                onOpenWallet: (id) => captured.add('open:$id'),
              ),
            ),
          ),
        ),
      );
      await t.pumpAndSettle();

      await t.tap(find.text('Create'));
      await t.tap(find.text('Import'));
      await t.tap(find.text('wlt-abc'));

      expect(captured, ['create', 'import', 'open:wlt-abc']);
    },
  );

  testWidgets(
    'WalletListScreen ignores taps for wallet ids outside the '
    'allowlist (defence against path-injection)',
    (t) async {
      final container = ProviderContainer(overrides: [
        walletCoreProvider.overrideWithValue(
          _FakeWalletCore(fixture: const [
            // `../settings` would otherwise hijack navigation.
            '../settings',
          ]),
        ),
      ]);
      addTearDown(container.dispose);

      final captured = <String>[];
      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: WalletListScreen(
                network: _kTestnet,
                onOpenWallet: (id) => captured.add('open:$id'),
              ),
            ),
          ),
        ),
      );
      await t.pumpAndSettle();

      // `../settings` contains `/` so the validator rejects it; the tap is
      // a no-op. Note: a CLI-returned id of `'new'` passes the allowlist
      // (alnum only) but COLLIDES with the `new` route segment -- that is a
      // UX-level footgun, deferred to v0.2 router-level `redirect:` (see
      // security-auditor LOW finding for Task 17).
      await t.tap(find.text('../settings'));
      expect(captured, isEmpty);
    },
  );

  test(
    'WalletRoutes builds the expected path templates + '
    'isValidWalletIdSegment enforces the allowlist',
    () {
      expect(WalletRoutes.wallets('testnet'), '/wallets/testnet');
      expect(WalletRoutes.create('testnet'), '/wallets/testnet/new');
      expect(WalletRoutes.import('testnet'), '/wallets/testnet/import');
      expect(
        WalletRoutes.detail('testnet', 'wlt-abc'),
        '/wallets/testnet/wlt-abc',
      );
      expect(
        WalletRoutes.send('testnet', 'wlt-abc'),
        '/wallets/testnet/wlt-abc/send',
      );
      expect(
        WalletRoutes.transactions('testnet', 'wlt-abc'),
        '/wallets/testnet/wlt-abc/transactions',
      );

      // Allowlist enforced: alnum + dash + underscore, length 1..64.
      expect(WalletRoutes.isValidWalletIdSegment('wlt-abc'), isTrue);
      expect(WalletRoutes.isValidWalletIdSegment('a'), isTrue);
      expect(WalletRoutes.isValidWalletIdSegment('a' * 64), isTrue);
      expect(WalletRoutes.isValidWalletIdSegment(''), isFalse);
      expect(WalletRoutes.isValidWalletIdSegment('../settings'), isFalse);
      expect(WalletRoutes.isValidWalletIdSegment('new'), isTrue);
      // (The 'new' collision is a UX bug, not a security boundary. Tracked
      //  in security-auditor LOW finding for v0.2 router-level redirect.)
      expect(WalletRoutes.isValidWalletIdSegment('a/b'), isFalse);
      expect(WalletRoutes.isValidWalletIdSegment('a b'), isFalse);
      expect(WalletRoutes.isValidWalletIdSegment('a' * 65), isFalse);
    },
  );
}
