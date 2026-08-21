// Task 10 (#216) — public interface for `WalletCore` to enable test
// doubles + prevent raw FFI pointer types from leaking to consumers.
//
// **Why this exists.** Task 8 made `WalletCore` a `final class` with a
// private constructor so subclasses can't reimplement the FFI handle
// lifecycle. The side effect: Riverpod's `Provider.overrideWithValue`
// requires the override to be assignable to the static provider type —
// a test fake cannot extend `final class WalletCore`.
//
// **L12 review HIGH #1 (Task 10).** The interface takes `String`
// `baseDir` / `walletId` parameters (not `Pointer<Utf8>`). The
// concrete `WalletCore` impl owns the `toNativeUtf8` + `calloc.free`
// + try/finally dance internally (it already owns the lifetime of
// `password` / `phrase` via `SecretBuffer`, so the precedent
// exists). Provider callers and test fakes no longer need to know
// about `calloc` or the `Pointer<Utf8>` ABI type.
//
// **Pattern.** Public `WalletCoreApi` interface exposes the
// consumer-facing methods; `WalletCore` (Task 8) implements it.
// `walletCoreProvider` returns `WalletCoreApi` so tests can swap a
// fake that `implements WalletCoreApi`.

import 'dart:ffi';

import 'btc/models/fee_estimate.dart';
import 'btc/models/wallet_detail.dart';
import 'ffi/ffi_enums.dart';
import 'ffi/mnemonic_view.dart';
import 'ffi/secret_buffer.dart';

/// Public contract every consumer of the FFI wallet surface depends on.
/// Implemented by the concrete `WalletCore` (Task 8) and by test fakes
/// (Task 10 `WalletsListNotifier` test suite).
abstract interface class WalletCoreApi {
  /// Returns the rust crate version as a UTF-8 String.
  String ffiVersion();

  /// Lists wallet IDs for the given network + baseDir.
  List<String> listWallets({
    required FfiNetwork network,
    required String baseDir,
  });

  /// Deletes the wallet blob for the given network + walletId.
  void deleteWallet({
    required FfiNetwork network,
    required String walletId,
    required String baseDir,
  });

  /// Creates a new wallet with a fresh random mnemonic.
  WalletCreatedData createWallet({
    required int words,
    required FfiNetwork network,
    required FfiAddressType addressType,
    required SecretBuffer password,
    required String baseDir,
  });

  /// Imports an existing BIP-39 mnemonic phrase as a new wallet.
  WalletImportedData importWallet({
    required FfiNetwork network,
    required SecretBuffer phrase,
    required SecretBuffer password,
    required String baseDir,
  });

  /// Read a wallet's metadata + first external address from the
  /// persisted blob (Task 13 / Issue #219). The `password`
  /// `SecretBuffer` is auto-disposed after the FFI call. Returns a
  /// `WalletDetail` (collapsed `Balance` — single `confirmedSat`
  /// field, no `utxos` list; see `lib/core/btc/models/wallet_detail.dart`
  /// for the plan-deviation rationale).
  ///
  /// **v0.2.0 read-only show**: `firstAddress` is always `''` (Rust
  /// `peek_addresses` requires bdk sync — deferred to v0.2.1);
  /// `balance.confirmedSat` is always `0` (no Esplora sync).
  /// The detail screen handles empty `firstAddress` by hiding
  /// `AddressChip`.
  ///
  /// **L12 collapse (HIGH #1 mirror):** wrong-password /
  /// not-found / wrong-AAD / corrupt-blob all surface as
  /// `FfiException(kind: FfiErrorKind.walletStore)`. The detail
  /// screen renders this as a single "could not unlock" copy — no
  /// enumeration signal for a network observer.
  WalletDetail showWallet({
    required FfiNetwork network,
    required String walletId,
    required SecretBuffer password,
    required String baseDir,
  });

  /// Fetch Esplora fee estimates via the FFI surface. (Task 16 /
  /// Issue #222.) The caller owns the `esploraHandle` (must come
  /// from `esploraClientNew`) and is responsible for freeing it via
  /// `esploraClientFree` after the call.
  ///
  /// **Inefficient call pattern (current):** the typical caller
  /// creates + frees an Esplora handle per call (no caching). The
  /// "right" architecture caches the handle in `WalletSessionNotifier`
  /// (Task 14 Sub-split B). For Task 16 the per-call pattern is
  /// acceptable; flag in the PR body as a Sub-split B follow-up.
  ///
  /// Throws `FfiException` on failure (Esplora HTTP failure →
  /// `FfiErrorKind.esplora` or `network`; null handle →
  /// `FfiErrorKind.notInitialized`).
  FeeEstimate feeEstimate({required Pointer<Void> esploraHandle});
}

/// Result of a successful `createWallet` call. The `mnemonic` field is a
/// typed `MnemonicView` so the plaintext phrase String lives ONLY
/// inside the view and is nulled on `dispose()` (L12 CRITICAL #2).
class WalletCreatedData {
  WalletCreatedData({
    required this.id,
    required this.mnemonic,
    required this.network,
    required this.addressType,
  });
  final String id;
  final MnemonicView mnemonic;
  final FfiNetwork network;
  final FfiAddressType addressType;

  // MnemonicView excluded from equality: identity-comparable via the
  // underlying handle, but its `read()` String is sensitive and
  // shouldn't be part of any value comparison (defense-in-depth).
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is WalletCreatedData &&
          other.id == id &&
          other.network == network &&
          other.addressType == addressType &&
          identical(other.mnemonic, mnemonic);

  @override
  int get hashCode =>
      Object.hash(id, network, addressType, identityHashCode(mnemonic));

  @override
  String toString() => 'WalletCreatedData(id: $id, '
      'network: $network, addressType: $addressType, '
      'mnemonic: <view>)';
}

/// Result of a successful `importWallet` call.
class WalletImportedData {
  WalletImportedData({
    required this.id,
    required this.network,
    required this.addressType,
  });
  final String id;
  final FfiNetwork network;
  final FfiAddressType addressType;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is WalletImportedData &&
          other.id == id &&
          other.network == network &&
          other.addressType == addressType;

  @override
  int get hashCode => Object.hash(id, network, addressType);

  @override
  String toString() => 'WalletImportedData(id: $id, '
      'network: $network, addressType: $addressType)';
}
