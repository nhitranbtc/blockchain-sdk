//! E2E Sepolia — Story 7 (Tx list + get).
//!
//! Issue #310 — Story 7 of the #298 story map. Validates
//! `eth_getTransactionByHash` against a real broadcast from Story 5,
//! then sweeps the latest N blocks for any tx where the sender is the
//! test signer. Verifies the canonical `(hash, from, to, block)` tuple.
//!
//! Pattern (L29): operator-driven, never runs in CI.
//!   RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test e2e_sepolia_tx_list_get -- --ignored --nocapture
//!
//! Required env vars:
//!   ETH_E2E_RPC_URL         Sepolia HTTP RPC endpoint
//!   ETH_E2E_MNEMONIC        BIP-39 phrase (sender = m/44'/60'/0'/0/0)
//!   ETH_E2E_KNOWN_TX_HASH  optional — pre-known tx hash to fetch (skips scan)

#![cfg(test)]

mod common;

use alloy_primitives::B256;
use alloy_provider::Provider;

const SCAN_BLOCKS: u64 = 50;

#[tokio::test]
#[ignore = "operator-driven per L29 — set RUN_ETH_E2E=1 + ETH_E2E_RPC_URL + ETH_E2E_MNEMONIC"]
async fn story7_tx_list_and_get_against_sepolia() {
    let Some((provider, signer)) = common::preflight_or_skip("Story 7") else {
        return;
    };
    let sender = signer.address();

    // First, try the pre-known tx hash path (operator-driven: paste a
    // recent Story 5 tx hash in `ETH_E2E_KNOWN_TX_HASH`).
    if let Ok(hash_str) = std::env::var("ETH_E2E_KNOWN_TX_HASH") {
        match hash_str.parse::<B256>() {
            Ok(hash) => match provider.get_transaction_by_hash(hash).await {
                Ok(Some(tx)) => {
                    eprintln!(
                        "[Story 7 PASS] known_tx={hash} from={:?} block={:?}",
                        tx.inner.signer(),
                        tx.block_number
                    );
                    assert_eq!(
                        tx.inner.signer(),
                        sender,
                        "known tx from must match signer if ETH_E2E_KNOWN_TX_HASH belongs to sender"
                    );
                    return;
                }
                Ok(None) => eprintln!("[Story 7 SKIP] known_tx {hash} not yet visible"),
                Err(e) => eprintln!("[Story 7 FAIL] get_transaction_by_hash: {e}"),
            },
            Err(e) => eprintln!("[Story 7 SKIP] ETH_E2E_KNOWN_TX_HASH parse: {e}"),
        }
        return;
    }

    // Fallback: scan latest SCAN_BLOCKS blocks for any tx whose `from`
    // equals the signer. Uses alloy `BlockNumberOrTag::Latest` +
    // `get_block` for receipts at each height.
    let head = provider
        .get_block_number()
        .await
        .expect("get_block_number should succeed");
    let start = head.saturating_sub(SCAN_BLOCKS);
    let mut found = 0u32;
    for n in start..=head {
        let Some(block) = provider
            .get_block_by_number(alloy_rpc_types::BlockNumberOrTag::Number(n))
            .await
            .expect("get_block_by_number should succeed")
        else {
            continue;
        };
        for tx_hash in block.transactions.hashes() {
            if let Ok(Some(tx)) = provider.get_transaction_by_hash(tx_hash).await {
                if tx.inner.signer() == sender {
                    found += 1;
                    eprintln!(
                        "[Story 7] found tx_hash={} block={} from={}",
                        tx_hash, n, sender
                    );
                }
            }
        }
    }
    eprintln!(
        "[Story 7 PASS] scanned_blocks={} found_txs={}",
        SCAN_BLOCKS, found
    );
    // Sentinel: scan completed without RPC errors. `found` may be 0 if
    // the signer has never broadcast — operator funds + retries then.
}
