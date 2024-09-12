use alloy_primitives::{BlockHash, TxHash};
use alloy_rpc_types::{Header, Log};

use defi_types::{ChainParameters, GethStateUpdateVec, MempoolTx};

use crate::Message;

#[derive(Clone, Debug)]
pub struct NodeMempoolDataUpdate {
    pub tx_hash: TxHash,
    pub mempool_tx: MempoolTx,
}

pub type MessageMempoolDataUpdate = Message<NodeMempoolDataUpdate>;

#[derive(Clone, Debug)]
pub struct BlockStateUpdate {
    pub block_hash: BlockHash,
    pub state_update: GethStateUpdateVec,
}

#[derive(Clone, Debug)]
pub struct BlockLogs {
    pub block_hash: BlockHash,
    pub logs: Vec<Log>,
}

#[derive(Clone, Debug, Default)]
pub struct BlockHeader {
    pub header: Header,
    pub next_block_base_fee: u128,
    pub next_block_number: u64,
    pub next_block_timestamp: u64,
}

pub type MessageBlockHeader = Message<BlockHeader>;

impl BlockHeader {
    pub fn new(chain_parameters: ChainParameters, header: Header) -> Self {
        let next_block_base_fee: u128 =
            chain_parameters.calc_next_block_base_fee(header.gas_used, header.gas_limit, header.base_fee_per_gas.unwrap_or_default());
        let next_block_number = header.number + 1;
        let next_block_timestamp = header.timestamp + 12;
        Self { header, next_block_base_fee, next_block_number, next_block_timestamp }
    }
}
