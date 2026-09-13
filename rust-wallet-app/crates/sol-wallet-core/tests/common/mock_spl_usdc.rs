//! Mock USDC mint deploy helper for surfpool-backed SPL tests.

use sol_wallet_core::{chain::client::RpcClient, Error};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};

/// Mint authority holder — kept in scope so the test can mint more
/// supply later or burn, etc.
pub struct DeployedMint {
    pub mint: Pubkey,
    pub mint_authority: Keypair,
    pub decimals: u8,
}

/// Deploy a fresh classic SPL Token mint on surfpool and mint
/// `initial_supply` raw units to `holder`.
///
/// Phase 7.2 SPL fixture wire-up stub: returns `Error::Unimplemented`
/// until the SPL mint helpers land. Tests exercise the structured-error
/// path so the contract is unambiguous when the fixture arrives.
pub async fn deploy_usdc_mint(
    _rpc: &RpcClient,
    _payer: &Keypair,
    _holder: &Pubkey,
    _initial_supply_raw: u64,
) -> Result<DeployedMint, Error> {
    Err(Error::Unimplemented(
        "deploy_usdc_mint lands with Phase 7.2 SPL fixture wiring",
    ))
}
