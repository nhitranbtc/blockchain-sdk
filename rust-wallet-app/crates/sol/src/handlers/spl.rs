//! SPL command dispatcher — Phase 7.1c stub.
//!
//! Input validation only (P7-14 + P7-23). Real RPC + signing bodies land
//! in 7.1c when WalletManager + chain client are wired through.

use anyhow::Result;

use crate::cli::{Cli, SplCmd};
use crate::handlers::AppContext;

pub async fn dispatch(cmd: &SplCmd, _ctx: &AppContext, _cli: &Cli) -> Result<()> {
    match cmd {
        SplCmd::Send {
            memo,
            skip_memo_required,
            i_understand_no_memo_enforcement,
            ..
        } => {
            // P7-14: clap `requires` enforces this at parse time; defense in
            // depth in case a future contributor drops the clap constraint.
            if *skip_memo_required && !*i_understand_no_memo_enforcement {
                return Err(anyhow::anyhow!(
                    "--skip-memo-required requires --i-understand-no-memo-enforcement"
                ));
            }
            // P7-23: SPL Memo program accepts up to 566 bytes; reject longer.
            if let Some(m) = memo {
                if m.len() > 566 {
                    return Err(anyhow::anyhow!(
                        "memo length {} exceeds SPL Memo program max 566 bytes",
                        m.len()
                    ));
                }
                if m.contains('\0') {
                    return Err(anyhow::anyhow!("memo may not contain NUL byte"));
                }
            }
            Err(anyhow::anyhow!(
                "spl send deferred to Task 7.1c — input validated (P7-14 + P7-23)"
            ))
        }
        SplCmd::Approve { .. } => Err(anyhow::anyhow!(
            "Phase 7.1c stub — spl approve lands with RPC + signing"
        )),
        SplCmd::Balance { .. } => Err(anyhow::anyhow!(
            "Phase 7.1c stub — spl balance lands with RPC"
        )),
        SplCmd::Allowance { .. } => Err(anyhow::anyhow!(
            "Phase 7.1c stub — spl allowance lands with RPC"
        )),
    }
}
