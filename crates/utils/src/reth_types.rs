use alloy_primitives::{B256, B64};
use alloy_rpc_types::{Block as ABlock, BlockNumHash, BlockTransactions, Header as AHeader, Log as ALog};
use eyre::{OptionExt, Result};
use reth_db::models::StoredBlockBodyIndices;
use reth_primitives::{Block as RBlock, BlockWithSenders, Header as RHeader, Receipt, SealedBlockWithSenders};

pub trait Convert<T> {
    fn convert(&self) -> T;
}

impl Convert<AHeader> for RHeader {
    fn convert(&self) -> AHeader {
        AHeader {
            hash: Some(self.hash_slow()),
            parent_hash: self.parent_hash,
            uncles_hash: B256::default(),
            miner: self.beneficiary,
            state_root: self.state_root,
            transactions_root: self.transactions_root,
            receipts_root: self.receipts_root,
            logs_bloom: self.logs_bloom,
            difficulty: self.difficulty,
            number: Some(self.number),
            gas_limit: self.gas_limit as u128,
            gas_used: self.gas_used as u128,
            timestamp: self.timestamp,
            total_difficulty: None,
            extra_data: self.extra_data.clone(),
            mix_hash: Some(self.mix_hash),
            nonce: Some(B64::from(self.nonce)),
            base_fee_per_gas: self.base_fee_per_gas.map(|x| x as u128),
            withdrawals_root: self.withdrawals_root,
            blob_gas_used: self.blob_gas_used.map(|x| x as u128),
            excess_blob_gas: self.excess_blob_gas.map(|x| x as u128),
            parent_beacon_block_root: self.parent_beacon_block_root,
            requests_root: self.requests_root,
        }
    }
}

impl Convert<ABlock> for RBlock {
    fn convert(&self) -> ABlock {
        ABlock {
            header: self.header.convert(),
            uncles: vec![],
            transactions: BlockTransactions::Uncle,
            size: None,
            withdrawals: None,
            other: Default::default(),
        }
    }
}
//
// impl Convert<ATransaction> for RTransactionSigned {
//     fn convert(&self) -> ATransaction {
//         reth_rpc_types_compat::transaction::from_recovered(TransactionSignedEcRecovered::from_signed_transaction(self.clone(), Address::ZERO))
//     }
// }

/// Appends all matching logs of a block's receipts.
/// If the log matches, look up the corresponding transaction hash.
pub fn append_all_matching_block_logs(
    all_logs: &mut Vec<ALog>,
    block_num_hash: BlockNumHash,
    receipts: Vec<Receipt>,
    removed: bool,
    block_body_indices: StoredBlockBodyIndices,
    block: BlockWithSenders,
) -> Result<()> {
    // Lazy loaded number of the first transaction in the block.
    // This is useful for blocks with multiple matching logs because it prevents
    // re-querying the block body indices.
    let loaded_first_tx_num = block_body_indices.first_tx_num;

    let mut tx_iter = block.transactions();

    // Iterate over receipts and append matching logs.
    for (log_index, (receipt_idx, receipt)) in (0_u64..).zip(receipts.iter().enumerate()) {
        // The transaction hash of the current receipt.
        let transaction_hash = tx_iter.next().ok_or_eyre("NO_NEXT_TX")?.hash();

        for log in &receipt.logs {
            let log = ALog {
                inner: log.clone(),
                block_hash: Some(block_num_hash.hash),
                block_number: Some(block_num_hash.number),
                transaction_hash: Some(transaction_hash),
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
    block: &SealedBlockWithSenders,
) -> Result<()> {
    let mut tx_iter = block.body.iter();

    // Iterate over receipts and append matching logs.
    for (log_index, (receipt_idx, receipt)) in (0_u64..).zip(receipts.iter().enumerate()) {
        // The transaction hash of the current receipt.
        let transaction_hash = tx_iter.next().ok_or_eyre("NO_NEXT_TX")?.hash();

        for log in &receipt.logs {
            let log = ALog {
                inner: log.clone(),
                block_hash: Some(block_num_hash.hash),
                block_number: Some(block_num_hash.number),
                transaction_hash: Some(transaction_hash),
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
