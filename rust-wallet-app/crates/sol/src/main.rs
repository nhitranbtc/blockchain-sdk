//! `sol` — Solana wallet CLI (Phase 0 skeleton).
//!
//! Phase 0 ships a clap-driven `--help` only. The 22 subcommands
//! (`wallet create/show`, `address new/from-base58`, `tx transfer/...`,
//! `spl send/create-ata`, `rpc ...`, etc.) land in Phase 7 per plan
//! §Phase 7 Task 7.1. The CLI is a thin shim over `sol-wallet-core`;
//! no signing or persistence happens here.

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "sol",
    version,
    about = "Solana wallet CLI — Anza stack, Anza-owned crypto"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Phase 0 placeholder. Real subcommands land in Phase 7.
    #[command(hide = true)]
    Placeholder,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Some(Cmd::Placeholder) => {
            eprintln!("sol: placeholder (Phase 0). Subcommands land in Phase 7.");
            Ok(())
        }
        None => {
            // No subcommand → clap already printed help + exit 2 via `parse`.
            // Unreachable in practice; kept for completeness.
            Ok(())
        }
    }
}
