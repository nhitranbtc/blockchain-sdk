//! `tron tx` handlers — plan §Phase 6 Task 5.6.
//!
//! `get` maps straight onto `TronGridClient::get_tx_info`. `wait` polls that
//! same call: the poll loop lives here rather than in the core because it is
//! scheduling, not chain semantics — no signing, no encoding, nothing an FFI
//! caller would want to inherit from us.

use std::path::Path;
use std::time::{Duration, Instant};

use tron_wallet_core::tx::broadcast::TransactionInfo;

use super::{emit, open_client, CliError, Result};
use crate::cli::NetworkArg;

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
    network: Option<NetworkArg>,
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
    network: Option<NetworkArg>,
    rpc_url: Option<String>,
    json: bool,
) -> Result<()> {
    if poll_interval_secs == 0 {
        return Err(CliError::BadInput(
            "--poll-interval must be at least 1 second".into(),
        ));
    }
    let client = open_client(data_dir, network, rpc_url)?;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let interval = Duration::from_secs(poll_interval_secs);

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
            Some(NetworkArg::Nile),
            None,
            false,
        )
        .await
        .expect_err("zero interval must be rejected before any network call");
        assert!(matches!(err, CliError::BadInput(_)));
    }
}
