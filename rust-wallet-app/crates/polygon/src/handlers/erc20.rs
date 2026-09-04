//! ERC-20 handlers — Issue #426 / T6d-2 (L25 sub-task #2).
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.6 + §6.5. T6d-2 critical-tier surface (per L13 step 10 Q4
//! carve-out): `erc20 send` + `erc20 approve` touch signing + key
//! material, so the L12 review cluster runs 3 sub-agents
//! (type-design-analyzer + code-reviewer + security-auditor).
//!
//! `guard_usdc_e` is the Story 31 USDC.e footgun guard (negative-only;
//! native USDC pass-through; bridged USDC.e `0x2791Bca1...4174` rejects).
//! Mirrors `eth/src/handlers.rs:699 wallet_send_erc20` (flat-arg
//! `sign_erc20_tx_bytes` call — NOT TransactionRequest).
//!
//! `erc20 register` deferred to follow-up (XDG-persisted user registry
//! is heavier scope than one PR).

use std::str::FromStr;

use alloy_network::Ethereum;
use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, RootProvider};
use alloy_signer_local::PrivateKeySigner;
use zeroize::Zeroizing;

use evm_wallet_core::{erc20 as evm_erc20, sign_erc20_tx_bytes, WalletManager};
use polygon_wallet_core::{new_http, new_http_polygon_amoy, Error, Network, PolygonChain, Result};

use crate::handlers::{
    fee::{build_provider, RPC_TIMEOUT},
    map_wallet_err, validate_rpc_scheme, validate_wallet_name,
};

/// Reject bridged USDC.e addresses. Negative-only — passes native USDC
/// through. Thin wrapper over `polygon_wallet_core::disambig::reject_bridged_usdc_e`
/// (L12 sub-agent 1 finding M1 follow-up: replace with `pub use` alias).
pub fn guard_usdc_e(token: Address) -> polygon_wallet_core::Result<()> {
    polygon_wallet_core::disambig::reject_bridged_usdc_e(token)
}

/// Validate `max_fee_gwei` / `priority_fee_gwei` from `--max-fee-gwei` /
/// `--priority-fee-gwei` flags BEFORE signing. Catches three footguns:
/// (1) NaN/Inf → bad u128 cast (NaN→0 stuck-tx, Inf→u128::MAX absurd-fee).
/// (2) Negative → wraps to huge u128.
/// (3) Priority > Max → opaque alloy error at sign time; should fail
///     fast at exit 2 with the canonical "priority must be <= max" message
///     (mirrors `eth/src/handlers.rs:1047-1068 resolve_overrides`).
fn validate_fees(max_fee_gwei: f64, priority_fee_gwei: f64) -> Result<(u128, u128)> {
    for (label, v) in [
        ("max-fee-gwei", max_fee_gwei),
        ("priority-fee-gwei", priority_fee_gwei),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(Error::InvalidInput(format!(
                "--{label} must be finite and non-negative; got {v}"
            )));
        }
        if v > 1e6 {
            // 1M gwei ceiling — mainnet typical <500 gwei; sign this many
            // gwei is almost certainly a CLI input error.
            return Err(Error::InvalidInput(format!(
                "--{label} = {v} gwei exceeds 1M gwei ceiling; check units (gwei, not wei)"
            )));
        }
    }
    let max = (max_fee_gwei * 1e9) as u128;
    let priority = (priority_fee_gwei * 1e9) as u128;
    if priority > max {
        return Err(Error::InvalidInput(format!(
            "--priority-fee-gwei ({priority_fee_gwei}) must be <= --max-fee-gwei ({max_fee_gwei})"
        )));
    }
    Ok((max, priority))
}

/// Resolve `--token USDC` (symbol) or `--token 0xabc...` (hex) to a
/// canonical `Address`. Looks up symbol in the bundled polygon-wallet-core
/// registry (mainnet or amoy per `network`). Hex addresses parsed
/// directly. Returns `Error::InvalidInput` for unknown symbols / bad hex
/// (exit 2 — operator sees the fix).
pub fn resolve_token_address(symbol: &str, network: Network) -> Result<Address> {
    let trimmed = symbol.trim();
    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        return Address::from_str(trimmed)
            .map_err(|e| Error::InvalidInput(format!("invalid token address '{trimmed}': {e}")));
    }
    let tokens = match network {
        Network::Polygon(PolygonChain::Mainnet) => polygon_wallet_core::tokens::load_mainnet()
            .map_err(|e| Error::Rpc(format!("load mainnet token registry: {e}")))?,
        Network::Polygon(PolygonChain::Amoy) => polygon_wallet_core::tokens::load_amoy()
            .map_err(|e| Error::Rpc(format!("load amoy token registry: {e}")))?,
        Network::Ethereum(_) => {
            return Err(Error::InvalidInput(format!(
                "polygon erc20: unsupported network {network:?}"
            )));
        }
    };
    for t in &tokens {
        if t.symbol.eq_ignore_ascii_case(trimmed) {
            return Ok(t.address);
        }
    }
    Err(Error::InvalidInput(format!(
        "unknown token symbol '{trimmed}' for network {network:?}; pass a 0x... address"
    )))
}

/// Raw ERC-20 balance query result. Returned by `erc20_balance` so the
/// dispatch layer can format both human-readable and `--json` outputs
/// from one canonical struct. Mirrors the handler/formatter split in
/// `wallet_balance` (handlers/wallet.rs:78) — handler returns the raw
/// value, `main.rs` decides the print shape.
#[derive(Debug, Clone)]
pub struct Erc20BalanceResult {
    pub holder: Address,
    pub token: Address,
    pub decimals: u8,
    pub raw: U256,
}

impl Erc20BalanceResult {
    /// Human-readable render: `raw / 10^decimals`, trailing fractional
    /// zeros stripped (preserves at least one decimal digit). Mirrors
    /// `amoy_faucet_and_verify.rs:341` (`usdc_raw as f64 / 1e6` → 6 dp)
    /// for the canonical USDC case; scales per-token for any decimals.
    /// `decimals=0` returns raw integer (no fractional part).
    pub fn formatted(&self) -> String {
        format_token_balance(self.raw, self.decimals)
    }
}

/// Format a raw U256 token balance as `whole.frac` string. Trims trailing
/// fractional zeros after the decimal; preserves ONE trailing zero when
/// the value is otherwise a whole number so the output stays recognisably
/// a token balance (e.g. `"10.0"` rather than `"10"` for USDC). For zero
/// raw + `decimals > 0`, returns `"0.0"` (matching the "always preserve
/// one decimal" rule — `decimals = 0` returns the raw integer).
fn format_token_balance(raw: U256, decimals: u8) -> String {
    if decimals == 0 {
        return raw.to_string();
    }
    let scale = U256::from(10u8).pow(U256::from(decimals));
    let whole = raw / scale;
    let frac = raw % scale;
    let s = format!("{whole}.{:0>w$}", frac, w = decimals as usize);
    let trimmed = s.trim_end_matches('0');
    match trimmed.strip_suffix('.') {
        Some(stripped) => format!("{stripped}.0"),
        None => trimmed.to_string(),
    }
}

/// Query ERC-20 `balanceOf(holder)` for `token` (Issue #523 / T6d-2.1).
///
/// Reads the raw balance via `eth_call` to selector `0x70a08231`
/// (handled by `evm_wallet_core::erc20::token_balance`) and the token's
/// decimal scale via selector `0x313ce567` (`query_decimals`), unless
/// the caller supplies `--decimals <N>` to skip the second RPC roundtrip
/// (per issue AC #5: "if --decimals supplied, use it; otherwise query
/// `decimals()` via a second `eth_call`").
///
/// Provider-build pattern mirrors `wallet_balance` (handlers/wallet.rs:78-94):
/// custom `--rpc-url` (with `validate_rpc_scheme` guard) or the Amoy
/// default `new_http_polygon_amoy()`. Network union narrowed here to
/// `Network::Polygon(...)` — token registry is Polygon-bundled per
/// `resolve_token_address`; calling this handler with an Ethereum
/// network variant is rejected upstream by `parse_network` (`super::mod`).
pub async fn erc20_balance(
    rpc_url: Option<&str>,
    // `network` is accepted for forward-compat per-network default
    // provider selection (sister to `wallet_send_native_v2` lines
    // 573-583). Today only Amoy is exercised end-to-end per Issue #523
    // AC; the default below hardcodes Amoy.
    _network: Network,
    holder: Address,
    token: Address,
    decimals_override: Option<u8>,
) -> Result<Erc20BalanceResult> {
    let provider = match rpc_url {
        Some(url_str) => {
            let url = url::Url::parse(url_str)
                .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
            validate_rpc_scheme(&url)?;
            new_http(url).map_err(|e| Error::Rpc(format!("provider new_http: {e}")))?
        }
        None => new_http_polygon_amoy()
            .map_err(|e| Error::Rpc(format!("provider new_http_polygon_amoy: {e}")))?,
    };
    let raw = evm_erc20::token_balance(&provider, token, holder)
        .await
        .map_err(|e| Error::Rpc(format!("erc20 token_balance (balanceOf): {e}")))?;
    let decimals = match decimals_override {
        Some(d) => d,
        None => evm_erc20::query_decimals(&provider, token)
            .await
            .map_err(|e| Error::Rpc(format!("erc20 query_decimals: {e}")))?,
    };
    Ok(Erc20BalanceResult {
        holder,
        token,
        decimals,
        raw,
    })
}

/// Print bundled token registry for `network` (Story 23).
pub fn erc20_list(network: Network, json: bool) -> Result<()> {
    let tokens = match network {
        Network::Polygon(PolygonChain::Mainnet) => polygon_wallet_core::tokens::load_mainnet()
            .map_err(|e| Error::Rpc(format!("load mainnet token registry: {e}")))?,
        Network::Polygon(PolygonChain::Amoy) => polygon_wallet_core::tokens::load_amoy()
            .map_err(|e| Error::Rpc(format!("load amoy token registry: {e}")))?,
        Network::Ethereum(_) => {
            return Err(Error::InvalidInput(format!(
                "polygon erc20 list: unsupported network {network:?}"
            )));
        }
    };
    if json {
        let rows: Vec<_> = tokens
            .iter()
            .map(|t| {
                serde_json::json!({
                    "symbol": t.symbol,
                    "address": format!("{:#x}", t.address),
                    "decimals": t.decimals,
                    "chain_id": t.chain_id,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
        );
    } else {
        for t in &tokens {
            println!(
                "{} {:#x} (decimals: {}, chain_id: {})",
                t.symbol, t.address, t.decimals, t.chain_id
            );
        }
        if tokens.is_empty() {
            eprintln!("(no tokens in bundled registry)");
        }
    }
    Ok(())
}

/// Build provider with the same scheme guard as fee + wallet paths
/// (https-only or http-loopback per L12 transport-security).
fn build_erc20_provider(rpc_url: Option<&str>, network: Network) -> Result<RootProvider<Ethereum>> {
    if let Some(url_str) = rpc_url {
        let url = url::Url::parse(url_str)
            .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
        validate_rpc_scheme(&url)?;
    }
    build_provider(rpc_url, network)
}

/// Sign + broadcast an ERC-20 transfer (Story 21). Mirrors
/// `eth/src/handlers.rs:699 wallet_send_erc20`: chain_id trust-boundary
/// check (Q7 + L12 security L-1/L-4) BEFORE any signing. USDC.e footgun
/// guard runs FIRST so a bridged-USDC.e transfer can't sneak through.
#[allow(clippy::too_many_arguments)]
pub async fn erc20_send(
    data_dir: &std::path::Path,
    rpc_url: Option<&str>,
    network: Network,
    name: &str,
    password: &Zeroizing<Vec<u8>>,
    token: Address,
    to: Address,
    amount_raw: U256,
    gas_limit: Option<u64>,
    max_fee_gwei: Option<f64>,
    priority_fee_gwei: Option<f64>,
    dry_run: bool,
) -> Result<alloy_primitives::B256> {
    guard_usdc_e(token)?;
    let provider = build_erc20_provider(rpc_url, network)?;
    let provider_chain_id = tokio::time::timeout(RPC_TIMEOUT, provider.get_chain_id())
        .await
        .map_err(|_| Error::Rpc(format!("get_chain_id timed out after {RPC_TIMEOUT:?}")))?
        .map_err(|e| Error::Rpc(format!("get_chain_id: {e}")))?;
    let expected_chain_id = network.chain_id();
    if provider_chain_id != expected_chain_id {
        return Err(Error::InvalidInput(format!(
            "rpc chain_id {provider_chain_id} does not match wallet network {network:?} (expected {expected_chain_id})"
        )));
    }
    validate_wallet_name(name)?;
    let mgr = WalletManager::open_at(data_dir.to_path_buf()).map_err(map_wallet_err)?;
    let wallet_id = mgr.lookup_by_name(name, network).map_err(map_wallet_err)?;
    let key_bytes = mgr
        .unlock_signer(wallet_id, password.as_slice())
        .map_err(map_wallet_err)?;
    let signer = PrivateKeySigner::from_slice(&*key_bytes)
        .map_err(|e| Error::Rpc(format!("signer from_slice: {e}")))?;
    drop(key_bytes); // Zeroizing drop

    let nonce = provider
        .get_transaction_count(signer.address())
        .await
        .map_err(|e| Error::Rpc(format!("get_transaction_count: {e}")))?;
    let calldata = evm_erc20::transfer_calldata(to, amount_raw);
    let gas = gas_limit.unwrap_or(100_000); // ERC-20 transfer baseline
    let (max_fee_per_gas, max_priority_fee_per_gas) = validate_fees(
        max_fee_gwei.unwrap_or(60.0),
        priority_fee_gwei.unwrap_or(30.0),
    )?;

    if dry_run {
        println!(
            "dry-run: would broadcast ERC-20 transfer from {} to {} of token {:#x}",
            signer.address(),
            to,
            token,
        );
        return Ok(alloy_primitives::B256::ZERO);
    }

    let signed = sign_erc20_tx_bytes(
        &signer,
        token,
        calldata,
        U256::ZERO, // ERC-20 transfer sends 0 native ETH
        nonce,
        provider_chain_id,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        gas,
    )
    .map_err(|e| Error::Rpc(format!("sign-erc20: {e}")))?;
    let bytes = evm_wallet_core::encoded_envelope(&signed);
    let pending = provider
        .send_raw_transaction(&bytes)
        .await
        .map_err(|e| Error::Rpc(format!("send_raw_transaction: {e}")))?;
    Ok(*pending.tx_hash())
}

/// Sign + broadcast an ERC-20 approve (Story 25). `--unlimited` implies
/// `U256::MAX`; mutually exclusive with `--amount > 0`.
#[allow(clippy::too_many_arguments)]
pub async fn erc20_approve(
    data_dir: &std::path::Path,
    rpc_url: Option<&str>,
    network: Network,
    name: &str,
    password: &Zeroizing<Vec<u8>>,
    token: Address,
    spender: Address,
    amount_raw: U256,
    gas_limit: Option<u64>,
    max_fee_gwei: Option<f64>,
    priority_fee_gwei: Option<f64>,
    unlimited: bool,
    dry_run: bool,
) -> Result<alloy_primitives::B256> {
    if unlimited && amount_raw != U256::ZERO {
        return Err(Error::InvalidInput(
            "--unlimited and --amount are mutually exclusive; --unlimited implies max".into(),
        ));
    }
    guard_usdc_e(token)?;
    let provider = build_erc20_provider(rpc_url, network)?;
    let provider_chain_id = tokio::time::timeout(RPC_TIMEOUT, provider.get_chain_id())
        .await
        .map_err(|_| Error::Rpc(format!("get_chain_id timed out after {RPC_TIMEOUT:?}")))?
        .map_err(|e| Error::Rpc(format!("get_chain_id: {e}")))?;
    let expected_chain_id = network.chain_id();
    if provider_chain_id != expected_chain_id {
        return Err(Error::InvalidInput(format!(
            "rpc chain_id {provider_chain_id} does not match wallet network {network:?} (expected {expected_chain_id})"
        )));
    }
    validate_wallet_name(name)?;
    let mgr = WalletManager::open_at(data_dir.to_path_buf()).map_err(map_wallet_err)?;
    let wallet_id = mgr.lookup_by_name(name, network).map_err(map_wallet_err)?;
    let key_bytes = mgr
        .unlock_signer(wallet_id, password.as_slice())
        .map_err(map_wallet_err)?;
    let signer = PrivateKeySigner::from_slice(&*key_bytes)
        .map_err(|e| Error::Rpc(format!("signer from_slice: {e}")))?;
    drop(key_bytes); // Zeroizing drop

    let nonce = provider
        .get_transaction_count(signer.address())
        .await
        .map_err(|e| Error::Rpc(format!("get_transaction_count: {e}")))?;
    let approve_amount = if unlimited { U256::MAX } else { amount_raw };
    let calldata = evm_erc20::approve_calldata(spender, approve_amount);
    let gas = gas_limit.unwrap_or(80_000); // approve baseline
    let (max_fee_per_gas, max_priority_fee_per_gas) = validate_fees(
        max_fee_gwei.unwrap_or(60.0),
        priority_fee_gwei.unwrap_or(30.0),
    )?;

    if dry_run {
        println!(
            "dry-run: would broadcast ERC-20 approve({spender}, {approve_amount}) for token {:#x}",
            token,
        );
        return Ok(alloy_primitives::B256::ZERO);
    }

    let signed = sign_erc20_tx_bytes(
        &signer,
        token,
        calldata,
        U256::ZERO,
        nonce,
        provider_chain_id,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        gas,
    )
    .map_err(|e| Error::Rpc(format!("sign-erc20-approve: {e}")))?;
    let bytes = evm_wallet_core::encoded_envelope(&signed);
    let pending = provider
        .send_raw_transaction(&bytes)
        .await
        .map_err(|e| Error::Rpc(format!("send_raw_transaction: {e}")))?;
    Ok(*pending.tx_hash())
}

#[cfg(test)]
mod tests {
    //! T6d-2 tests: USDC.e footgun + resolve_token_address + erc20_list
    //! (pure, no live RPC). Live send/approve covered by L29 operator smoke.
    use super::*;
    use polygon_wallet_core::disambig::BRIDGED_USDC_E_ADDRESSES;

    fn bridged_usdc_e() -> Address {
        assert!(
            !BRIDGED_USDC_E_ADDRESSES.is_empty(),
            "BRIDGED_USDC_E_ADDRESSES must not be empty"
        );
        BRIDGED_USDC_E_ADDRESSES[0]
    }

    fn native_usdc() -> Address {
        Address::new([
            0x3c, 0x49, 0x9c, 0x54, 0x2c, 0xEF, 0x5E, 0x38, 0x11, 0xe1, 0x19, 0x2c, 0xe7, 0x0d,
            0x8c, 0xC0, 0x3d, 0x5c, 0x33, 0x59,
        ])
    }

    #[test]
    fn guard_usdc_e_rejects_bridged_usdce_address() {
        let r = guard_usdc_e(bridged_usdc_e());
        assert!(
            matches!(r, Err(Error::InvalidInput(ref msg)) if msg.contains("BRIDGED_USDC_REJECTED")),
            "bridged USDC.e must be rejected with BRIDGED_USDC_REJECTED marker; got {r:?}"
        );
    }

    #[test]
    fn guard_usdc_e_accepts_native_usdc_address() {
        assert!(guard_usdc_e(native_usdc()).is_ok());
    }

    #[test]
    fn guard_usdc_e_accepts_zero_address() {
        assert!(guard_usdc_e(Address::ZERO).is_ok());
    }

    #[test]
    fn guard_usdc_e_accepts_other_token_address() {
        let other = Address::new([
            0xde, 0xad, 0xbe, 0xef, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        ]);
        assert!(guard_usdc_e(other).is_ok());
    }

    #[test]
    fn resolve_token_address_parses_hex_address() {
        let addr = resolve_token_address(
            "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",
            Network::Polygon(PolygonChain::Mainnet),
        )
        .expect("parses hex address");
        assert_eq!(addr, native_usdc());
    }

    #[test]
    fn resolve_token_address_rejects_malformed_hex() {
        let r = resolve_token_address("0xZZZZ", Network::Polygon(PolygonChain::Mainnet));
        assert!(matches!(r, Err(Error::InvalidInput(_))), "got {r:?}");
    }

    #[test]
    fn resolve_token_address_rejects_unknown_symbol() {
        let r = resolve_token_address("NOPE", Network::Polygon(PolygonChain::Mainnet));
        match r {
            Err(Error::InvalidInput(msg)) => {
                assert!(msg.contains("unknown token symbol"), "got: {msg}");
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn erc20_list_mainnet_prints_bundled_registry() {
        erc20_list(Network::Polygon(PolygonChain::Mainnet), false)
            .expect("mainnet list should not error");
        erc20_list(Network::Polygon(PolygonChain::Amoy), false)
            .expect("amoy list should not error");
    }

    #[test]
    fn erc20_list_mainnet_json_emits_rows() {
        erc20_list(Network::Polygon(PolygonChain::Mainnet), true)
            .expect("mainnet list JSON should not error");
    }

    // ===== Issue #523 — erc20_balance formatter (pure, no RPC) =====

    /// `formatted()` of raw `0` returns `"0.0"` — preserves the
    /// "one-decimal" rule so the output is recognisably a token balance
    /// even at the zero edge case (don't collapse to bare `"0"` which
    /// could be misread as an integer field next to a value column).
    #[test]
    fn formatted_zero_raw_returns_zero_point_zero() {
        let r = Erc20BalanceResult {
            holder: Address::ZERO,
            token: Address::ZERO,
            decimals: 6,
            raw: U256::ZERO,
        };
        assert_eq!(r.formatted(), "0.0");
    }

    /// USDC 6-decimal canonical vector: raw `10_000_000` → `"10.0"`.
    /// (Trailing fractional zeros trimmed; at least one decimal digit
    /// preserved so the value is recognisably a token balance.)
    #[test]
    fn formatted_usdc_10_canonical_vector() {
        let r = Erc20BalanceResult {
            holder: Address::ZERO,
            token: Address::ZERO,
            decimals: 6,
            raw: U256::from(10_000_000u64),
        };
        assert_eq!(r.formatted(), "10.0");
    }

    /// USDC fractional: raw `12_500_000` → `"12.5"`.
    #[test]
    fn formatted_usdc_fractional_trims_trailing_zeros() {
        let r = Erc20BalanceResult {
            holder: Address::ZERO,
            token: Address::ZERO,
            decimals: 6,
            raw: U256::from(12_500_000u64),
        };
        assert_eq!(r.formatted(), "12.5");
    }

    /// `decimals = 0` returns the raw integer (no fractional part).
    /// Sentinel for tokens that don't expose `decimals()` (would error
    /// at auto-detect time; handler callers either suppress via explicit
    /// `--decimals 0` or use the lib's erc20 surface differently).
    #[test]
    fn formatted_zero_decimals_returns_raw_integer() {
        let r = Erc20BalanceResult {
            holder: Address::ZERO,
            token: Address::ZERO,
            decimals: 0,
            raw: U256::from(1_234u64),
        };
        assert_eq!(r.formatted(), "1234");
    }

    /// `decimals = 18` (canonical ETH-style) vector: raw `1_000_000_000_000_000_000` → `"1.0"`.
    #[test]
    fn formatted_18_decimals_one_eth() {
        let r = Erc20BalanceResult {
            holder: Address::ZERO,
            token: Address::ZERO,
            decimals: 18,
            raw: U256::from(1_000_000_000_000_000_000u128),
        };
        assert_eq!(r.formatted(), "1.0");
    }
}
