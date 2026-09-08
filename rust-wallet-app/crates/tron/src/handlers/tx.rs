//! `tron tx` handlers — plan §Phase 6 Task 5.6.
//!
//! `get` maps straight onto `TronGridClient::get_tx_info`. `wait` polls that
//! same call: the poll loop lives here rather than in the core because it is
//! scheduling, not chain semantics — no signing, no encoding, nothing an FFI
//! caller would want to inherit from us.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tron_wallet_core::tx::broadcast::TransactionInfo;
use tron_wallet_core::tx::submit;

use super::{emit, open_client, CliError, Result};
use crate::cli::Network;

/// Renders a `TransactionInfo` for both `get` and `wait`.
fn render(info: &TransactionInfo, json: bool) {
    let value = serde_json::json!({
        "txid": info.id,
        "block_number": info.block_number,
        "contract_result": info.contract_result,
        "fee": info.fee,
    });
    let plain = format!(
        "txid          {}\nblock         {}\nfee           {}\ncontract_res  {}",
        info.id.clone().unwrap_or_else(|| "-".into()),
        info.block_number
            .map(|b| b.to_string())
            .unwrap_or_else(|| "-".into()),
        info.fee
            .map(|f| f.to_string())
            .unwrap_or_else(|| "-".into()),
        if info.contract_result.is_empty() {
            "-".to_string()
        } else {
            info.contract_result.join(",")
        }
    );
    emit(json, value, &plain);
}

/// A transaction counts as confirmed once the node reports a block number.
fn is_confirmed(info: &TransactionInfo) -> bool {
    info.block_number.is_some()
}

/// `tron tx get --txid`.
pub async fn get(
    data_dir: &Path,
    txid: String,
    network: Option<Network>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    let client = open_client(data_dir, network, rpc_url)?;
    let info = client.get_tx_info(&txid).await?;
    render(&info, json);
    Ok(())
}

/// `tron tx wait --txid [--timeout] [--poll-interval]`.
///
/// A timeout is a failure (exit 3), not a success with an empty result: a
/// script that reads "not yet confirmed" as "confirmed" ships goods unpaid.
#[allow(clippy::too_many_arguments)]
pub async fn wait(
    data_dir: &Path,
    txid: String,
    timeout_secs: u64,
    poll_interval_secs: u64,
    network: Option<Network>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    let interval = Duration::from_secs(poll_interval_secs);
    // Validated through the core helper so `tx wait` and `wallet send --wait`
    // cannot disagree about what a zero interval means. The core answers with
    // `Error::Config`, which `exit_code` maps to 2 (operator input) — the same
    // class as the `BadInput` this used to return.
    submit::validate_poll_interval(interval)?;
    let client = open_client(data_dir, network, rpc_url)?;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        // A not-yet-indexed txid comes back as an empty body rather than an
        // error, so a transport failure here is still fatal.
        let info = client.get_tx_info(&txid).await?;
        if is_confirmed(&info) {
            render(&info, json);
            return Ok(());
        }
        if Instant::now() + interval > deadline {
            return Err(CliError::Core(tron_wallet_core::Error::Node(format!(
                "{txid} not confirmed within {timeout_secs}s"
            ))));
        }
        eprintln!("{txid} pending; retrying in {poll_interval_secs}s");
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(block: Option<u64>) -> TransactionInfo {
        TransactionInfo {
            id: Some("deadbeef".into()),
            block_number: block,
            contract_result: vec![],
            fee: None,
        }
    }

    #[test]
    fn confirmation_requires_a_block_number() {
        assert!(!is_confirmed(&info(None)));
        assert!(is_confirmed(&info(Some(1))));
    }

    #[tokio::test]
    async fn wait_rejects_a_zero_poll_interval() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err = wait(
            dir.path(),
            "deadbeef".into(),
            10,
            0,
            Some(Network::Nile),
            None,
            false,
        )
        .await
        .expect_err("zero interval must be rejected before any network call");
        // Now sourced from the core helper: `Config` maps to exit 2, the same
        // operator-input class as the `BadInput` this used to return.
        assert!(matches!(
            err,
            CliError::Core(tron_wallet_core::Error::Config(_))
        ));
        assert_eq!(super::super::exit_code(&err), 2);
    }
}

/// `tron tx broadcast --hex <envelope> [--network]`] [--rpc-url]`.
///
/// Re-POST a previously signed envelope. Plan §Task 7.15 spike invariant:
/// the black-box `trc20_nile::row_2` rebroadcast-idempotency test asserts
/// this surface, then expects a `DUP_TRANSACTION_ERROR` (or local-network
/// `DUP` sentinel) when the same envelope hits the network a second time.
///
/// `hex` and `file` are mutually exclusive: `hex` carries the raw envelope
/// directly, `file` reads `{txid, signed_envelope_hex}` JSON. Either way,
/// the result is the broadcast receipt rendered through `report_broadcast`.
#[allow(clippy::too_many_arguments)]
pub async fn broadcast(
    data_dir: &Path,
    hex: Option<String>,
    file: Option<PathBuf>,
    network: Option<Network>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    use crate::handlers::wallet::report_broadcast;

    let envelope = match (hex, file) {
        (Some(h), _) => h,
        (None, Some(path)) => {
            let raw = std::fs::read_to_string(&path).map_err(|e| {
                CliError::Core(tron_wallet_core::Error::Config(format!(
                    "read {}: {e}",
                    path.display()
                )))
            })?;
            let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
                CliError::BadInput(format!(
                    "broadcast file must be JSON with signed_envelope_hex: {e}"
                ))
            })?;
            v.get("signed_envelope_hex")
                .and_then(|x| x.as_str())
                .ok_or_else(|| {
                    CliError::BadInput(
                        "broadcast file JSON missing `signed_envelope_hex` string".into(),
                    )
                })?
                .to_string()
        }
        (None, None) => {
            return Err(CliError::BadInput(
                "one of --hex or --file is required".into(),
            ))
        }
    };
    let client = open_client(data_dir, network, rpc_url)?;
    let receipt = client.broadcast(&envelope).await?;
    // txid is unknown when the caller hands us a raw envelope; fall back to
    // "unknown" in the JSON output rather than panicking.
    report_broadcast("unknown", &receipt, json)
}
