// Task 7 (#213) — typed enums for the small-byte scalar parameters
// used across the FFI surface.
//
// Per L12 review H3: the wallet_create / wallet_from_mnemonic /
// wallet_peek_addresses entry points take scalars (`u8` network,
// `u8` address_type, `u8` keychain_kind) that the Rust side parses
// into the corresponding domain enum. Dart `int` is 64-bit; passing
// `1000` becomes `232` silently (`1000 % 256`) and surfaces
// downstream as `FfiError::Unknown` — invisible to the type system.
//
// These typed enums encode the byte value at the language boundary
// without forcing the Dart side to carry the full domain model
// (Networks, AddressType, KeychainKind newtypes wrap the Rust enums
// and live in wallet-core). The plan for the v0.2 release is to
// replace these with the wallet-core newtypes once Task 8 wires the
// facade; for now `FfiNetwork.testnet.code` etc. is the L12-correct
// affordance.
//
// Byte values MUST match the Rust `parse_*` match arms in
// `wallet_ops.rs:70-87` and `bdk_extras.rs:100-122`. Drift here
// is a silent failure mode in production.

/// Network byte for the FFI surface.
///
/// Only `testnet` is wired via FFI today (per the `parse_network` arm
/// in `wallet_ops.rs:70-75` and `bdk_extras.rs:100-105`). `unknown`
/// is reserved for the error code returned by Rust on invalid input.
enum FfiNetwork {
  testnet(1),
  unknown(0);

  const FfiNetwork(this.code);

  /// Byte value sent across the FFI boundary. Range-checked at
  /// construction by the enum itself.
  final int code;
}

/// Address-type byte for the FFI surface.
///
/// Values mirror `parse_address_type` in `wallet_ops.rs:77-87` and
/// `bdk_extras.rs:107-114`. `unknown` is the byte that Rust rejects
/// with `FfiError::Unknown`.
enum FfiAddressType {
  nativeSegwit(0),
  nestedSegwit(1),
  taproot(2),
  unknown(255);

  const FfiAddressType(this.code);

  final int code;
}

/// Keychain-kind byte for `wallet_peek_addresses` (Task 5).
///
/// Values mirror `ffi_parse_keychain_kind` in `bdk_extras.rs:116-122`.
enum FfiKeychainKind {
  external(0),
  internal(1),
  unknown(255);

  const FfiKeychainKind(this.code);

  final int code;
}
