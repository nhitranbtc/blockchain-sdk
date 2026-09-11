//! Balance command dispatcher — Phase 7.1a scaffold.
//!
//! Real bodies land in Task 7.1d (raw RPC queries — no unlock needed).

use anyhow::Result;

use crate::cli::{BalanceCmd, Cli};
use crate::handlers::AppContext;

pub async fn dispatch(cmd: &BalanceCmd, _ctx: &AppContext, _cli: &Cli) -> Result<()> {
    match cmd {
        BalanceCmd::Sol { .. } => Err(anyhow::anyhow!(
            "Phase 7.1a scaffold — balance --address lands in Task 7.1d"
        )),
        BalanceCmd::Spl { .. } => Err(anyhow::anyhow!(
            "Phase 7.1a scaffold — balance --address --token lands in Task 7.1d"
        )),
    }
}
