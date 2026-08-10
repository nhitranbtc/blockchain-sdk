//! Threat-model type-level primitives.
//!
//! Per F21 (threat model lines 109-122): `MessageClass` enum forces the
//! caller to declare intent before signing, defending against U5 (arbitrary
//! hash phishing). `Sighash` enum (BIP-143) lives separately for
//! transaction-signing contexts (Task 9).
//!
//! **F21 type-level defense:** `sign_recoverable` requires
//! `&MessageHash<Bip137Message>`. Passing `MessageHash<Transaction>` or
//! `MessageHash<Generic>` fails to compile. See compile_fail doc test below.
//!
//! **Defends against:** U5 (user signs a transaction sighash as a message).
//! **Does NOT defend:** T3 (timing side-channel — `secp256k1` is already
//! constant-time).

use std::marker::PhantomData;

/// Marker trait restricting which message classes can be constructed.
///
/// Sealed so external crates cannot add new variants. Adding a class
/// requires touching this file (the threat-model source-of-truth).
pub trait MessageClass: sealed::Sealed {
    /// Short human-readable name for error messages and Debug output.
    const NAME: &'static str;
}

/// Marker type for BIP-137 message signing. Used by `crypto::bip137`.
pub struct Bip137Message;

/// Marker type for transaction sighash contexts. Reserved for Task 9
/// (wallet transaction signing). NOT yet wired into signing surfaces;
/// defining the marker now blocks accidental cross-pollination.
pub struct Transaction;

/// Marker type for unspecified raw bytes. `sign_recoverable` rejects
/// this at compile time.
pub struct Generic;

impl MessageClass for Bip137Message {
    const NAME: &'static str = "Bip137Message";
}
impl MessageClass for Transaction {
    const NAME: &'static str = "Transaction";
}
impl MessageClass for Generic {
    const NAME: &'static str = "Generic";
}

mod sealed {
    use super::{Bip137Message, Generic, Transaction};
    pub trait Sealed {}
    impl Sealed for Bip137Message {}
    impl Sealed for Transaction {}
    impl Sealed for Generic {}
}

/// Typed 32-byte hash paired with the caller's declared signing intent.
///
/// Phantom-typed so the compiler refuses wrong-class usage at the signing
/// boundary. `sign_recoverable(&self, msg: &MessageHash<Bip137Message>)`
/// only accepts the BIP-137 variant.
///
/// F21 type-level defense — the following snippet fails to compile because
/// `signer.sign_recoverable` expects the Bip137Message variant:
///
/// ```compile_fail
/// # use bitcoin_wallet_core::keys::Signer;
/// # use bitcoin_wallet_core::keys::Secret;
/// # use bitcoin_wallet_core::threat::MessageHash;
/// # use bitcoin_wallet_core::threat::Transaction;
/// # let sk_bytes = [0x42u8; 32];
/// # let signer = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
/// # let hash = [0u8; 32];
/// let msg = MessageHash::<Transaction>::transaction(hash);
/// let _ = signer.sign_recoverable(&msg);
/// ```
pub struct MessageHash<C: MessageClass> {
    hash: [u8; 32],
    _class: PhantomData<C>,
}

impl<C: MessageClass> MessageHash<C> {
    /// Construct a typed hash from raw bytes. Class is fixed by the
    /// constructor in use (see `bip137` / `transaction` / `generic`).
    fn new(hash: [u8; 32]) -> Self {
        Self {
            hash,
            _class: PhantomData,
        }
    }

    /// Borrow the inner 32-byte hash. Public for callers that need to
    /// forward the hash (e.g. `verify_message` recovery compares
    /// hash160 of the recovered pubkey against an address's pkh).
    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }
}

impl MessageHash<Bip137Message> {
    /// Construct a typed hash for BIP-137 message signing.
    pub fn bip137(hash: [u8; 32]) -> Self {
        Self::new(hash)
    }
}

impl MessageHash<Transaction> {
    /// Construct a typed hash for transaction sighash contexts (Task 9).
    pub fn transaction(hash: [u8; 32]) -> Self {
        Self::new(hash)
    }
}

impl MessageHash<Generic> {
    /// Construct a typed hash for unspecified raw bytes. Rejected at
    /// the signing boundary; exists so callers can pass `Generic` to
    /// non-signing APIs (e.g. hash display) without runtime checks.
    pub fn generic(hash: [u8; 32]) -> Self {
        Self::new(hash)
    }
}

impl<C: MessageClass> std::fmt::Debug for MessageHash<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageHash")
            .field("class", &C::NAME)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_hash_debug_hides_inner_bytes() {
        let msg = MessageHash::bip137([0x42u8; 32]);
        let dbg = format!("{msg:?}");
        assert!(dbg.contains("MessageHash"));
        assert!(dbg.contains("Bip137Message"));
        // Inner hash bytes are intentionally hidden via finish_non_exhaustive.
        assert!(!dbg.contains("42"), "Debug leaks hash byte: {dbg}");
    }

    #[test]
    fn message_hash_borrow_returns_inner() {
        let bytes = [0xabu8; 32];
        let msg = MessageHash::bip137(bytes);
        assert_eq!(msg.hash(), &bytes);
    }

    #[test]
    fn message_hash_sealed_disallows_external_impls() {
        // Compile-time witness: trait is sealed to the 3 declared variants.
        // Adding a new MessageClass impl outside this file is a compile error.
        fn assert_sealed<T: sealed::Sealed>() {}
        assert_sealed::<Bip137Message>();
        assert_sealed::<Transaction>();
        assert_sealed::<Generic>();
    }
}
