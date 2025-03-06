use alloy_primitives::BlockNumber;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::PoolId;

#[derive(Clone, Debug)]
pub enum MarketEvents<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    BlockHeaderUpdate { block_number: BlockNumber, block_hash: LDT::BlockHash, timestamp: u64, base_fee: u64, next_base_fee: u64 },
    BlockTxUpdate { block_number: BlockNumber, block_hash: LDT::BlockHash },
    BlockLogsUpdate { block_number: BlockNumber, block_hash: LDT::BlockHash },
    BlockStateUpdate { block_hash: LDT::BlockHash },
    NewPoolLoaded { pool_id: PoolId<LDT>, swap_path_idx_vec: Vec<usize> },
}

#[derive(Clone, Debug)]
pub enum MempoolEvents<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    /// The transaction has a valid nonce and provides enough gas to pay for the base fee of the next block.
    MempoolActualTxUpdate {
        tx_hash: LDT::TxHash,
    },
    /// The transaction has been added to the mempool without any validation.
    MempoolTxUpdate {
        tx_hash: LDT::TxHash,
    },
    MempoolStateUpdate {
        tx_hash: LDT::TxHash,
    },
    MempoolLogUpdate {
        tx_hash: LDT::TxHash,
    },
}
