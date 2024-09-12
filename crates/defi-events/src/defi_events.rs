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
    MempoolActualTxUpdate { tx_hash: TxHash },
    MempoolTxUpdate { tx_hash: TxHash },
    MempoolTraceUpdate { tx_hash: TxHash },
    MempoolStateUpdate { tx_hash: TxHash },
    MempoolLogUpdate { tx_hash: TxHash },
}
