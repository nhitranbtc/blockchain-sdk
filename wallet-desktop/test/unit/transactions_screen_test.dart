import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/btc_command.dart';
import 'package:wallet_desktop/core/btc/btc_error.dart';
import 'package:wallet_desktop/core/btc/btc_invoker.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';
import 'package:wallet_desktop/features/wallet_transactions/transactions_screen.dart';
import 'package:wallet_desktop/providers/btc_providers.dart';
import 'package:wallet_desktop/providers/esplora_config_provider.dart';
import 'package:wallet_desktop/providers/wallet_providers.dart';

const _kTestnet = 'testnet';
const _kWalletId = 'wlt-abc';
const _kMnemonic = 'legal winner thank year wave sausage worth useful legal '
    'winner thank yellow';

/// Test double — returns canned tx list (or empty) without spawning a
/// subprocess. Mirrors the seam established in Tasks 13/17/20/21.
class _FakeBtcInvoker extends BtcInvoker {
  _FakeBtcInvoker({this.txs, this.errorToThrow}) : super(binaryPath: '');

  /// Either a `List` of tx JSON maps (snake_case) or `null` (empty).
  final List<Map<String, dynamic>>? txs;

  /// When non-null, `invoke` throws this error instead of returning
  /// parsed data — used by the BtcError-surface test.
  final BtcError? errorToThrow;

  @override
  Future<T> invoke<T>(
    BtcCommand cmd, {
    required T Function(dynamic json) parse,
  }) async {
    final err = errorToThrow;
    if (err != null) throw err;
    final fx = txs;
    if (fx == null) return parse(<Map<String, dynamic>>[]);
    return parse(fx);
  }
}

/// Stub AsyncNotifier that returns the default EsploraConfig without
/// hitting disk. Tests override `esploraConfigProvider` with this so
/// the screen's `ref.read(esploraConfigProvider.future)` resolves
/// deterministically (real impl reads disk, which is brittle in
/// unit-test environments).
class _StubEsploraConfigNotifier extends EsploraConfigNotifier {
  @override
  Future<EsploraConfig> build() async => EsploraConfig.defaults('testnet');
}

/// Helper — seed an unlocked session (with mnemonic) for tests that
/// want to exercise the tx-list view.
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
        btcInvokerProvider.overrideWith((_) async => _FakeBtcInvoker()),
        esploraConfigProvider.overrideWith(_StubEsploraConfigNotifier.new),
      ]);
      addTearDown(container.dispose);
      container.read(walletSessionProvider(_kWalletId).notifier).unlock(
            mnemonic: _kMnemonic,
            detail: _seedDetail(),
          );

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
      // `pump` (not `pumpAndSettle`) — the `CircularProgressIndicator`
      // inside `ProcessProgressOverlay` keeps animating while
      // `_running` is true, so `pumpAndSettle` times out.
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
        btcInvokerProvider.overrideWith((_) async => _FakeBtcInvoker()),
        esploraConfigProvider.overrideWith(_StubEsploraConfigNotifier.new),
      ]);
      addTearDown(container.dispose);
      container
          .read(walletSessionProvider(_kWalletId).notifier)
          .unlockWithDetail(_seedDetail());

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
      // mnemonic before we call `btc tx-list --mnemonic ''`.
      expect(find.textContaining('txid'), findsNothing);
      // Re-prompt surface renders.
      expect(find.textContaining('mnemonic'), findsAtLeastNWidgets(1));
    },
  );

  testWidgets(
    'TransactionsScreen shows the LockedView when the wallet session '
    'is null (deep-link entry without prior unlock)',
    (t) async {
      // No `unlock()` call — `walletSessionProvider(_kWalletId)` is
      // the default `null`. The screen must render LockedView
      // (back-to-unlock prompt) so deep-link entry to
      // /transactions doesn't crash on `session.mnemonic.value`.
      final container = ProviderContainer(overrides: [
        btcInvokerProvider.overrideWith((_) async => _FakeBtcInvoker()),
        esploraConfigProvider.overrideWith(_StubEsploraConfigNotifier.new),
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
      // Tx list MUST NOT render.
      expect(find.textContaining('txid'), findsNothing);
    },
  );

  testWidgets(
    'TransactionsScreen renders the BtcError surface (StatusBadge + '
    'user message) when the invoker throws',
    (t) async {
      final container = ProviderContainer(overrides: [
        btcInvokerProvider.overrideWith(
          (_) async => _FakeBtcInvoker(
            errorToThrow: const BtcError(
              exitCode: 1,
              stderr: 'esplora unreachable',
              kind: BtcErrorKind.networkError,
            ),
          ),
        ),
        esploraConfigProvider.overrideWith(_StubEsploraConfigNotifier.new),
      ]);
      addTearDown(container.dispose);
      container.read(walletSessionProvider(_kWalletId).notifier).unlock(
            mnemonic: _kMnemonic,
            detail: _seedDetail(),
          );

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
      // Pump enough frames for the async load + error to surface.
      for (var i = 0; i < 10; i++) {
        await t.pump(const Duration(milliseconds: 50));
      }

      // Kind-mapped user message surfaces; raw `stderr` MUST NOT.
      expect(
          find.text('Network error. Check your connection.'), findsOneWidget);
      expect(find.textContaining('esplora'), findsNothing);
      expect(find.textContaining('unreachable'), findsNothing);
    },
  );

  testWidgets(
    'TransactionsScreen renders one row per tx returned by the CLI',
    (t) async {
      final container = ProviderContainer(overrides: [
        btcInvokerProvider.overrideWith(
          (_) async => _FakeBtcInvoker(txs: const [
            {
              'txid': 'a1b2c3d4e5f6',
              'direction': 'incoming',
              'amount_sat': 50000,
              'fee_sat': 0,
              'confirmations': 6,
              'timestamp': 1700000000,
            },
            {
              'txid': 'f6e5d4c3b2a1',
              'direction': 'outgoing',
              'amount_sat': 12345,
              'fee_sat': 420,
              'confirmations': 1,
              'timestamp': 1700001000,
            },
          ]),
        ),
        esploraConfigProvider.overrideWith(_StubEsploraConfigNotifier.new),
      ]);
      addTearDown(container.dispose);
      container.read(walletSessionProvider(_kWalletId).notifier).unlock(
            mnemonic: _kMnemonic,
            detail: _seedDetail(),
          );

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
      // `pump` (not `pumpAndSettle`) — the `CircularProgressIndicator`
      // inside `ProcessProgressOverlay` keeps animating while
      // `_running` is true, so `pumpAndSettle` times out.
      await t.pump();
      // Pump multiple frames so the postFrameCallback fires + the
      // async `_load` resolves (awaits esploraConfigProvider disk
      // read) + the tx rows paint.
      for (var i = 0; i < 10; i++) {
        await t.pump(const Duration(milliseconds: 50));
      }

      // Both txids surface (Lesson 32.1 — never display raw mnemonic,
      // but txids are public on the blockchain so safe to render).
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
