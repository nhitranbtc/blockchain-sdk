//! Tx command dispatcher — Phase 7.1a scaffold.
//!
//! Real bodies land in Task 7.1d (P7-11 timeout clamp; get_signature_statuses + wait_for_confirm).

use anyhow::Result;

use crate::cli::{Cli, TxCmd};
use crate::handlers::AppContext;

pub async fn dispatch(cmd: &TxCmd, _ctx: &AppContext, _cli: &Cli) -> Result<()> {
    match cmd {
        TxCmd::Get { .. } => Err(anyhow::anyhow!(
            "Phase 7.1a scaffold — tx get lands in Task 7.1d"
        )),
        TxCmd::Wait { .. } => Err(anyhow::anyhow!(
            "Phase 7.1a scaffold — tx wait lands in Task 7.1d (P7-11 timeout clamp)"
        )),
    }
}
