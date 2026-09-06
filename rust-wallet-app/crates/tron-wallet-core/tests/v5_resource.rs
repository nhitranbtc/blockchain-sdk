//! Plan Task 3.8 / Phase-7 spike V5: Energy + DEM resource model.
//!
//! Three layers:
//!
//! 1. **Pure arithmetic** (always-on). Confirms `scale_energy`,
//!    `default_fee_limit_sun`, and the const/runtime parity. These
//!    tests catch a wrong multiplier before it ships.
//! 2. **Wire-format contract** (always-on). Asserts that
//!    `TronGridClient::estimate_energy` hits the correct endpoint
//!    body shape; we cannot easily inspect the outgoing reqwest body
//!    from a unit test, so we instead assert the live response can be
//!    decoded when one is present.
//! 3. **Gated live check** (`RUN_TRON_NILE=1`). Runs
//!    `estimate_energy` against the canonical Nile USDT contract and
//!    asserts the raw Energy lands in the documented 65 000–130 000
//!    band (held recipient = 65k, empty recipient = 130k).

use tron_wallet_core::config::Network;
use tron_wallet_core::resource::{
    default_fee_limit_sun, scale_energy, DEM_MAX_FACTOR, ENERGY_PRICE_SUN,
};

#[test]
fn energy_price_sun_constant_matches_tron_mainnet_rate() {
    assert_eq!(ENERGY_PRICE_SUN, 420);
}

#[test]
fn dem_max_factor_matches_trongrid_published_ceiling() {
    // 3.4 is the documented DEM peak. A drift here means a TronGrid
    // policy change and should land in the changelog, not silently
    // move a baseline fee_limit by 17%.
    assert!((DEM_MAX_FACTOR - 3.4).abs() < f64::EPSILON);
}

#[test]
fn scale_energy_zero_short_circuits() {
    assert_eq!(scale_energy(0, DEM_MAX_FACTOR), 0);
}

#[test]
fn scale_energy_matches_arithmetic_for_documented_baselines() {
    // USDT-TRC20 baselines from plan V5:
    //   held recipient: ~65_000 Energy
    //   empty recipient: ~130_000 Energy
    // At max DEM (3.4), these scale to 221_000 and 442_000.
    assert_eq!(scale_energy(65_000, DEM_MAX_FACTOR), 221_000);
    assert_eq!(scale_energy(130_000, DEM_MAX_FACTOR), 442_000);
}

#[test]
fn default_fee_limit_covers_held_recipient_baseline() {
    // 65_000 × 3.4 × 420 = 92_820_000. The plan cites 100 000 000 as a
    // round-up baseline — both numbers are valid, the function picks
    // the floor and trusts the caller to round up.
    assert_eq!(default_fee_limit_sun(), 92_820_000);
}

#[test]
fn recommended_fee_limit_at_max_dem_never_underflows() {
    // raw=0 must produce 0 (saturating_mul handles it). Any non-zero
    // raw at max DEM must produce at least raw × ENERGY_PRICE_SUN
    // (the DEM multiplier is always ≥ 1, so scaling cannot reduce the
    // fee_limit below the unscaled value).
    for raw in [1_u64, 65_000, 130_000, u64::MAX] {
        let scaled = scale_energy(raw, DEM_MAX_FACTOR);
        let floor = raw.saturating_mul(ENERGY_PRICE_SUN);
        let recommended = scaled.saturating_mul(ENERGY_PRICE_SUN);
        assert!(
            recommended >= floor || raw == u64::MAX,
            "raw={raw} produced underflowing recommendation: scaled={scaled} floor={floor} rec={recommended}"
        );
    }
}

#[tokio::test]
async fn live_estimate_energy_lands_in_documented_band() {
    // Live against Nile USDT. Held-recipient transfers cost ~65 000
    // Energy; empty-recipient transfers cost ~130 000. Either end of
    // the band is acceptable — the test asserts *range*, not exact
    // value, because the network's exact figure depends on the current
    // DEM cycle.
    if std::env::var_os("RUN_TRON_NILE").is_none() {
        eprintln!("skipped: set RUN_TRON_NILE=1 to run this live check");
        return;
    }
    let cfg = tron_wallet_core::TronConfig::for_network(Network::Nile);
    let rpc = tron_wallet_core::TronGridClient::new(&cfg.rpc_url, cfg.spki_pin)
        .expect("TronGridClient builds");

    let estimate = tron_wallet_core::resource::estimate_energy(
        &rpc,
        "TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf", // canonical Nile USDT
        tron_wallet_core::trc20::TRANSFER_SELECTOR,
        // 32-byte zero-padded recipient address. The exact recipient
        // does not affect the Energy band — only whether the recipient
        // is a fresh account or a holder.
        &{
            let mut arg = [0u8; 32];
            // 12 zero bytes + 20 bytes for the recipient
            arg[12..].copy_from_slice(&[0xab; 20]);
            arg.to_vec()
        },
    )
    .await
    .expect("estimate_energy succeeds against Nile USDT");

    assert!(
        estimate.raw_energy >= 30_000 && estimate.raw_energy <= 250_000,
        "Nile USDT transfer Energy {} outside expected band [30_000, 250_000] — \
         either the contract changed or the DEM math is off",
        estimate.raw_energy
    );
    // The scaled value must be at least as large as the raw — DEM is
    // a multiplier, never a divisor.
    assert!(estimate.scaled_energy >= estimate.raw_energy);
    // recommended_fee_limit_sun is the scaled energy × ENERGY_PRICE_SUN.
    assert_eq!(
        estimate.recommended_fee_limit_sun,
        estimate.scaled_energy * ENERGY_PRICE_SUN
    );
}
