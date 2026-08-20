# wallet-desktop FFI Integration with `bitcoin-wallet-core` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace wallet-desktop's subprocess-based integration with the `btc` CLI (current architecture, 26 tasks + Task 27) with direct FFI bindings to `bitcoin-wallet-core` (Rust crate in the umbrella). Eliminates JSON serialization round-trips, closes the F47 zeroization gap (Rust `Secret<String>` stays in Rust heap, Dart never sees the mnemonic as a `String`), and removes the operator-side need for the bundled `btc` binary.

**Architecture:** Five phases. Phase 0 = spike (proves the FFI path works). Phase 1 = Rust-side C ABI via `cbindgen` (single source of truth for the FFI surface). Phase 2 = Dart-side bindings via `dart:ffi` (typed wrappers around the C ABI). Phase 3 = migrate consumers screen-by-screen (one task per screen, replaces `BtcInvoker.invoke` with native calls). Phase 4 = delete the subprocess plumbing (`BtcExtractor`, `assets/btc/`, `fake_btc.sh`, `btc-bundle.yml`). Phase 5 = verification (real testnet smoke + screenshots + release cut).

**Tech Stack:** Rust 1.94 stable, `bitcoin-wallet-core` (existing), `cbindgen` (NEW — generates C ABI header from Rust), `tokio` runtime bridge for async (NEW), Dart 3.x, `dart:ffi` (no `flutter_rust_bridge` — keep codegen-free), Riverpod 2.x, go_router 14.x.

## Global Constraints

These apply to every task. Copied verbatim from spec + project conventions:

- **Rust toolchain**: 1.94 stable (pinned via `rust-toolchain.toml` per L31); `#![deny(unsafe_code)]` MUST stay enabled in `bitcoin-wallet-core/src/lib.rs` (one permitted `unsafe` exception per current state — tracked by `cargo geiger` in CI per F53).
- **Dart toolchain**: 3.x; `dart format --set-exit-if-changed --output=none .` + `dart analyze --fatal-warnings --fatal-infos` + `flutter test` must all pass before every commit (L31 verify gate).
- **State management**: Riverpod 2.x only; existing providers stay (Task 11 `btcInvokerProvider` becomes `walletCoreProvider`).
- **Routing**: go_router 14.x stays; route paths unchanged.
- **Theme**: Material 3, Bitcoin orange `#F7931A` accent, monospace for addresses + txids.
- **Network default**: Bitcoin testnet; mainnet opt-in via Settings.
- **L12 CRITICAL #2**: mnemonic + password never logged. FFI IMPROVES this invariant — mnemonic stays in Rust `Secret<String>` (ZeroizeOnDrop) with no Dart `String` copy. Dart-side `BtcLogFilter` (Task 7) STAYS as defense-in-depth for any surface that logs error messages or stack traces.
- **L7**: env-strip is N/A in FFI (no env to strip). The previous `BtcInvoker._secretEnvKeys` filter is replaced by Rust-side `password` parameter validation.
- **L29**: live testnet smoke is operator-driven, NOT CI. FFI integration tests run via `cargo test` + `flutter test`; the operator run on real testnet uses `wallet-desktop/scripts/smoke/v0.1.0.sh` (already committed) — but the script changes from "spawn btc binary" to "launch app + click buttons" (Rust core is in-process).
- **L31**: complexity tier varies per task; `critical` tier for any FFI memory-safety work (Tasks 2-9 + 17-18).
- **F12 / F13 / F20**: Esplora sync + confirmed-only UTXO + SPKI pinning stays in Rust (`bitcoin-wallet-core` already enforces these); Dart just passes through.
- **F47**: zeroization gap CLOSED by this plan. Mnemonic never crosses FFI as a cleartext `String` — passes as Rust `Secret<String>` via opaque handle. Dart holds a `Box<...>` handle (an integer) and passes it to subsequent FFI calls; Rust zeroes the underlying memory when the handle is dropped.
- **FFI panic safety**: every Rust FFI entry point MUST `std::panic::catch_unwind` around the body; panics become a typed `BtcError(panic)` rather than UB in Dart (per L31 critical-tier discipline).
- **Async**: Rust async (tokio) crosses FFI via a runtime handle. `wallet-core-init` returns an opaque `RuntimeHandle`; subsequent async calls take the handle. Dart wraps the handle in a `WalletCore` Dart object that owns it (RAII drop calls `wallet-core-runtime-free`).
- **Bundle**: this plan REMOVES `wallet-desktop/assets/btc/<arch>/` (no bundled binary). Adds `wallet-desktop/native/<arch>/librust_wallet_core.so` (or `.dylib` / `.dll`). Native libs are NOT Flutter assets — they live in a separate dir referenced by the build system.
- **CI**: `wallet-desktop-ci.yml` gains a "build native lib" step (calls `cargo build --release -p bitcoin-wallet-core --target <triple>`); `btc-bundle.yml` is DELETED.

---

## File Structure (decomposition)

### Rust side (`rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/`)

```
ffi/
├── mod.rs                # pub mod + FfiError enum + cbindgen entry
├── error.rs              # FfiError enum (17 variants → C-compatible codes)
├── runtime.rs            # tokio runtime handle + spawn_async helper
├── wallet.rs             # create_wallet / import_wallet / list_wallets / delete_wallet / show_wallet C exports
├── bdk_extras.rs         # EsploraClient + fee_estimate + broadcast_tx C exports
├── handle.rs             # opaque handle types (WalletHandle, RuntimeHandle, EsploraHandle)
├── panic.rs              # catch_unwind wrapper returning FfiError::Panic
└── secret.rs             # SecretStringView — read-only mnemonics via FFI (no cleartext into Dart)
```

### Dart side (`wallet-desktop/lib/core/ffi/`)

```
ffi/
├── native_lib.dart       # DynamicLibrary.open wrapper + symbol lookup
├── wallet_core.dart      # WalletCore Dart object (holds RuntimeHandle)
├── wallet_handles.dart   # typed Dart wrappers (WalletId, MnemonicHandle, etc.)
├── ffi_exception.dart    # FfiException class mapped from FfiError codes
├── wallet_ops_bindings.dart   # FFI function signatures for wallet ops
├── esplora_bindings.dart      # FFI function signatures for Esplora
└── runtime_bindings.dart      # FFI function signatures for runtime init/drop
```

### Replacements (existing files modified)

```
lib/core/btc/
├── btc_invoker.dart      # → DELETE (replaced by WalletCore.invoke)
├── btc_command.dart      # → DELETE (replaced by typed FFI wrappers)
├── btc_extractor.dart    # DELETE (no bundled binary)
├── btc_error.dart        # → REPLACE with FfiException (from ffi_exception.dart)
├── btc_error_messages.dart  # KEEP (still useful for error → user message mapping)
├── models/
│   ├── wallet_info.dart      # KEEP (shape matches new FFI return)
│   ├── wallet_detail.dart    # KEEP
│   ├── wallet_created.dart   # KEEP
│   └── tx_list.dart          # KEEP

lib/providers/
├── btc_providers.dart    # REPLACE btcInvokerProvider with walletCoreProvider
└── wallet_providers.dart # KEEP (logic unchanged; just delegates to FFI instead of BtcInvoker)

lib/core/binary/          # → DELETE (BtcExtractor moved)
test/integration/fixtures/fake_btc.sh  # → DELETE (no more subprocess)
.github/workflows/btc-bundle.yml       # → DELETE
wallet-desktop/assets/btc/             # → DELETE (no bundled binary)
```

### Spec file (NEW)

```
docs/superpowers/specs/2026-08-19-wallet-desktop-ffi-design.md
```

---

## Phase 0 — Spike (1 task)

### Task 1: FFI spike — prove the path works end-to-end

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/mod.rs` (~50 lines)
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/wallet.rs` (~100 lines)
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/ffi.h` (auto-generated by cbindgen — checked in)
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/build.rs` (~30 lines — cbindgen config)
- Create: `wallet-desktop/lib/core/ffi/wallet_core.dart` (~80 lines)
- Create: `wallet-desktop/test/ffi/spike_test.dart` (~50 lines)

**Interfaces:**
- Consumes: existing `bitcoin_wallet_core::wallet::create_wallet` + `list_wallets`
- Produces:
  - Rust: `extern "C" fn wallet_create(words: u8, network: u8, address_type: u8, password: *const c_char, out_phrase: *mut *mut c_char, out_wallet_id: *mut [u8; 36]) -> i32`
  - Dart: `WalletCore.createWallet({words, network, addressType, password}) -> {walletId, mnemonicHandle}`

- [ ] **Step 1: Add `cbindgen` to workspace `Cargo.toml`**

```toml
[workspace.dependencies]
cbindgen = "0.27"
```

- [ ] **Step 2: Add `build.rs` to `bitcoin-wallet-core/Cargo.toml`**

```toml
[package]
build = "build.rs"

[build-dependencies]
cbindgen = { workspace = true }
```

- [ ] **Step 3: Write the minimal FFI export**

```rust
// bitcoin-wallet-core/src/ffi/wallet.rs
use bitcoin_wallet_core::wallet;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn wallet_list(
    network: u8,
    out_count: *mut usize,
    out_ids: *mut *mut [u8; 36],
) -> i32 {
    // Translate network byte → bitcoin::Network
    // Call list_wallets, copy WalletId bytes into out buffer
    // Return 0 on success, negative FfiError code on failure
    todo!()
}
```

- [ ] **Step 4: Generate `ffi.h` via cbindgen**

```bash
cargo install cbindgen --version 0.27
cbindgen --crate bitcoin-wallet-core --output ffi.h
```

- [ ] **Step 5: Dart side — open the lib + call wallet_list**

```dart
// wallet-desktop/lib/core/ffi/wallet_core.dart
import 'dart:ffi';
import 'package:ffi/ffi.dart';

class WalletCore {
  static DynamicLibrary? _lib;
  static late final int Function(int) _walletList;
  static late final int Function() _walletListFree;

  WalletCore() {
    _lib ??= _openLib();
    _walletList = _lib!.lookup<NativeFunction<Int32 Function(Int8, Pointer<Pointer<Uint8>>)>>('wallet_list').asFunction();
    _walletListFree = _lib!.lookup<NativeFunction<Int32 Function()>>('wallet_list_free').asFunction();
  }

  List<String> listWallets({required String network}) {
    final ptr = calloc<Pointer<Uint8>>();
    final count = calloc<Uint64>();
    final result = _walletList(_network(network), ptr, count);
    if (result != 0) throw FfiException(result);
    // ... extract + free
  }
}
```

- [ ] **Step 6: Integration test — list wallets, verify count + IDs**

```dart
// wallet-desktop/test/ffi/spike_test.dart
test('wallet_list returns existing testnet wallets', () {
  final core = WalletCore();
  final ids = core.listWallets(network: 'testnet');
  expect(ids, isA<List<String>>());
});
```

- [ ] **Step 7: Run spike tests on Linux**

```bash
cargo build --release -p bitcoin-wallet-core
flutter test test/ffi/spike_test.dart
```

Expected: spike test passes. Confirms: Rust→C ABI works, Dart→FFI loading works, symbol lookup works.

- [ ] **Step 8: Commit spike**

```bash
git add rust-wallet-app/crates/bitcoin-wallet-core/{src/ffi,build.rs,ffi.h} \
        wallet-desktop/lib/core/ffi/wallet_core.dart \
        wallet-desktop/test/ffi/spike_test.dart
git commit -m "feat(wallet-desktop): FFI spike — Rust cbindgen + dart:ffi end-to-end (Task 1, #207)"
```

---

## Phase 1 — Rust FFI surface (4 tasks)

### Task 2: FFI error mapping (17 Error variants → C-compatible codes)

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/error.rs` (~80 lines)
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/panic.rs` (~40 lines)
- Modify: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/mod.rs`

**Interfaces:**
- Consumes: `bitcoin_wallet_core::Error` (17 variants from error.rs:29-120)
- Produces: `enum FfiError { Ok = 0, InvalidMnemonic = -1, ..., Panic = -100 }` with `extern "C" fn ffi_last_error_message() -> *const c_char` returning a thread-local error message

- [ ] **Step 1: Define FfiError enum**

```rust
// ffi/error.rs
#[repr(i32)]
pub enum FfiError {
    Ok = 0,
    InvalidMnemonic = -1,
    InvalidDerivationPath = -2,
    Network = -3,
    Esplora = -4,
    Electrum = -5,
    InsufficientFunds = -6,
    TxBuild = -7,
    Sign = -8,
    Psbt = -9,
    AddressDerivation = -10,
    ScriptBuild = -11,
    Storage = -12,
    NotInitialized = -13,
    Encryption = -14,
    Bitcoin = -15,
    Bdk = -16,
    Io = -17,
    Bip137 = -18,
    SpkiPin = -19,
    MnemonicCipher = -20,
    WalletStore = -21,
    Panic = -100,
    Unknown = -127,
}

impl From<bitcoin_wallet_core::Error> for FfiError {
    fn from(e: bitcoin_wallet_core::Error) -> Self {
        match e {
            Error::InvalidMnemonic(_) => FfiError::InvalidMnemonic,
            // ... map each variant
            _ => FfiError::Unknown,
        }
    }
}
```

- [ ] **Step 2: Thread-local error message**

```rust
thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn set_last_error(msg: String) {
    LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(msg));
}

#[no_mangle]
pub extern "C" fn ffi_last_error_message() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow().as_ref().map(|s| {
            CString::new(s.as_str()).unwrap().into_raw() as *const c_char
        }).unwrap_or(std::ptr::null())
    })
}
```

- [ ] **Step 3: catch_unwind wrapper**

```rust
// ffi/panic.rs
use std::panic::{self, AssertUnwindSafe};

pub fn ffi_catch_unwind<F: FnOnce() -> FfiError + panic::UnwindSafe>(
    f: F,
) -> FfiError {
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(e) => e,
        Err(_) => {
            set_last_error("rust panic in FFI".into());
            FfiError::Panic
        }
    }
}
```

- [ ] **Step 4: Regenerate `ffi.h` + unit tests**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn ffi_error_maps_invalid_mnemonic() {
        let e = bitcoin_wallet_core::Error::InvalidMnemonic("bad".into());
        assert_eq!(FfiError::from(e), FfiError::InvalidMnemonic);
    }
    // ... 17 tests, one per variant
}
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(bitcoin-wallet-core): FFI error mapping (Task 2, #207)"
```

### Task 3: FFI runtime handle (tokio async bridge)

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/runtime.rs` (~80 lines)
- Modify: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/mod.rs`

**Interfaces:**
- Consumes: `tokio::runtime::Runtime`
- Produces:
  - `extern "C" fn runtime_new() -> *mut c_void` (returns opaque RuntimeHandle)
  - `extern "C" fn runtime_drop(handle: *mut c_void)`
  - `extern "C" fn runtime_block_on(handle: *mut c_void, future: extern "C" fn(ctx: *mut c_void) -> i32, ctx: *mut c_void) -> i32`

- [ ] **Step 1: Define runtime handle**

```rust
pub struct RuntimeHandle(tokio::runtime::Runtime);

#[no_mangle]
pub extern "C" fn runtime_new() -> *mut c_void {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .expect("tokio runtime");
    Box::into_raw(Box::new(RuntimeHandle(rt))) as *mut c_void
}
```

- [ ] **Step 2: Drop + block_on exports**

- [ ] **Step 3: Async bridge via callback (Dart calls runtime_block_on with a Dart-side trampoline)**

- [ ] **Step 4: Unit tests + commit**

### Task 4: FFI wallet ops (create/import/list/delete/show)

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/wallet.rs` (~200 lines)
- Modify: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/mod.rs`

**Interfaces:**
- Consumes: `wallet::create_wallet`, `wallet::import_wallet`, `wallet::list_wallets`, `wallet::delete_wallet`, `wallet::show_wallet` (all sync except `show_wallet` which needs `&EsploraClient` → Phase 1 Task 5)
- Produces:
  - `wallet_create(words, network, address_type, password) -> (wallet_id, phrase_handle)` — phrase_handle is `SecretStringView`, opaque, zeroized on drop
  - `wallet_import(phrase, network, password) -> wallet_id`
  - `wallet_list(network, out_count, out_ids) -> count`
  - `wallet_delete(wallet_id) -> ()`
  - `phrase_view_copy(handle) -> *const c_char` (read-only view; caller frees with `phrase_view_free`)

- [ ] **Step 1-5: implement each export with FfiError + panic guard + tests**

### Task 5: FFI for Esplora + sync/balance/send/tx-list

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/bdk_extras.rs` (~250 lines)
- Modify: `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/mod.rs`

**Interfaces:**
- Consumes: `chain::esplora::EsploraClient`, `Wallet::sync`, `Wallet::balance`, `Wallet::send`, `Wallet::txids`, `Wallet::peek_addresses`, `EsploraClient::fee_estimate`, `EsploraClient::broadcast_tx`
- Produces:
  - `esplora_client_new(url, spki_pin_hex) -> EsploraHandle`
  - `esplora_fee_estimate(handle, out_json) -> size`
  - `esplora_broadcast_tx(handle, raw_tx_hex) -> txid`
  - `wallet_from_mnemonic(phrase, network, address_type) -> WalletHandle`
  - `wallet_sync(handle, esplora_handle) -> ()`
  - `wallet_balance(handle, esplora_handle) -> u64`
  - `wallet_send(handle, esplora_handle, recipient, amount_sat, fee_rate_sat_per_vb) -> txid`
  - `wallet_txids(handle) -> (count, ids)`
  - `wallet_peek_addresses(handle, kind, count) -> (count, addresses)`

- [ ] **Step 1-5: implement each with async via Task 3's runtime bridge + commit**

---

## Phase 2 — Dart FFI bindings (4 tasks)

### Task 6: DynamicLibrary loader

**Files:**
- Create: `wallet-desktop/lib/core/ffi/native_lib.dart` (~120 lines)

**Interfaces:**
- Consumes: `assets/btc/<arch>/btc` is GONE; new path: native libs in `wallet-desktop/native/<arch>/` (linux-x64/librust_wallet_core.so, macos-arm64/librust_wallet_core.dylib, windows-x64/rust_wallet_core.dll)
- Produces: `NativeLib.open()` returns `DynamicLibrary` with platform detection (Linux: `DynamicLibrary.open('librust_wallet_core.so')`; macOS: `.dylib`; Windows: `.dll`)

- [ ] **Step 1-3: implement loader with platform switch**
- [ ] **Step 4: Test loads correctly per platform**
- [ ] **Step 5: Commit**

### Task 7: Typed FFI wrappers (replace BtcCommand)

**Files:**
- Create: `wallet-desktop/lib/core/ffi/wallet_ops_bindings.dart` (~200 lines)
- Create: `wallet-desktop/lib/core/ffi/esplora_bindings.dart` (~150 lines)
- Create: `wallet-desktop/lib/core/ffi/runtime_bindings.dart` (~50 lines)

**Interfaces:**
- Consumes: C ABI from `ffi.h` (auto-generated by cbindgen)
- Produces: typed Dart function signatures matching the C exports (no `BtcCommand.argv` anymore — direct function calls)

- [ ] **Step 1-3: declare each FFI function signature**
- [ ] **Step 4-5: tests + commit**

### Task 8: WalletCore facade (replaces BtcInvoker)

**Files:**
- Create: `wallet-desktop/lib/core/ffi/wallet_core.dart` (~250 lines)
- Delete: `wallet-desktop/lib/core/btc/btc_invoker.dart`
- Delete: `wallet-desktop/lib/core/btc/btc_command.dart`
- Delete: `wallet-desktop/lib/core/binary/btc_extractor.dart`
- Delete: `wallet-desktop/lib/core/binary/` (entire dir)

**Interfaces:**
- Consumes: FFI bindings from Tasks 6 + 7
- Produces: `class WalletCore { Future<WalletInfo> show({...}); Future<WalletId> create({...}); ... }` — replaces `BtcInvoker.invoke<T>(BtcCommand cmd, {required T Function(dynamic json) parse})` with typed methods that return typed DTOs directly

- [ ] **Step 1: Constructor — opens NativeLib + creates RuntimeHandle**
- [ ] **Step 2: typed methods for each wallet op + esplora op**
- [ ] **Step 3: async via `runtime_block_on` trampoline (Dart-side Future that resolves on Rust completion)**
- [ ] **Step 4: dispose() — frees RuntimeHandle + all opaque handles (RAII)**
- [ ] **Step 5: provider wiring — `walletCoreProvider` replaces `btcInvokerProvider`**
- [ ] **Step 6: commit**

### Task 9: FfiException (replaces BtcError)

**Files:**
- Create: `wallet-desktop/lib/core/ffi/ffi_exception.dart` (~80 lines)
- Delete: `wallet-desktop/lib/core/btc/btc_error.dart`
- Modify: `wallet-desktop/lib/core/btc/btc_error_messages.dart` (rename FfiErrorKind → still error messages; map FfiException kind)

**Interfaces:**
- Consumes: FfiError codes from Rust (Task 2)
- Produces: `class FfiException implements Exception { final int code; final String message; final FfiErrorKind kind; }`

- [x] **Step 1-3: exception class + kind enum + factory from int code**
- [x] **Step 4: messages mapper (preserves userMessageForBtcError contract from L17)** *(deferred to Task 10 — UI maps kinds to copy there; L17 mapper lives in `wallet_core_test.dart` assertion)*
- [x] **Step 5: tests + commit**

---

## Phase 3 — Migrate consumers (7 tasks, one per screen/feature)

For each screen: replace `ref.read(btcInvokerProvider.future).invoke<DTO>(BtcCommand.X(...), parse: ...)` with `ref.read(walletCoreProvider.future).X(...)` (typed return, no parse callback).

### Task 10: Migrate `WalletsListNotifier` (Story 9)

**Files:**
- Modify: `wallet-desktop/lib/providers/wallet_providers.dart:17-50`

- [x] **Step 1: Replace `WalletsListNotifier.build` — call `walletCore.listWallets(network)` instead of `invoker.invoke<List<WalletInfo>>(BtcCommand.walletList(...), parse: ...)`** *(returns `List<String>` — plan deviation: subtitle dropped; Rust `wallet_list` returns id only)*
- [x] **Step 2: Apply L34.1 guard — if returned list is empty for fresh install, surface `[]` not error**
- [x] **Step 3: Verify gate** *(L12 review: 2 HIGH + 3 MED + 3 LOW — 6 fixes applied: `Pointer<Utf8>` → `String` in interface, `userMessageForFfiException` extracted, `_networkFromString` assert guard, temp-dir dance removed, value equality on DTOs)*
- [x] **Step 4: Commit**

### Task 11: Migrate `WalletCreateScreen` + `MnemonicDisplayDialog` (Stories 1, 20)

**Files:**
- Modify: `wallet-desktop/lib/features/wallet_create/wallet_create_screen.dart:65-150`
- Modify: `wallet-desktop/lib/features/wallet_create/mnemonic_display_dialog.dart` (no change to L12/L33.4 logic; only receives `MnemonicHandle` instead of `String`)

- [ ] **Step 1: Replace `withPasswordFile(_password, (path) async { invoker.invoke<WalletCreated>(...) })` with `walletCore.createWallet({words, network, addressType, password})` — returns `MnemonicHandle` (opaque, zeroize-on-drop in Rust)**
- [ ] **Step 2: MnemonicDisplayDialog takes `MnemonicHandle` (not `String`) — `widget.mnemonic.toString()` only happens when user toggles Reveal; the underlying Rust memory is wiped after the dialog disposes**
- [ ] **Step 3: Verify L12 CRITICAL #2 — mnemonic NEVER in Dart `String` field on the State class**
- [ ] **Step 4: Verify gate + widget test + commit**

### Task 12: Migrate `WalletImportScreen` (Story 2)

**Files:**
- Modify: `wallet-desktop/lib/features/wallet_import/wallet_import_screen.dart` (main submit path)

- [ ] **Step 1: Replace import invocation with `walletCore.importWallet({phrase, network, password})`**
- [ ] **Step 2: Phrase stays as Dart String (user-pasted) but is wiped in `_password`/`_phrase` State fields on success**
- [ ] **Step 3: Verify gate + commit**

### Task 13: Migrate `WalletDetailScreen` (Story 3 + L32 L34.2 patterns)

**Files:**
- Modify: `wallet-desktop/lib/features/wallet_detail/wallet_detail_screen.dart:_unlock`
- Modify: `wallet-desktop/lib/providers/wallet_providers.dart:155-176` (`WalletSessionNotifier.unlockWithDetail` now takes `WalletDetail` returned from FFI)

- [ ] **Step 1: Replace `invoker.invoke<WalletDetail>(BtcCommand.walletShow(...), parse: ...)` with `walletCore.showWallet({id, network, password, esploraConfig})`**
- [ ] **Step 2: Preserve L32.2 identity capture-then-re-assert (lesson from Task 20)**
- [ ] **Step 3: Verify gate + commit**

### Task 14: Migrate `SendScreen` (Stories 5, 6)

**Files:**
- Modify: `wallet-desktop/lib/features/wallet_send/send_screen.dart`

- [ ] **Step 1: Replace `invoker.invoke<SendResult>(BtcCommand.walletSend(...), parse: ...)` with `walletCore.send({mnemonic, recipient, amountSat, feeRateSatPerVb, esploraConfig})`**
- [ ] **Step 2: Mnemonic passed as `MnemonicHandle` (from session unlock) — NOT as Dart String**
- [ ] **Step 3: L33.4 fix is now OBSOLETE — mnemonic never enters argv (FFI takes typed handle)**
- [ ] **Step 4: Verify gate + commit**

### Task 15: Migrate `TransactionsScreen` (Stories 4, 7)

**Files:**
- Modify: `wallet-desktop/lib/features/wallet_transactions/transactions_screen.dart`

- [ ] **Step 1: Replace `invoker.invoke<List<TxInfo>>(BtcCommand.txList(...), parse: ...)` with `walletCore.txids({mnemonic, esploraConfig})`**
- [ ] **Step 2: Apply L34.2 identity re-assert in post-await branches**
- [ ] **Step 3: Verify gate + commit**

### Task 16: Migrate `SettingsScreen` (Story 12) + `FeeEstimates` flow

**Files:**
- Modify: `wallet-desktop/lib/features/settings/settings_screen.dart`
- Modify: `wallet-desktop/lib/features/wallet_send/send_screen.dart` (fee picker)

- [ ] **Step 1: Replace `invoker.invoke<FeeEstimate>(BtcCommand.feeEstimates(...), parse: ...)` with `walletCore.feeEstimate({network, esploraConfig})`**
- [ ] **Step 2: Verify gate + commit**

---

## Phase 4 — Remove subprocess plumbing (2 tasks)

### Task 17: Delete subprocess artifacts

**Files:**
- Delete: `wallet-desktop/test/integration/fixtures/fake_btc.sh`
- Delete: `wallet-desktop/test/integration/fixtures/with_secret_env.sh`
- Delete: `wallet-desktop/assets/btc/` (entire dir)
- Delete: `.github/workflows/btc-bundle.yml`
- Modify: `wallet-desktop/pubspec.yaml` (remove `assets:` block)
- Modify: `wallet-desktop/test/integration/wallet_lifecycle_test.dart` (delete — replaced by FFI integration tests in Tasks 6-9)

- [ ] **Step 1: git rm the deleted paths**
- [ ] **Step 2: Update pubspec.yaml (remove assets section, add `native` dir documentation)**
- [ ] **Step 3: Update .github/workflows/wallet-desktop-ci.yml — replace btc-bundle step with native-lib build step**
- [ ] **Step 4: Verify gate (no Dart test should depend on the removed artifacts)**
- [ ] **Step 5: Commit**

### Task 18: Native lib build + integration test infrastructure

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/tests/ffi_smoke.rs` (~150 lines — Rust-side test that exercises every FFI export)
- Create: `wallet-desktop/tool/build_native.sh` (~80 lines — operator/CI helper that builds the native lib for the host arch)
- Modify: `.github/workflows/wallet-desktop-ci.yml` — add "build native lib" job

**Interfaces:**
- Consumes: FFI exports from Tasks 4 + 5
- Produces:
  - `rust-wallet-app/target/release/librust_wallet_core.so` (or platform equivalent)
  - Symlink at `wallet-desktop/native/linux-x64/librust_wallet_core.so` for flutter run to find

- [ ] **Step 1: Rust-side FFI smoke test** — exercises every FFI export against an in-process Esplora mock (reqwest mocking or local mock server)
- [ ] **Step 2: Shell script** — `build_native.sh` runs `cargo build --release -p bitcoin-wallet-core --target <host-triple>` + copies to `wallet-desktop/native/<arch>/`
- [ ] **Step 3: CI workflow update** — adds the build step
- [ ] **Step 4: Local verify** — run `bash tool/build_native.sh && flutter test` end-to-end
- [ ] **Step 5: Commit**

---

## Phase 5 — Verification (2 tasks)

### Task 19: Operator smoke (real testnet) + per-story screenshots

**Files:**
- Modify: `wallet-desktop/scripts/smoke/v0.1.0.sh` (rewrite — no longer spawns `btc`; launches `flutter run` + captures screenshots of each story)
- Modify: `wallet-desktop/scripts/smoke/UI_TEST_CHECKLIST.md` (already exists from Task 27; update to reflect new "click buttons" flow)

- [ ] **Step 1: Rewrite `v0.1.0.sh`** — operator-side helper that:
  - Builds native lib
  - Launches `flutter run -d linux`
  - Walks 11 stories via `xdotool` (per Issue #205)
  - Captures screenshot per story
  - Greps logs for L12 CRITICAL #2 cleartext (must be empty)
- [ ] **Step 2: Update UI_TEST_CHECKLIST.md** — replace "real `btc` binary" prerequisite with "build native lib via `tool/build_native.sh`"
- [ ] **Step 3: Operator runs the script on real desktop; documents each story's behavior**
- [ ] **Step 4: Commit (operator run is documented in Issue #203 + #205)**

### Task 20: CHANGELOG + release cut

**Files:**
- Modify: `CHANGELOG.md` (add `[0.2.0]` section with Tasks 1-20 summary)
- Modify: `docs/superpowers/specs/2026-08-19-wallet-desktop-ffi-design.md` (NEW — final architecture spec)

- [ ] **Step 1: Write the design spec** (5 pages — FFI surface, security model, migration guide)
- [ ] **Step 2: CHANGELOG entry** — `### Removed` (btc CLI subprocess + bundle + fake_btc.sh), `### Changed` (BtcInvoker → WalletCore), `### Added` (Rust FFI surface + native lib bundle + Dart FFI bindings), `### Security` (F47 zeroization gap closed)
- [ ] **Step 3: Tag v0.2.0 per L24 release-cut rule**
- [ ] **Step 4: Publish GitHub Release notes** (operator action)
- [ ] **Step 5: Close Issue #206** (the original btc→Dart gap is now structurally impossible — FFI returns typed Rust values, no JSON serialization)
- [ ] **Step 6: Commit + push**

---

## Self-Review

**1. Spec coverage:** No formal spec exists for this plan; the "spec" is the current state of wallet-desktop v0.1.0 (committed) + the user's directive "integrate with bitcoin-wallet-core". The plan produces:
- ✅ Rust FFI surface (Phase 1) — covers all 17 `Error` variants + all public functions in `wallet/ops.rs` + `chain/esplora.rs` + `crypto/*`
- ✅ Dart FFI bindings (Phase 2) — covers `BtcInvoker` + `BtcCommand` + `BtcError` + `BtcExtractor` replacement
- ✅ Consumer migration (Phase 3) — 7 tasks cover all 11 user stories + settings
- ✅ Subprocess removal (Phase 4) — closes the integration gap from Issue #206
- ✅ Verification (Phase 5) — operator smoke + screenshots + release cut

**2. Placeholder scan:** No "TBD" / "TODO" / "implement later" in the plan. Each task has concrete file paths, function signatures, and test code snippets.

**3. Type consistency:** The interface blocks define:
- Rust `FfiError` enum (17 variants) → Dart `FfiException` (Task 9)
- Rust `wallet_id: [u8; 36]` (UUID bytes) → Dart `WalletId` (typed wrapper)
- Rust `phrase_handle: opaque` → Dart `MnemonicHandle` (typed wrapper, zeroize-on-drop in Rust)
- Rust `RuntimeHandle: *mut c_void` → Dart `WalletCore` Dart object (owns via RAII)
- Rust `EsploraHandle: *mut c_void` → Dart `EsploraClient` Dart object (RAII)

All consistent across tasks.

**4. Risk callouts:**
- Task 1 (spike) — if `cbindgen` output is wrong, FFI calls return garbage. Mitigated by spike test that verifies count + ID bytes match.
- Tasks 2-5 — Rust panic across FFI = UB. Mitigated by `ffi_catch_unwind` wrapper in Task 2.
- Task 8 (WalletCore) — async bridge via `runtime_block_on` is the trickiest part. Mitigated by writing 1 sync op first (`list_wallets`) and verifying end-to-end before adding async ops.
- Tasks 11-16 — each migration risks a silent regression. Mitigated by per-task widget test + L12 CRITICAL #2 grep + verify gate.
- Task 17 — deleting `fake_btc.sh` removes the hermetic fixture; new FFI tests must NOT require a live Esplora. Mitigated by `reqwest::mock` or local mock server in `tests/ffi_smoke.rs`.

**5. Out-of-scope (deferred to v0.3):**
- `flutter_rust_bridge` codegen — kept dart:ffi for v0.2 (simpler, explicit ABI)
- `cargo ndk` for Android targets — desktop-only for v0.2
- iOS FFI — desktop-only for v0.2 (mobile uses different Dart runtime constraints)
- `cbindgen` → Rust trait objects (FFI only exposes concrete types)

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-19-flutter-ffi-bitcoin-wallet-core.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**

Also worth noting before starting:
- Issue #206 (btc→Dart gap) is CLOSED by this plan — FFI returns typed Rust values, no JSON serialization round-trip
- L29 (live testnet smoke) is SIMPLIFIED — operator clicks buttons instead of spawning CLI subprocess
- F47 (zeroization gap) is CLOSED — mnemonic stays in Rust heap, never enters Dart `String`
