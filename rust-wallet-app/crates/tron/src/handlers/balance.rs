//! `tron balance` handler — plan §Phase 6 Task 5.4.
//!
//! Two forms of one command: with `--token` it is a TRC-20 `balanceOf` read;
//! without, it is the native TRX balance via `chain::get_account`.

use std::path::Path;

use super::{emit, format_units, open_client, render_trx, Result};
use crate::cli::BalanceArgs;

/// `tron balance --address <T-addr> [--token ...]`.
pub async fn run(data_dir: &Path, args: BalanceArgs) -> Result<()> {
    if let Some(token) = args.token {
        return super::trc20::print_balance(
            data_dir,
            args.address,
            token,
            args.network,
            args.rpc_url,
            args.json,
        )
        .await;
    }

    let client = open_client(data_dir, args.network, args.rpc_url)?;
    let account = client.get_account(&args.address).await?;
    let sun = u128::from(account.balance_sun);

    emit(
        args.json,
        serde_json::json!({
            "address": args.address,
            "exists": account.exists,
            "balance_sun": account.balance_sun,
            "balance_trx": format_units(sun, 6),
        }),
        &render_trx(sun, args.unit),
    );
    // An unfunded address is a valid answer, not an error — but saying so keeps
    // an operator from reading "0" as "the node is broken".
    if !account.exists {
        eprintln!("note: this address has no on-chain record yet (never funded)");
    }
    Ok(())
}
