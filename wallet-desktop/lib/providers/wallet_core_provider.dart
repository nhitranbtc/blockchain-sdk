// Task 8 (#214) — Riverpod provider for the `WalletCore` facade.
//
// Singleton accessor via `WalletCore.instance` is the raw entry point;
// this provider exists so Riverpod consumers can `ref.watch(
// walletCoreProvider)` and gain dep tracking + lifecycle integration.
//
// **NOT autoDispose:** the facade holds the tokio runtime handle +
// native lib reference. Dropping the facade on screen unmount would
// invalidate those handles. The lifetime matches the app process.
//
// **Replaces `btcInvokerProvider` (Task 10).** Tasks 10-16 will
// migrate the consumers; both providers may coexist briefly.

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/wallet_core.dart';

/// Process-lifetime handle to the typed `WalletCore` facade.
final walletCoreProvider = Provider<WalletCore>((ref) {
  return WalletCore.instance;
});