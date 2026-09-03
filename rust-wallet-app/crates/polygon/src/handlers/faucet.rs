//! `polygon faucet` handler — Issue #512 / P8-T0 (G11).
//!
//! Per `docs/superpowers/engineering/2026-09-02-polygon-amoy-test-plan.md`
//! §P8-T0. PK-free URL print path: validates `--network == "amoy"`,
//! loads the canonical Amoy faucet URL from `polygon/tokens/amoy.json`
//! (extended schema — sole SoT for Amoy config; see §Network Configuration),
//! and prints the URL + a one-line drip instruction with the address
//! rendered in EIP-55 mixed-case form via `alloy::primitives::Address`'s
//! `Display` impl (per the existing comment at `main.rs:461`).
//!
//! Scope is intentionally narrow: only the URL print path. The `--auto`
//! flag stays unimplemented per `cli.rs:484` (reserved for T7 per L29
//! operator-driven smoke). Mainnet has no canonical faucet, so any
//! non-`amoy` value is rejected early with `Error::InvalidInput`.

use crate::cli::FaucetArgs;
use polygon_wallet_core::{Error, Result};
use serde_json::Value;

/// Bundled Amoy config (extended schema: network + tokens + test_harness).
/// Compile-time embedded — `tokens/amoy.json` MUST be valid at build time
/// or this module fails to compile. Path is relative to this file's
/// crate root (`rust-wallet-app/crates/polygon/`).
const AMOY_CONFIG_JSON: &str = include_str!("../../tokens/amoy.json");

/// Print the canonical Amoy faucet URL + drip-to instructions.
///
/// PK-free: no RPC, no signing, no balance query. The operator pastes
/// `--address` into the faucet's web form; the printed line is the
/// single source of truth for the URL.
///
/// `args.network` is honored from `--network` or `POLYGON_NETWORK` env
/// (defaults to `"amoy"` per `cli.rs:480`); only `amoy` is accepted
/// here — mainnet has no canonical faucet (per §Network Configuration
/// "Faucet (native POL)" row, marked "n/a (mainnet not free)").
///
/// `args.faucet_token` and `args.auto` are intentionally ignored:
/// `auto` is reserved for T7 (operator-driven per L29); token-aware
/// routing (USDC → `faucet_circle_url`) is a follow-up that lands
/// once `erc20 balance` parity (#523) is unblocked.
pub fn faucet_print_url(args: &FaucetArgs) -> Result<()> {
    if args.network != "amoy" {
        return Err(Error::InvalidInput(format!(
            "polygon faucet: only --network amoy is supported (got {:?}); \
             mainnet has no canonical faucet",
            args.network
        )));
    }

    let faucet_url = parse_faucet_pol_url(AMOY_CONFIG_JSON)?;
    // alloy's `Address` Display impl produces the canonical EIP-55
    // mixed-case form (verified at `handlers/wallet.rs:1179-1196`).
    // `parse_address` at `cli.rs:478` already validated the input, so
    // re-formatting here is purely cosmetic — the live CLI shows the
    // checksummed form the operator pastes into the faucet form.
    let addr = args.address.to_string();

    println!("Amoy faucet URL: {faucet_url}");
    println!("Paste your address {addr} and request 0.5 POL.");
    Ok(())
}

/// Extract the `faucet_pol_url` field from the bundled Amoy config JSON.
/// Malformed JSON or missing/wrong-type field → `Error::Rpc`
/// (mirrors `polygon-wallet-core::tokens::load_amoy`'s pattern at
/// `polygon-wallet-core/src/tokens.rs:35-49`).
fn parse_faucet_pol_url(json: &str) -> Result<String> {
    let parsed: Value = serde_json::from_str(json).map_err(|e| {
        Error::Rpc(format!(
            "polygon/tokens/amoy.json malformed (compile-time include_str): {e}"
        ))
    })?;
    parsed
        .get("faucet_pol_url")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            Error::Rpc(
                "polygon/tokens/amoy.json missing string field `faucet_pol_url` \
                 (drift from §Network Configuration)"
                    .into(),
            )
        })
}

#[cfg(test)]
mod tests {
    //! P8-T0 handler unit tests. Pure resolution — no live RPC needed.
    //! The acceptance criteria (`amoy_faucet_url_print` at
    //! `tests/amoy_smoke.rs:175`) is operator-gated via `RUN_POLYGON_AMOY`;
    //! these unit tests run on plain `cargo test -p polygon`.

    use super::*;
    use alloy_primitives::address;

    fn args_with(network: &str) -> FaucetArgs {
        FaucetArgs {
            address: address!("0x0000000000000000000000000000000000000042"),
            network: network.to_string(),
            faucet_token: "POL".to_string(),
            auto: false,
        }
    }

    #[test]
    fn parse_faucet_pol_url_returns_canonical_https_url() {
        let url = parse_faucet_pol_url(AMOY_CONFIG_JSON).expect("URL parses");
        assert_eq!(url, "https://faucet.polygon.technology");
        assert!(
            url.contains("faucet.polygon.technology"),
            "URL must match the canonical substring the integration test asserts; got {url}"
        );
    }

    #[test]
    fn faucet_print_url_rejects_non_amoy_network() {
        let r = faucet_print_url(&args_with("mainnet"));
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "mainnet must be rejected (no canonical faucet); got {r:?}"
        );
    }

    #[test]
    fn faucet_print_url_rejects_unknown_network() {
        let r = faucet_print_url(&args_with("fakenet"));
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "unknown network must be rejected; got {r:?}"
        );
    }
}
