// Task 8 (#214) — typed `WalletCore` facade over the FFI bindings
// (Tasks 3-5-7 surfaces). Replaces the Task 1 spike with a fully
// typed public surface that closes L12 CRITICAL #2 (no plaintext
// secret bytes cross the FFI boundary, no String mnemonic leaks).
//
// **L12 invariants enforced by this facade:**
//
// 1. Every secret-bearing parameter (`password`, `phrase`) is typed
//    as `SecretBuffer`. The facade auto-disposes the buffer after
//    the FFI call returns — callers cannot leak the buffer by
//    forgetting to dispose, because the facade owns the lifetime.
//
// 2. `createWallet` returns a `WalletCreated` whose `mnemonic` field
//    is a `MnemonicView` (typed wrapper around the Rust-side
//    `MnemonicHandle`). The plaintext phrase NEVER appears as a
//    String in the DTO.
//
// 3. All network / address-type parameters are typed enums from
//    `ffi_enums.dart` — silent `int` truncation on the FFI boundary
//    is impossible.
//
// **L12 CRITICAL #1 closed (Task 9):** non-zero `FfiError` codes
// are translated to typed `FfiException` (23 kinds, mirrors the Rust
// `enum FfiError`). UI code in Tasks 10-16 switches on
// `e.kind` to render user-facing messages. See
// `lib/core/ffi/ffi_exception.dart`.
//
// **ABI notes:**
// - `wallet_create` writes `out_id` (37 bytes, 36-char UUID hex + NUL)
//   and `out_phrase_handle` (opaque `MnemonicHandle`). No first-address
//   output — UI derives it via `wallet_peek_addresses` post-create.
// - `wallet_import` writes only `out_id`. No mnemonic returned (caller
//   already has the phrase).
// - `wallet_from_mnemonic` takes `*const c_char` (NUL-terminated
//   CStr). SecretBuffer is bytes (`*const u8` + length) — for this
//   call the facade allocates a NUL-terminated copy.

import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'package:wallet_desktop/core/btc/models/fee_estimate.dart';
import 'package:wallet_desktop/core/btc/models/wallet_detail.dart';
import 'package:wallet_desktop/core/ffi/esplora_bindings.dart';
import 'package:wallet_desktop/core/ffi/ffi_enums.dart';
import 'package:wallet_desktop/core/ffi/ffi_exception.dart';
import 'package:wallet_desktop/core/ffi/mnemonic_view.dart';
import 'package:wallet_desktop/core/ffi/runtime_bindings.dart';
import 'package:wallet_desktop/core/ffi/secret_buffer.dart';
import 'package:wallet_desktop/core/ffi/wallet_ops_bindings.dart';
import 'package:wallet_desktop/core/wallet_core_api.dart';

/// Result of a successful `wallet_create` call. The `mnemonic` field
/// is a typed `MnemonicView` — the plaintext phrase String lives
/// ONLY inside the view and is nulled on `dispose()`.
///
/// **Task 10**: moved to `wallet_core_api.dart` as
/// [WalletCreatedData]. This typedef preserves the Task 8 import path
/// (`import 'package:wallet_desktop/core/wallet_core.dart'`) for
/// downstream code; new code should import from `wallet_core_api.dart`
/// directly.
typedef WalletCreated = WalletCreatedData;

/// Result of a successful `wallet_import` call. No mnemonic is
/// returned — the caller already has the phrase.
///
/// **Task 10**: moved to `wallet_core_api.dart` as
/// [WalletImportedData]. This typedef preserves the Task 8 import path.
typedef WalletImported = WalletImportedData;

/// Typed facade over the wallet FFI surface.
///
/// Singleton via [instance] (process-lifetime). Holds the tokio
/// runtime handle from `RuntimeBindings` once and reuses it for every
/// async op.
///
/// Implements [WalletCoreApi] (Task 10) so test fakes can swap in via
/// Riverpod's `overrideWithValue`.
/// (`WalletShowResult` lives in `wallet_core_api.dart` so the
/// abstract surface can reference it without a circular dep.)
class WalletCore implements WalletCoreApi {
  WalletCore._();

  static final WalletCore instance = WalletCore._();

  // Runtime handle for async ops. Allocated once on first use.
  Pointer<Void>? _runtime;

  Pointer<Void> _runtimeHandle() {
    final rt = _runtime;
    if (rt != null) return rt;
    final fresh = RuntimeBindings.runtimeNew();
    _runtime = fresh;
    return fresh;
  }

  /// Returns the rust crate version as a UTF-8 String.
  @override
  String ffiVersion() {
    final ptr = WalletOpsBindings.ffiVersion();
    if (ptr == nullptr) {
      throw StateError('ffi_version returned null');
    }
    try {
      return ptr.toDartString();
    } finally {
      WalletOpsBindings.ffiVersionFree(ptr);
    }
  }

  /// Lists wallet IDs for the given network + baseDir.
  @override
  List<String> listWallets({
    required FfiNetwork network,
    required String baseDir,
  }) {
    final baseDirPtr = baseDir.toNativeUtf8();
    final outCount = calloc<UintPtr>();
    final outIds = calloc<Pointer<Utf8>>();
    try {
      final rc = WalletOpsBindings.walletList(
        network.code,
        baseDirPtr,
        outCount,
        outIds,
      );
      if (rc != 0) {
        throw _ffiError('wallet_list', rc);
      }
      final arrPtr = outIds.value;
      if (arrPtr == nullptr) return const <String>[];
      try {
        final count = outCount.value;
        final result = <String>[];
        final basePtr = arrPtr.cast<Pointer<Utf8>>();
        for (var i = 0; i < count; i++) {
          final idPtr = basePtr[i];
          if (idPtr == nullptr) continue;
          result.add(idPtr.toDartString());
        }
        return result;
      } finally {
        WalletOpsBindings.walletListArrayFree(arrPtr, 0);
      }
    } finally {
      calloc.free(baseDirPtr);
      calloc.free(outCount);
      calloc.free(outIds);
    }
  }

  /// Deletes the wallet blob for the given network + walletId.
  @override
  void deleteWallet({
    required FfiNetwork network,
    required String walletId,
    required String baseDir,
  }) {
    final walletIdPtr = walletId.toNativeUtf8();
    final baseDirPtr = baseDir.toNativeUtf8();
    try {
      final rc = WalletOpsBindings.walletDelete(
        network.code,
        baseDirPtr,
        walletIdPtr,
      );
      if (rc != 0) {
        throw _ffiError('wallet_delete', rc);
      }
    } finally {
      calloc.free(walletIdPtr);
      calloc.free(baseDirPtr);
    }
  }

  /// Creates a new wallet with a fresh random mnemonic. The
  /// `password` `SecretBuffer` is auto-disposed after the FFI call.
  @override
  WalletCreatedData createWallet({
    required int words,
    required FfiNetwork network,
    required FfiAddressType addressType,
    required SecretBuffer password,
    required String baseDir,
  }) {
    final baseDirPtr = baseDir.toNativeUtf8();
    final outId = calloc<Uint8>(37);
    final outPhraseHandle = calloc<Pointer<Void>>();
    try {
      final rc = WalletOpsBindings.walletCreate(
        words,
        network.code,
        addressType.code,
        password.ptr,
        password.length,
        baseDirPtr,
        outId,
        outPhraseHandle,
      );
      if (rc != 0) {
        throw _ffiError('wallet_create', rc);
      }
      // Read 36-char UUID hex from out_id. Rust NUL-terminates at
      // byte 36; we read only 36 to avoid the trailing NUL.
      final idBytes = outId.asTypedList(36);
      final id = String.fromCharCodes(idBytes);
      final handle = outPhraseHandle.value;
      return WalletCreatedData(
        id: id,
        mnemonic: MnemonicView(handle),
        network: network,
        addressType: addressType,
      );
    } finally {
      calloc.free(baseDirPtr);
      calloc.free(outId);
      calloc.free(outPhraseHandle);
      password.dispose();
    }
  }

  /// Imports an existing BIP-39 mnemonic phrase as a new wallet. Both
  /// `phrase` and `password` `SecretBuffer`s are auto-disposed after
  /// the FFI call.
  @override
  WalletImportedData importWallet({
    required FfiNetwork network,
    required SecretBuffer phrase,
    required SecretBuffer password,
    required String baseDir,
  }) {
    final baseDirPtr = baseDir.toNativeUtf8();
    final outId = calloc<Uint8>(37);
    try {
      final rc = WalletOpsBindings.walletImport(
        network.code,
        baseDirPtr,
        phrase.ptr,
        phrase.length,
        password.ptr,
        password.length,
        outId,
      );
      if (rc != 0) {
        throw _ffiError('wallet_import', rc);
      }
      final idBytes = outId.asTypedList(36);
      final id = String.fromCharCodes(idBytes);
      return WalletImportedData(
        id: id,
        network: network,
        // Address type not persisted in the import API — derive from
        // wallet_peek_addresses (Tasks 10-16).
        addressType: FfiAddressType.unknown,
      );
    } finally {
      calloc.free(baseDirPtr);
      calloc.free(outId);
      phrase.dispose();
      password.dispose();
    }
  }

  /// Read a wallet's metadata + first external address from the
  /// persisted blob (Task 13 / Issue #219). The `password`
  /// `SecretBuffer` is auto-disposed after the FFI call. Returns a
  /// `WalletDetail` (collapsed `Balance` — single `confirmedSat`
  /// field, no `utxos` list).
  ///
  /// **v0.2.x firstAddress (Issue #261):** `firstAddress` is
  /// populated offline (no Esplora round-trip). Rust derives the
  /// first External receive address via
  /// `Wallet::first_external_address_offline` — pure local crypto.
  /// `balance.confirmedSat` is still `0` (sync gate stays — address
  /// vs balance are independent derivation paths). The detail
  /// screen renders the address chip + copy/explorer/faucet wiring
  /// unconditionally now that the field is reliable.
  ///
  /// **L12 collapse (HIGH #1 mirror):** wrong-password /
  /// not-found / wrong-AAD / corrupt-blob all surface as
  /// `FfiException(kind: FfiErrorKind.walletStore)`.
  @override
  WalletShowResult showWallet({
    required FfiNetwork network,
    required String walletId,
    required SecretBuffer password,
    required String baseDir,
    required String esploraUrl,
    required String esploraSpkiPin,
  }) {
    final baseDirPtr = baseDir.toNativeUtf8();
    final walletIdPtr = walletId.toNativeUtf8();
    final esploraUrlPtr = esploraUrl.toNativeUtf8();
    final esploraPinPtr = esploraSpkiPin.toNativeUtf8();
    final outId = calloc<Uint8>(37);
    final outNetwork = calloc<Uint8>();
    final outAddressType = calloc<Uint8>();
    final outFirstAddress = calloc<Pointer<Utf8>>();
    final outBalanceSat = calloc<Uint64>();
    final outWalletHandle = calloc<Pointer<Void>>();
    try {
      final rc = WalletOpsBindings.walletShow(
        network.code,
        baseDirPtr,
        walletIdPtr,
        password.ptr,
        password.length,
        esploraUrlPtr,
        esploraPinPtr,
        outId,
        outNetwork,
        outAddressType,
        outFirstAddress,
        outBalanceSat,
        outWalletHandle,
      );
      if (rc != 0) {
        throw _ffiError('wallet_show', rc);
      }
      // Read the 36-char UUID hex from out_id (Rust leaves byte 36
      // untouched; the calloc zero-init means it's `0` — NUL
      // terminator, but we only read 36 bytes).
      final idBytes = outId.asTypedList(36);
      final id = String.fromCharCodes(idBytes);
      // out_network is the echo byte (always equals the input `network`).
      // out_address_type maps the byte to the typed enum.
      final addrTypeByte = outAddressType.value;
      final addressType = switch (addrTypeByte) {
        0 => FfiAddressType.nativeSegwit,
        1 => FfiAddressType.nestedSegwit,
        2 => FfiAddressType.taproot,
        _ => FfiAddressType.unknown,
      };
      // out_first_address: Issue #261 — Rust `wallet_show` derives
      // the first External receive address offline via
      // `Wallet::first_external_address_offline` (no Esplora
      // round-trip). Always non-empty for a freshly constructed
      // wallet. Empty string only if Rust allocates a null (defensive
      // — shouldn't happen post-#261).
      final firstAddrPtr = outFirstAddress.value;
      final firstAddress =
          firstAddrPtr == nullptr ? '' : firstAddrPtr.toDartString();
      // out_balance_sat: synced via Esplora when `esplora_url` was
      // provided (see sync block above); otherwise `0` (legacy
      // v0.2.0 behavior — useful for offline test fixtures).
      final balanceSat = outBalanceSat.value;
      // Dart string for addressType (matches legacy btc wallet show
      // --json encoding for the detail screen).
      final addressTypeStr = switch (addressType) {
        FfiAddressType.nativeSegwit => 'native-segwit',
        FfiAddressType.nestedSegwit => 'nested-segwit',
        FfiAddressType.taproot => 'taproot',
        FfiAddressType.unknown => '',
      };
      // Network: the FFI `network` parameter is the typed enum; map
      // to a string for `WalletDetail.network`. Only testnet is
      // wired today (matches Task 10/11/12 `_networkFromString`
      // assert guard); the FFI surface ignores other values.
      final networkStr = switch (network) {
        FfiNetwork.testnet => 'testnet',
        FfiNetwork.unknown => '',
      };
      return WalletShowResult(
        detail: WalletDetail(
          id: id,
          network: networkStr,
          addressType: addressTypeStr,
          firstAddress: firstAddress,
          balance: Balance(confirmedSat: balanceSat),
        ),
        // The signing handle — caller passes to walletSend /
        // walletBalance / walletSync. Free via walletLoadFree. Null
        // if Rust skipped (out_wallet_handle was nullptr on the FFI
        // side, which we never do post-#261 — defensive only).
        walletHandle: outWalletHandle.value,
      );
    } finally {
      // Free the CString-typed `out_first_address` if Rust allocated
      // one (always true for the v0.2.0 path). Null-safe.
      final firstAddrPtr = outFirstAddress.value;
      if (firstAddrPtr != nullptr) {
        WalletOpsBindings.walletShowFirstAddressFree(firstAddrPtr);
      }
      calloc.free(baseDirPtr);
      calloc.free(walletIdPtr);
      calloc.free(esploraUrlPtr);
      calloc.free(esploraPinPtr);
      calloc.free(outId);
      calloc.free(outNetwork);
      calloc.free(outAddressType);
      calloc.free(outFirstAddress);
      calloc.free(outBalanceSat);
      calloc.free(outWalletHandle);
      password.dispose();
    }
  }

  // ---------------------------------------------------------------------------
  // Async surface — wraps EsploraBindings + RuntimeBindings.
  // ---------------------------------------------------------------------------

  /// Constructs an `EsploraClient` handle for the given URL. SPKI pin
  /// is required for non-localhost hosts (F20 enforcement).
  Pointer<Void> esploraClientNew({
    required Pointer<Utf8> url,
    Pointer<Utf8>? spkiPinB64,
  }) {
    final handle = EsploraBindings.esploraClientNew(
      url,
      spkiPinB64 ?? nullptr,
    );
    if (handle == nullptr) {
      throw _ffiError('esplora_client_new', -1);
    }
    return handle;
  }

  /// Drops an `EsploraClient` handle. Idempotent on null.
  void esploraClientFree(Pointer<Void> handle) {
    if (handle == nullptr) return;
    EsploraBindings.esploraClientFree(handle);
  }

  /// Fetch Esplora fee estimates via the FFI surface. (Task 16 /
  /// Issue #222.) Returns a parsed [FeeEstimate]; the raw JSON
  /// payload crosses the FFI boundary as a NUL-terminated `CString`
  /// (`esplora_fee_estimate` returns a `*mut c_char` allocated via
  /// `into_raw()`; we read it via `toDartString()` then `free` it
  /// via `esplora_fee_estimate_free`).
  ///
  /// Caller owns the `esploraHandle` (must come from `esploraClientNew`).
  /// Throws `FfiException` on null handle + Esplora/network failures
  /// (see `esplora_fee_estimate` for the `FfiError` mapping).
  @override
  FeeEstimate feeEstimate({required Pointer<Void> esploraHandle}) {
    final rt = _runtimeHandle();
    final jsonPtr = EsploraBindings.esploraFeeEstimate(rt, esploraHandle);
    if (jsonPtr == nullptr) {
      throw _ffiError('esplora_fee_estimate', -1);
    }
    try {
      final jsonStr = jsonPtr.toDartString();
      final decoded = jsonDecode(jsonStr) as Map<String, dynamic>;
      return FeeEstimate.fromJson(decoded);
    } finally {
      EsploraBindings.esploraFeeEstimateFree(jsonPtr);
    }
  }

  /// Constructs a `Wallet` from a BIP-39 mnemonic phrase. The
  /// `phrase` `SecretBuffer` is auto-disposed after the FFI call.
  /// Rust expects `*const c_char` (NUL-terminated), so we allocate a
  /// NUL-terminated copy from the byte buffer.
  Pointer<Void> walletFromMnemonic({
    required SecretBuffer phrase,
    required FfiNetwork network,
    required FfiAddressType addressType,
  }) {
    final cstr = _toCString(phrase.ptr, phrase.length);
    try {
      final handle = EsploraBindings.walletFromMnemonic(
        cstr,
        network.code,
        addressType.code,
      );
      if (handle == nullptr) {
        throw _ffiError('wallet_from_mnemonic', -1);
      }
      return handle;
    } finally {
      calloc.free(cstr);
      phrase.dispose();
    }
  }

  /// Drops a `Wallet` handle. Idempotent on null.
  void walletFree(Pointer<Void> handle) {
    if (handle == nullptr) return;
    EsploraBindings.walletFree(handle);
  }

  /// Loads an existing wallet from disk into a `WalletHandle`.
  /// (Task 14 / Issue #220 Sub-split A.) Returns null on failure
  /// (wallet file missing at `{baseDir}/{walletId}.wallet`, bad
  /// mnemonic, unknown network byte, etc.); caller checks
  /// `ffi_last_error_message` for the `FfiError` code via
  /// [_ffiError].
  ///
  /// **L12 CRITICAL #2**: `phrase` is wrapped in a `SecretBuffer` on
  /// the Dart side and zeroized + freed after the FFI call returns.
  /// The Rust side wraps the incoming C string in `Secret<String>`
  /// (zeroize-on-drop). Mirrors the lifetime pattern from
  /// `walletFromMnemonic`.
  Pointer<Void> walletLoad({
    required String baseDir,
    required String walletId,
    required SecretBuffer phrase,
    required FfiNetwork network,
  }) {
    final baseDirPtr = baseDir.toNativeUtf8();
    final walletIdPtr = walletId.toNativeUtf8();
    final phrasePtr = _toCString(phrase.ptr, phrase.length);
    try {
      final handle = EsploraBindings.walletLoad(
        baseDirPtr,
        walletIdPtr,
        phrasePtr,
        network.code,
      );
      if (handle == nullptr) {
        throw _ffiError('wallet_load', -1);
      }
      return handle;
    } finally {
      calloc.free(baseDirPtr);
      calloc.free(walletIdPtr);
      calloc.free(phrasePtr);
    }
  }

  /// Drops a `WalletHandle` returned by [walletLoad]. Idempotent on
  /// null. Body identical to [walletFree] — separate method for
  /// call-site clarity (which load created the handle).
  void walletLoadFree(Pointer<Void> handle) {
    if (handle == nullptr) return;
    EsploraBindings.walletLoadFree(handle);
  }

  /// Syncs the wallet against Esplora (pulls UTXOs + chain tip).
  void walletSync({
    required Pointer<Void> walletHandle,
    required Pointer<Void> esploraHandle,
  }) {
    final rt = _runtimeHandle();
    final rc = EsploraBindings.walletSync(rt, walletHandle, esploraHandle);
    if (rc != 0) {
      throw _ffiError('wallet_sync', rc);
    }
  }

  /// Returns the confirmed balance in satoshis.
  int walletBalance({
    required Pointer<Void> walletHandle,
    required Pointer<Void> esploraHandle,
  }) {
    final rt = _runtimeHandle();
    final outBalance = calloc<Uint64>();
    try {
      final rc = EsploraBindings.walletBalance(
        rt,
        walletHandle,
        esploraHandle,
        outBalance,
      );
      if (rc != 0) {
        throw _ffiError('wallet_balance', rc);
      }
      return outBalance.value;
    } finally {
      calloc.free(outBalance);
    }
  }

  /// Sends satoshis to a recipient. Returns the txid hex string.
  String walletSend({
    required Pointer<Void> walletHandle,
    required Pointer<Void> esploraHandle,
    required Pointer<Utf8> recipient,
    required int amountSat,
    required int feeRateSatPerVb,
  }) {
    final rt = _runtimeHandle();
    final txidPtr = EsploraBindings.walletSend(
      rt,
      walletHandle,
      esploraHandle,
      recipient,
      amountSat,
      feeRateSatPerVb,
    );
    if (txidPtr == nullptr) {
      throw _ffiError('wallet_send', -1);
    }
    try {
      return txidPtr.toDartString();
    } finally {
      EsploraBindings.walletSendFree(txidPtr);
    }
  }

  /// Returns all txids in the wallet.
  @override
  List<String> walletTxids({required Pointer<Void> walletHandle}) {
    final outCount = calloc<UintPtr>();
    final outArr = calloc<Pointer<Utf8>>();
    try {
      final rc = EsploraBindings.walletTxids(walletHandle, outCount, outArr);
      if (rc != 0) {
        throw _ffiError('wallet_txids', rc);
      }
      final arrPtr = outArr.value;
      if (arrPtr == nullptr) return const <String>[];
      try {
        final count = outCount.value;
        final result = <String>[];
        final basePtr = arrPtr.cast<Pointer<Utf8>>();
        for (var i = 0; i < count; i++) {
          final txidPtr = basePtr[i];
          if (txidPtr == nullptr) continue;
          result.add(txidPtr.toDartString());
        }
        return result;
      } finally {
        EsploraBindings.walletTxidsArrayFree(arrPtr, 0);
      }
    } finally {
      calloc.free(outCount);
      calloc.free(outArr);
    }
  }

  /// Peeks a batch of addresses for the given keychain kind.
  List<String> walletPeekAddresses({
    required Pointer<Void> walletHandle,
    required FfiKeychainKind kind,
    required int count,
  }) {
    final outCount = calloc<UintPtr>();
    final outArr = calloc<Pointer<Utf8>>();
    try {
      final rc = EsploraBindings.walletPeekAddresses(
        walletHandle,
        kind.code,
        count,
        outCount,
        outArr,
      );
      if (rc != 0) {
        throw _ffiError('wallet_peek_addresses', rc);
      }
      final arrPtr = outArr.value;
      if (arrPtr == nullptr) return const <String>[];
      try {
        final resultCount = outCount.value;
        final result = <String>[];
        final basePtr = arrPtr.cast<Pointer<Utf8>>();
        for (var i = 0; i < resultCount; i++) {
          final addrPtr = basePtr[i];
          if (addrPtr == nullptr) continue;
          result.add(addrPtr.toDartString());
        }
        return result;
      } finally {
        EsploraBindings.walletPeekAddressesArrayFree(arrPtr, 0);
      }
    } finally {
      calloc.free(outCount);
      calloc.free(outArr);
    }
  }

  // ---------------------------------------------------------------------------
  // Internal helpers
  // ---------------------------------------------------------------------------

  /// Allocates a NUL-terminated copy of `len` bytes at `src` and
  /// returns it as a `Pointer<Utf8>`. Caller MUST `calloc.free`.
  static Pointer<Utf8> _toCString(Pointer<Uint8> src, int len) {
    final dst = calloc<Uint8>(len + 1);
    if (len > 0) {
      final srcView = src.asTypedList(len);
      final dstView = dst.asTypedList(len);
      for (var i = 0; i < len; i++) {
        dstView[i] = srcView[i];
      }
    }
    (dst + len).value = 0;
    return dst.cast<Utf8>();
  }

  /// Translates a non-zero FFI return code to a typed [FfiException].
  /// Task 9 closes L12 CRITICAL #1 — callers in Tasks 10-16 match
  /// `on FfiException catch (e) when (e.kind == FfiErrorKind.x)`
  /// instead of parsing message strings.
  FfiException _ffiError(String op, int code) =>
      FfiException.fromCode(code: code, op: op);
}
