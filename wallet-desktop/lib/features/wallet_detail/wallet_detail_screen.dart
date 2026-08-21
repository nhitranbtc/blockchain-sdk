// Task 13 (#219) — `WalletDetailScreen` migrated from
// `btcInvokerProvider` + `withPasswordFile` (Task 5/6) +
// `BtcCommand.walletShow` to `walletCoreProvider` +
// `walletCore.showWallet(network, walletId, password: SecretBuffer,
// baseDir)`. Returns `WalletDetail { id, network, addressType,
// firstAddress, balance }` (collapsed `Balance` per plan deviation
// #3 — single `confirmedSat`; `utxos` dropped per deviation #5).
//
// **Unlock flow** (Story 11):
// - User types password + taps Unlock → `core.showWallet(...)` runs
//   with the password wrapped in a `SecretBuffer` (auto-disposed in
//   the FFI facade's `finally` block). The cleartext password NEVER
//   lives in a Dart `String` field on the State class (L12 CRITICAL
//   #2 mirror — `SecretBuffer` RAII is the typed handle).
// - On success: detail parsed → `walletSessionProvider(walletId)
//   .notifier.unlockWithDetail(d)`. The empty-string mnemonic
//   sentinel is constructed inside `unlockWithDetail` (Task 20 L12
//   type-design HIGH) so the convention doesn't leak to callers.
//
// **v0.2.0 firstAddress plan deviation #4:** Rust `wallet_show`
// returns an empty `first_address` because `peek_addresses` requires
// bdk sync (deferred to v0.2.1). The detail screen hides
// `AddressChip` when `firstAddress.isEmpty` (graceful UX).
//
// **L12 collapse (HIGH #1 mirror):** wrong-password / not-found /
// wrong-AAD / corrupt-blob all surface as
// `FfiException(kind: FfiErrorKind.walletStore)` via
// `userMessageForFfiException(e)` (Task 10 MED #3 helper) — no
// enumeration signal for a network observer.
//
// **v0.2 deferred:** app-lifecycle auto-lock on backgrounding +
// `Network` enum + `WalletId` value class (cross-cutting refactor).

import 'dart:developer' as developer;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../core/btc/models/wallet_detail.dart';
import '../../core/ffi/ffi_enums.dart';
import '../../core/ffi/ffi_exception.dart';
import '../../core/ffi/secret_buffer.dart';
import '../../core/format/wallet_id.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../providers/app_paths_provider.dart';
import '../../providers/wallet_core_provider.dart';
import '../../providers/wallet_providers.dart';
import '../../routing/wallet_routes.dart';
import '../../widgets/balance_card.dart';
import '../../widgets/password_field.dart';

class WalletDetailScreen extends ConsumerStatefulWidget {
  const WalletDetailScreen({
    super.key,
    required this.network,
    required this.walletId,
  });
  final String network;
  final String walletId;

  @override
  ConsumerState<WalletDetailScreen> createState() => _WalletDetailScreenState();
}

class _WalletDetailScreenState extends ConsumerState<WalletDetailScreen> {
  String _password = '';
  bool _running = false;
  String? _error;

  @override
  void dispose() {
    // Defense-in-depth (Task 20 LOW): zero screen-side password on
    // unmount. Real zeroization is FFI Uint8List + Finalizable
    // (v0.2 backlog).
    _password = '';
    super.dispose();
  }

  Future<void> _unlock() async {
    if (_password.isEmpty) return;
    // L12 flutter HIGH (Task 13): the FFI surface only supports
    // testnet today (mirrors `WalletsListNotifier._networkFromString`
    // assert guard at `wallet_providers.dart:78-84`). A router
    // refactor that passes `'mainnet'` (or any new value) would
    // silently route to the testnet blob dir + render a misleading
    // "wrong password" error. Assert loudly so the operator can
    // extend the FFI's `parse_network` alongside the UI.
    assert(
      widget.network == 'testnet',
      'WalletDetailScreen._unlock only supports testnet today; '
      'got: ${widget.network}. v0.2: extend when FFI parse_network grows.',
    );
    // Capture the routing identity BEFORE the async suspension
    // (L12 type-design Task 20 MEDIUM). If the parent rebuilds the
    // screen with a different `walletId` mid-await, the FFI call
    // runs against wallet A but `walletSessionProvider(widget.walletId)`
    // lands in wallet B's family — the detail would be mis-attributed.
    final walletId = widget.walletId;
    final network = widget.network;
    setState(() {
      _running = true;
      _error = null;
    });
    try {
      final core = ref.read(walletCoreProvider);
      final appPaths = await ref.read(appPathsProvider.future);
      // FFI call: facade owns the SecretBuffer lifetime (auto-dispose
      // in `finally`). Returns `WalletDetail` (no mnemonic returned —
      // the FFI never exposes cleartext; matches the legacy
      // `btc wallet show --json` which also didn't return the phrase).
      final detail = core.showWallet(
        network: FfiNetwork.testnet,
        walletId: walletId,
        password: SecretBuffer.fromUtf8(_password),
        baseDir: appPaths.walletDataDir.path,
      );
      if (!mounted) return;
      // Re-assert identity: if widget was rebuilt with a different
      // walletId mid-await, do NOT populate the wrong family.
      if (widget.walletId != walletId || widget.network != network) return;
      // Force-clear screen-side password before unlocking the session.
      // No `setState` wrap (Task 11 flutter M2 fix): the field is
      // never read by `build`; the password field's own controller
      // owns the typed-text lifecycle.
      _password = '';
      // Populate the session via the dedicated read-only factory
      // (Task 20 L12 type-design HIGH). The empty-string mnemonic
      // sentinel is constructed inside `unlockWithDetail`.
      ref
          .read(walletSessionProvider(walletId).notifier)
          .unlockWithDetail(detail);
    } on FfiException catch (e) {
      // Kind-mapped user copy (Task 10 MED #3). For `walletStore`:
      // 'Could not unlock — check the password and try again.'
      if (mounted) setState(() => _error = userMessageForFfiException(e));
    } catch (e, st) {
      // Defense-in-depth: dart:developer.log bypasses package:logging.
      // The `FfiException.toString()` contract excludes
      // `messageForDebug` so the redacted form is safe by
      // construction for typed errors; non-typed `catch` clauses
      // re-redact via `BtcLogFilter` for safety.
      const filter = BtcLogFilter();
      developer.log(
        'wallet_detail unlock failed',
        name: 'WalletDetailScreen',
        error: filter.redact(e.toString()),
        stackTrace: st,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Could not unlock wallet.')),
        );
      }
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  void _lock() {
    ref.read(walletSessionProvider(widget.walletId).notifier).lock();
    setState(() {
      _password = '';
      _error = null;
    });
  }

  @override
  Widget build(BuildContext context) {
    // Defense-in-depth (Task 20 LOW): router-level `redirect:` is
    // the canonical walletId validator, but a parent that bypasses
    // the router (or a future refactor that swaps the redirect for
    // one that doesn't run on initial-load) could surface a
    // path-injection-shaped id. Assert here so the failure mode is
    // a visible runtime error rather than a silently-truncated
    // format.
    assert(
      WalletRoutes.isValidWalletIdSegment(widget.walletId),
      'invalid walletId segment: ${widget.walletId}',
    );
    final session = ref.watch(walletSessionProvider(widget.walletId));
    if (session == null) return _buildUnlockForm(context);
    final detail = session.detail;
    if (detail == null) return _buildUnlockForm(context);
    return _buildUnlockedView(context, detail);
  }

  Widget _buildUnlockForm(BuildContext context) {
    final error = _error;
    return Scaffold(
      appBar: AppBar(
        title: Text('Unlock ${formatWalletId(widget.walletId)}'),
      ),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            PasswordField(
              onChanged: (v) => _password = v,
              onSubmitted: (_) => _unlock(),
            ),
            const SizedBox(height: 16),
            if (error != null) ...[
              Text(error),
              const SizedBox(height: 8),
            ],
            FilledButton(
              key: const Key('wallet_detail_unlock'),
              onPressed: _running ? null : _unlock,
              child: _running ? const Text('Unlocking…') : const Text('Unlock'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildUnlockedView(BuildContext context, WalletDetail d) {
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      appBar: AppBar(
        // Full UUID in title (no truncation) — operator needs the
        // exact id for support / cross-referencing. `formatWalletId`
        // is the list-row helper (shoulder-surf hygiene on the
        // overview screen); the detail screen is the focused
        // context where the user explicitly unlocked.
        title: SelectableText(
          d.id,
          maxLines: 1,
          style: textTheme.titleMedium?.copyWith(fontFamily: 'monospace'),
        ),
        actions: [
          IconButton(
            key: const Key('wallet_detail_copy_id'),
            icon: const Icon(Icons.copy),
            tooltip: 'Copy wallet ID',
            onPressed: () async {
              await Clipboard.setData(ClipboardData(text: d.id));
              if (!context.mounted) return;
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('Wallet ID copied')),
              );
            },
          ),
          IconButton(
            key: const Key('wallet_detail_send'),
            icon: const Icon(Icons.send),
            tooltip: 'Send',
            onPressed: () => context.go(
              WalletRoutes.send(widget.network, widget.walletId),
            ),
          ),
          IconButton(
            key: const Key('wallet_detail_history'),
            icon: const Icon(Icons.history),
            tooltip: 'Transactions',
            onPressed: () => context.go(
              WalletRoutes.transactions(widget.network, widget.walletId),
            ),
          ),
          IconButton(
            key: const Key('wallet_detail_lock'),
            icon: const Icon(Icons.lock),
            tooltip: 'Lock',
            onPressed: _lock,
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            BalanceCard(balance: d.balance),
            const SizedBox(height: 16),
            Text(
              'Network: ${d.network}',
              style: textTheme.bodyMedium,
            ),
            Text(
              'Type: ${d.addressType.isEmpty ? '(unknown — sync required)' : d.addressType}',
              style: textTheme.bodyMedium,
            ),
            const SizedBox(height: 16),
            // Plan deviation #4: firstAddress is empty in v0.2.0
            // (peek_addresses requires bdk sync — deferred to v0.2.1).
            // Hide the chip when empty; v0.2.1 wires the sync path.
            // L12 flutter MED: the previous "(sync required — open
            // SendScreen)" was misleading — opening SendScreen does
            // not trigger sync (Tasks 14+15 are still pending). The
            // new copy accurately reflects the v0.2.0 state.
            if (d.firstAddress.isNotEmpty)
              Row(
                children: [
                  Expanded(
                    child: SelectableText(
                      d.firstAddress,
                      style: textTheme.bodyMedium
                          ?.copyWith(fontFamily: 'monospace'),
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.copy),
                    tooltip: 'Copy address',
                    onPressed: () async {
                      await Clipboard.setData(
                        ClipboardData(text: d.firstAddress),
                      );
                      if (!context.mounted) return;
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(content: Text('Address copied')),
                      );
                    },
                  ),
                ],
              )
            else
              Text(
                'First address: (sync pending — v0.2.1)',
                style: textTheme.bodySmall,
              ),
            const SizedBox(height: 8),
            // Explorer + Faucet buttons live OUTSIDE the address
            // branch — always reachable so the user can browse /
            // fund a wallet even while sync is pending (v0.2.1).
            // URLs degrade gracefully: address-specific when known,
            // generic testnet home when not.
            Row(
              children: [
                TextButton.icon(
                  icon: const Icon(Icons.open_in_new),
                  label: const Text('Explorer'),
                  onPressed: () async {
                    final addr = d.firstAddress;
                    final url = addr.isEmpty
                        ? 'https://blockstream.info/testnet'
                        : 'https://blockstream.info/testnet/address/$addr';
                    await Process.start('xdg-open', [url]);
                  },
                ),
                const SizedBox(width: 8),
                TextButton.icon(
                  icon: const Icon(Icons.water_drop),
                  label: const Text('Faucet'),
                  onPressed: () async {
                    // mempool's testnet-faucet subdomain redirects
                    // to mempool.space root (dead since ~2024). Use
                    // coinfaucet.eu — accepts ?address= for prefilled
                    // tb1 receive, no account.
                    final addr = d.firstAddress;
                    final url = addr.isEmpty
                        ? 'https://coinfaucet.eu/en/btc-testnet/'
                        : 'https://coinfaucet.eu/en/btc-testnet/?address=$addr';
                    await Process.start('xdg-open', [url]);
                  },
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
