//! `disambig` — SPL Token program disambiguation (Q6 footgun guard).
//!
//! Classic SPL (`TokenkegQ...`) and Token-2022 (`TokenzQdB...`) live
//! at DIFFERENT program IDs but expose the SAME wire ABI for the base
//! Mint / Token / Multisig state. Two consequences:
//!
//! 1. **Different ATAs.** `spl_associated_token_account` derives the
//!    ATA from `[owner, token_program_id, mint]`, so the same `(owner,
//!    mint)` pair produces DIFFERENT ATA addresses depending on which
//!    token program the mint actually lives under. A classic-SPL
//!    transfer_checked against a Token-2022 mint silently moves funds
//!    into a non-existent classic ATA — the user loses the funds.
//!
//! 2. **`mint.owner` is the source of truth.** `getAccountInfo(mint).owner`
//!    returns `TokenkegQ...` for classic mints and `TokenzQdB...` for
//!    Token-2022 mints. The guard below rejects any attempt to
//!    construct a transfer whose `token_program_id` argument does not
//!    match the mint's `owner` — Phase 5 RPC fetches the owner;
//!    Phase 4 builds the guard + derives ATAs + decodes decimals.
//!
//! Phase 4 deliberately keeps this module pure-Rust (no RPC, no
//! async). The detection path (`chain::account::mint_token_program`)
//! lives in Phase 5 next to the `RpcClient` wrapper.
//!
//! Plan reference: §Phase 4 Task 4.1 Steps 1 + 8.

use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;

use crate::error::{Error, Result};

/// Classic SPL Token program ID.
///
/// `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` — canonical address
/// for the classic SPL Token program. Same value `spl_token::id()`
/// returns; declared directly so the lib does not need `spl-token`
/// in scope just to fetch the constant.
///
/// Anza SDK 4.1.0 dropped the `pubkey_const!` macro re-export from
/// `solana_sdk::pubkey`, so the constants are constructed at first
/// use via `Pubkey::from_str` wrapped in `expect`. The strings are
/// canonical (base58-32, hard-coded — wrong byte would not parse)
/// so the unwrap is bug-safe: a panic here means the literal was
/// edited, which would be caught at unit-test time.
pub fn classic_token_program_id() -> Pubkey {
    Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
        .expect("hard-coded canonical classic SPL Token program ID")
}

/// Token-2022 program ID.
///
/// `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` — canonical address
/// for the Token-2022 program (the extension-aware successor to the
/// classic SPL Token program). Same value `spl_token_2022::id()`
/// returns.
pub fn token_2022_program_id() -> Pubkey {
    Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")
        .expect("hard-coded canonical Token-2022 program ID")
}

/// Which SPL token program a mint lives under.
///
/// Resolved from `mint.owner` (returned by `getAccountInfo`) per Q6.
/// The discriminant is a 2-variant enum because Solana has only two
/// deployed SPL token programs at v0.1; a third (hypothetical) would
/// require a breaking error rather than silent acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenProgram {
    /// Classic SPL Token program (`TokenkegQ...`).
    Classic,
    /// Token-2022 program (`TokenzQdB...`).
    Token2022,
}

impl TokenProgram {
    /// Resolve from a mint's `owner` field (as returned by `getAccountInfo`).
    ///
    /// Returns `Err(Error::InvalidTokenProgram)` for any program ID that
    /// is neither classic nor Token-2022 — including the System Program,
    /// the Compute Budget program, or a random account. Callers should
    /// treat that as a hard reject, not a fall-through.
    pub fn from_program_id(program_id: &Pubkey) -> Result<Self> {
        if program_id == &classic_token_program_id() {
            Ok(TokenProgram::Classic)
        } else if program_id == &token_2022_program_id() {
            Ok(TokenProgram::Token2022)
        } else {
            Err(Error::InvalidTokenProgram(format!(
                "unknown token program: {program_id}"
            )))
        }
    }

    /// Resolve to the `Pubkey` form expected by `spl_*` crate APIs.
    ///
    /// Thin accessor over the two hard-coded canonical program IDs.
    /// `Pubkey` is `Copy`, so the call is a 32-byte copy.
    pub fn program_id(&self) -> Pubkey {
        match self {
            TokenProgram::Classic => classic_token_program_id(),
            TokenProgram::Token2022 => token_2022_program_id(),
        }
    }
}

/// Reject a transfer whose `token_program_id` does not match the program
/// ID the mint actually lives under.
///
/// `claimed_program` = the `mint.owner` field as returned by RPC
/// `getAccountInfo(mint)`. `attempted_program` = the `token_program_id`
/// argument the caller is about to pass into
/// `spl_token::instruction::transfer_checked` (or `approve`,
/// `close_account`, etc.).
///
/// On mismatch, returns `Error::InvalidTokenProgram` with both program
/// IDs in the message so the operator can trace which wallet code
/// path constructed the wrong argument.
///
/// This function is pure — no RPC, no I/O, no allocation beyond the
/// `String` for the error message. Cheap to call from any hot path
/// (tx builder, CLI handler, FFI export).
pub fn reject_wrong_token_program(
    claimed_program: &Pubkey,
    attempted_program: &Pubkey,
) -> Result<()> {
    if claimed_program == attempted_program {
        Ok(())
    } else {
        Err(Error::InvalidTokenProgram(format!(
            "attempted={attempted_program}, mint.owner={claimed_program}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_id_constants_match_spl_crate_constants() {
        // Smoke against the upstream spl-token / spl-token-2022 crate
        // constants — guarantees our hard-coded base58 literals did not
        // drift from the canonical values.
        assert_eq!(classic_token_program_id(), spl_token::id());
        assert_eq!(token_2022_program_id(), spl_token_2022::id());
    }

    #[test]
    fn token_program_program_id_round_trips() {
        assert_eq!(
            TokenProgram::Classic.program_id(),
            classic_token_program_id(),
        );
        assert_eq!(
            TokenProgram::Token2022.program_id(),
            token_2022_program_id(),
        );
    }
}
