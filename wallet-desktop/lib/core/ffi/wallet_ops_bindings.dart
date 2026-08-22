// Task 7 (#213) — typed FFI wrappers for wallet ops (Task 4 surface).
//
// Mirrors the `extern "C"` exports in
// `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/wallet_ops.rs`
// + `ffi/wallet.rs` (version helpers).
//
// All symbols are resolved via `NativeLib.open()` (Task 6). Callers
// receive a typed Dart function (via `lookupFunction<CFn, DartFn>`)
// — no manual `Pointer<NativeFunction>` plumbing at call sites.
//
// **L12 CRITICAL #2 contract**: typed wrappers do NOT log their
// arguments. The mnemonic / password bytes cross the FFI boundary as
// raw `Pointer<Uint8>`; callers MUST zero them via `calloc<Uint8>()`
// + `.fillRange(0, len, 0)` and calloc.free after the FFI call
// returns. The Rust side wraps incoming data in `Secret<Vec<u8>>`
// (zeroize-on-drop); the Dart side mirrors the lifetime by zeroing
// the calloc buffer before free. Task 8 (facade) will introduce a
// typed `SecretBuffer` RAII wrapper that closes this foot-gun.
//
// **L12 CRITICAL #2 contract**: `phraseViewCopy` returns a
// `Pointer<Utf8>` borrowed from the Rust-side `MnemonicHandle` heap.
// DO NOT log the resulting string. Pass it to `phraseViewFree` (which
// zeroizes + drops the handle) once displayed. Future `PhraseView`
// newtype (Task 8+) will replace the raw pointer with a typed
// handle that bans `.toDartString()`.

// `unused_field` on `_lib` is intentional — the field is a GC anchor
// that keeps the DynamicLibrary loaded while cached function pointers
// are alive. The Dart analyzer doesn't recognize this usage pattern.
// ignore_for_file: unused_field

import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'package:wallet_desktop/core/ffi/native_lib.dart';

// ---------------------------------------------------------------------------
// ffi_version + ffi_version_free
// ---------------------------------------------------------------------------

typedef _FfiVersionC = Pointer<Utf8> Function();
typedef _FfiVersionDart = Pointer<Utf8> Function();

typedef _FfiVersionFreeC = Void Function(Pointer<Utf8>);
typedef _FfiVersionFreeDart = void Function(Pointer<Utf8>);

// ---------------------------------------------------------------------------
// ffi_last_error_message (Issue #263 — surface sync-fail diagnostic to UI)
// ---------------------------------------------------------------------------

typedef _FfiLastErrorMessageC = Pointer<Utf8> Function();
typedef _FfiLastErrorMessageDart = Pointer<Utf8> Function();

// ---------------------------------------------------------------------------
// wallet_create
// ---------------------------------------------------------------------------

typedef _WalletCreateC = Int32 Function(
  Uint8,
  Uint8,
  Uint8,
  Pointer<Uint8>,
  IntPtr,
  Pointer<Utf8>,
  Pointer<Uint8>,
  Pointer<Pointer<Void>>,
);
typedef _WalletCreateDart = int Function(
  int,
  int,
  int,
  Pointer<Uint8>,
  int,
  Pointer<Utf8>,
  Pointer<Uint8>,
  Pointer<Pointer<Void>>,
);

// ---------------------------------------------------------------------------
// phrase_view_copy / phrase_view_free
// ---------------------------------------------------------------------------

typedef _PhraseViewCopyC = Pointer<Utf8> Function(Pointer<Void>);
typedef _PhraseViewCopyDart = Pointer<Utf8> Function(Pointer<Void>);

typedef _PhraseViewFreeC = Void Function(Pointer<Void>);
typedef _PhraseViewFreeDart = void Function(Pointer<Void>);

// ---------------------------------------------------------------------------
// wallet_list + wallet_list_array_free
// ---------------------------------------------------------------------------

typedef _WalletListC = Int32 Function(
  Uint8,
  Pointer<Utf8>,
  Pointer<UintPtr>,
  Pointer<Pointer<Utf8>>,
);
typedef _WalletListDart = int Function(
  int,
  Pointer<Utf8>,
  Pointer<UintPtr>,
  Pointer<Pointer<Utf8>>,
);

typedef _WalletListArrayFreeC = Void Function(Pointer<Utf8>, UintPtr);
typedef _WalletListArrayFreeDart = void Function(Pointer<Utf8>, int);

// ---------------------------------------------------------------------------
// wallet_delete
// ---------------------------------------------------------------------------

typedef _WalletDeleteC = Int32 Function(Uint8, Pointer<Utf8>, Pointer<Utf8>);
typedef _WalletDeleteDart = int Function(int, Pointer<Utf8>, Pointer<Utf8>);

// ---------------------------------------------------------------------------
// wallet_import
// ---------------------------------------------------------------------------

typedef _WalletImportC = Int32 Function(
  Uint8,
  Pointer<Utf8>,
  Pointer<Uint8>,
  IntPtr,
  Pointer<Uint8>,
  IntPtr,
  Pointer<Uint8>,
);
typedef _WalletImportDart = int Function(
  int,
  Pointer<Utf8>,
  Pointer<Uint8>,
  int,
  Pointer<Uint8>,
  int,
  Pointer<Uint8>,
);

// ---------------------------------------------------------------------------
// wallet_show (Task 13 / Issue #219)
// ---------------------------------------------------------------------------

typedef _WalletShowC = Int32 Function(
  Uint8,
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Uint8>,
  IntPtr,
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Uint8>,
  Pointer<Uint8>,
  Pointer<Uint8>,
  Pointer<Pointer<Utf8>>,
  Pointer<Uint64>,
  Pointer<Uint8>,
  Pointer<Pointer<Void>>,
);
typedef _WalletShowDart = int Function(
  int,
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Uint8>,
  int,
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Uint8>,
  Pointer<Uint8>,
  Pointer<Uint8>,
  Pointer<Pointer<Utf8>>,
  Pointer<Uint64>,
  Pointer<Uint8>,
  Pointer<Pointer<Void>>,
);

typedef _WalletShowFirstAddressFreeC = Void Function(Pointer<Utf8>);
typedef _WalletShowFirstAddressFreeDart = void Function(Pointer<Utf8>);

// ---------------------------------------------------------------------------
// Bindings
// ---------------------------------------------------------------------------

/// Typed FFI wrappers for the synchronous wallet ops surface
/// (Task 4 + version helpers).
///
/// All fields are `static final` — resolved once on first access via
/// late binding. The shared `DynamicLibrary` is kept alive by the
/// `_lib` field anchor.
class WalletOpsBindings {
  WalletOpsBindings._();

  static final DynamicLibrary _lib = NativeLib.open();

  /// Returns the rust crate version as a NUL-terminated C string.
  static final Pointer<Utf8> Function() ffiVersion =
      _lib.lookupFunction<_FfiVersionC, _FfiVersionDart>('ffi_version');

  /// Frees a string returned by [ffiVersion].
  static final void Function(Pointer<Utf8> ptr) ffiVersionFree =
      _lib.lookupFunction<_FfiVersionFreeC, _FfiVersionFreeDart>(
    'ffi_version_free',
  );

  /// **Issue #263** — surface the last FFI error message (set via
  /// `set_last_error` in Rust) to the UI. Returns a borrowed
  /// `*const c_char` into a thread-local `CString`; the pointer is
  /// invalidated by the NEXT FFI call on the same thread that
  /// triggers `set_last_error`. Caller MUST read it via
  /// `toDartString` (or copy bytes) IMMEDIATELY after the FFI call
  /// returns — DO NOT retain across subsequent FFI calls. Returns
  /// null if no error has been recorded.
  ///
  /// Used by [WalletCore.showWallet] to surface Esplora sync failure
  /// diagnostics (`wallet_show esplora client: <err>`, etc.) in the
  /// red SyncFailed banner. The C1 fix from L12 review (replaces the
  /// dead `lastError` parameter that was never populated).
  static final Pointer<Utf8> Function() ffiLastErrorMessage =
      _lib.lookupFunction<_FfiLastErrorMessageC, _FfiLastErrorMessageDart>(
    'ffi_last_error_message',
  );

  /// Generates a new random mnemonic + persists the encrypted wallet
  /// blob. On success, writes a 36-char UUID hex to `outId` and a
  /// `MnemonicHandle` to `outPhraseHandle`. Caller MUST call
  /// [phraseViewFree] on the handle after the displayed mnemonic is
  /// acknowledged (L12 CRITICAL #2).
  ///
  /// **OutParams:**
  /// - `outId`: caller allocates a 37-byte buffer via `calloc<Uint8>()`
  ///   (the 37th byte MUST be zero on input — Rust leaves it
  ///   untouched). After the call, bytes [0..36) hold the 36-char
  ///   UUID hex (no NUL terminator).
  /// - `outPhraseHandle`: caller allocates a `Pointer<Pointer<Void>>`
  ///   slot via `calloc<Pointer<Void>>()`. After the call, the slot
  ///   holds the opaque handle (read via `slot.value`). Free with
  ///   [phraseViewFree] once the displayed mnemonic is acknowledged.
  ///
  /// **Zeroize contract (L12 CRITICAL #2):** `password` MUST be zeroed
  /// via `password.fillRange(0, passwordLen, 0)` and freed via
  /// `calloc.free(password)` after the call returns.
  ///
  /// **Network + addressType scalars:** pass `FfiNetwork.testnet.code`
  /// and `FfiAddressType.<variant>.code` (see `ffi_enums.dart`) to
  /// avoid silent `int` truncation on the FFI boundary.
  static final int Function(
    int words,
    int network,
    int addressType,
    Pointer<Uint8> password,
    int passwordLen,
    Pointer<Utf8> baseDir,
    Pointer<Uint8> outId,
    Pointer<Pointer<Void>> outPhraseHandle,
  ) walletCreate =
      _lib.lookupFunction<_WalletCreateC, _WalletCreateDart>('wallet_create');

  /// Borrows a `*const c_char` into the cleartext phrase buffer held
  /// by `handle`. ZERO-COPY pointer — invalid after [phraseViewFree]
  /// or any subsequent FFI call on the same thread that mutates the
  /// handle. DO NOT log the returned string.
  static final Pointer<Utf8> Function(Pointer<Void> handle) phraseViewCopy =
      _lib.lookupFunction<_PhraseViewCopyC, _PhraseViewCopyDart>(
    'phrase_view_copy',
  );

  /// Zeroizes + frees a `MnemonicHandle` (L12 CRITICAL #2).
  /// Null is a no-op.
  static final void Function(Pointer<Void> handle) phraseViewFree =
      _lib.lookupFunction<_PhraseViewFreeC, _PhraseViewFreeDart>(
    'phrase_view_free',
  );

  /// Lists all wallet IDs for the given network. Atomic:
  /// out_count + out_ids written together or both untouched.
  ///
  /// **OutParams:**
  /// - `outCount`: caller allocates a `Pointer<UintPtr>` via
  ///   `calloc<UintPtr>()` and reads the count from `outCount.value`.
  /// - `outIds`: caller allocates a `Pointer<Pointer<Utf8>>` slot via
  ///   `calloc<Pointer<Utf8>>()`. After the call, the slot holds the
  ///   array pointer (read via `slot.value`). Free the array with
  ///   [walletListArrayFree] (passing the array pointer, not the slot).
  ///   The caller-supplied count is ignored by the free — the
  ///   canonical count lives in the heap header.
  ///
  /// **Network scalar:** pass `FfiNetwork.testnet.code` (see
  /// `ffi_enums.dart`).
  static final int Function(
    int network,
    Pointer<Utf8> baseDir,
    Pointer<UintPtr> outCount,
    Pointer<Pointer<Utf8>> outIds,
  ) walletList =
      _lib.lookupFunction<_WalletListC, _WalletListDart>('wallet_list');

  /// Frees a wallet list returned by [walletList]. The caller-supplied
  /// count is ignored; the canonical count lives in the heap header
  /// (L40 mirror).
  static final void Function(Pointer<Utf8> arr, int count) walletListArrayFree =
      _lib.lookupFunction<_WalletListArrayFreeC, _WalletListArrayFreeDart>(
    'wallet_list_array_free',
  );

  /// Deletes the wallet blob at `<base>/wallets/<network>/<id>.blob`.
  /// Returns `FfiError::Storage` if the wallet doesn't exist or the id
  /// cannot be decoded as a UUID.
  ///
  /// **Network scalar:** pass `FfiNetwork.testnet.code` (see
  /// `ffi_enums.dart`).
  static final int Function(
    int network,
    Pointer<Utf8> baseDir,
    Pointer<Utf8> walletId,
  ) walletDelete =
      _lib.lookupFunction<_WalletDeleteC, _WalletDeleteDart>('wallet_delete');

  /// Imports an existing BIP-39 mnemonic phrase + persists the
  /// encrypted wallet blob. Writes the 36-char UUID hex to `outId`
  /// (no NUL terminator — same as `walletCreate`).
  ///
  /// **Zeroize contract (L12 CRITICAL #2):** `phrase` and `password`
  /// MUST be zeroed via `.fillRange(0, len, 0)` and freed via
  /// `calloc.free(ptr)` after the call returns.
  ///
  /// **Network scalar:** pass `FfiNetwork.testnet.code` (see
  /// `ffi_enums.dart`).
  static final int Function(
    int network,
    Pointer<Utf8> baseDir,
    Pointer<Uint8> phrase,
    int phraseLen,
    Pointer<Uint8> password,
    int passwordLen,
    Pointer<Uint8> outId,
  ) walletImport =
      _lib.lookupFunction<_WalletImportC, _WalletImportDart>('wallet_import');

  // ---------------------------------------------------------------------------
  // wallet_show (Task 13 / Issue #219)
  // ---------------------------------------------------------------------------

  /// Read a wallet's metadata + first external address from the
  /// persisted blob (Task 13). Returns the wallet id, network,
  /// address type, first address (heap-allocated CString; free via
  /// [walletShowFirstAddressFree]), balance, sync status (Issue #263),
  /// and an optional signing handle.
  ///
  /// **OutParams:**
  /// - `outId`: caller allocates a 37-byte buffer (calloc zero-initialises
  ///   byte 36). Reads the 36-char UUID hex from `outId.value` after
  ///   the call returns.
  /// - `outNetwork`: caller allocates `calloc<Uint8>()`; reads the
  ///   echoed network byte (1 = Testnet).
  /// - `outAddressType`: caller allocates `calloc<Uint8>()`; reads
  ///   the address-type byte (0 = NativeSegwit, 1 = NestedSegwit,
  ///   2 = Taproot).
  /// - `outFirstAddress`: caller allocates
  ///   `calloc<Pointer<Utf8>>()`. After the call, the slot holds a
  ///   pointer to a NUL-terminated CString (empty string in v0.2.0
  ///   per plan deviation #4 — peek_addresses requires sync).
  ///   Free via [walletShowFirstAddressFree].
  /// - `outBalanceSat`: caller allocates `calloc<Uint64>()`; reads
  ///   the confirmed balance (always 0 in v0.2.0; v0.2.1 wires sync).
  /// - `outSyncStatus` (Issue #263): caller allocates `calloc<Uint8>()`;
  ///   reads the `WalletSyncStatus` byte (0 = Synced, 1 = EmptyWallet,
  ///   2 = SyncFailed). Lets the UI render a red banner + Retry button
  ///   when sync fails (previously silent — operator couldn't
  ///   distinguish empty wallet from broken Esplora sync).
  /// - `outWalletHandle`: caller allocates `calloc<Pointer<Void>>()`;
  ///   receives the opaque `Box<WalletHandle>` for `walletSend` /
  ///   `walletBalance` / `walletSync`. Free via `walletLoadFree`.
  ///
  /// **L12 collapse (L12 HIGH #1 mirror):** file-not-found, wrong
  /// password, wrong network AAD, and corrupt blob all surface as
  /// `FfiError::WalletStore`. The detail screen renders this via
  /// `userMessageForFfiException` as a single "could not unlock"
  /// copy — no enumeration signal for a network observer.
  ///
  /// **Zeroize contract (L12 CRITICAL #2):** `password` MUST be
  /// zeroed via `SecretBuffer.fromUtf8` (auto-disposed in `finally`)
  /// after the call returns.
  ///
  /// **Network scalar:** pass `FfiNetwork.testnet.code`.
  static final int Function(
    int network,
    Pointer<Utf8> baseDir,
    Pointer<Utf8> walletId,
    Pointer<Uint8> password,
    int passwordLen,
    Pointer<Utf8> esploraUrl,
    Pointer<Utf8> esploraSpkiPin,
    Pointer<Uint8> outId,
    Pointer<Uint8> outNetwork,
    Pointer<Uint8> outAddressType,
    Pointer<Pointer<Utf8>> outFirstAddress,
    Pointer<Uint64> outBalanceSat,
    Pointer<Uint8> outSyncStatus,
    Pointer<Pointer<Void>> outWalletHandle,
  ) walletShow =
      _lib.lookupFunction<_WalletShowC, _WalletShowDart>('wallet_show');

  /// Frees a first-address CString returned by [walletShow]. Null is
  /// a no-op.
  static final void Function(Pointer<Utf8> ptr) walletShowFirstAddressFree =
      _lib.lookupFunction<_WalletShowFirstAddressFreeC,
          _WalletShowFirstAddressFreeDart>(
    'wallet_show_first_address_free',
  );
}
