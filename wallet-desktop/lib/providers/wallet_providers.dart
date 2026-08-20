import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:meta/meta.dart';

import '../core/btc/models/wallet_detail.dart';
import '../core/ffi/ffi_enums.dart';
import 'wallet_core_provider.dart';

/// Loads the persisted-wallet list for the active network.
///
/// Reads via `walletCore.listWallets(network)` (Task 6+8 FFI surface)
/// and returns a `List<String>` of wallet IDs — no JSON parsing
/// (Rust returns the id list directly).
///
/// **Task 10 migration.** Previously routed through `btcInvokerProvider`
/// + `BtcCommand.walletList(network:)` + `WalletInfo.fromJson` parsing
/// + a parse-callback that swallowed non-list responses. The new path
/// has typed errors (`FfiException.kind`) instead of string-dump
/// `BtcError(kind: other)`; UI catch blocks in `WalletListScreen`
/// (Task 10 follow-up) match `on FfiException catch (e)` for kind-
/// mapped copy.
///
/// **Plan deviation:** the legacy `List<WalletInfo>` DTO included
/// `addressType` per wallet (the subtitle in `WalletListScreen`).
/// Rust `wallet_list` returns id only; surfacing `addressType`
/// requires an extra `wallet_peek_addresses` call per wallet (N extra
/// FFI calls per list load) OR a Rust-side `wallet_list` enrichment
/// (returns full `Vec<WalletInfo>`). Defer to v0.2 — for v0.2.0 the
/// list view drops the subtitle's address type.
///
/// `family<String>` keyed by network so switching networks invalidates
/// only that network's cache. `autoDispose` so the cache drops when
/// the list screen unmounts.
class WalletsListNotifier
    extends AutoDisposeFamilyAsyncNotifier<List<String>, String> {
  @override
  Future<List<String>> build(String network) async {
    final core = ref.watch(walletCoreProvider);
    // L12 review MED #1 fix (Task 10): dropped the per-call
    // `Directory.systemTemp.createTempSync` dance. The list path
    // reads wallet-list metadata; the Rust side never writes to the
    // dir on this code path (list returns ids, not blobs).
    // Production `baseDir` will come from `appPathsProvider.baseDir`
    // (Task 10 follow-up / app-shell integration).
    return core.listWallets(
      network: _networkFromString(network),
      // v0.2.0 stand-in: empty baseDir is acceptable for the list
      // path (no blob I/O). Tasks 11-16 wire `appPathsProvider`.
      baseDir: '',
    );
  }

  /// Force a re-fetch (e.g. after wallet create / import). Delegates to
  /// `ref.invalidateSelf` so Riverpod's lifecycle (dep tracking,
  /// listener coalescing) stays intact.
  Future<void> refresh() async {
    ref.invalidateSelf();
    await future;
  }
}

/// Maps the v0.1 string network identifier to the FFI network enum.
///
/// **Task 10 scope + L12 review HIGH #2.** The Rust-side `wallet_list`
/// ABI (Task 5) currently only handles `FfiNetwork.testnet` — every
/// other value returns `FfiError::Unknown` at the parse step
/// (`wallet_ops.rs:70-75`). The UI's 5-option `NetworkPicker` is a
/// v0.2 deferred scope; for v0.2.0 we map every non-testnet
/// identifier to `testnet` (the only ABI-supported value).
///
/// **HIGH #2 guard.** The function asserts the input is `'testnet'` —
/// silently discarding a non-testnet input would route every wallet
/// list to the testnet blob dir without an error, masking the
/// multi-network rollout when Rust `parse_network` grows. The
/// assertion fails loudly so the operator can extend the mapping
/// alongside the Rust enum.
FfiNetwork _networkFromString(String network) {
  assert(
      network == 'testnet',
      'WalletsListNotifier only supports testnet today; '
      'got: $network. v0.2: extend when Rust parse_network grows.');
  return FfiNetwork.testnet;
}

final walletsListProvider = AsyncNotifierProvider.autoDispose
    .family<WalletsListNotifier, List<String>, String>(WalletsListNotifier.new);

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
