use alloy::consensus::TxEnvelope;
use alloy::eips::eip2718::Decodable2718;
use alloy::primitives::Bytes;
use alloy::rpc::types::Transaction;
use alloy::rpc::types::{BlockNumHash, Log as ALog};
use eyre::{OptionExt, Result};
use reth_db::models::StoredBlockBodyIndices;
use reth_primitives::{Block, Receipt, RecoveredBlock};

pub trait Convert<T> {
    fn convert(&self) -> T;
}

/// Appends all matching logs of a block's receipts.
/// If the log matches, look up the corresponding transaction hash.
pub fn append_all_matching_block_logs(
    all_logs: &mut Vec<ALog>,
    block_num_hash: BlockNumHash,
    receipts: Vec<Receipt>,
    removed: bool,
    block_body_indices: StoredBlockBodyIndices,
    block: RecoveredBlock<Block>,
) -> Result<()> {
    // Lazy loaded number of the first transaction in the block.
    // This is useful for blocks with multiple matching logs because it prevents
    // re-querying the block body indices.
    let loaded_first_tx_num = block_body_indices.first_tx_num;

    let mut tx_iter = block.body().transactions.iter();

    // Iterate over receipts and append matching logs.
    for (log_index, (receipt_idx, receipt)) in (0_u64..).zip(receipts.iter().enumerate()) {
        // The transaction hash of the current receipt.
        let transaction_hash = tx_iter.next().ok_or_eyre("NO_NEXT_TX")?.hash();

        for log in &receipt.logs {
            let log = ALog {
                inner: log.clone(),
                block_hash: Some(block_num_hash.hash),
                block_number: Some(block_num_hash.number),
                transaction_hash: Some(*transaction_hash),
                // The transaction and receipt index is always the same.
                transaction_index: Some(receipt_idx as u64 + loaded_first_tx_num),
                log_index: Some(log_index),
                removed,
                block_timestamp: Some(block.timestamp),
            };
            all_logs.push(log);
        }
    }
    Ok(())
}

/// Appends all matching logs of a block's receipts.
/// If the log matches, look up the corresponding transaction hash.
pub fn append_all_matching_block_logs_sealed(
    all_logs: &mut Vec<ALog>,
    block_num_hash: BlockNumHash,
    receipts: Vec<Receipt>,
    removed: bool,
    block: &RecoveredBlock<Block>,
) -> Result<()> {
    let mut tx_iter = block.body().transactions.iter();

    // Iterate over receipts and append matching logs.
    for (log_index, (receipt_idx, receipt)) in (0_u64..).zip(receipts.iter().enumerate()) {
        // The transaction hash of the current receipt.
        let transaction_hash = tx_iter.next().ok_or_eyre("NO_NEXT_TX")?.hash();

        for log in &receipt.logs {
            let log = ALog {
                inner: log.clone(),
                block_hash: Some(block_num_hash.hash),
                block_number: Some(block_num_hash.number),
                transaction_hash: Some(*transaction_hash),
                // The transaction and receipt index is always the same.
                transaction_index: Some(receipt_idx as u64),
                log_index: Some(log_index),
                removed,
                block_timestamp: Some(block.timestamp),
            };
            all_logs.push(log);
        }
    }
    Ok(())
}

pub fn decode_into_transaction(rlp_tx: &Bytes) -> Result<Transaction> {
    let raw_tx = rlp_tx.clone().to_vec();
    let mut raw_tx = raw_tx.as_slice();
    //let transaction_recovered: TransactionSignedEcRecovered = TransactionSignedEcRecovered::decode(&mut raw_tx)?;
    //let transaction_recovered: TransactionSignedEcRecovered = TransactionSignedEcRecovered::decode(&mut raw_tx)?;
    //let transaction_recovered = TransactionSignedEcRecovered::decode(&mut raw_tx)?;

    let inner = TxEnvelope::decode_2718(&mut raw_tx)?;
    let from = inner.recover_signer()?;

    let tx = Transaction { inner, block_hash: None, block_number: None, transaction_index: None, effective_gas_price: None, from };

    //let env: TxEnvelope = tx.into();

    //let tx: Transaction = transaction_recovered.transaction().try_into()?;

    Ok(tx)
}
