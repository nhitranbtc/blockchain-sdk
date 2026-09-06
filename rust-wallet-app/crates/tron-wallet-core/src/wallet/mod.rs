//! Phase 5 — wallet module root.
//!
//! Re-exports the public wallet types. Currently:
//!
//! - [`id::WalletId`] — storage handle, declared via Phase 5 Task 4.1
//! - [`persist::WalletManager`] — create / unlock / list / rename /
//!   delete + Argon2id-encrypted persistence (Task 4.7)

pub mod id;
pub mod persist;

pub use id::WalletId;
pub use persist::WalletManager;
// `EncryptedWallet` lives in `crate::crypto`; re-exporting it here
// would shadow that path. Callers should import it from `crate::crypto`.
