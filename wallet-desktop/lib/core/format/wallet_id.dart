/// Display formatting for wallet identifiers.
///
/// `btc` wallet ids are 32+ char hex/sha256 fingerprints — full-display
/// is a shoulder-surfing / screen-recording hygiene concern. Every
/// wallet-area screen (Task 17 list, Task 18 detail, Task 20 send,
/// Task 22 transactions) renders an id in monospace, so the truncation
/// rule belongs in a shared module.
///
/// **v0.2 follow-up**: when `WalletId` becomes a value type, drop this
/// helper — the value class's own `toShortString()` should own it.
library;

/// Renders [id] as a legibly-truncated monospace string. Shows the
/// full id for <= 12 chars, otherwise `first4…last4`.
///
/// Examples:
/// - `'wlt-abc'` (7 chars) -> `'wlt-abc'`
/// - `'abcdef0123456789abcdef0123456789'` (32 chars) -> `'abcd…6789'`
String formatWalletId(String id) {
  if (id.length <= 12) return id;
  return '${id.substring(0, 4)}…${id.substring(id.length - 4)}';
}
