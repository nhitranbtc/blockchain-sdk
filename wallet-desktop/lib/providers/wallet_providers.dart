import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:meta/meta.dart';

import '../core/btc/btc_command.dart';
import '../core/btc/models/wallet_detail.dart';
import '../core/btc/models/wallet_info.dart';
import 'btc_providers.dart';

/// Loads the persisted-wallet list for the active network.
///
/// Reads via `btc wallet list --network <NET> --json` and parses the
/// JSON array into [WalletInfo] DTOs. `family<String>` keyed by network
/// so switching networks invalidates only that network's cache.
/// `autoDispose` so the cache drops when the list screen unmounts.
class WalletsListNotifier
    extends AutoDisposeFamilyAsyncNotifier<List<WalletInfo>, String> {
  @override
  Future<List<WalletInfo>> build(String network) async {
    final invoker = await ref.watch(btcInvokerProvider.future);
    return invoker.invoke<List<WalletInfo>>(
      BtcCommand.walletList(network: network),
      // BtcInvoker passes `null` on empty stdout and a string fallback
      // on non-JSON responses (defensive). Treat either as an empty
      // list rather than a parse failure — a fresh install with no
      // wallets should surface `data: []`, not `BtcError(kind: other)`.
      parse: (j) => (j is List)
          ? j
              .map((e) => WalletInfo.fromJson(e as Map<String, dynamic>))
              .toList(growable: false)
          : const <WalletInfo>[],
    );
  }

  /// Force a re-fetch (e.g. after wallet create / import). Used by
  /// Task 17 WalletListScreen's pull-to-refresh + Task 18 / 19
  /// post-action invalidation. Delegates to `ref.invalidateSelf` so
  /// Riverpod's lifecycle (dep tracking, listener coalescing) stays
  /// intact — manually re-invoking `build(arg)` would race with
  /// concurrent invalidations and double-subscribe `btcInvokerProvider`.
  Future<void> refresh() async {
    ref.invalidateSelf();
    await future;
  }
}

final walletsListProvider = AsyncNotifierProvider.autoDispose
    .family<WalletsListNotifier, List<WalletInfo>, String>(
        WalletsListNotifier.new);

// ─── Task 14: walletSessionProvider ─────────────────────────────────────

/// Mutable unlocked session state. Mnemonic lives in [OpaqueMnemonic].
///
/// **`OpaqueMnemonic` is NOT real zeroization** — Dart strings are
/// immutable + interned. `_value = ''` only clears this field; the
/// original string still sits in the runtime heap + string-pool
/// until GC. Real zeroization requires FFI to a native allocator +
/// explicit `Finalizable` (Dart 3) — tracked as v0.2 backlog. For
/// v0.1 we rely on the defense-in-depth chain: TempSecretFile (Task 5)
/// never holds the cleartext mnemonic, only the password; BtcInvoker
/// (Task 10) strips env + uses `includeParentEnvironment: false`;
/// BtcLogFilter (Task 7) redacts mnemonic/password from logs. The
/// mnemonic copy that lives in `OpaqueMnemonic` is the last cleartext
/// surface — and it disappears from `state` on `lock()`.
@immutable
class WalletSession {
  const WalletSession({
    required this.walletId,
    required this.mnemonic,
    this.detail,
  });
  final String walletId;
  final OpaqueMnemonic mnemonic;
  final WalletDetail? detail;

  /// `copyWith` cannot clear `detail` back to null (the `?? this.detail`
  /// pattern conflates absent with null). v0.2 backlog: switch to a
  /// sentinel object or add an explicit `clearDetail: bool` flag.
  WalletSession copyWith({WalletDetail? detail}) => WalletSession(
        walletId: walletId,
        mnemonic: mnemonic,
        detail: detail ?? this.detail,
      );
}

/// Best-effort wrapper around a mnemonic String. NOT real zeroization
/// (see [WalletSession] doc). Used as a typed handle for the `lock()`
/// call site so the contract is explicit: "this handle must be
/// disposed when the session ends". The actual heap zeroization gap
/// is documented at the type level so callers don't over-trust.
class OpaqueMnemonic {
  OpaqueMnemonic(this._value);
  String _value;

  /// Returns the cleartext mnemonic. **The returned String is a
  /// reference, not a copy — holding it locally extends the cleartext
  /// lifetime past `lock()`.** Callers (Task 21 SendScreen) must use
  /// the mnemonic synchronously inside `withTempSecretFile` and not
  /// retain it past the closure return.
  String get value => _value;

  /// Clears the local handle. **Does NOT zeroize the underlying heap
  /// (Dart strings are immutable + interned).** See [WalletSession]
  /// doc for the full zeroization gap.
  void dispose() {
    _value = '';
  }
}

class WalletSessionNotifier extends FamilyNotifier<WalletSession?, String> {
  @override
  WalletSession? build(String walletId) {
    // Cleanup hook fires when the ProviderContainer tears down (app
    // exit). Ensures the mnemonic handle gets `dispose()`'d on exit
    // even if `lock()` was never explicitly called.
    ref.onDispose(() {
      state?.mnemonic.dispose();
    });
    return null;
  }

  /// Unlock the session for the family key. The `walletId` is derived
  /// from the family arg, NOT passed by the caller — eliminates the
  /// cross-key footgun where `walletSessionProvider('abc').notifier
  /// .unlock(walletId: 'xyz', ...)` would silently store a mismatched
  /// identity.
  void unlock({required String mnemonic, WalletDetail? detail}) {
    final prev = state;
    prev?.mnemonic.dispose(); // never leak a prior mnemonic
    state = WalletSession(
      walletId: arg, // family key, not user input
      mnemonic: OpaqueMnemonic(mnemonic),
      detail: detail,
    );
  }

  /// Read-only unlock: detail is loaded but no mnemonic is cached.
  /// Used by `WalletDetailScreen` (Task 20) after `btc wallet show`
  /// returns the parsed detail — the CLI does NOT return the mnemonic
  /// (it re-decrypts per call), so the session's `mnemonic` is set to
  /// the empty-string sentinel. Task 21 SendScreen detects
  /// `state.mnemonic.value.isEmpty` and prompts the user to paste the
  /// mnemonic before signing.
  ///
  /// **Sentinel encoding** (L12 type-design post-PR Task 20 HIGH):
  /// the empty-string value is overloaded between "sentinel" and
  /// "post-`dispose()`" — both currently signal `value.isEmpty`. v0.2
  /// follow-up: replace with a typed `OpaqueMnemonic.sentinel()` or a
  /// dedicated `bool isUnlockedWithoutMnemonic` flag on
  /// [WalletSession] so the two states can't collide at the value
  /// level. For v0.1 the convention lives in one place (this method)
  /// instead of leaking to every caller of `unlock(mnemonic: '', …)`.
  void unlockWithDetail(WalletDetail detail) {
    final prev = state;
    prev?.mnemonic.dispose(); // never leak a prior mnemonic
    state = WalletSession(
      walletId: arg, // family key, not user input
      mnemonic: OpaqueMnemonic(''), // sentinel — see doc above
      detail: detail,
    );
  }

  void updateDetail(WalletDetail detail) {
    final current = state;
    if (current == null) return;
    state = current.copyWith(detail: detail);
  }

  void lock() {
    final current = state;
    current?.mnemonic.dispose();
    state = null;
  }
}

/// Non-autoDispose: the unlocked session must persist while the app
/// is running (Task 20 WalletDetailScreen + Task 21 SendScreen both
/// read it). `lock()` clears state and zeroes the mnemonic. App-lifecycle
/// auto-lock on backgrounding (paused/hidden/detached) is a v0.2
/// follow-up — see WalletSession doc.
final walletSessionProvider =
    NotifierProvider.family<WalletSessionNotifier, WalletSession?, String>(
        WalletSessionNotifier.new);
