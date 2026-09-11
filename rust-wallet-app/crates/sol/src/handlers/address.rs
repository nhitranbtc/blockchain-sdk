//! Address command dispatcher — Phase 7.1a scaffold.
//!
//! Real bodies land in Task 7.1d (P7-16 reject `--mnemonic`; P7-17 derive_pubkey library fix).

use anyhow::Result;

use crate::cli::{AddressCmd, Cli};
use crate::handlers::AppContext;

pub async fn dispatch(cmd: &AddressCmd, _ctx: &AppContext, _cli: &Cli) -> Result<()> {
    match cmd {
        AddressCmd::New { .. } => Err(anyhow::anyhow!(
            "Phase 7.1a scaffold — address new lands in Task 7.1d (P7-16 reject --mnemonic)"
        )),
        AddressCmd::Pubkey { .. } => Err(anyhow::anyhow!(
            "Phase 7.1a scaffold — address pubkey lands in Task 7.1d (P7-17 derive_pubkey)"
        )),
    }
}
