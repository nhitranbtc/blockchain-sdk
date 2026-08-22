// Task 13 (#219) — `WalletDetail` collapsed to match the Rust
// `wallet_show` FFI return shape.
//
// **Plan deviation #3 + #5** (vs. legacy `btc wallet show --json`):
// - `Balance` collapsed from 4-tuple to single `confirmedSat` field.
//   Rust `wallet_show` returns `balance_sat: u64`; the legacy
//   4-tuple (confirmed/trustedPending/untrustedPending/immature) is
//   not exposed by the FFI. v0.2.1 will re-introduce the pending/
//   immature breakdown once the Esplora sync is wired into the
//   `wallet_show` FFI (the sync surface lives in
//   `bdk_extras::wallet_balance`, deferred to v0.2.1).
// - `utxos` field dropped. Rust `wallet_show` (v0.2.0 read-only
//   show, no sync) returns no UTXO list. v0.2.1: wire
//   `wallet_utxos` FFI export that returns a typed `Utxo[]`.
//
// **Path note:** this file lives under `lib/core/btc/models/` per
// the v0.1.0 layout. Task 17 moves the btc/ folder out; for Task 13
// the path stays as-is to minimise cross-file churn. The class name
// (`WalletDetail`) is also reused by the FFI DTO (Task 10 pattern of
// the same name living in both the legacy and FFI worlds during
// Phase 3 migration).

import 'package:meta/meta.dart';

import '../../ffi/ffi_enums.dart';
import 'dto_parse_exception.dart';

@immutable
class Balance {
  /// Confirmed balance in satoshis. `0` for a fresh wallet with no
  /// confirmed UTXOs. v0.2.0 read-only show always surfaces `0` —
  /// the FFI defers Esplora sync to v0.2.1.
  const Balance({required this.confirmedSat});
  final int confirmedSat;

  factory Balance.fromJson(Map<String, dynamic> j) => Balance(
        confirmedSat: dtoInt(j, 'confirmed_sat'),
      );
}

@immutable
class WalletDetail {
  const WalletDetail({
    required this.id,
    required this.network,
    required this.addressType,
    required this.firstAddress,
    required this.balance,
    this.syncStatus = FfiSyncStatus.emptyWallet,
    this.lastError,
  });
  final String id;
  final String network;

  /// Address-type string, matches the legacy `btc wallet show --json`
  /// encoding (`native-segwit` / `nested-segwit` / `taproot`).
  /// Empty in v0.2.0 if Rust returns `FfiAddressType.unknown` (the
  /// `unknown` byte from `read_address_type_or_default`'s default
  /// path is mapped to an empty Dart string; v0.2.1 surfaces the
  /// exact byte mapping).
  final String addressType;

  /// First external address. **Empty in v0.2.0** — the FFI defers
  /// `peek_addresses` (requires bdk sync) to v0.2.1. The detail
  /// screen hides `AddressChip` when this is empty.
  final String firstAddress;
  final Balance balance;

  /// Issue #263 — sync classification returned by `walletShow`.
  /// Defaults to [FfiSyncStatus.emptyWallet] for legacy JSON-decoded
  /// data (where the field is absent) — preserves the pre-#263 UX of
  /// rendering the "no funds yet" hint instead of the sync-failed
  /// banner.
  final FfiSyncStatus syncStatus;

  /// Issue #263 — diagnostic message from Rust's `set_last_error`
  /// (e.g. `wallet_show esplora client: ...`). Populated by
  /// [WalletCore.showWallet] via [WalletOpsBindings.ffiLastErrorMessage]
  /// when [syncStatus] is [FfiSyncStatus.syncFailed]; `null` for
  /// other statuses (defensive — they don't emit diagnostic context).
  final String? lastError;

  factory WalletDetail.fromJson(Map<String, dynamic> j) {
    final balanceRaw = j['balance'];
    if (balanceRaw is! Map<String, dynamic>) {
      throw DtoParseException('balance', balanceRaw);
    }
    return WalletDetail(
      id: dtoString(j, 'id'),
      network: dtoString(j, 'network'),
      addressType: dtoString(j, 'address_type'),
      firstAddress: dtoString(j, 'first_address'),
      balance: Balance.fromJson(balanceRaw),
    );
  }
}
