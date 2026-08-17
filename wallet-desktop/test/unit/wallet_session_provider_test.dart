import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/providers/wallet_providers.dart';

void main() {
  test('walletSessionProvider starts null (locked)', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    expect(container.read(walletSessionProvider('abc')), isNull);
  });

  test('lock() on unlocked state disposes mnemonic and clears state', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final notifier = container.read(walletSessionProvider('abc').notifier);
    notifier.unlock(
      mnemonic: 'word1 word2 word3 word4 word5 word6 word7 word8 '
          'word9 word10 word11 word12',
    );
    final session = container.read(walletSessionProvider('abc'));
    expect(session, isNotNull);

    notifier.lock();
    expect(container.read(walletSessionProvider('abc')), isNull);
  });

  test('lock() on already-null state is a no-op', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final notifier = container.read(walletSessionProvider('abc').notifier);
    notifier.lock();
    expect(container.read(walletSessionProvider('abc')), isNull);
  });

  test('unlock() derives walletId from family key (not caller)', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final notifier = container.read(walletSessionProvider('abc').notifier);
    // Caller CANNOT pass walletId — derived from `arg` (family key).
    notifier.unlock(
      mnemonic: 'word1 word2 word3 word4 word5 word6 word7 word8 '
          'word9 word10 word11 word12',
    );
    expect(
      container.read(walletSessionProvider('abc'))?.walletId,
      'abc',
      reason: 'walletId must equal the family key',
    );
    notifier.lock();
  });

  test('re-unlock disposes prior mnemonic (no heap leak)', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final notifier = container.read(walletSessionProvider('abc').notifier);

    notifier.unlock(mnemonic: 'aaa-bip39-words-original-mnemonic');
    final first = container.read(walletSessionProvider('abc'))!;
    expect(first.mnemonic.value, 'aaa-bip39-words-original-mnemonic');

    // Re-unlock — the prior ZeroizingString must be dispose()'d.
    notifier.unlock(mnemonic: 'bbb-bip39-words-replacement-mnemonic');
    final second = container.read(walletSessionProvider('abc'))!;
    expect(second.mnemonic.value, 'bbb-bip39-words-replacement-mnemonic');

    // The first handle's local field is zeroed.
    expect(first.mnemonic.value, '', reason: 'prior handle must be disposed');

    notifier.lock();
  });

  test('per-walletId family isolates sessions', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final abcNotifier = container.read(walletSessionProvider('abc').notifier);
    final defNotifier = container.read(walletSessionProvider('def').notifier);

    abcNotifier.unlock(mnemonic: 'aaa-bip39-words');
    expect(container.read(walletSessionProvider('abc'))?.walletId, 'abc');
    expect(container.read(walletSessionProvider('def')), isNull);

    defNotifier.unlock(mnemonic: 'bbb-bip39-words');
    expect(container.read(walletSessionProvider('def'))?.walletId, 'def');
    // 'abc' session untouched by 'def' unlock
    expect(container.read(walletSessionProvider('abc'))?.walletId, 'abc');

    abcNotifier.lock();
    defNotifier.lock();
  });
}
