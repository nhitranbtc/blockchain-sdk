// Task 9 (#215) — typed `FfiException` hierarchy translating the
// stable C ABI error codes from `bitcoin-wallet-core` (see
// `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/error.rs`)
// into Dart exception classes.
//
// **L12 CRITICAL #1 closure.** Replaces the generic `Exception`
// placeholder in `WalletCore._ffiError()` (Task 8). UI screens
// (Tasks 10-16) match against `FfiException.kind` to render
// user-facing messages instead of dumping raw codes.
//
// **L12 CRITICAL #2 closure (Dart-side only — see Issue #242).**
// The Rust side `sanitize_for_ffi` does NOT redact BIP-39 word
// sequences or 64-char hex hashes (only replaces NUL bytes). The
// Rust-side redaction claim in earlier versions of this comment was
// false (L12 review R-H1).
//
// The single layer of defense on the Dart side is:
// (a) [FfiException.toString] never includes the `messageForDebug`
//     field — the field is preserved on the object for debugging
//     but excluded from any string the user might log, print, or
//     send to a crash reporter;
// (b) callers MUST scrub via `BtcLogFilter.redact()` (or equivalent)
//     before any logger / Sentry / Slack path.
//
// **Sealed hierarchy.** `FfiException` is a single concrete class
// with a `kind` discriminator rather than a sealed class tree —
// Dart's `sealed` keyword requires separate subclasses per case,
// which adds boilerplate without value when callers always switch
// on `kind`. The enum is the discriminator.

/// Kinds of failures that can surface from the FFI boundary. Each
/// value maps 1:1 to a `FfiError` variant in
/// `bitcoin-wallet-core/src/ffi/error.rs`, with two additions:
///
/// - [panic] (`FfiError::Panic = -100`): Rust panic caught by
///   `ffi_catch_unwind`. Never originates from
///   `bitcoin_wallet_core::Error`. **Operational meaning:** bug;
///   should be reported via Sentry. UI: generic error toast.
/// - [unknown] (`FfiError::Unknown = -127`): catch-all for future
///   `Error` variants added upstream while `Error` remains
///   `#[non_exhaustive]`. **Operational meaning:** benign; wallet
///   needs update. UI: "wallet needs update" toast.
///
/// The numeric code → kind mapping is frozen by `FfiError` (Task 2).
/// Adding a new variant upstream is a deliberate, reviewed ABI
/// change; renumbering is forbidden.
enum FfiErrorKind {
  // --- Key / mnemonic / derivation ---
  /// BIP-39 mnemonic invalid (bad word count, invalid checksum).
  /// UI: "Invalid recovery phrase" — user-fixable (re-enter).
  invalidMnemonic,

  /// BIP-32 derivation path malformed.
  /// UI: "Invalid derivation path" — user-fixable.
  invalidDerivationPath,

  /// Address derivation failed (bdk keystore rejected the keychain).
  /// UI: "Address derivation failed" — fatal (wallet bug).
  addressDerivation,

  /// Bitcoin script build failed (sighash / witness / P2SH wrap).
  /// UI: "Transaction script invalid" — fatal.
  scriptBuild,

  // --- Network / chain backend ---
  /// Generic network failure (reqwest, DNS, TLS).
  /// UI: "Network error" — retryable.
  network,

  /// Esplora HTTP/RPC failure.
  /// UI: "Block explorer error" — retryable.
  esplora,

  /// Electrum protocol failure (unused in v0.1 but reserved per F43).
  /// UI: "Electrum server error" — retryable.
  electrum,

  // --- Transaction lifecycle ---
  /// Insufficient confirmed balance for the requested send + fee.
  /// UI: "Insufficient funds" — user-fixable.
  insufficientFunds,

  /// bdk tx build failed (no inputs, dust, etc.).
  /// UI: "Transaction build failed" — fatal.
  txBuild,

  /// PSBT signing failed.
  /// UI: "Signing failed" — fatal.
  sign,

  /// PSBT parse/serialize failed.
  /// UI: "Transaction format invalid" — fatal.
  psbt,

  // --- Storage / wallet persistence ---
  /// Generic filesystem IO failure on wallet blob / DB.
  /// UI: "Disk error" — fatal.
  storage,

  /// BDK wallet not initialized (sync not called yet).
  /// UI: "Wallet not synced" — retryable (call sync).
  notInitialized,

  /// Encryption / decryption primitive failure (Argon2id / AES-GCM).
  /// UI: "Encryption error" — fatal.
  encryption,

  /// MnemonicCipherBlob malformed (wrong format, tamper, wrong password).
  /// UI: "Wallet file corrupted" — fatal.
  mnemonicCipher,

  /// WalletStore: blob missing, wrong-password, wrong-network AAD, or
  /// corrupt. Single indistinguishable message for N2 oracle-attack
  /// mitigation. UI: "Cannot unlock wallet" — user-fixable (re-enter
  /// password).
  walletStore,

  // --- Upstream library errors ---
  /// `bitcoin` consensus encode/decode failure.
  /// UI: "Bitcoin protocol error" — fatal.
  bitcoin,

  /// `bdk_wallet` internal error (descriptor / sync / persistence).
  /// UI: "Wallet library error" — fatal.
  bdk,

  /// `std::io::Error` from filesystem ops.
  /// UI: "I/O error" — fatal.
  io,

  // --- Per-protocol variants ---
  /// BIP-137 message sign/verify protocol error.
  /// UI: "Message signing failed" — fatal.
  bip137,

  /// SPKI pin parse / validation error.
  /// UI: "TLS pin invalid" — user-fixable (re-enter pin).
  spkiPin,

  // --- FFI-layer sentinels ---
  /// Rust panic in FFI body (catch_unwind triggered). Bug. Report.
  /// UI: generic error toast + Sentry report.
  panic,

  /// Catch-all for future `bitcoin_wallet_core::Error` variants
  /// (`#[non_exhaustive]` allows silent addition). Benign.
  /// UI: "Wallet needs update" — contact-support.
  unknown,
}

/// Typed exception thrown by [WalletCore] when an FFI call returns a
/// non-zero [code]. Carries the operation name, the numeric code
/// (stable C ABI), the discriminator [kind], and an optional sanitized
/// message and root cause.
///
/// Implements [Exception] so `on Exception catch (e)` blocks in UI
/// code match.
final class FfiException implements Exception {
  /// Build an exception from a raw FFI return code. The Ok code (0)
  /// is forbidden — callers MUST NOT throw on success; an assertion
  /// fires in debug builds to catch the mistake. Unknown codes fall
  /// back to [FfiErrorKind.unknown].
  ///
  /// Const factory impossible: `_kindFromCode` is runtime-mapped.
  factory FfiException.fromCode({
    required int code,
    required String op,
    String? messageForDebug,
    Object? cause,
  }) {
    assert(code != 0,
        'FfiException.fromCode called with Ok code 0; callers must not throw on success');
    final kind = _kindFromCode(code);
    return FfiException._(
      kind: kind,
      code: code,
      op: op,
      messageForDebug: messageForDebug,
      cause: cause,
    );
  }

  const FfiException._({
    required this.kind,
    required this.code,
    required this.op,
    this.messageForDebug,
    this.cause,
  });

  /// Failure category. UI code switches on this to pick a user-facing
  /// message.
  final FfiErrorKind kind;

  /// Raw FFI return code. Stable across versions per the `FfiError`
  /// ABI contract.
  final int code;

  /// Name of the FFI operation that failed (e.g. `wallet_create`,
  /// `esplora_fee_estimate`). For diagnostics only — never shown
  /// to the user.
  final String op;

  /// Optional message from the Rust side. **Not sanitized on the Rust
  /// side** — `sanitize_for_ffi` only replaces NUL bytes (see Issue #242,
  /// 2026-08-21). May contain mnemonic or password bytes verbatim.
  ///
  /// **NEVER include in [toString] (L12 CRITICAL #2) — and NEVER route
  /// through a logger/crash-reporter without scrubbing first.** The
  /// renamed field name (`messageForDebug`) is the type-system signal:
  /// this is debug-only text, NOT user-safe.
  ///
  /// Callers MUST route this through `BtcLogFilter.redact()` (or an
  /// equivalent scrubber) before any log/Sentry/Slack message.
  final String? messageForDebug;

  /// Optional underlying cause (e.g. an `ArgumentError` from
  /// `calloc` failure on the Dart side).
  ///
  /// **Callers MUST NOT set `cause` to an object whose `toString()`
  /// may contain plaintext secrets.** If a Dart-side precondition
  /// fails with a sensitive payload (e.g., a hex digest of a key),
  /// the caller is responsible for scrubbing before chaining.
  final Object? cause;

  /// Renders the exception for logs and crash reporters.
  ///
  /// **Redaction contract:** never includes [messageForDebug]. The Rust
  /// side does NOT sanitize the message (Issue #242, 2026-08-21) — it
  /// may contain mnemonic or password bytes verbatim. Includes `op`,
  /// `kind.name`, and `code` only.
  @override
  String toString() => 'FfiException(op: $op, kind: ${kind.name}, code: $code)';

  /// Maps the stable C ABI code to a [FfiErrorKind]. Unknown codes
  /// (or the Ok code, which callers should not throw on) fall back
  /// to [FfiErrorKind.unknown].
  static FfiErrorKind _kindFromCode(int code) {
    switch (code) {
      case -1:
        return FfiErrorKind.invalidMnemonic;
      case -2:
        return FfiErrorKind.invalidDerivationPath;
      case -3:
        return FfiErrorKind.addressDerivation;
      case -4:
        return FfiErrorKind.scriptBuild;
      case -10:
        return FfiErrorKind.network;
      case -11:
        return FfiErrorKind.esplora;
      case -12:
        return FfiErrorKind.electrum;
      case -20:
        return FfiErrorKind.insufficientFunds;
      case -21:
        return FfiErrorKind.txBuild;
      case -22:
        return FfiErrorKind.sign;
      case -23:
        return FfiErrorKind.psbt;
      case -30:
        return FfiErrorKind.storage;
      case -31:
        return FfiErrorKind.notInitialized;
      case -32:
        return FfiErrorKind.encryption;
      case -33:
        return FfiErrorKind.mnemonicCipher;
      case -34:
        return FfiErrorKind.walletStore;
      case -40:
        return FfiErrorKind.bitcoin;
      case -41:
        return FfiErrorKind.bdk;
      case -42:
        return FfiErrorKind.io;
      case -50:
        return FfiErrorKind.bip137;
      case -51:
        return FfiErrorKind.spkiPin;
      case -100:
        return FfiErrorKind.panic;
      case -127:
        return FfiErrorKind.unknown;
      default:
        return FfiErrorKind.unknown;
    }
  }
}

/// User-facing copy for an [FfiException]. L12 review MED #3 (Task 10):
/// extracted from the per-variant dartdoc on [FfiErrorKind] so every UI
/// consumer (Tasks 11-16 screens + this Task 10 list screen) can
/// render kind-mapped messages without re-implementing the table.
///
/// **Recovery tier (UI hint, not a control flow contract):**
/// - `userFixable` — operator can resolve (e.g. wrong password, bad
///   SPKI pin, wrong word count). Surface a retry / re-enter dialog.
/// - `retryable` — transient (network, sync not yet run). Surface a
///   retry button.
/// - `fatal` — operator can't resolve; surface a "contact support"
///   banner with the kind code for triage.
///
/// **NEVER** include the exception's `messageForDebug` field in the
/// returned copy — L12 CRITICAL #2.
String userMessageForFfiException(FfiException e) {
  switch (e.kind) {
    case FfiErrorKind.invalidMnemonic:
      return 'Invalid recovery phrase — please re-enter.';
    case FfiErrorKind.invalidDerivationPath:
      return 'Invalid derivation path — please re-enter.';
    case FfiErrorKind.addressDerivation:
      return 'Address derivation failed.';
    case FfiErrorKind.scriptBuild:
      return 'Transaction script invalid.';
    case FfiErrorKind.network:
      return 'Network error — please try again.';
    case FfiErrorKind.esplora:
      return 'Block explorer error — please try again.';
    case FfiErrorKind.electrum:
      return 'Electrum server error — please try again.';
    case FfiErrorKind.insufficientFunds:
      return 'Insufficient funds for this transaction.';
    case FfiErrorKind.txBuild:
      return 'Transaction build failed.';
    case FfiErrorKind.sign:
      return 'Signing failed.';
    case FfiErrorKind.psbt:
      return 'Transaction format invalid.';
    case FfiErrorKind.storage:
      return 'Disk error — cannot read wallet storage.';
    case FfiErrorKind.notInitialized:
      return 'Wallet not synced — please sync first.';
    case FfiErrorKind.encryption:
      return 'Encryption error — cannot decrypt wallet.';
    case FfiErrorKind.mnemonicCipher:
      return 'Wallet file is corrupted.';
    case FfiErrorKind.walletStore:
      // N2 oracle mitigation: the Rust side intentionally returns one
      // indistinguishable message for "wrong password" / "wrong blob" /
      // "wrong network". UI copy follows suit.
      return 'Cannot unlock wallet — check password.';
    case FfiErrorKind.bitcoin:
      return 'Bitcoin protocol error.';
    case FfiErrorKind.bdk:
      return 'Wallet library error.';
    case FfiErrorKind.io:
      return 'I/O error.';
    case FfiErrorKind.bip137:
      return 'Message signing failed.';
    case FfiErrorKind.spkiPin:
      return 'TLS pin invalid — please re-enter.';
    case FfiErrorKind.panic:
      return 'Internal error — please try again.';
    case FfiErrorKind.unknown:
      return 'Wallet needs update — please contact support.';
  }
}
