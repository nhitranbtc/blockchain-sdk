//! Tx command dispatcher — Phase 7.1d.
//!
//! - `tx get --sig` — fetch a confirmed transaction by signature.
//! - `tx wait --sig` — poll for confirmation up to `--timeout` (P7-11: 1..=600s).

use anyhow::{anyhow, Result};
use sol_wallet_core::chain::{get_signature_status, get_transaction, ConfirmationStatus};

use crate::cli::{Cli, TxCmd};
use crate::handlers::AppContext;

pub async fn dispatch(cmd: &TxCmd, ctx: &AppContext, _cli: &Cli) -> Result<()> {
    match cmd {
        TxCmd::Get { sig, json } => get_tx(ctx, sig, *json).await,
        TxCmd::Wait {
            sig,
            timeout,
            poll_interval,
            json,
        } => wait_tx(ctx, sig, *timeout, *poll_interval, *json).await,
    }
}

async fn get_tx(ctx: &AppContext, sig_str: &str, json: bool) -> Result<()> {
    let sig: solana_sdk::signature::Signature = sig_str
        .parse()
        .map_err(|e| anyhow!("invalid signature base58: {e}"))?;

    let rpc = sol_wallet_core::chain::RpcClient::new(&ctx.rpc_url)
        .map_err(|e| anyhow!("RpcClient::new: {e}"))?;
    let response = get_transaction(&rpc, &sig)
        .await
        .map_err(|e| anyhow!("get_transaction: {e}"))?;

    if json {
        let val = serde_json::json!({
            "signature": sig_str,
            "response": response,
        });
        println!("{}", serde_json::to_string_pretty(&val)?);
    } else {
        match response {
            Some(r) => {
                println!("slot: {}", r.slot);
                if let Some(bt) = r.block_time {
                    println!("block_time: {}", bt);
                }
                println!("meta: {}", r.meta);
            }
            None => println!("transaction not found (signature not yet confirmed or invalid)"),
        }
    }
    Ok(())
}

async fn wait_tx(
    ctx: &AppContext,
    sig_str: &str,
    timeout_secs: u64,
    poll_interval: u64,
    json: bool,
) -> Result<()> {
    // P7-11: 1..=600s timeout enforced by clap value_parser at parse time.
    let sig: solana_sdk::signature::Signature = sig_str
        .parse()
        .map_err(|e| anyhow!("invalid signature base58: {e}"))?;

    let rpc = sol_wallet_core::chain::RpcClient::new(&ctx.rpc_url)
        .map_err(|e| anyhow!("RpcClient::new: {e}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let poll_dur = std::time::Duration::from_secs(poll_interval.max(1));

    while std::time::Instant::now() < deadline {
        match get_signature_status(&rpc, &sig).await {
            Ok(Some(ts)) => {
                match ts.confirmation_status {
                    Some(ConfirmationStatus::Confirmed) => {
                        return print_status(sig_str, "confirmed", json);
                    }
                    Some(ConfirmationStatus::Finalized) => {
                        return print_status(sig_str, "finalized", json);
                    }
                    Some(ConfirmationStatus::Processed) | None => {
                        // processed < confirmed or unknown — keep polling.
                    }
                }
            }
            Ok(None) => {
                // signature not visible on cluster yet — keep polling.
            }
            Err(e) => {
                return Err(anyhow!("get_signature_status: {e}"));
            }
        }
        tokio::time::sleep(poll_dur).await;
    }
    Err(anyhow!(
        "timed out after {}s waiting for signature {}",
        timeout_secs,
        sig_str
    ))
}

fn print_status(sig: &str, status: &str, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "signature": sig,
                "status": status,
            }))?
        );
    } else {
        println!("{}: {}", sig, status);
    }
    Ok(())
}
