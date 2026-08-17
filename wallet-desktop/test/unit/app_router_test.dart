import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:wallet_desktop/routing/app_router.dart';

void main() {
  test(
    'routes include wallet list, create, import, detail, send, '
    'transactions, settings',
    () {
      final paths = <String>[];
      for (final r in appRouter().configuration.routes) {
        _collectPaths(r, '', paths);
      }
      expect(
        paths,
        containsAll([
          '/',
          '/wallets/:network',
          '/wallets/:network/new',
          '/wallets/:network/import',
          '/wallets/:network/:walletId',
          '/wallets/:network/:walletId/send',
          '/wallets/:network/:walletId/transactions',
          '/settings',
        ]),
      );
    },
  );
}

/// Walk the route tree, joining parent + child paths. Only GoRoute
/// carries a path; ShellRoute (parent of HomeShell) is a structural
/// container that passes the parent prefix through to its children.
void _collectPaths(RouteBase route, String parent, List<String> out) {
  var full = parent;
  if (route is GoRoute) {
    final p = route.path;
    full = p.startsWith('/') ? p : '$parent/$p';
    out.add(full);
  }
  for (final c in route.routes) {
    _collectPaths(c, full, out);
  }
}
