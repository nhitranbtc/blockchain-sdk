import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/btc_command.dart';
import 'package:wallet_desktop/core/btc/btc_error.dart';
import 'package:wallet_desktop/core/btc/btc_invoker.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';
import 'package:wallet_desktop/features/wallet_detail/wallet_detail_screen.dart';
import 'package:wallet_desktop/providers/btc_providers.dart';
import 'package:wallet_desktop/providers/wallet_providers.dart';

const _kTestnet = 'testnet';
const _kWalletId = 'wlt-abc';

/// Test double — always throws (the screen-level tests seed the
/// session directly via `walletSessionProvider.notifier.unlock(...)`
/// rather than going through `walletShow`). Mirrors the seam
/// established in Task 13/17 — only the success path is exercised
/// here; error paths are covered by `wallets_list_provider_test.dart`.
class _FakeBtcInvoker extends BtcInvoker {
  _FakeBtcInvoker() : super(binaryPath: '');

  @override
  Future<T> invoke<T>(
    BtcCommand cmd, {
    required T Function(dynamic json) parse,
  }) async {
    throw const BtcError(
      exitCode: 1,
      stderr: 'no fixture — tests seed session directly',
      kind: BtcErrorKind.other,
    );
  }
}

void main() {
  testWidgets(
    'WalletDetailScreen shows the Unlock form (Password + Unlock button) '
    'when the wallet session is null',
    (t) async {
      final container = ProviderContainer(overrides: [
        btcInvokerProvider.overrideWith((_) async => _FakeBtcInvoker()),
      ]);
      addTearDown(container.dispose);

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(
              body: WalletDetailScreen(
                network: _kTestnet,
                walletId: _kWalletId,
              ),
            ),
          ),
        ),
      );
      await t.pump();

      // PasswordField renders one TextField (obscured).
      expect(find.byType(TextField), findsOneWidget);
      expect(find.text('Unlock'), findsOneWidget);
    },
  );

  testWidgets(
    'WalletDetailScreen shows balance + first address + nav buttons '
    'when the wallet session has a parsed detail',
    (t) async {
      final container = ProviderContainer(overrides: [
        btcInvokerProvider.overrideWith((_) async => _FakeBtcInvoker()),
      ]);
      addTearDown(container.dispose);

      // Seed the session so the screen boots into the unlocked view.
      // `OpaqueMnemonic('')` is the v0.1 sentinel for "unlocked but
      // no mnemonic cached" — Task 21 SendScreen will prompt the user
      // to re-enter or fall back to a re-import. Documented per
      // Task 18 L12 type-design post-PR MEDIUM #5 (Task 20 carry-over).
      container
          .read(walletSessionProvider(_kWalletId).notifier)
          .unlockWithDetail(
            const WalletDetail(
              id: _kWalletId,
              network: _kTestnet,
              addressType: 'native-segwit',
              firstAddress: 'tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx',
              balance: Balance(
                confirmedSat: 12345,
              ),
            ),
          );
      // Document the v0.1 sentinel contract in code: a regression that
      // re-populates the mnemonic (e.g., a future "cache on unlock"
      // change) would silently bypass Task 21's `isEmpty` check.
      expect(
        container.read(walletSessionProvider(_kWalletId))!.mnemonic.value,
        isEmpty,
        reason: 'Task 20 v0.1 sentinel: read-only unlock uses '
            'OpaqueMnemonic("") so Task 21 SendScreen can detect '
            '"no mnemonic cached" via `value.isEmpty`.',
      );

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(
              body: WalletDetailScreen(
                network: _kTestnet,
                walletId: _kWalletId,
              ),
            ),
          ),
        ),
      );
      await t.pump();

      // Balance confirmed-sat surfaces in BalanceCard.
      expect(find.text('12345 sats'), findsOneWidget);
      // AddressChip renders the truncated form (first 8 + last 4).
      expect(
        find.textContaining('tb1qw508…jzsx'),
        findsOneWidget,
      );
      // AppBar title surfaces `formatWalletId(d.id)` (L12 flutter-reviewer
      // Task 20 NIT — assert it exercises the formatter).
      expect(find.text('Wallet wlt-abc'), findsOneWidget);
      // Network + type text (L12 pr-test-analyzer Task 20 LOW — these
      // previously went unverified; cheap to assert).
      expect(find.text('Network: testnet'), findsOneWidget);
      expect(find.text('Type: native-segwit'), findsOneWidget);
      // Send + transactions + lock nav buttons render in the AppBar
      // actions.
      expect(find.byKey(const Key('wallet_detail_send')), findsOneWidget);
      expect(find.byKey(const Key('wallet_detail_history')), findsOneWidget);
      expect(find.byKey(const Key('wallet_detail_lock')), findsOneWidget);
    },
  );

  testWidgets(
    'WalletDetailScreen lock button clears the wallet session '
    '(returns to the Unlock form)',
    (t) async {
      final container = ProviderContainer(overrides: [
        btcInvokerProvider.overrideWith((_) async => _FakeBtcInvoker()),
      ]);
      addTearDown(container.dispose);

      // Seed the session as unlocked.
      container
          .read(walletSessionProvider(_kWalletId).notifier)
          .unlockWithDetail(
            const WalletDetail(
              id: _kWalletId,
              network: _kTestnet,
              addressType: 'native-segwit',
              firstAddress: 'tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx',
              balance: Balance(
                confirmedSat: 12345,
              ),
            ),
          );
      // Document the v0.1 sentinel contract in code: a regression that
      // re-populates the mnemonic (e.g., a future "cache on unlock"
      // change) would silently bypass Task 21's `isEmpty` check.
      expect(
        container.read(walletSessionProvider(_kWalletId))!.mnemonic.value,
        isEmpty,
        reason: 'Task 20 v0.1 sentinel: read-only unlock uses '
            'OpaqueMnemonic("") so Task 21 SendScreen can detect '
            '"no mnemonic cached" via `value.isEmpty`.',
      );

      await t.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(
              body: WalletDetailScreen(
                network: _kTestnet,
                walletId: _kWalletId,
              ),
            ),
          ),
        ),
      );
      await t.pump();

      // AppBar Lock button (key-based finder per L12 pr-test-analyzer
      // Task 20 MEDIUM — `find.text('Lock')` would collide with any
      // future 'Lock' Text widget).
      final lockBtn = find.byKey(const Key('wallet_detail_lock'));
      expect(lockBtn, findsOneWidget);
      await t.tap(lockBtn);
      await t.pump();

      // Provider state cleared (L12 pr-test-analyzer Task 20 MEDIUM —
      // assert the provider state, not just the UI re-render, so a
      // future `_lock()` that drops the `notifier.lock()` call would
      // fail loudly).
      expect(
        container.read(walletSessionProvider(_kWalletId)),
        isNull,
        reason: 'lock() must clear the WalletSession family state',
      );
      // UI re-rendered to Unlock form.
      expect(find.byType(TextField), findsOneWidget);
      expect(find.text('Unlock'), findsOneWidget);
      // Balance card gone.
      expect(find.text('12345 sats'), findsNothing);
    },
  );

  // v0.2 deferred (Task 18/19 lesson): end-to-end "type password →
  // submit → wallet show returns detail → balance renders" widget
  // test. The `enterText` pipeline has known issues with the
  // obscured PasswordField controller (Task 17/18 lesson); the full
  // path is covered by Task 24's `fake_btc.sh` integration test
  // (operator-driven per L29). The `skip:` flag prevents this from
  // being miscounted as coverage in audits (L12 pr-test-analyzer
  // Task 20 LOW).
  test('unlock submit coverage deferred to Task 24 fake_btc.sh', () {
    // empty body — deferred per Task 17/18 lesson (flutter_test
    // enterText on obscured PasswordField is unreliable).
  }, skip: 'Task 24 integration test');
}
