//! Threat-model type-level primitives.
//!
//! Per F21 (threat model lines 109-122): `MessageClass` markers force the
//! caller to declare intent before signing, defending against U5
//! (arbitrary-hash phishing). The `Sighash` enum (BIP-143 variants)
//! ships separately with Task 9 (wallet transaction signing).
//!
//! **F21 type-level defense:** `sign_recoverable` requires
//! `&MessageHash<Bip137Message>`. Phantom-typed `MessageHash<C>` makes
//! `MessageHash<Transaction>` a distinct, non-coercible type — the
//! compiler refuses assignment across variants (verified by
//! `compile_fail` doc test below). U5 phishing via "sign this hash" is
//! defeated because no public signing API accepts a non-Bip137 variant.
//!
//! **Defends against:** U5 (user signs a transaction sighash as a message).
//! **Does NOT defend:** T3 (timing side-channel — `secp256k1` is already
//! constant-time).

use std::marker::PhantomData;

use zeroize::ZeroizeOnDrop;

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

impl MessageClass for Bip137Message {
    const NAME: &'static str = "Bip137Message";
}
impl MessageClass for Transaction {
    const NAME: &'static str = "Transaction";
}

mod sealed {
    use super::{Bip137Message, Transaction};
    pub trait Sealed {}
    impl Sealed for Bip137Message {}
    impl Sealed for Transaction {}
}

/// Typed 32-byte hash paired with the caller's declared signing intent.
///
/// Phantom-typed so the compiler refuses wrong-class usage at the signing
/// boundary. `sign_recoverable(&self, msg: &MessageHash<Bip137Message>)`
/// only accepts the BIP-137 variant.
///
/// `ZeroizeOnDrop` derived: both fields are Zeroize (`[u8; 32]` via
/// `DefaultIsZeroes`, `PhantomData<C>` is a ZST). Defense-in-depth
/// wipes the signing-input bytes when the wrapper drops. Per L16
/// (every field must impl Zeroize) and L15 caveat (only the wrapper's
/// own copy is protected, not caller copies).
///
/// F21 type-level defense — `MessageHash<C>` is invariant in C; the
/// compiler refuses to coerce `MessageHash<Bip137Message>` into
/// `MessageHash<Transaction>`. The following snippet fails to compile
/// with E0308 (type mismatch), demonstrating the variant barrier.
///
/// (Earlier versions of this doc-test invoked `signer.sign_recoverable`,
/// but external doc-tests cannot reach the `pub(crate)` signing API and
/// the failure fired on E0624 (private method), not the intended E0308.
/// This version verifies the type barrier directly — no signing call.)
///
/// ```compile_fail
/// fn _check() {
///     let _: bitcoin_wallet_core::threat::MessageHash<
///         bitcoin_wallet_core::threat::Transaction,
///     > = bitcoin_wallet_core::threat::MessageHash::<
///         bitcoin_wallet_core::threat::Bip137Message,
///     >::bip137([0u8; 32]);
/// }
/// ```
#[derive(ZeroizeOnDrop)]
pub struct MessageHash<C: MessageClass> {
    hash: [u8; 32],
    _class: PhantomData<C>,
}

impl<C: MessageClass> MessageHash<C> {
    /// Borrow the inner 32-byte hash. `pub(crate)` because the only
    /// current consumer is `Signer::sign_recoverable`; widens if a
    /// verified external consumer emerges. (Originally `pub`; narrowed
    /// per security-auditor finding on commit 153a2d8 — leaving it
    /// `pub` would create a future phishing-vector affordance via
    /// `signer.sign_ecdsa(&bytes)?` once Task 9 wires Transaction.)
    pub(crate) fn hash(&self) -> &[u8; 32] {
        &self.hash
    }
}

impl MessageHash<Bip137Message> {
    /// Construct a typed hash for BIP-137 message signing.
    pub fn bip137(hash: [u8; 32]) -> Self {
        Self {
            hash,
            _class: PhantomData,
        }
    }
}

impl MessageHash<Transaction> {
    /// Construct a typed hash for transaction sighash contexts (Task 9).
    pub fn transaction(hash: [u8; 32]) -> Self {
        Self {
            hash,
            _class: PhantomData,
        }
    }
}

// L17: manual Debug, no field names — match project convention used by
// Mnemonic, Secret, Signer, XPrvHolder. Inner hash bytes stay hidden.
impl<C: MessageClass> std::fmt::Debug for MessageHash<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageHash").finish_non_exhaustive()
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
        // L17 strict form: no field names exposed, inner hash hidden.
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
        // Compile-time witness: trait is sealed to the 2 declared variants.
        // Adding a new MessageClass impl outside this file is a compile error.
        fn assert_sealed<T: sealed::Sealed>() {}
        assert_sealed::<Bip137Message>();
        assert_sealed::<Transaction>();
    }
}
