import 'package:go_router/go_router.dart';

import '../features/home_shell.dart';
import '../features/wallet_create/wallet_create_screen.dart';
import '../features/wallet_detail/wallet_detail_screen.dart';
import '../features/wallet_import/wallet_import_screen.dart';
import '../features/wallet_list/wallet_list_screen.dart';
import '../features/wallet_send/send_screen.dart';
import '../features/wallet_transactions/transactions_screen.dart';
import '../features/settings/settings_screen.dart';

/// Routes per spec §5.2. Testnet is the default landing per L29
/// (live testnet is operator-driven, not CI; mainnet opt-in via
/// Settings, not the default).
///
/// Path-param validation: the per-screen builders below pull
/// `network` / `walletId` via `s.pathParameters[...]!`. v0.1 uses the
/// bang assertion because the route hierarchy guarantees the param
/// is present when the builder fires. v0.2 will swap to a `Network`
/// enum (cross-cutting refactor across Tasks 13/15/16) plus
/// `redirect:` callbacks validating against an allowlist — Task 21
/// will introduce mnemonic-access guards once SendScreen reads the
/// walletId path param.
GoRouter appRouter() {
  return GoRouter(
    initialLocation: '/wallets/testnet',
    routes: [
      ShellRoute(
        builder: (context, state, child) => HomeShell(child: child),
        routes: [
          GoRoute(path: '/', redirect: (_, __) => '/wallets/testnet'),
          GoRoute(
            path: '/wallets/:network',
            builder: (c, s) {
              final network = s.pathParameters['network']!;
              return WalletListScreen(network: network);
            },
            routes: [
              GoRoute(
                path: 'new',
                builder: (c, s) {
                  final network = s.pathParameters['network']!;
                  return WalletCreateScreen(network: network);
                },
              ),
              GoRoute(
                path: 'import',
                builder: (c, s) {
                  final network = s.pathParameters['network']!;
                  return WalletImportScreen(network: network);
                },
              ),
              GoRoute(
                path: ':walletId',
                builder: (c, s) {
                  final network = s.pathParameters['network']!;
                  final walletId = s.pathParameters['walletId']!;
                  return WalletDetailScreen(
                    network: network,
                    walletId: walletId,
                  );
                },
                routes: [
                  GoRoute(
                    path: 'send',
                    builder: (c, s) {
                      final network = s.pathParameters['network']!;
                      final walletId = s.pathParameters['walletId']!;
                      return SendScreen(
                        network: network,
                        walletId: walletId,
                      );
                    },
                  ),
                  GoRoute(
                    path: 'transactions',
                    builder: (c, s) {
                      final network = s.pathParameters['network']!;
                      final walletId = s.pathParameters['walletId']!;
                      return TransactionsScreen(
                        network: network,
                        walletId: walletId,
                      );
                    },
                  ),
                ],
              ),
            ],
          ),
          GoRoute(
            path: '/settings',
            builder: (c, s) => const SettingsScreen(),
          ),
        ],
      ),
    ],
  );
}
