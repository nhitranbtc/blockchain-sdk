import 'package:flutter/material.dart';
import '../core/btc/models/wallet_detail.dart';
import '../core/ffi/ffi_enums.dart';

/// Card showing the wallet's confirmed balance.
///
/// **Task 13 collapse** (plan deviation #3): the legacy 4-tuple
/// breakdown (confirmed / trustedPending / untrustedPending /
/// immature) is gone — Rust `wallet_show` returns a single
/// `balance_sat: u64`. v0.2.1 re-introduces the pending/immature
/// breakdown once the Esplora sync is wired into `wallet_show`.
///
/// **L12 flutter HIGH** (Task 13): v0.2.0 always shows "0 sats" —
/// the FFI defers sync. The user sees a hint below the headline
/// explaining the "0 sats" is a placeholder, not the wallet's real
/// state. v0.2.1 removes the hint when sync populates real values.
///
/// **Issue #263** — distinct 3-way sync classification. The card now
/// switches on [Balance.syncStatus]:
/// - [FfiSyncStatus.synced]: balance headline + nothing else (real
///   sync completed; `0` here means a legitimately empty wallet).
/// - [FfiSyncStatus.emptyWallet]: existing "no funds yet" hint copy
///   (legacy v0.2.0 path — caller passed no `esploraUrl`).
/// - [FfiSyncStatus.syncFailed]: red error banner with the
///   [lastError] diagnostic + Retry button that invokes
///   [onRetry]. Pre-#263, this state rendered identically to
///   [FfiSyncStatus.emptyWallet] (silent swallow) — operators
///   couldn't distinguish a fresh wallet from a broken Esplora sync.
///
/// **L12 review I1 (Issue #263):** an unknown / unrecognised
/// [FfiSyncStatus] byte (Rust ABI drift) falls through to the
/// [FfiSyncStatus.syncFailed] branch — fail-loud instead of
/// silently rendering nothing.
///
/// **L12 review C1 (Issue #263):** [lastError] surfaces the Rust
/// `set_last_error` diagnostic (e.g. `wallet_show esplora client:
/// ...`). Read via [WalletOpsBindings.ffiLastErrorMessage]
/// immediately after `walletShow` returns (next FFI call on the
/// same thread invalidates the borrowed `CString`).
class BalanceCard extends StatelessWidget {
  const BalanceCard({
    super.key,
    required this.balance,
    required this.syncStatus,
    this.lastError,
    this.onRetry,
  });

  final Balance balance;
  final FfiSyncStatus syncStatus;

  /// Diagnostic message from Rust's `set_last_error`. Ignored
  /// when status is [FfiSyncStatus.synced] or
  /// [FfiSyncStatus.emptyWallet] (defensive — neither path
  /// emits an error). Pass `null` to render the banner without
  /// a message body (just the header + Retry).
  final String? lastError;

  /// Optional retry callback. Required to show the Retry button.
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Confirmed', style: theme.textTheme.labelMedium),
            Text('${balance.confirmedSat} sats',
                style: theme.textTheme.headlineSmall),
            // L12 review I1: unknown status defaults to
            // syncFailed (fail-loud on ABI drift).
            if (syncStatus == FfiSyncStatus.emptyWallet) ...[
              const SizedBox(height: 4),
              Text(
                'Balance syncs on unlock against the configured Esplora endpoint',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ] else if (syncStatus == FfiSyncStatus.syncFailed ||
                syncStatus == FfiSyncStatus.unknown) ...[
              const SizedBox(height: 8),
              _SyncFailedBanner(lastError: lastError, onRetry: onRetry),
            ],
          ],
        ),
      ),
    );
  }
}

/// Red error banner for [FfiSyncStatus.syncFailed] (Issue #263).
class _SyncFailedBanner extends StatelessWidget {
  const _SyncFailedBanner({required this.lastError, required this.onRetry});

  final String? lastError;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: theme.colorScheme.errorContainer,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(Icons.error_outline,
                  color: theme.colorScheme.onErrorContainer, size: 20),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  'Sync failed — balance may be stale',
                  style: theme.textTheme.titleSmall?.copyWith(
                    color: theme.colorScheme.onErrorContainer,
                  ),
                ),
              ),
            ],
          ),
          // L12 review C1 — surface the Rust diagnostic so the
          // operator can see WHY sync failed (not just THAT it
          // failed). Body text uses a monospace family so wrapped
          // long error messages (Rust error: prefix + context)
          // stay readable. Truncate-to-ellipsis deferred — the
          // banner is a fixed-height stack inside a
          // SingleChildScrollView on the detail screen.
          if (lastError != null && lastError!.isNotEmpty) ...[
            const SizedBox(height: 4),
            Text(
              lastError!,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onErrorContainer,
                fontFamily: 'monospace',
              ),
            ),
          ],
          if (onRetry != null) ...[
            const SizedBox(height: 8),
            Align(
              alignment: Alignment.centerRight,
              child: TextButton.icon(
                key: const Key('balance_card_retry'),
                onPressed: onRetry,
                icon: Icon(Icons.refresh,
                    color: theme.colorScheme.onErrorContainer, size: 18),
                label: Text(
                  'Retry',
                  style: TextStyle(color: theme.colorScheme.onErrorContainer),
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
