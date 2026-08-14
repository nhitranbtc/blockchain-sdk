//! Bitcoin transaction lifecycle (Task 11 + 13, Story 5 / Issue #118).
//!
//! Three modules wrap `bdk_wallet::TxBuilder` + `wallet.sign` + Esplora
//! broadcast into testable, single-purpose units:
//!
//! - [`builder`] — `build_send_tx(&mut BdkWallet, recipient, amount,
//!   fee_rate) -> Psbt`. Wraps `bdk.build_tx().add_recipient().fee_rate().finish()`.
//! - [`sign`] — `sign_psbt(&BdkWallet, &mut Psbt) + extract_tx(&Psbt) -> Tx`.
//!   Wraps `wallet.sign(&mut psbt, SignOptions::default())` + `psbt.extract_tx()`.
//! - [`broadcast`] — `broadcast(&EsploraClient, &Tx) -> Txid`. POSTs raw
//!   tx hex to Esplora `/tx`.
//!
//! **Why split?** Per task-sdk-map §Task 11/12/13: each step is a single
//! concern, individually testable, and `Wallet::send` (in `wallet/mod.rs`)
//! composes them. CLI never touches these directly — it goes through
//! `Wallet::send` → `tx::*`.
//!
//! **Threat-model coverage:**
//! - F25 (transaction signing) — `sign::sign_psbt` is the only signing
//!   surface; uses `bdk`'s `SignerError` mapping → `Error::Sign`.
//! - U1 (unauthorized spend) — F21 typed Sighash guards are upstream of
//!   bdk's PSBT handling; we trust bdk for signature material binding.
//!
//! **Out of scope (deferred):**
//! - RBF bump-fee (Task 14) — Story 6 work.
//! - Custom fee rate from CLI (Story 6) — Story 5 uses a hardcoded
//!   default (1 sat/vB); Story 6 adds `--fee-rate`.
//! - PSBT inspection — Story 7 / Task 12 partial; Story 5 ships only
//!   the build→sign→broadcast happy path.

pub mod broadcast;
pub mod builder;
pub mod sign;
