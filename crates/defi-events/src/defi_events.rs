use alloy_primitives::{BlockHash, BlockNumber, TxHash};

#[derive(Clone, Debug)]
pub enum MarketEvents {
    BlockHeaderUpdate { block_number: BlockNumber, block_hash: BlockHash, timestamp: u64, base_fee: u128, next_base_fee: u128 },
    BlockTxUpdate { block_number: BlockNumber, block_hash: BlockHash },
    BlockLogsUpdate { block_number: BlockNumber, block_hash: BlockHash },
    BlockStateUpdate { block_hash: BlockHash },
}

#[derive(Clone, Debug)]
pub enum MempoolEvents {
    /// The transaction has a valid nonce and provides enough gas to pay for the base fee of the next block.
    MempoolActualTxUpdate {
        tx_hash: TxHash,
    },
    /// The transaction has been added to the mempool without any validation.
    MempoolTxUpdate {
        tx_hash: TxHash,
    },
    MempoolStateUpdate {
        tx_hash: TxHash,
    },
    MempoolLogUpdate {
        tx_hash: TxHash,
    },
}
