// Task 7 (#213) — typed FFI wrappers for Esplora + Wallet async ops
// (Task 5 surface).
//
// Mirrors the `extern "C"` exports in
// `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/bdk_extras.rs`.
//
// All async ops take the runtime handle from `runtime_bindings.dart`
// as the first argument and `block_on` internally. The Dart side
// blocks the calling isolate until the future resolves — same
// synchronous call shape as the Task 4 surface, just exercising the
// chain backend under the hood.
//
// **L12 CRITICAL #2 contract**: `walletFromMnemonic` takes the phrase
// as a `Pointer<Utf8>` borrowed from the caller. The Dart side MUST
// zero the calloc buffer (via `.fillRange(0, len, 0)`) before
// `calloc.free` and MUST NOT log the phrase. Task 8 (facade) will
// introduce a `SecretBuffer` newtype that closes the foot-gun.
//
// **F20 enforcement**: `esploraClientNew` rejects a null SPKI pin on
// non-localhost hosts. Localhost dev mode retains the null-pin
// escape.

// `unused_field` on `_lib` is intentional — see wallet_ops_bindings.
// ignore_for_file: unused_field

import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'package:wallet_desktop/core/ffi/native_lib.dart';

// ---------------------------------------------------------------------------
// EsploraClient handle
// ---------------------------------------------------------------------------

typedef _EsploraClientNewC = Pointer<Void> Function(
  Pointer<Utf8>,
  Pointer<Utf8>,
);
typedef _EsploraClientNewDart = Pointer<Void> Function(
  Pointer<Utf8>,
  Pointer<Utf8>,
);

typedef _EsploraClientFreeC = Void Function(Pointer<Void>);
typedef _EsploraClientFreeDart = void Function(Pointer<Void>);

// ---------------------------------------------------------------------------
// esplora_fee_estimate + free
// ---------------------------------------------------------------------------

typedef _EsploraFeeEstimateC = Pointer<Utf8> Function(
  Pointer<Void>,
  Pointer<Void>,
);
typedef _EsploraFeeEstimateDart = Pointer<Utf8> Function(
  Pointer<Void>,
  Pointer<Void>,
);

typedef _EsploraFeeEstimateFreeC = Void Function(Pointer<Utf8>);
typedef _EsploraFeeEstimateFreeDart = void Function(Pointer<Utf8>);

// ---------------------------------------------------------------------------
// esplora_broadcast_tx + free
// ---------------------------------------------------------------------------

typedef _EsploraBroadcastTxC = Pointer<Utf8> Function(
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Utf8>,
);
typedef _EsploraBroadcastTxDart = Pointer<Utf8> Function(
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Utf8>,
);

typedef _EsploraBroadcastTxFreeC = Void Function(Pointer<Utf8>);
typedef _EsploraBroadcastTxFreeDart = void Function(Pointer<Utf8>);

// ---------------------------------------------------------------------------
// wallet_from_mnemonic + wallet_free
// ---------------------------------------------------------------------------

typedef _WalletFromMnemonicC = Pointer<Void> Function(
  Pointer<Utf8>,
  Uint8,
  Uint8,
);
typedef _WalletFromMnemonicDart = Pointer<Void> Function(
  Pointer<Utf8>,
  int,
  int,
);

typedef _WalletFreeC = Void Function(Pointer<Void>);
typedef _WalletFreeDart = void Function(Pointer<Void>);

// ---------------------------------------------------------------------------
// wallet_sync / wallet_balance / wallet_send + free
// ---------------------------------------------------------------------------

typedef _WalletSyncC = Int32 Function(
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Void>,
);
typedef _WalletSyncDart = int Function(
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Void>,
);

typedef _WalletBalanceC = Int32 Function(
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Uint64>,
);
typedef _WalletBalanceDart = int Function(
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Uint64>,
);

typedef _WalletSendC = Pointer<Utf8> Function(
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Utf8>,
  Uint64,
  Uint64,
);
typedef _WalletSendDart = Pointer<Utf8> Function(
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Void>,
  Pointer<Utf8>,
  int,
  int,
);

typedef _WalletSendFreeC = Void Function(Pointer<Utf8>);
typedef _WalletSendFreeDart = void Function(Pointer<Utf8>);

// ---------------------------------------------------------------------------
// wallet_txids + wallet_txids_array_free
// ---------------------------------------------------------------------------

typedef _WalletTxidsC = Int32 Function(
  Pointer<Void>,
  Pointer<UintPtr>,
  Pointer<Pointer<Utf8>>,
);
typedef _WalletTxidsDart = int Function(
  Pointer<Void>,
  Pointer<UintPtr>,
  Pointer<Pointer<Utf8>>,
);

typedef _WalletTxidsArrayFreeC = Void Function(Pointer<Utf8>, UintPtr);
typedef _WalletTxidsArrayFreeDart = void Function(Pointer<Utf8>, int);

// ---------------------------------------------------------------------------
// wallet_peek_addresses + wallet_peek_addresses_array_free
// ---------------------------------------------------------------------------

typedef _WalletPeekAddressesC = Int32 Function(
  Pointer<Void>,
  Uint8,
  Uint32,
  Pointer<UintPtr>,
  Pointer<Pointer<Utf8>>,
);
typedef _WalletPeekAddressesDart = int Function(
  Pointer<Void>,
  int,
  int,
  Pointer<UintPtr>,
  Pointer<Pointer<Utf8>>,
);

typedef _WalletPeekAddressesArrayFreeC = Void Function(Pointer<Utf8>, UintPtr);
typedef _WalletPeekAddressesArrayFreeDart = void Function(
  Pointer<Utf8>,
  int,
);

// ---------------------------------------------------------------------------
// Bindings
// ---------------------------------------------------------------------------

/// Typed FFI wrappers for the Esplora + Wallet async ops surface
/// (Task 5).
class EsploraBindings {
  EsploraBindings._();

  static final DynamicLibrary _lib = NativeLib.open();

  /// Constructs a new `EsploraClient` from a URL and optional SPKI pin.
  /// F20: requires a non-null pin for non-localhost hosts.
  static final Pointer<Void> Function(
    Pointer<Utf8> url,
    Pointer<Utf8> spkiPinB64,
  ) esploraClientNew =
      _lib.lookupFunction<_EsploraClientNewC, _EsploraClientNewDart>(
          'esplora_client_new');

  /// Drops an `EsploraClient` handle. Null is a no-op.
  static final void Function(Pointer<Void> handle) esploraClientFree =
      _lib.lookupFunction<_EsploraClientFreeC, _EsploraClientFreeDart>(
    'esplora_client_free',
  );

  /// Fetches fee estimates from Esplora. Returns a NUL-terminated JSON
  /// string; caller frees via [esploraFeeEstimateFree].
  static final Pointer<Utf8> Function(Pointer<Void> rt, Pointer<Void> handle)
      esploraFeeEstimate =
      _lib.lookupFunction<_EsploraFeeEstimateC, _EsploraFeeEstimateDart>(
          'esplora_fee_estimate');

  /// Frees a JSON buffer returned by [esploraFeeEstimate].
  static final void Function(Pointer<Utf8> ptr) esploraFeeEstimateFree = _lib
      .lookupFunction<_EsploraFeeEstimateFreeC, _EsploraFeeEstimateFreeDart>(
          'esplora_fee_estimate_free');

  /// Broadcasts a raw transaction. Returns a NUL-terminated txid hex
  /// string; caller frees via [esploraBroadcastTxFree].
  static final Pointer<Utf8> Function(
    Pointer<Void> rt,
    Pointer<Void> handle,
    Pointer<Utf8> rawTxHex,
  ) esploraBroadcastTx =
      _lib.lookupFunction<_EsploraBroadcastTxC, _EsploraBroadcastTxDart>(
          'esplora_broadcast_tx');

  /// Frees a txid string returned by [esploraBroadcastTx].
  static final void Function(Pointer<Utf8> ptr) esploraBroadcastTxFree = _lib
      .lookupFunction<_EsploraBroadcastTxFreeC, _EsploraBroadcastTxFreeDart>(
          'esplora_broadcast_tx_free');

  /// Constructs a `Wallet` from a BIP-39 mnemonic phrase + network +
  /// address type. Returns opaque `WalletHandle` or null on error.
  ///
  /// **Zeroize contract (L12 CRITICAL #2):** `phrase` is a
  /// NUL-terminated `Pointer<Utf8>`. The phrase buffer MUST be zeroed
  /// via `phrase.fillRange(0, phrase.length, 0)` and freed via
  /// `calloc.free(phrase)` after the call returns. The Rust side
  /// reads until NUL — the Dart side must NUL-terminate the buffer
  /// before the call. Task 8 (facade) will introduce a `SecretBuffer`
  /// newtype that closes this foot-gun.
  ///
  /// **Network + addressType scalars:** pass `FfiNetwork.testnet.code`
  /// and `FfiAddressType.<variant>.code` (see `ffi_enums.dart`).
  static final Pointer<Void> Function(
    Pointer<Utf8> phrase,
    int network,
    int addressType,
  ) walletFromMnemonic =
      _lib.lookupFunction<_WalletFromMnemonicC, _WalletFromMnemonicDart>(
          'wallet_from_mnemonic');

  /// Drops a `Wallet` handle. Null is a no-op.
  static final void Function(Pointer<Void> handle) walletFree =
      _lib.lookupFunction<_WalletFreeC, _WalletFreeDart>('wallet_free');

  /// Syncs the wallet against Esplora (pulls UTXOs + chain tip).
  static final int Function(
    Pointer<Void> rt,
    Pointer<Void> walletHandle,
    Pointer<Void> esploraHandle,
  ) walletSync =
      _lib.lookupFunction<_WalletSyncC, _WalletSyncDart>('wallet_sync');

  /// Returns confirmed balance in satoshis via `outBalance`.
  static final int Function(
    Pointer<Void> rt,
    Pointer<Void> walletHandle,
    Pointer<Void> esploraHandle,
    Pointer<Uint64> outBalance,
  ) walletBalance = _lib.lookupFunction<_WalletBalanceC, _WalletBalanceDart>(
    'wallet_balance',
  );

  /// Sends satoshis to a recipient. Returns a NUL-terminated txid hex
  /// string; caller frees via [walletSendFree].
  static final Pointer<Utf8> Function(
    Pointer<Void> rt,
    Pointer<Void> walletHandle,
    Pointer<Void> esploraHandle,
    Pointer<Utf8> recipient,
    int amountSat,
    int feeRateSatPerVb,
  ) walletSend =
      _lib.lookupFunction<_WalletSendC, _WalletSendDart>('wallet_send');

  /// Frees a txid string returned by [walletSend].
  static final void Function(Pointer<Utf8> ptr) walletSendFree =
      _lib.lookupFunction<_WalletSendFreeC, _WalletSendFreeDart>(
    'wallet_send_free',
  );

  /// Returns all txids in the wallet. Writes a heap-allocated array
  /// of NUL-terminated txid hex strings to `outArr` and the count to
  /// `outCount`. Caller frees via [walletTxidsArrayFree].
  ///
  /// **OutParams:**
  /// - `outCount`: caller allocates a `Pointer<UintPtr>` via
  ///   `calloc<UintPtr>()` and reads the count from `outCount.value`.
  /// - `outArr`: caller allocates a `Pointer<Pointer<Utf8>>` slot via
  ///   `calloc<Pointer<Utf8>>()`. After the call, the slot holds the
  ///   array pointer (read via `slot.value`). Free the array with
  ///   [walletTxidsArrayFree] (passing the array pointer, not the slot).
  ///   The caller-supplied count is ignored by the free — the
  ///   canonical count lives in the heap header.
  static final int Function(
    Pointer<Void> walletHandle,
    Pointer<UintPtr> outCount,
    Pointer<Pointer<Utf8>> outArr,
  ) walletTxids =
      _lib.lookupFunction<_WalletTxidsC, _WalletTxidsDart>('wallet_txids');

  /// Frees the array returned by [walletTxids]. The `count` argument
  /// is ignored; canonical count lives in the heap header (L40).
  static final void Function(Pointer<Utf8> arr, int count)
      walletTxidsArrayFree =
      _lib.lookupFunction<_WalletTxidsArrayFreeC, _WalletTxidsArrayFreeDart>(
    'wallet_txids_array_free',
  );

  /// Peeks a batch of addresses for the given keychain kind. Writes
  /// a heap-allocated array of NUL-terminated address strings to
  /// `outArr` and the count to `outCount`. Caller frees via
  /// [walletPeekAddressesArrayFree].
  ///
  /// **OutParams:** same pattern as [walletTxids].
  ///
  /// **Keychain-kind scalar:** pass `FfiKeychainKind.<variant>.code`
  /// (see `ffi_enums.dart`).
  static final int Function(
    Pointer<Void> walletHandle,
    int kind,
    int count,
    Pointer<UintPtr> outCount,
    Pointer<Pointer<Utf8>> outArr,
  ) walletPeekAddresses =
      _lib.lookupFunction<_WalletPeekAddressesC, _WalletPeekAddressesDart>(
          'wallet_peek_addresses');

  /// Frees the array returned by [walletPeekAddresses]. L40 pattern.
  static final void Function(Pointer<Utf8> arr, int count)
      walletPeekAddressesArrayFree = _lib.lookupFunction<
              _WalletPeekAddressesArrayFreeC,
              _WalletPeekAddressesArrayFreeDart>(
          'wallet_peek_addresses_array_free');
}
