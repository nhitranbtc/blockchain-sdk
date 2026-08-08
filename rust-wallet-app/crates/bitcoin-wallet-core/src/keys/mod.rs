//! Key material.
//!
//! Filled by Tasks 1.5 (`Secret<T>`), 3 (`Mnemonic`), 4 (`Signer` + derivation).

mod derivation;
mod mnemonic;
mod secret;
mod signer;

pub use derivation::{address_type_to_path, AddressType, XPrvHolder};
pub use mnemonic::Mnemonic;
pub use secret::Secret;
pub use signer::Signer;
