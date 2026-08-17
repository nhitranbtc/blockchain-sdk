import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/btc_command.dart';
import 'package:wallet_desktop/core/btc/btc_error.dart';
import 'package:wallet_desktop/core/btc/btc_invoker.dart';
import 'package:wallet_desktop/features/wallet_list/wallet_list_screen.dart';
import 'package:wallet_desktop/providers/btc_providers.dart';
import 'package:wallet_desktop/providers/wallet_providers.dart';
import 'package:wallet_desktop/routing/wallet_routes.dart';

/// Test double — returns canned JSON (or throws a [BtcError]) without
/// spawning a subprocess. Mirrors the seam established in Task 13's
/// `wallets_list_provider_test.dart`: override `btcInvokerProvider`
/// (the only async dep `WalletsListNotifier` reaches for).
class _FakeBtcInvoker extends BtcInvoker {
  _FakeBtcInvoker({this.fixture = const [], this.throwError}) : super(binaryPath: '');

  /// Either a list of wallet JSON maps (snake_case: `address_type`) or
  /// an empty `const []`. The real [WalletsListNotifier] parses this
  /// through `WalletInfo.fromJson`.
  final Object fixture;

  /// When non-null, `invoke` throws this error instead of returning
  /// parsed data — used by the error-branch test.
  final BtcError? throwError;

  @override
  Future<T> invoke<T>(
    BtcCommand cmd, {
    required T Function(dynamic json) parse,
  }) async {
    final err = throwError;
    if (err != null) throw err;
    return parse(fixture);
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
        btcInvokerProvider.overrideWith((_) async => _FakeBtcInvoker()),
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
        btcInvokerProvider.overrideWith(
          (_) async => _FakeBtcInvoker(fixture: const [
            {
              'id': 'wlt-abc',
              'network': 'testnet',
              'address_type': 'native-segwit',
            },
            {
              'id': 'wlt-def',
              'network': 'testnet',
              'address_type': 'taproot',
            },
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

      // ids ≤12 chars render in full (security-auditor Task 17 review).
      expect(find.text('wlt-abc'), findsOneWidget);
      expect(find.text('wlt-def'), findsOneWidget);
    },
  );

  testWidgets(
    'WalletListScreen truncates long wallet ids to first4…last4',
    (t) async {
      final container = ProviderContainer(overrides: [
        btcInvokerProvider.overrideWith(
          (_) async => _FakeBtcInvoker(fixture: const [
            {
              // 32-char hex (matches btc's fingerprint shape).
              'id': 'abcdef0123456789abcdef0123456789',
              'network': 'testnet',
              'address_type': 'native-segwit',
            },
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
    'when the invoker throws',
    (t) async {
      final container = ProviderContainer(overrides: [
        btcInvokerProvider.overrideWith(
          (_) async => _FakeBtcInvoker(
            throwError: const BtcError(
              exitCode: 1,
              stderr: 'panic: wallet db corrupted',
              kind: BtcErrorKind.other,
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

      // Kind-mapped message + retry button. The raw `panic: …` string
      // MUST NOT surface (security-auditor Task 17 review).
      expect(find.text('Something went wrong.'), findsOneWidget);
      expect(find.text('Retry'), findsOneWidget);
      expect(find.textContaining('panic'), findsNothing);
      expect(find.textContaining('db corrupted'), findsNothing);
    },
  );

  testWidgets(
    'WalletListScreen invokes onCreate / onImport / onOpenWallet '
    'callbacks when the user taps them',
    (t) async {
      final container = ProviderContainer(overrides: [
        btcInvokerProvider.overrideWith(
          (_) async => _FakeBtcInvoker(fixture: const [
            {
              'id': 'wlt-abc',
              'network': 'testnet',
              'address_type': 'native-segwit',
            },
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
        btcInvokerProvider.overrideWith(
          (_) async => _FakeBtcInvoker(fixture: const [
            // `../settings` would otherwise hijack navigation.
            {'id': '../settings', 'network': 'testnet', 'address_type': 'taproot'},
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
      // (alnum only) but COLLIDES with the `new` route segment — that is a
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
