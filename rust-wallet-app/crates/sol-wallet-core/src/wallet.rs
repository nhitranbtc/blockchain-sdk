//! Phantom-equivalent `Wallet` keypair (Phase 1).
//!
//! Phase 0 ships an empty module — the module declaration itself is the
//! Phase 0 surface. Behaviour lands in Phase 1 per plan §Phase 1 Task 1.1:
//! `Wallet::fromMnemonic`, `fromMnemonicAt`, `fromBase58`, `fromPublicKey`,
//! and the Ed25519 sign APIs over `solana_keypair::Keypair`.
