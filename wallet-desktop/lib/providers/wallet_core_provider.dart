// Task 8 (#214) + Task 10 (#216) — Riverpod provider for the FFI
// wallet surface.
//
// Singleton accessor via `WalletCore.instance` is the raw entry point;
// this provider exists so Riverpod consumers can `ref.watch(
// walletCoreProvider)` and gain dep tracking + lifecycle integration.
//
// **NOT autoDispose:** the facade holds the tokio runtime handle +
// native lib reference. Dropping the facade on screen unmount would
// invalidate those handles. The lifetime matches the app process.
//
// **Task 10 mockability seam.** Provider type changed from
// `Provider<WalletCore>` to `Provider<WalletCoreApi>`. Test fakes
// `implements WalletCoreApi` and the provider's `overrideWithValue`
// accepts them — Riverpod requires the override to be assignable to
// the static provider type, and `final class WalletCore` blocks
// subclassing.
//
// **Replaces `btcInvokerProvider` (Task 10).** Tasks 10-16 migrate
// the consumers; both providers may coexist briefly during the
// migration window.

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/wallet_core.dart';
import '../core/wallet_core_api.dart';

/// Process-lifetime handle to the FFI wallet surface, exposed as the
/// public [WalletCoreApi] interface so tests can override the value.
final walletCoreProvider = Provider<WalletCoreApi>((ref) {
  return WalletCore.instance;
});
