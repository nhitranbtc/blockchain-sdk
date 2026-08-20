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

import 'dart:ffi';

import 'package:ffi/ffi.dart';

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
