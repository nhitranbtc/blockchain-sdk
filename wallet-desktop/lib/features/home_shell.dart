import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

/// App shell with sidebar (NavigationRail) + content. The rail index
/// is derived from the current GoRouter location (so /settings
/// correctly highlights the Settings destination). When the user taps
/// "Wallets" we preserve their active network instead of resetting
/// to testnet.
class HomeShell extends StatelessWidget {
  const HomeShell({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final location = GoRouterState.of(context).uri.path;
    final selectedIndex = location.startsWith('/settings') ? 1 : 0;
    final activeNetwork =
        GoRouterState.of(context).pathParameters['network'] ?? 'testnet';

    return Scaffold(
      body: Row(
        children: [
          NavigationRail(
            selectedIndex: selectedIndex,
            labelType: NavigationRailLabelType.all,
            destinations: const [
              NavigationRailDestination(
                icon: Icon(Icons.account_balance_wallet),
                label: Text('Wallets'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.settings),
                label: Text('Settings'),
              ),
            ],
            onDestinationSelected: (i) {
              switch (i) {
                case 0:
                  context.go('/wallets/$activeNetwork');
                case 1:
                  context.go('/settings');
              }
            },
          ),
          const VerticalDivider(width: 1),
          Expanded(child: child),
        ],
      ),
    );
  }
}
