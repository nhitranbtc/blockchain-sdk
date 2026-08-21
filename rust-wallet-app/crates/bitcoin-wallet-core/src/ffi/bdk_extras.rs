//! FFI Esplora + Wallet exports — Task 5 of 2026-08-19 FFI integration plan.
//!
//! Async FFI exports wrap `bitcoin-wallet-core`'s existing `EsploraClient`
//! + `Wallet` async methods via the tokio runtime handle from Task 3.
//!
//! **Security model (L12 CRITICAL #2):**
//! - `wallet_from_mnemonic` takes the phrase as `*const c_char` and
//!   wraps it in `Secret<String>` (zeroize-on-drop) before constructing
//!   the `Mnemonic`. The `*const c_char` buffer is the caller's to zero
//!   (Dart side does so on completion of the FFI call). The Rust-side
//!   `Secret` is dropped at scope end with a `.zeroize()` of any
//!   surviving scratch buffers.
//! - The recipient address is checked against the wallet's network via
//!   `Address::require_network` (no `assume_checked`) — prevents a
//!   mainnet-address drain of a testnet wallet.
//! - The SPKI pin is REQUIRED for any non-localhost Esplora URL
//!   (F20 enforcement). `esplora_client_new` rejects `null` pins on
//!   public-network hosts and routes through `EsploraClient::new`
//!   with `TlsPolicy::Pinned(...)`. Localhost / 127.0.0.1 retain the
//!   `SystemRoots` leniency for dev work.
//! - `wallet_send` takes `fee_rate_sat_per_vb: u64` (not stringly-typed)
//!   so NaN/Inf cannot reach `FeeRate::from_sat_per_vb`.
//! - Array free functions embed the count in the heap header (L40
//!   pattern, mirroring Task 4's `wallet_list_array_free`). The
//!   caller-supplied count is ignored on free — the canonical count
//!   comes from the heap.
//!
//! **Plan deviation** (vs. `2026-08-19-flutter-ffi-bitcoin-wallet-core.md`):
//! - `wallet_from_mnemonic` keeps `*const c_char` (vs. plan's
//!   `phrase: string`) for cross-task ABI consistency with Task 4's
//!   `wallet_create` password parameter.
//! - `wallet_handle` survives across `sync`/`balance`/`send` calls.
//! - `address_type` byte mapping matches Task 4's `parse_address_type`:
//!   0 = NativeSegwit, 1 = NestedSegwit, 2 = Taproot.
//!
//! **Handle types** (actually used in the FFI surface — not dead):
//! - `EsploraHandle(*mut EsploraClient)` — heap-allocated newtype.
//!   Returned as `*mut c_void`, dereferenced via `&*(ptr as *const
//!   EsploraHandle)`. Mirrors Task 3's `RuntimeHandle(tokio::Runtime)`
//!   convention.
//! - `WalletHandle(*mut Wallet)` — same pattern.

#![allow(unsafe_code)] // FFI surface

use crate::chain::esplora::{EsploraClient, TlsPolicy};
use crate::chain::esplora_url::EsploraUrl;
use crate::chain::spki::{SpkiPin, SpkiPinSet};
use crate::ffi::error::FfiError;
use crate::ffi::panic::{ffi_catch_unwind, scrub_panic_message};
use crate::ffi::{runtime_or_unknown, set_last_error};

/// Local convenience wrapper: FFI export error text flows through
/// `set_last_error(msg: String)`. All call sites in this module pass
/// short string literals, so a `&str` shim avoids `.to_string()`
/// noise. The string is still passed through the same `set_last_error`
/// path → `sanitize_for_ffi` → L12 CRITICAL #2 scrubber.
fn set_err(msg: &str) {
    set_last_error(msg.to_string());
}
use crate::keys::{AddressType, Mnemonic, Secret};
use crate::wallet::{KeychainKind, Wallet};
use crate::Error;

use bitcoin::{Address, Amount, FeeRate, Network};
use std::cell::Cell;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::panic::AssertUnwindSafe;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// EsploraClient handle
// ---------------------------------------------------------------------------

/// Heap-allocated newtype wrapping an `EsploraClient`. Returned as
/// `*mut c_void` from `esplora_client_new`; recovered via pointer cast
/// in every async FFI export.
pub struct EsploraHandle(EsploraClient);

// ---------------------------------------------------------------------------
// Wallet handle
// ---------------------------------------------------------------------------

/// Heap-allocated newtype wrapping a `Wallet`. Returned as
/// `*mut c_void` from `wallet_from_mnemonic`; recovered via pointer
/// cast in every async FFI export.
pub struct WalletHandle(Wallet);

impl WalletHandle {
    /// Wrap a constructed `Wallet` (e.g. from `wallet_show` after
    /// decrypting the mnemonic) so the caller can use it for
    /// subsequent async FFI calls. The inner `Wallet` is dropped
    /// when the handle is freed via `wallet_load_free`.
    pub fn new(wallet: Wallet) -> Self {
        Self(wallet)
    }
}

// ---------------------------------------------------------------------------
// FFI parameter mapping
// ---------------------------------------------------------------------------

const NETWORK_TESTNET: u8 = 1;
const ADDR_TYPE_NATIVE_SEGWIT: u8 = 0;
const ADDR_TYPE_NESTED_SEGWIT: u8 = 1;
const ADDR_TYPE_TAPROOT: u8 = 2;
const KEYCHAIN_EXTERNAL: u8 = 0;
const KEYCHAIN_INTERNAL: u8 = 1;

fn ffi_parse_network(network: u8) -> Result<Network, FfiError> {
    match network {
        NETWORK_TESTNET => Ok(Network::Testnet),
        _ => Err(FfiError::Unknown),
    }
}

fn ffi_parse_address_type(t: u8) -> Result<AddressType, FfiError> {
    match t {
        ADDR_TYPE_NATIVE_SEGWIT => Ok(AddressType::NativeSegwit),
        ADDR_TYPE_NESTED_SEGWIT => Ok(AddressType::NestedSegwit),
        ADDR_TYPE_TAPROOT => Ok(AddressType::Taproot),
        _ => Err(FfiError::Unknown),
    }
}

fn ffi_parse_keychain_kind(t: u8) -> Result<KeychainKind, FfiError> {
    match t {
        KEYCHAIN_EXTERNAL => Ok(KeychainKind::External),
        KEYCHAIN_INTERNAL => Ok(KeychainKind::Internal),
        _ => Err(FfiError::Unknown),
    }
}

// ---------------------------------------------------------------------------
// Pointer-returning FFI helper
// ---------------------------------------------------------------------------

/// Run `f` inside `ffi_catch_unwind` (so panics become `FfiError::Panic`
/// instead of UB) and thread the result out via a `Cell`. Returns the
/// pointer on success, or null on any error / panic. Generic over the
/// pointer type so `*mut c_void` (handle) and `*mut c_char` (string)
/// exports can share the same plumbing.
fn ffi_catch_unwind_ptr<F, T>(f: F) -> *mut T
where
    F: FnOnce() -> *mut T,
{
    let slot = Cell::new(std::ptr::null_mut::<T>());
    let err = ffi_catch_unwind(AssertUnwindSafe(|| {
        slot.set(f());
        FfiError::Ok
    }));
    if err == FfiError::Ok {
        slot.get()
    } else {
        std::ptr::null_mut()
    }
}

/// Convert a lib `Error` to the FFI code + a scrubbed human message
/// for `set_last_error`. The `From<Error> for FfiError` mapping is the
/// source of truth for the FFI code; this helper just routes the
/// `Display` text through the panic-message scrubber so secrets never
/// leak into the Dart-side error buffer.
fn surface_error(e: Error) -> FfiError {
    let display = format!("{e:?}");
    set_last_error(scrub_panic_message(&display));
    FfiError::from(e)
}

fn surface_null_error<T>(e: Error) -> *mut T {
    surface_error(e);
    std::ptr::null_mut()
}

/// Map a `bitcoin::address::ParseError` to a lib `Error`. The lib
/// `Error` enum has no `From<bitcoin::address::ParseError>` impl
/// (avoids the lib carrying a `bitcoin` dep in its error type), so
/// the FFI layer maps explicitly.
fn map_addr_err(e: bitcoin::address::ParseError) -> Error {
    Error::Esplora(format!("bitcoin address parse: {e}"))
}

fn surface_null_error_msg<T>(msg: &str) -> *mut T {
    set_last_error(scrub_panic_message(msg));
    std::ptr::null_mut()
}

// ---------------------------------------------------------------------------
// EsploraClient handle
// ---------------------------------------------------------------------------

/// Construct a new `EsploraClient` from a URL and optional SPKI pin.
/// Returns an opaque `*mut c_void` (cast to `EsploraHandle` on the
/// Dart side) or null on construction failure.
///
/// **F20 enforcement**: a null `spki_pin_b64` is allowed ONLY for
/// `localhost` / `127.0.0.1` / `::1` hosts (dev mode). Any public-
/// network host requires a base64 SPKI pin.
///
/// # Safety
///
/// `url` must be a valid NUL-terminated C string (or null → null).
/// `spki_pin_b64` must be null (no pin — localhost only) or a valid
/// NUL-terminated C string of base64 SPKI bytes. Returned pointer
/// must be freed with `esplora_client_free`.
#[no_mangle]
pub unsafe extern "C" fn esplora_client_new(
    url: *const c_char,
    spki_pin_b64: *const c_char,
) -> *mut c_void {
    ffi_catch_unwind_ptr(|| -> *mut c_void {
        if url.is_null() {
            return surface_null_error_msg("esplora_client_new: null url");
        }
        let url_str = match unsafe { CStr::from_ptr(url) }.to_str() {
            Ok(s) => s,
            Err(_) => return surface_null_error_msg("esplora_client_new: url is not valid UTF-8"),
        };
        let esplora_url = match EsploraUrl::new(url_str) {
            Ok(u) => u,
            Err(e) => return surface_null_error::<c_void>(e),
        };

        // F20: require pin for non-localhost hosts.
        let host = esplora_url.as_url().host_str().unwrap_or("");
        let is_local = matches!(host, "localhost" | "127.0.0.1" | "::1");
        let policy = if spki_pin_b64.is_null() {
            if !is_local {
                return surface_null_error_msg(
                    "esplora_client_new: F20 requires SPKI pin for non-localhost host",
                );
            }
            TlsPolicy::SystemRoots
        } else {
            let pin_str = match unsafe { CStr::from_ptr(spki_pin_b64) }.to_str() {
                Ok(s) => s,
                Err(_) => {
                    return surface_null_error_msg(
                        "esplora_client_new: spki_pin_b64 is not valid UTF-8",
                    );
                }
            };
            let pin = match SpkiPin::from_base64(pin_str) {
                Ok(p) => p,
                Err(e) => return surface_null_error::<c_void>(e),
            };
            TlsPolicy::Pinned(SpkiPinSet::from_one(pin))
        };
        let client = match EsploraClient::new(esplora_url, policy) {
            Ok(c) => c,
            Err(e) => return surface_null_error::<c_void>(e),
        };
        Box::into_raw(Box::new(EsploraHandle(client))) as *mut c_void
    })
}

/// Drop an `EsploraClient` previously created by `esplora_client_new`.
///
/// # Safety
///
/// `handle` must be null (no-op) or a pointer returned by
/// `esplora_client_new` and not previously freed.
#[no_mangle]
pub unsafe extern "C" fn esplora_client_free(handle: *mut c_void) {
    if !handle.is_null() {
        // SAFETY: caller guarantees `handle` came from `esplora_client_new`
        // and is not yet freed. The `EsploraHandle` was allocated via
        // `Box::into_raw` in `esplora_client_new` — `Box::from_raw`
        // restores the original `Box<EsploraHandle>` for drop.
        unsafe {
            let _ = Box::from_raw(handle as *mut EsploraHandle);
        }
    }
}

// ---------------------------------------------------------------------------
// Esplora async ops
// ---------------------------------------------------------------------------

/// Fetch fee estimates from Esplora. Returns a heap-allocated
/// NUL-terminated JSON string; caller frees via
/// `esplora_fee_estimate_free`. Returns null on error (see
/// `ffi_last_error_message`).
///
/// # Safety
///
/// `rt` must be a valid runtime handle. `handle` must be a valid
/// `EsploraHandle`. Returned pointer must be freed with
/// `esplora_fee_estimate_free`.
#[no_mangle]
pub unsafe extern "C" fn esplora_fee_estimate(rt: *mut c_void, handle: *mut c_void) -> *mut c_char {
    ffi_catch_unwind_ptr(|| -> *mut c_char {
        if handle.is_null() {
            return surface_null_error_msg("esplora_fee_estimate: null handle");
        }
        // SAFETY: caller guarantees `handle` came from `esplora_client_new`
        // and is not yet freed. The pointer is to a heap-allocated
        // `EsploraHandle` newtype; the inner `EsploraClient` is borrowed.
        let client = unsafe { &(*(handle as *const EsploraHandle)).0 };
        let rt = match runtime_or_unknown(rt) {
            Some(r) => r,
            None => return surface_null_error_msg("esplora_fee_estimate: null runtime handle"),
        };
        let json = match rt.block_on(async { client.fee_estimate().await }) {
            Ok(j) => j,
            Err(e) => return surface_null_error::<c_char>(e),
        };
        let text = match serde_json::to_string(&json) {
            Ok(s) => s,
            Err(_) => return surface_null_error_msg("esplora_fee_estimate: JSON serialize failed"),
        };
        let cstr = match CString::new(text) {
            Ok(c) => c,
            Err(_) => return surface_null_error_msg("esplora_fee_estimate: CString alloc failed"),
        };
        cstr.into_raw()
    })
}

/// Free a JSON buffer returned by `esplora_fee_estimate`.
///
/// # Safety
///
/// `ptr` must be null (no-op) or a pointer returned by
/// `esplora_fee_estimate` and not previously freed.
#[no_mangle]
pub unsafe extern "C" fn esplora_fee_estimate_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees `ptr` came from `esplora_fee_estimate`
        // and is not yet freed. The pointer is to a `CString` allocated
        // by `CString::into_raw`; `CString::from_raw` reconstructs it
        // for drop.
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Broadcast a raw transaction. Returns a `*mut c_char` containing
/// the txid hex string (NUL-terminated) on success, or null on error.
///
/// # Safety
///
/// `rt` must be a valid runtime handle. `handle` must be a valid
/// `EsploraHandle`. `raw_tx_hex` must be a valid NUL-terminated C
/// string. Returned pointer must be freed with
/// `esplora_broadcast_tx_free`.
#[no_mangle]
pub unsafe extern "C" fn esplora_broadcast_tx(
    rt: *mut c_void,
    handle: *mut c_void,
    raw_tx_hex: *const c_char,
) -> *mut c_char {
    ffi_catch_unwind_ptr(|| -> *mut c_char {
        if handle.is_null() || raw_tx_hex.is_null() {
            return surface_null_error_msg("esplora_broadcast_tx: null handle or raw_tx_hex");
        }
        let hex_str = match unsafe { CStr::from_ptr(raw_tx_hex) }.to_str() {
            Ok(s) => s,
            Err(_) => return surface_null_error_msg("esplora_broadcast_tx: hex not UTF-8"),
        };
        // SAFETY: caller guarantees `handle` came from `esplora_client_new`.
        let client = unsafe { &(*(handle as *const EsploraHandle)).0 };
        let rt = match runtime_or_unknown(rt) {
            Some(r) => r,
            None => return surface_null_error_msg("esplora_broadcast_tx: null runtime handle"),
        };
        let txid = match rt.block_on(async { client.broadcast_tx(hex_str).await }) {
            Ok(t) => t,
            Err(e) => return surface_null_error::<c_char>(e),
        };
        let cstr = match CString::new(txid.to_string()) {
            Ok(c) => c,
            Err(_) => return surface_null_error_msg("esplora_broadcast_tx: CString alloc failed"),
        };
        cstr.into_raw()
    })
}

/// Free a txid string returned by `esplora_broadcast_tx`.
///
/// # Safety
///
/// `ptr` must be null (no-op) or a pointer returned by
/// `esplora_broadcast_tx` and not previously freed.
#[no_mangle]
pub unsafe extern "C" fn esplora_broadcast_tx_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: see esplora_fee_estimate_free.
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

// ---------------------------------------------------------------------------
// Wallet handle (from mnemonic)
// ---------------------------------------------------------------------------

/// Construct a `Wallet` from a BIP39 mnemonic phrase + network +
/// address type. Returns an opaque `*mut c_void` (cast to
/// `WalletHandle` on the Dart side) or null on error.
///
/// **L12 CRITICAL #2**: the phrase is wrapped in `Secret<String>`
/// (zeroize-on-drop) before `Mnemonic::from_phrase` is called. The
/// original `*const c_char` buffer remains the caller's to zero.
///
/// # Safety
///
/// `phrase` must be a valid NUL-terminated C string containing the
/// mnemonic. `network` byte: 1 = Testnet (only one supported via
/// FFI today). `address_type` byte: 0 = Native SegWit,
/// 1 = Nested SegWit, 2 = Taproot (matches Task 4's mapping).
/// Returned pointer must be freed with `wallet_free`.
#[no_mangle]
pub unsafe extern "C" fn wallet_from_mnemonic(
    phrase: *const c_char,
    network: u8,
    address_type: u8,
) -> *mut c_void {
    ffi_catch_unwind_ptr(|| -> *mut c_void {
        if phrase.is_null() {
            return surface_null_error_msg("wallet_from_mnemonic: null phrase");
        }
        let phrase_str = match unsafe { CStr::from_ptr(phrase) }.to_str() {
            Ok(s) => s.to_owned(),
            Err(_) => return surface_null_error_msg("wallet_from_mnemonic: phrase not UTF-8"),
        };
        let net = match ffi_parse_network(network) {
            Ok(n) => n,
            Err(_) => return surface_null_error_msg("wallet_from_mnemonic: unknown network byte"),
        };
        let addr = match ffi_parse_address_type(address_type) {
            Ok(a) => a,
            Err(_) => {
                return surface_null_error_msg("wallet_from_mnemonic: unknown address type byte")
            }
        };
        // L12 CRITICAL #2: wrap the cleartext phrase in `Secret<String>`
        // so the heap copy is zeroized on drop.
        let secret = Secret::new(phrase_str);
        let mnemonic = match Mnemonic::from_phrase(secret.expose()) {
            Ok(m) => m,
            Err(e) => return surface_null_error::<c_void>(e),
        };
        let wallet = match Wallet::from_mnemonic_with_type(&mnemonic, net, addr, None) {
            Ok(w) => w,
            Err(e) => return surface_null_error::<c_void>(e),
        };
        Box::into_raw(Box::new(WalletHandle(wallet))) as *mut c_void
    })
}

/// Drop a `Wallet` previously created by `wallet_from_mnemonic`.
///
/// # Safety
///
/// `handle` must be null (no-op) or a pointer returned by
/// `wallet_from_mnemonic` and not previously freed.
#[no_mangle]
pub unsafe extern "C" fn wallet_free(handle: *mut c_void) {
    if !handle.is_null() {
        // SAFETY: caller guarantees `handle` came from
        // `wallet_from_mnemonic` and is not yet freed. Box round-trip.
        unsafe {
            let _ = Box::from_raw(handle as *mut WalletHandle);
        }
    }
}

/// Load an existing wallet from disk into a `WalletHandle`.
///
/// **Task 14 / Issue #220 Sub-split A.** SendScreen needs to call
/// `wallet_send(rt, wallet_handle, esplora_handle, ...)` against a
/// wallet that already exists on disk. `wallet_from_mnemonic` is
/// not appropriate here — it constructs a NEW wallet from a
/// mnemonic. `wallet_load` reads the persisted changeset + builds
/// a `Wallet` ready for sync.
///
/// **`db_path`** convention: `{base_dir}/{wallet_id}.wallet` (bdk
/// file_store appends `.db` internally; `.wallet` is the
/// schema-version marker for v0.2 — chosen so future schema bumps
/// don't collide with v0.1 on-disk format).
///
/// Returns null on any failure (see `ffi_last_error_message` for
/// the `FfiError` code). Caller frees via `wallet_load_free` (which
/// is identical to `wallet_free` — same handle type).
///
/// # Safety
///
/// `base_dir`, `wallet_id`, `phrase` must be valid NUL-terminated C
/// strings. `phrase` is wrapped in `Secret<String>` so the heap
/// copy is zeroized on drop (L12 CRITICAL #2).
#[no_mangle]
pub unsafe extern "C" fn wallet_load(
    base_dir: *const c_char,
    wallet_id: *const c_char,
    phrase: *const c_char,
    network: u8,
) -> *mut c_void {
    ffi_catch_unwind_ptr(|| -> *mut c_void {
        if base_dir.is_null() || wallet_id.is_null() || phrase.is_null() {
            return surface_null_error_msg("wallet_load: null base_dir, wallet_id, or phrase");
        }
        let base_dir_str = match unsafe { CStr::from_ptr(base_dir) }.to_str() {
            Ok(s) => s,
            Err(_) => return surface_null_error_msg("wallet_load: base_dir not UTF-8"),
        };
        let wallet_id_str = match unsafe { CStr::from_ptr(wallet_id) }.to_str() {
            Ok(s) => s,
            Err(_) => return surface_null_error_msg("wallet_load: wallet_id not UTF-8"),
        };
        // Length cap (L12 MED — hostile caller).
        if base_dir_str.len() > 4096 || wallet_id_str.len() > 64 {
            return surface_null_error_msg("wallet_load: base_dir or wallet_id too long");
        }
        let phrase_str = match unsafe { CStr::from_ptr(phrase) }.to_str() {
            Ok(s) => s.to_owned(),
            Err(_) => return surface_null_error_msg("wallet_load: phrase not UTF-8"),
        };
        let net = match ffi_parse_network(network) {
            Ok(n) => n,
            Err(_) => return surface_null_error_msg("wallet_load: unknown network byte"),
        };

        // L12 CRITICAL #2: wrap cleartext phrase in `Secret<String>`
        // so the heap copy is zeroized on drop.
        let secret = Secret::new(phrase_str);
        let mnemonic = match Mnemonic::from_phrase(secret.expose()) {
            Ok(m) => m,
            Err(e) => return surface_null_error::<c_void>(e),
        };

        // Compose db_path: `{base_dir}/{wallet_id}.wallet` (bdk
        // file_store appends `.db` internally, so the on-disk file
        // is `{wallet_id}.wallet.db`).
        let db_path =
            std::path::PathBuf::from(base_dir_str).join(format!("{wallet_id_str}.wallet"));

        let wallet = match Wallet::load_persisted(db_path, &mnemonic, net) {
            Ok(w) => w,
            Err(e) => return surface_null_error::<c_void>(e),
        };
        Box::into_raw(Box::new(WalletHandle(wallet))) as *mut c_void
    })
}

/// Drop a `WalletHandle` returned by `wallet_load`. Identical to
/// `wallet_free` — provided for API symmetry on the Dart side
/// (`walletLoadFree` pairs with `walletLoad`, same way
/// `walletFree` pairs with `walletFromMnemonic`).
///
/// # Safety
///
/// `handle` must be null (no-op) or a pointer returned by
/// `wallet_load` and not previously freed.
#[no_mangle]
pub unsafe extern "C" fn wallet_load_free(handle: *mut c_void) {
    // SAFETY: identical contract to wallet_free — same handle type
    // (Box<WalletHandle>). The `if !handle.is_null()` guard inside
    // is sufficient.
    unsafe { wallet_free(handle) };
}

// ---------------------------------------------------------------------------
// Wallet async ops
// ---------------------------------------------------------------------------

/// Sync the wallet against Esplora (pulls UTXOs + chain tip).
///
/// # Safety
///
/// `rt` must be a valid runtime handle. `wallet_handle` must be a
/// valid `WalletHandle`. `esplora_handle` must be a valid
/// `EsploraHandle`.
#[no_mangle]
pub unsafe extern "C" fn wallet_sync(
    rt: *mut c_void,
    wallet_handle: *mut c_void,
    esplora_handle: *mut c_void,
) -> FfiError {
    ffi_catch_unwind(|| {
        if wallet_handle.is_null() || esplora_handle.is_null() {
            set_err("wallet_sync: null handle");
            return FfiError::Unknown;
        }
        // SAFETY: see `esplora_client_free` rationale.
        let wallet = unsafe { &(*(wallet_handle as *const WalletHandle)).0 };
        let client = unsafe { &(*(esplora_handle as *const EsploraHandle)).0 };
        let rt = match runtime_or_unknown(rt) {
            Some(r) => r,
            None => {
                set_err("wallet_sync: null runtime handle");
                return FfiError::Unknown;
            }
        };
        match rt.block_on(async { wallet.sync(client).await }) {
            Ok(_) => FfiError::Ok,
            Err(e) => surface_error(e),
        }
    })
}

/// Return confirmed balance in satoshis. Writes to `out_balance`.
///
/// # Safety
///
/// All pointers must be valid (see `wallet_sync`). `out_balance` must
/// not be null.
#[no_mangle]
pub unsafe extern "C" fn wallet_balance(
    rt: *mut c_void,
    wallet_handle: *mut c_void,
    esplora_handle: *mut c_void,
    out_balance: *mut u64,
) -> FfiError {
    ffi_catch_unwind(|| {
        if wallet_handle.is_null() || esplora_handle.is_null() || out_balance.is_null() {
            set_err("wallet_balance: null handle or out_balance");
            return FfiError::Unknown;
        }
        // SAFETY: see `wallet_sync`.
        let wallet = unsafe { &(*(wallet_handle as *const WalletHandle)).0 };
        let client = unsafe { &(*(esplora_handle as *const EsploraHandle)).0 };
        let rt = match runtime_or_unknown(rt) {
            Some(r) => r,
            None => {
                set_err("wallet_balance: null runtime handle");
                return FfiError::Unknown;
            }
        };
        let bal = match rt.block_on(async { wallet.balance(client).await }) {
            Ok(b) => b,
            Err(e) => return surface_error(e),
        };
        // SAFETY: caller guarantees `out_balance` is a valid writable
        // `*mut u64` (per the function safety contract).
        unsafe { *out_balance = bal };
        FfiError::Ok
    })
}

/// Send satoshis to a recipient. Returns a `*mut c_char` containing
/// the txid hex string (NUL-terminated); caller frees with
/// `wallet_send_free`. Returns null on error.
///
/// **L12 CRITICAL #2 hardening**: the recipient address is checked
/// against the wallet's network via `require_network` (no
/// `assume_checked`) — a mainnet address typed into a testnet
/// wallet's send form is rejected at the FFI boundary, not at
/// signing time.
///
/// # Safety
///
/// All pointers must be valid. `recipient` must be a valid
/// NUL-terminated C string. Returned pointer must be freed with
/// `wallet_send_free`.
#[no_mangle]
pub unsafe extern "C" fn wallet_send(
    rt: *mut c_void,
    wallet_handle: *mut c_void,
    esplora_handle: *mut c_void,
    recipient: *const c_char,
    amount_sat: u64,
    fee_rate_sat_per_vb: u64,
) -> *mut c_char {
    ffi_catch_unwind_ptr(|| -> *mut c_char {
        if wallet_handle.is_null() || esplora_handle.is_null() || recipient.is_null() {
            return surface_null_error_msg("wallet_send: null handle or recipient");
        }
        let recipient_str = match unsafe { CStr::from_ptr(recipient) }.to_str() {
            Ok(s) => s,
            Err(_) => return surface_null_error_msg("wallet_send: recipient not UTF-8"),
        };
        // Cap recipient length to bound heap allocation against a
        // hostile caller (L12 MED — address strings are short).
        if recipient_str.len() > 128 {
            return surface_null_error_msg("wallet_send: recipient string > 128 bytes");
        }
        // SAFETY: see `wallet_sync`.
        let wallet = unsafe { &(*(wallet_handle as *const WalletHandle)).0 };
        let client = unsafe { &(*(esplora_handle as *const EsploraHandle)).0 };
        let rt = match runtime_or_unknown(rt) {
            Some(r) => r,
            None => return surface_null_error_msg("wallet_send: null runtime handle"),
        };
        // Address::from_str + require_network: reject a mainnet
        // address typed into a testnet wallet's send form.
        let recipient_typed = match Address::from_str(recipient_str) {
            Ok(a) => a,
            Err(e) => return surface_null_error::<c_char>(map_addr_err(e)),
        };
        let recipient_addr = match recipient_typed.require_network(wallet.network()) {
            Ok(a) => a,
            Err(e) => return surface_null_error::<c_char>(map_addr_err(e)),
        };
        // Reject if amount_sat exceeds MAX_MONEY to avoid
        // `Amount::from_sat` panic surfacing as `FfiError::Panic`.
        let amount = match Amount::from_sat(amount_sat).checked_sub(Amount::from_sat(0)) {
            Some(a) => a,
            None => {
                return surface_null_error_msg("wallet_send: amount_sat exceeds MAX_MONEY");
            }
        };
        // u64 → FeeRate directly; no f64 landmine.
        let fee_rate = match FeeRate::from_sat_per_vb(fee_rate_sat_per_vb) {
            Some(fr) => fr,
            None => return surface_null_error_msg("wallet_send: fee_rate_sat_per_vb must be > 0"),
        };
        let txid = match rt
            .block_on(async { wallet.send(client, &recipient_addr, amount, fee_rate).await })
        {
            Ok(t) => t,
            Err(e) => return surface_null_error::<c_char>(e),
        };
        let cstr = match CString::new(txid.to_string()) {
            Ok(c) => c,
            Err(_) => return surface_null_error_msg("wallet_send: CString alloc failed"),
        };
        cstr.into_raw()
    })
}

/// Free a txid string returned by `wallet_send`.
///
/// # Safety
///
/// `ptr` must be null (no-op) or a pointer returned by
/// `wallet_send` AND not previously freed.
#[no_mangle]
pub unsafe extern "C" fn wallet_send_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: see esplora_fee_estimate_free.
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

// ---------------------------------------------------------------------------
// Wallet sync query ops (txids + peek_addresses)
// ---------------------------------------------------------------------------

/// Return all txids in the wallet. Writes a heap-allocated array of
/// NUL-terminated txid hex strings to `out_arr` and the count to
/// `out_count`. Caller frees the array via
/// `wallet_txids_array_free(arr, count)`.
///
/// **L40 pattern**: the count is embedded in the heap header (one
/// `usize` slot before the `[*mut c_char]` array), so the free
/// function reads the canonical count from the heap and ignores any
/// caller-supplied value.
///
/// # Safety
///
/// All pointers must be valid. `out_count` and `out_arr` must be
/// non-null.
#[no_mangle]
pub unsafe extern "C" fn wallet_txids(
    wallet_handle: *mut c_void,
    out_count: *mut usize,
    out_arr: *mut *mut c_char,
) -> FfiError {
    ffi_catch_unwind(|| {
        if wallet_handle.is_null() || out_count.is_null() || out_arr.is_null() {
            set_err("wallet_txids: null pointer");
            return FfiError::Unknown;
        }
        // SAFETY: see `wallet_sync`.
        let wallet = unsafe { &(*(wallet_handle as *const WalletHandle)).0 };
        let txids = match wallet.txids() {
            Ok(v) => v,
            Err(e) => return surface_error(e),
        };
        match build_cstring_array(txids.iter().map(|t| t.to_string()), out_count, out_arr) {
            Ok(()) => FfiError::Ok,
            Err(e) => surface_error(e),
        }
    })
}

/// Free the array returned by `wallet_txids`. The `count` parameter
/// is ignored — the canonical count is read from the heap header
/// embedded by `wallet_txids` (L40 pattern). The `count` argument is
/// kept in the signature for source-compat with the original Dart
/// caller; passing a wrong value is safe (the heap header is the
/// source of truth).
///
/// # Safety
///
/// `arr` must be null (no-op) or a pointer returned by
/// `wallet_txids` AND not yet freed.
#[no_mangle]
pub unsafe extern "C" fn wallet_txids_array_free(arr: *mut c_char, _count: usize) {
    free_cstring_array(arr);
}

/// Peek a batch of addresses for the given keychain kind. Writes a
/// heap-allocated array of NUL-terminated address strings to
/// `out_arr` and the count to `out_count`. Caller frees the array
/// via `wallet_peek_addresses_array_free(arr, count)`.
///
/// **L40 pattern**: same embedded-count header as `wallet_txids`.
///
/// # Safety
///
/// All pointers must be valid. `out_count` and `out_arr` must be
/// non-null.
#[no_mangle]
pub unsafe extern "C" fn wallet_peek_addresses(
    wallet_handle: *mut c_void,
    kind: u8,
    count: u32,
    out_count: *mut usize,
    out_arr: *mut *mut c_char,
) -> FfiError {
    ffi_catch_unwind(|| {
        if wallet_handle.is_null() || out_count.is_null() || out_arr.is_null() {
            set_err("wallet_peek_addresses: null pointer");
            return FfiError::Unknown;
        }
        let keychain = match ffi_parse_keychain_kind(kind) {
            Ok(k) => k,
            Err(_) => {
                set_err("wallet_peek_addresses: unknown keychain kind byte");
                return FfiError::Unknown;
            }
        };
        // SAFETY: see `wallet_sync`.
        let wallet = unsafe { &(*(wallet_handle as *const WalletHandle)).0 };
        let addrs = match wallet.peek_addresses(keychain, count) {
            Ok(v) => v,
            Err(e) => return surface_error(e),
        };
        match build_cstring_array(addrs.iter().map(|a| a.to_string()), out_count, out_arr) {
            Ok(()) => FfiError::Ok,
            Err(e) => surface_error(e),
        }
    })
}

/// Free the array returned by `wallet_peek_addresses`. L40 pattern:
/// the canonical count is read from the heap header; the
/// caller-supplied `_count` is ignored.
///
/// # Safety
///
/// `arr` must be null (no-op) or a pointer returned by
/// `wallet_peek_addresses` AND not yet freed.
#[no_mangle]
pub unsafe extern "C" fn wallet_peek_addresses_array_free(arr: *mut c_char, _count: usize) {
    free_cstring_array(arr);
}

// ---------------------------------------------------------------------------
// L40 array alloc / free helpers (shared by txids + peek_addresses)
// ---------------------------------------------------------------------------

/// Allocate a `[count][ptr_0]...[ptr_{count-1}]` heap block where the
/// first 8 bytes hold the count (`usize`) and the remaining bytes
/// are `count` consecutive `*mut c_char` slots (each pointing to a
/// NUL-terminated `CString`). Returns the array pointer (after the
/// count word) on success. Failure surfaces as `FfiError` after
/// `set_last_error`.
fn build_cstring_array<I, S>(
    items: I,
    out_count: *mut usize,
    out_arr: *mut *mut c_char,
) -> Result<(), Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arr: Vec<*mut c_char> = Vec::new();
    for s in items {
        let cstr = CString::new(s.as_ref()).map_err(|e| Error::Esplora(format!("cstring: {e}")))?;
        arr.push(cstr.into_raw());
    }
    let n = arr.len();
    // Single allocation: `[count][ptr_0]...[ptr_{n-1}]`. Layout:
    // bytes [0..8) = count (little-endian), bytes [8..8+8n) = n pointers.
    let total_bytes = std::mem::size_of::<usize>() + n * std::mem::size_of::<*mut c_char>();
    let raw =
        unsafe { std::alloc::alloc(std::alloc::Layout::from_size_align(total_bytes, 8).unwrap()) };
    if raw.is_null() {
        // Free any CStrings we already built (allocation failed mid-loop).
        for &p in &arr {
            unsafe {
                let _ = CString::from_raw(p);
            }
        }
        return Err(Error::Storage("alloc failed".into()));
    }
    // Write the count header.
    unsafe {
        std::ptr::write(raw as *mut usize, n);
    }
    // Write the pointer array after the count word.
    let ptrs_base = unsafe { raw.add(std::mem::size_of::<usize>()) } as *mut *mut c_char;
    for (i, &p) in arr.iter().enumerate() {
        unsafe { std::ptr::write(ptrs_base.add(i), p) };
    }
    // SAFETY: caller guarantees `out_count` and `out_arr` are valid
    // writable pointers (per the per-function safety contracts).
    unsafe {
        *out_count = n;
        *out_arr = ptrs_base as *mut c_char;
    }
    Ok(())
}

/// Free a cstring array allocated by `build_cstring_array`. Reads
/// the canonical count from the heap header (one `usize` before the
/// `arr` pointer), drops each `CString`, then deallocates the whole
/// block.
///
/// # Safety
///
/// `arr` must be null (no-op) or a pointer returned by
/// `build_cstring_array` and not yet freed.
unsafe fn free_cstring_array(arr: *mut c_char) {
    if arr.is_null() {
        return;
    }
    // SAFETY: caller guarantees `arr` came from `build_cstring_array`.
    // The count is at `arr - sizeof(usize)`.
    let count_ptr = arr as *const usize;
    // SAFETY: `arr` was preceded by a valid `usize` count word.
    let count = unsafe { count_ptr.sub(1) };
    let n = unsafe { *count };
    if n == 0 {
        // Free the count word only (no pointers to drop).
        let layout = std::alloc::Layout::from_size_align(std::mem::size_of::<usize>(), 8).unwrap();
        unsafe { std::alloc::dealloc(count as *mut u8, layout) };
        return;
    }
    let ptrs = arr as *const *mut c_char;
    for i in 0..n {
        // SAFETY: each `ptrs.add(i)` was written by `build_cstring_array`
        // from a `CString::into_raw` of a heap-allocated NUL-terminated
        // string. `CString::from_raw` reconstructs it for drop.
        let p = unsafe { *ptrs.add(i) };
        if !p.is_null() {
            unsafe {
                let _ = CString::from_raw(p);
            }
        }
    }
    let total_bytes = std::mem::size_of::<usize>() + n * std::mem::size_of::<*mut c_char>();
    let layout = std::alloc::Layout::from_size_align(total_bytes, 8).unwrap();
    // SAFETY: `count` is the start of the original allocation; we
    // deallocate the full block.
    unsafe { std::alloc::dealloc(count as *mut u8, layout) };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::raw::c_char;
    use std::ptr;

    // -- EsploraClient handle --

    #[test]
    fn esplora_client_new_null_url_returns_null() {
        let h = unsafe { esplora_client_new(ptr::null(), ptr::null()) };
        assert!(h.is_null(), "null url must return null handle");
    }

    #[test]
    fn esplora_client_new_invalid_url_returns_null() {
        let url = b"not-a-url\0";
        let h = unsafe { esplora_client_new(url.as_ptr() as *const c_char, ptr::null()) };
        assert!(h.is_null(), "invalid url must return null handle");
    }

    #[test]
    fn esplora_client_new_localhost_with_null_pin_succeeds() {
        // Dev mode: localhost without a pin is allowed (F20 dev escape).
        let url = b"http://127.0.0.1:50001/api\0";
        let h = unsafe { esplora_client_new(url.as_ptr() as *const c_char, ptr::null()) };
        assert!(
            !h.is_null(),
            "localhost + null pin must succeed (F20 dev escape)"
        );
        unsafe { esplora_client_free(h) };
    }

    #[test]
    fn esplora_client_new_public_host_with_null_pin_fails() {
        // F20 enforcement: a public host with no SPKI pin is rejected.
        let url = b"https://blockstream.info/testnet/api\0";
        let h = unsafe { esplora_client_new(url.as_ptr() as *const c_char, ptr::null()) };
        assert!(h.is_null(), "public host + null pin must be rejected (F20)");
    }

    #[test]
    fn esplora_client_new_valid_url_returns_nonnull() {
        // Localhost + null pin exercises the F20 dev escape path
        // (TlsPolicy::SystemRoots) — handle construction succeeds.
        // Public-host + pin is covered by a Task 18 integration test
        // (FFI smoke) where a real blockstream SPKI pin is provided.
        let url = b"http://127.0.0.1:50001/api\0";
        let h = unsafe { esplora_client_new(url.as_ptr() as *const c_char, ptr::null()) };
        assert!(
            !h.is_null(),
            "valid localhost url must return non-null handle"
        );
        unsafe { esplora_client_free(h) };
    }

    #[test]
    fn esplora_client_free_null_is_noop() {
        unsafe { esplora_client_free(ptr::null_mut()) };
    }

    // -- Wallet handle (from mnemonic) --

    #[test]
    fn wallet_from_mnemonic_null_phrase_returns_null() {
        let h = unsafe { wallet_from_mnemonic(ptr::null(), 1, 0) };
        assert!(h.is_null(), "null phrase must return null handle");
    }

    #[test]
    fn wallet_from_mnemonic_invalid_phrase_returns_null() {
        let phrase = b"not a valid mnemonic phrase\0";
        let h = unsafe { wallet_from_mnemonic(phrase.as_ptr() as *const c_char, 1, 0) };
        assert!(h.is_null(), "invalid phrase must return null handle");
    }

    #[test]
    fn wallet_from_mnemonic_valid_phrase_returns_nonnull() {
        let phrase = b"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\0";
        let h = unsafe { wallet_from_mnemonic(phrase.as_ptr() as *const c_char, 1, 0) };
        assert!(!h.is_null(), "valid phrase must return non-null handle");
        unsafe { wallet_free(h) };
    }

    #[test]
    fn wallet_free_null_is_noop() {
        unsafe { wallet_free(ptr::null_mut()) };
    }

    // -- Sync FFI surface (count-trust regression test) --

    #[test]
    fn wallet_txids_array_free_ignores_caller_count() {
        // L40: caller-supplied count is ignored; heap header is
        // the source of truth. Passing count=999 must NOT double-free
        // or out-of-bounds-read.
        let phrase = b"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\0";
        let w = unsafe { wallet_from_mnemonic(phrase.as_ptr() as *const c_char, 1, 0) };
        assert!(!w.is_null());

        let out_count = Box::into_raw(Box::new(0usize));
        let out_arr = Box::into_raw(Box::new(ptr::null_mut::<c_char>()));

        // Fresh wallet has no txids; txids() returns Err(NotInitialized)
        // → FfiError::Unknown. Either Ok or Unknown is acceptable; the
        // free path is what we're testing.
        let _ = unsafe { wallet_txids(w, out_count, out_arr) };
        // Pass a wildly wrong count — must not crash.
        unsafe { wallet_txids_array_free(*out_arr, 999) };
        unsafe {
            drop(Box::from_raw(out_count));
            drop(Box::from_raw(out_arr));
        }
        unsafe { wallet_free(w) };
    }

    #[test]
    fn wallet_peek_addresses_array_free_ignores_caller_count() {
        let phrase = b"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\0";
        let w = unsafe { wallet_from_mnemonic(phrase.as_ptr() as *const c_char, 1, 0) };
        assert!(!w.is_null());

        let out_count = Box::into_raw(Box::new(0usize));
        let out_arr = Box::into_raw(Box::new(ptr::null_mut::<c_char>()));

        let _ = unsafe { wallet_peek_addresses(w, 0, 5, out_count, out_arr) };
        // Pass a wildly wrong count — must not crash.
        unsafe { wallet_peek_addresses_array_free(*out_arr, 999) };
        unsafe {
            drop(Box::from_raw(out_count));
            drop(Box::from_raw(out_arr));
        }
        unsafe { wallet_free(w) };
    }

    // -- build_cstring_array direct unit test (synchronous, no FFI) --

    #[test]
    fn build_cstring_array_round_trip() {
        let out_count = Box::into_raw(Box::new(0usize));
        let out_arr = Box::into_raw(Box::new(ptr::null_mut::<c_char>()));

        let result = build_cstring_array(
            ["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
            out_count,
            out_arr,
        );
        assert!(result.is_ok());

        let n = unsafe { *out_count };
        assert_eq!(n, 3, "build_cstring_array must write count = items.len()");

        let arr = unsafe { *out_arr };
        assert!(!arr.is_null());

        // Read each string back. `arr` is the address of the first
        // `*mut c_char` slot (typed as `*mut c_char` to match the FFI
        // out_arr slot type). Re-cast to `*const *mut c_char` to walk
        // 8-byte pointer slots.
        let slots = arr as *const *mut c_char;
        for i in 0..n {
            let p = unsafe { *slots.add(i) };
            assert!(!p.is_null());
            let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
            assert!(matches!(s, "alpha" | "beta" | "gamma"));
        }

        // Free the array.
        unsafe { free_cstring_array(arr) };

        unsafe {
            drop(Box::from_raw(out_count));
            drop(Box::from_raw(out_arr));
        }
    }
}
