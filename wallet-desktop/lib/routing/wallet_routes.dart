/// Wallet-route path templates + segment builders.
///
/// **Single source of truth** for the wallet-area go_router paths
/// (`wallet_list_screen`, `wallet_create_screen`, `wallet_import_screen`,
/// `wallet_detail_screen`, `send_screen`, `transactions_screen`).
/// Both the screens (for `context.go(...)` navigation) and `app_router.dart`
/// (for `GoRoute(path:)`) reference these constants so a rename is a
/// compile error rather than a runtime 404.
///
/// **v0.2 deferred** (Tasks 13/15/16 reviews): replace `String network`
/// here with the planned `Network` enum + a typed `WalletId` value class;
/// add a router-level `redirect:` callback that validates `:network` /
/// `:walletId` against those types. The current `:walletId` parameter is
/// validated at the call site by [isValidWalletIdSegment] as the last line
/// of defence until v0.2 lands.
class WalletRoutes {
  WalletRoutes._();

  /// App shell — `/wallets/<network>`. Segment builder for the
  /// wallet-list landing.
  static String wallets(String network) => '/wallets/$network';

  /// `/wallets/<network>/new` — wallet create screen.
  static String create(String network) => '/wallets/$network/new';

  /// `/wallets/<network>/import` — wallet import screen.
  static String import(String network) => '/wallets/$network/import';

  /// `/wallets/<network>/<walletId>` — wallet detail screen.
  static String detail(String network, String walletId) =>
      '/wallets/$network/$walletId';

  /// `/wallets/<network>/<walletId>/send` — send screen.
  static String send(String network, String walletId) =>
      '/wallets/$network/$walletId/send';

  /// `/wallets/<network>/<walletId>/transactions` — tx list screen.
  static String transactions(String network, String walletId) =>
      '/wallets/$network/$walletId/transactions';

  /// Allowlist for the `:walletId` path segment. Mirrors the regex the
  /// CLI uses for wallet ids (`[A-Za-z0-9_-]{1,64}`); anything that does
  /// not match is rejected before navigation can fire `context.go`,
  /// closing the path-injection footgun that security-auditor flagged
  /// (a CLI returning `id: 'new'` would otherwise hijack `context.go`
  /// into the create screen).
  static final RegExp walletIdSegment = RegExp(r'^[A-Za-z0-9_-]{1,64}$');

  /// True iff [s] is a safe wallet-id segment to interpolate into a
  /// route path. Empty string is rejected.
  static bool isValidWalletIdSegment(String s) => walletIdSegment.hasMatch(s);
}
