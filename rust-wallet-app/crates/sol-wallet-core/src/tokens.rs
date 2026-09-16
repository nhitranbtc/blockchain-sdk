//! `tokens` — bundled SPL stablecoin mint registry (Q10 fix).
//!
//! USDC, USDT, USDS, PYUSD, BONK, JUP, JitoSOL — every popular SPL
//! token has a different `decimals` value, and the only authoritative
//! source is the on-chain Mint account's `decimals` byte (offset 44 in
//! the 82-byte classic Mint state). Hardcoding decimals at the call
//! site is the Q10 footgun — a 6/9 mismatch silently truncates 1e3 of
//! the smaller-decimal token.
//!
//! This module ships a bundled registry of canonical `(symbol, program,
//! mint, decimals)` tuples for mainnet + devnet so the wallet CLI can
//! resolve "USDC" → canonical mint + decimals without an RPC round-trip.
//! RPC-fetched decimals live in Phase 5 `chain::account::fetch_decimals`,
//! which uses [`decimals_from_state_bytes`] on the raw account data.
//!
//! Bundled via `include_str!` so the lib ships with the registry baked
//! in — no runtime CDN / file fetch. JSON parse failure is a
//! compile-time artifact bug, not a runtime error.
//!
//! Plan reference: §Phase 4 Task 4.1 Step 7 (fetch_decimals) + Step 10
//! (bundled registry).

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
// `spl_token::state::Mint::unpack` + `Mint::SIZE` are trait methods on
// `solana_program_pack::Pack`. spl-token 9.0.0 re-exports the inner
// crate under `spl_token::solana_program::program_pack::Pack` (always
// available, no feature gate). Without the trait in scope, both the
// const and the function are inaccessible.
use spl_token::solana_program::program_pack::Pack as _;

use crate::disambig::TokenProgram;
use crate::error::{Error, Result};

/// Bundled mainnet registry (USDC + USDT + USDS).
///
/// Parsed at every call site (small JSON, ~3 entries). Wrap in
/// `OnceCell` if the Phase 9 mainnet smoke shows measurable parse
/// overhead — Phase 4 ships the simple version.
const MAINNET_JSON: &str = include_str!("tokens/mainnet.json");

/// Bundled devnet registry (Circle devnet USDC).
const DEVNET_JSON: &str = include_str!("tokens/devnet.json");

/// One registry entry: `(symbol, program, mint, decimals)`.
///
/// `mint` is stored as base58 string (not `Pubkey`) because JSON
/// serialization of `Pubkey` requires the `serde` feature on
/// `solana-sdk` and produces a different wire format (32 raw bytes)
/// than the conventional base58 — keeping it as `String` matches what
/// humans see on Solana Explorer and what docs cite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintEntry {
    /// Ticker symbol (e.g. `"USDC"`). Case-sensitive — registry
    /// matches Phantom's CLI conventions exactly.
    pub symbol: String,
    /// Which SPL token program the mint lives under.
    pub program: TokenProgram,
    /// Canonical mint address as base58 string.
    pub mint: String,
    /// `decimals` field as recorded on-chain at registry-bake time.
    /// **Stale-data caveat:** the registry is bundled at compile time;
    /// if a token issuer migrates `decimals` (extremely rare; never
    /// happens in practice) callers should fall back to
    /// [`decimals_from_state_bytes`] via Phase 5 RPC. Q10 mandates
    /// the unpacked value as the source of truth at runtime.
    pub decimals: u8,
}

/// Parse the bundled mainnet registry.
///
/// `serde_json::from_str` failures here indicate a malformed
/// `tokens/mainnet.json` — that's a compile-time artifact bug, not a
/// runtime input. The `expect` keeps the lib's error surface small
/// (no parse-error variant in `Error`).
pub fn load_mainnet() -> Vec<MintEntry> {
    serde_json::from_str(MAINNET_JSON)
        .expect("sol-wallet-core: tokens/mainnet.json failed to parse — rebuild artifact")
}

/// Parse the bundled devnet registry.
pub fn load_devnet() -> Vec<MintEntry> {
    serde_json::from_str(DEVNET_JSON)
        .expect("sol-wallet-core: tokens/devnet.json failed to parse — rebuild artifact")
}

/// Look up a registry entry by ticker symbol on mainnet.
///
/// Returns `None` for any symbol that is not in the bundled mainnet
/// registry. Callers that need exotic tokens should fall back to RPC
/// `getAccountInfo(mint)` + [`decimals_from_state_bytes`] (Phase 5).
pub fn by_symbol(symbol: &str) -> Option<MintEntry> {
    load_mainnet().into_iter().find(|e| e.symbol == symbol)
}

/// Look up `decimals` for a known mint on mainnet.
///
/// Returns `None` for any mint not in the bundled mainnet registry.
/// Phase 5 `chain::account::fetch_decimals` is the universal path —
/// this function is the fast-path shortcut for the 3-4 stablecoins
/// the wallet CLI ships by default.
pub fn decimals_for_mint(mint: &Pubkey) -> Option<u8> {
    load_mainnet()
        .into_iter()
        .find(|e| {
            // Parse each mint lazily — JSON deserialization is the
            // hot path for this function (1 alloc per call). The
            // bundled registry is small (<10 entries); per-call parse
            // dominates only after Phase 9 surfaces real latency.
            Pubkey::from_str(&e.mint).ok() == Some(*mint)
        })
        .map(|e| e.decimals)
}

/// Decode the `decimals: u8` byte from raw mint account data.
///
/// Q10 — decimals are NEVER hardcoded at the call site. The 82-byte
/// classic Mint state layout (and the Token-2022 base Mint state
/// which shares the same first 82 bytes before any TLV extensions)
/// places `decimals` at offset 44:
///
/// ```text
/// offset  size  field
///   0      4    mint_authority (COption<Pubkey> discriminant)
///   4     32    mint_authority (Pubkey)
///  36      8    supply (u64)
///  44      1    decimals (u8)         ← we read this byte
///  45      1    is_initialized (bool)
///  46     36    freeze_authority (COption<Pubkey>)
/// ```
///
/// `spl_token::state::Mint::unpack` (and the token-2022 sibling)
/// does the full validation pass; Phase 4 uses `Mint::unpack`'s
/// round-trip and only reads the `decimals` field on success.
///
/// Returns `Error::InvalidTokenState` for truncated data or
/// unpack failure (wrong owner / corrupted bytes).
pub fn decimals_from_state_bytes(data: &[u8], program: TokenProgram) -> Result<u8> {
    // Classic SPL Mint state is 82 bytes; Token-2022 base Mint state
    // shares the same 82-byte prefix before any TLV extensions start.
    // `spl_token::state::Mint::SIZE` would be cleaner but it sits
    // behind the `SizedTypeProperties` trait which spl-token 9.0.0
    // does not re-export; the literal `82` matches the on-chain wire
    // layout documented at
    // https://github.com/solana-program/token/blob/main/program/src/state.rs.
    const MINT_STATE_SIZE: usize = 82;
    if data.len() < MINT_STATE_SIZE {
        return Err(Error::InvalidTokenState(format!(
            "mint account data too short: {} < {MINT_STATE_SIZE} bytes",
            data.len(),
        )));
    }
    match program {
        TokenProgram::Classic => {
            let mint = spl_token::state::Mint::unpack(data)
                .map_err(|e| Error::InvalidTokenState(format!("classic Mint::unpack: {e:?}")))?;
            Ok(mint.decimals)
        }
        TokenProgram::Token2022 => {
            let mint = spl_token_2022::state::Mint::unpack(data)
                .map_err(|e| Error::InvalidTokenState(format!("token-2022 Mint::unpack: {e:?}")))?;
            Ok(mint.decimals)
        }
    }
}

/// Decode the SPL token-account state bytes (`spl_token::state::Account` /
/// `spl_token_2022::state::Account`) into `(delegate, amount)`.
///
/// **Layout** (per spl-program/token program/src/state.rs `Account`):
/// ```text
/// offset  size  field
///   0     32    mint (Pubkey)
///  32     32    owner (Pubkey)
///  64      8    amount (u64 LE)
///  72      4    delegate discriminant (0 = None, 1 = Some)
///  76     32    delegate (Pubkey, only present when discriminant = 1)
/// 108      1    state (u8)
/// 109      8    is_native (COption<u64>)
/// 117      8    delegated_amount (u64 LE)
/// 125     36    close_authority (COption<Pubkey>)
/// ```
///
/// Returns `(None, 0)` for the empty/delegate-less case. Returns
/// `Error::InvalidTokenState` for truncated data or `Pack::unpack`
/// failure (wrong owner / corrupted bytes / wrong program tag).
pub fn delegate_amount_from_state_bytes(
    data: &[u8],
    program: TokenProgram,
) -> Result<(Option<Pubkey>, u64)> {
    // `Account::SIZE` is 165 on spl-token 9.0.0; the literal
    // matches the on-chain wire layout documented above and avoids
    // the `SizedTypeProperties` trait gate.
    const ACCOUNT_STATE_SIZE: usize = 165;
    if data.len() < ACCOUNT_STATE_SIZE {
        return Err(Error::InvalidTokenState(format!(
            "token-account data too short: {} < {ACCOUNT_STATE_SIZE} bytes",
            data.len(),
        )));
    }
    match program {
        TokenProgram::Classic => {
            let acct = spl_token::state::Account::unpack(data)
                .map_err(|e| Error::InvalidTokenState(format!("classic Account::unpack: {e:?}")))?;
            Ok((acct.delegate.into(), acct.amount))
        }
        TokenProgram::Token2022 => {
            let acct = spl_token_2022::state::Account::unpack(data).map_err(|e| {
                Error::InvalidTokenState(format!("token-2022 Account::unpack: {e:?}"))
            })?;
            Ok((acct.delegate.into(), acct.amount))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hand-picked known-good inputs for delegate-amount parse tests.
    // Independent of the parser's internal `Account::unpack` — we
    // build the expected tuple from the same fields we put into
    // the encoder. The byte layout is the on-chain wire spec, not
    // the parser's logic.
    const DELEGATE_PUBKEY_BASE58: &str = "7EYnhToRck5MV9y7s8KgZL6Wt7iV6JHv3eJ87rZ6q2HR";

    #[test]
    fn mainnet_registry_parses_without_panic() {
        // Smoke: JSON shape is valid + serde derives are correct.
        let entries = load_mainnet();
        assert!(
            !entries.is_empty(),
            "mainnet registry must seed at least one entry"
        );
    }

    #[test]
    fn delegate_amount_classic_with_delegate() {
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let delegate = Pubkey::from_str(DELEGATE_PUBKEY_BASE58)
            .expect("DELEGATE_PUBKEY_BASE58 must be a valid base58 pubkey");
        let amount: u64 = 100_000_000; // 0.1 USDC at 6 decimals — arbitrary known value

        let acct = spl_token::state::Account {
            mint,
            owner,
            amount,
            delegate: Some(delegate).into(),
            state: spl_token::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: amount,
            close_authority: None.into(),
        };
        let mut buf = vec![0u8; spl_token::state::Account::get_packed_len()];
        spl_token::state::Account::pack_into_slice(&acct, &mut buf);

        let (got_delegate, got_amount) =
            delegate_amount_from_state_bytes(&buf, TokenProgram::Classic)
                .expect("classic Account with delegate must parse");

        assert_eq!(got_delegate, Some(delegate));
        assert_eq!(got_amount, amount);
    }

    #[test]
    fn delegate_amount_token2022_with_delegate() {
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let delegate = Pubkey::from_str(DELEGATE_PUBKEY_BASE58)
            .expect("DELEGATE_PUBKEY_BASE58 must be a valid base58 pubkey");
        let amount: u64 = 250_000_000; // 0.25 token-2022 at 6 decimals — arbitrary known value

        let acct = spl_token_2022::state::Account {
            mint,
            owner,
            amount,
            delegate: Some(delegate).into(),
            state: spl_token_2022::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: amount,
            close_authority: None.into(),
        };
        let mut buf = vec![0u8; spl_token_2022::state::Account::get_packed_len()];
        spl_token_2022::state::Account::pack_into_slice(&acct, &mut buf);

        let (got_delegate, got_amount) =
            delegate_amount_from_state_bytes(&buf, TokenProgram::Token2022)
                .expect("token-2022 Account with delegate must parse");

        assert_eq!(got_delegate, Some(delegate));
        assert_eq!(got_amount, amount);
    }

    #[test]
    fn delegate_amount_classic_no_delegate() {
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let amount: u64 = 0; // empty-but-initialized ATA — no tokens, no delegate

        let acct = spl_token::state::Account {
            mint,
            owner,
            amount,
            delegate: None.into(),
            state: spl_token::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut buf = vec![0u8; spl_token::state::Account::get_packed_len()];
        spl_token::state::Account::pack_into_slice(&acct, &mut buf);

        let (got_delegate, got_amount) =
            delegate_amount_from_state_bytes(&buf, TokenProgram::Classic)
                .expect("classic Account without delegate must parse");

        assert_eq!(got_delegate, None);
        assert_eq!(got_amount, 0);
    }

    #[test]
    fn delegate_amount_truncated_data_errors() {
        // 100 bytes < 165-byte Account state — length guard must trip
        // before `Account::unpack` runs.
        let truncated = vec![0u8; 100];
        let err = delegate_amount_from_state_bytes(&truncated, TokenProgram::Classic)
            .expect_err("truncated data must reject");
        match err {
            Error::InvalidTokenState(msg) => {
                assert!(
                    msg.contains("too short"),
                    "error message must mention length: got {msg}"
                );
            }
            other => panic!("expected InvalidTokenState, got {other:?}"),
        }
    }
}
