//! Key material.
//!
//! Filled by Tasks 1.5 (`Secret<T>`), 3 (`Mnemonic`), 4 (`Signer` + derivation).

mod mnemonic;
mod secret;

pub use mnemonic::Mnemonic;
pub use secret::Secret;
