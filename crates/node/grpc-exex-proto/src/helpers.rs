use crate::proto::{Block, BlockReceipts};
use alloy_primitives::{BlockHash, TxHash};
use alloy_rpc_types::Log as ALog;
use eyre::OptionExt;
use reth_primitives::Receipt;

/// Appends all matching logs of a block's receipts.
/// If the log matches, look up the corresponding transaction hash.
pub fn append_all_matching_block_logs_sealed(receipts: BlockReceipts, removed: bool, block: Block) -> eyre::Result<Vec<ALog>> {
    // Tracks the index of a log in the entire block.
    let mut all_logs: Vec<ALog> = Vec::new();

    let mut log_index: u64 = 0;

    let mut tx_iter = block.body.iter();

    let header = block.header.clone().unwrap_or_default();

    let block_hash = BlockHash::try_from(header.hash.as_slice())?;
    let header = header.header.unwrap_or_default();

    let block_number = header.number;

    let receipts: Vec<Receipt> = receipts.receipts.iter().filter_map(|r| r.try_into().ok()).collect();

    // Iterate over receipts and append matching logs.
    for (receipt_idx, receipt) in receipts.iter().enumerate() {
        // The transaction hash of the current receipt.
        let transaction_hash = TxHash::try_from(tx_iter.next().ok_or_eyre("NO_NEXT_TX")?.hash.as_slice())?;

        for log in &receipt.logs {
            let log = ALog {
                inner: log.clone(),
                block_hash: Some(block_hash),
                block_number: Some(block_number),
                transaction_hash: Some(transaction_hash),
                // The transaction and receipt index is always the same.
                transaction_index: Some(receipt_idx as u64),
                log_index: Some(log_index),
                removed,
                block_timestamp: Some(header.timestamp),
            };
            all_logs.push(log);
            log_index += 1;
        }
    }
    Ok(all_logs)
}
