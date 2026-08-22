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
// **v0.2.x firstAddress (Issue #261 — deviance closed):** Rust
// `wallet_show` now derives the first External receive address
// offline via `Wallet::first_external_address_offline` (pure local
// crypto, no Esplora round-trip). `WalletDetail.firstAddress`
// carries a real `tb1…` 42-char string on every unlock. The chip +
// copy + Explorer + Faucet wiring renders unconditionally; the only
// remaining fallback is a defensive `'(unavailable — unlock failed)'`
// for an unexpected FFI failure.
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

import 'dart:ffi';

import '../../core/btc/models/wallet_detail.dart';
import '../../core/ffi/ffi_enums.dart';
import '../../core/ffi/ffi_exception.dart';
import '../../core/ffi/secret_buffer.dart';
import '../../core/format/wallet_id.dart';
import '../../core/logging/btc_log_filter.dart';
import '../../providers/app_paths_provider.dart';
import '../../providers/esplora_config_provider.dart';
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
  // Subscription to `esploraConfigProvider` that fires when Settings →
  // Save mutates the on-disk esplora.json. Used to surface a
  // "Re-unlock to refresh balance" snackbar — `wallet_show` only Esplora-
  // syncs at unlock, so a stale balance (cached at 0 from a previous bad
  // pin) won't update until the operator re-unlocks. Listener registered
  // in `initState`, cancelled in `dispose`. Tighter than ref.listen() in
  // build() (which re-subscribes per rebuild) and avoids caching the
  // cleartext password across unlocks.
  ProviderSubscription<AsyncValue<EsploraConfig>>? _esploraCfgSub;
  // Snapshot of the Esplora config at screen mount. Compared post-mount
  // against the current value to detect off-screen Settings → Save
  // changes (the listener subscribes here and cancels in dispose, so
  // changes while this route is unmounted would be silently missed).
  EsploraConfig? _initialEsploraCfg;

  @override
  void initState() {
    super.initState();
    _initialEsploraCfg = ref.read(esploraConfigProvider).value;
    // Post-mount check for off-screen config mutations: if Settings
    // was visited and saved a new config while this route was
    // unmounted, the listener's `fireImmediately: true` path sees
    // `prev == null` and skips — leaving an off-screen refresh
    // undetected. Snapshotted comparison catches that case.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final current = ref.read(esploraConfigProvider).value;
      if (current == null) return;
      final initial = _initialEsploraCfg;
      if (initial == null) return;
      if (_sameEsploraConfig(initial, current)) return;
      _onEsploraConfigChanged();
    });
    _esploraCfgSub = ref.listenManual<AsyncValue<EsploraConfig>>(
      esploraConfigProvider,
      (prev, next) {
        // `fireImmediately: true` triggers one initial callback
        // (`prev == null`) which we skip — no prior to compare against.
        if (prev == null) return;
        final prevCfg = prev.value;
        final nextCfg = next.value;
        if (nextCfg == null) return; // save() should always emit AsyncData
        if (prevCfg == null) return; // first non-initial state with no prior
        if (_sameEsploraConfig(prevCfg, nextCfg)) return;
        _onEsploraConfigChanged();
      },
      fireImmediately: true,
    );
  }

  @override
  void dispose() {
    _esploraCfgSub?.close();
    _esploraCfgSub = null;
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
      // Esplora config for the in-FFI sync (Issue #261 follow-up).
      // Rust syncs against this URL + SPKI pin and returns the
      // confirmed balance. Empty URL → Rust skips (legacy v0.2.0
      // behavior, `balance_sat: 0`).
      final esploraCfg = await ref.read(esploraConfigProvider.future);
      // FFI call: facade owns the SecretBuffer lifetime (auto-dispose
      // in `finally`). Returns `WalletDetail` (no mnemonic returned —
      // the FFI never exposes cleartext; matches the legacy
      // `btc wallet show --json` which also didn't return the phrase).
      final showResult = core.showWallet(
        network: FfiNetwork.testnet,
        walletId: walletId,
        password: SecretBuffer.fromUtf8(_password),
        baseDir: appPaths.walletDataDir.path,
        esploraUrl: esploraCfg.url,
        esploraSpkiPin: esploraCfg.spkiPin,
      );
      final detail = showResult.detail;
      // The wallet handle from `wallet_show` is freed on lock via the
      // session notifier — detail screen reads only `showResult.detail`
      // (no signing material lives in the Dart session; SendScreen
      // re-calls `walletShow` with the password when needed).
      // Explicit free here is a defensive null-check (the FFI always
      // populates the out param post-#261).
      if (showResult.walletHandle != nullptr) {
        core.walletLoadFree(showResult.walletHandle);
      }
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
            BalanceCard(
              balance: d.balance,
              // Issue #263 — thread the 3-way sync classification
              // from `wallet_show` FFI into the balance card. The
              // card renders a red banner + Retry for
              // `FfiSyncStatus.syncFailed` (previously silent —
              // operator couldn't distinguish empty wallet from
              // broken Esplora). Retry re-runs the unlock flow,
              // which re-invokes `wallet_show` (idempotent) and
              // refreshes the balance via fresh Esplora sync.
              // L12 review M2: gate Retry on `_running` to prevent
              // stacking dialogs from a double-tap.
              // L12 review C1: surface the Rust `set_last_error`
              // diagnostic in the banner so the operator can see
              // WHY sync failed (not just THAT it failed).
              syncStatus: d.syncStatus,
              lastError: d.lastError,
              onRetry: (d.syncStatus == FfiSyncStatus.syncFailed &&
                      !_running)
                  ? _showReUnlockDialog
                  : null,
            ),
            // Manual resync: triggers the same re-unlock dialog as the
            // off-screen Esplora config-change snackbar. Useful when
            // balance drifted from new blocks (since `wallet_show` only
            // syncs at unlock) or as a no-config-change sanity check.
            // Gated on `_running` to avoid stacking dialogs while a
            // concurrent unlock is in flight.
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton.icon(
                key: const Key('wallet_detail_resync'),
                icon: const Icon(Icons.refresh),
                label: const Text('Resync balance'),
                onPressed: _running ? null : _showReUnlockDialog,
              ),
            ),
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
            // Issue #261: firstAddress is populated offline by Rust
            // `wallet_show` (via `Wallet::first_external_address_offline`
            // — pure local crypto, no Esplora round-trip). Render
            // the address chip + copy/explorer/faucet wiring
            // unconditionally. Empty fallback kept as a safety net
            // for an unexpected FFI failure (should not happen in
            // v0.2.x).
            if (d.firstAddress.isEmpty)
              Text(
                'First address: (unavailable — unlock failed)',
                style: textTheme.bodySmall,
              )
            else
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
              ),
            const SizedBox(height: 8),
            // Explorer + Faucet buttons: address-specific URLs
            // (prefilled via `?address=<addr>` for coinfaucet). Both
            // rely on `d.firstAddress` being non-empty post-#261.
            // Empty fallback degrades to the generic testnet home
            // (safety net for the unexpected-failure path above).
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

  /// Field-by-field equality on the Esplora config — `AsyncValue`
  /// wrapper identity changes across reads even when the inner
  /// value is unchanged, so `prev == next` would fire the snackbar
  /// on every state touch instead of only on real config saves.
  bool _sameEsploraConfig(EsploraConfig a, EsploraConfig b) =>
      a.network == b.network &&
      a.url == b.url &&
      a.spkiPin == b.spkiPin;

  /// Fires when Settings → Save mutates esplora.json. Surfaces a
  /// snackbar asking the operator to re-unlock. Re-unlock re-runs
  /// `core.showWallet(...)` including Esplora sync, which is the
  /// only path that refreshes the cached balance. Suppressed when
  /// the wallet is locked (no balance to refresh).
  void _onEsploraConfigChanged() {
    if (!mounted) return;
    final session = ref.read(walletSessionProvider(widget.walletId));
    if (session == null) return;
    // Listener fires outside the build phase, so defer context
    // reads to the next frame to avoid a "deactivated element"
    // crash when the route is unmounting.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: const Text(
            'Esplora config changed — re-unlock to refresh balance',
          ),
          duration: const Duration(seconds: 6),
          action: SnackBarAction(
            label: 'Re-unlock',
            onPressed: _showReUnlockDialog,
          ),
        ),
      );
    });
  }

  /// Modal dialog asking for the wallet password. On submit sets
  /// `_password` and calls the existing `_unlock()` flow — same
  /// path as first-time unlock, so Esplora sync re-runs against
  /// the fresh config and balance updates. Avoids caching the
  /// cleartext password (v0.2.x design: zeroize on each unlock).
  Future<void> _showReUnlockDialog() async {
    if (!mounted) return;
    final controller = TextEditingController();
    try {
      final entered = await showDialog<String>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: const Text('Re-unlock to refresh balance'),
          content: TextField(
            controller: controller,
            obscureText: true,
            enableSuggestions: false,
            autocorrect: false,
            decoration: const InputDecoration(
              labelText: 'Wallet password',
              border: OutlineInputBorder(),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(null),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () => Navigator.of(ctx).pop(controller.text),
              child: const Text('Re-unlock'),
            ),
          ],
        ),
      );
      if (entered == null || entered.isEmpty || !mounted) return;
      setState(() {
        _password = entered;
        _error = null;
      });
      await _unlock();
    } finally {
      controller.dispose();
    }
  }
}
