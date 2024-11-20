use alloy_primitives::TxHash;
use alloy_rpc_types::Header;

use crate::Message;
use loom_types_blockchain::loom_data_types::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_blockchain::{GethStateUpdateVec, MempoolTx};

#[derive(Clone, Debug)]
pub struct NodeMempoolDataUpdate<D: LoomDataTypes = LoomDataTypesEthereum> {
    pub tx_hash: TxHash,
    pub mempool_tx: MempoolTx<D>,
}

#[derive(Clone, Debug)]
pub struct BlockUpdate<D: LoomDataTypes = LoomDataTypesEthereum> {
    pub block: D::Block,
}

#[derive(Clone, Debug)]
pub struct BlockStateUpdate<D: LoomDataTypes = LoomDataTypesEthereum> {
    pub block_header: D::Header,
    pub state_update: GethStateUpdateVec,
}

#[derive(Clone, Debug)]
pub struct BlockLogs<D: LoomDataTypes = LoomDataTypesEthereum> {
    pub block_header: D::Header,
    pub logs: Vec<D::Log>,
}

#[derive(Clone, Debug, Default)]
pub struct BlockHeader<D: LoomDataTypes = LoomDataTypesEthereum> {
    pub header: D::Header,
    pub next_block_number: u64,
    pub next_block_timestamp: u64,
}

pub type MessageMempoolDataUpdate<LDT = LoomDataTypesEthereum> = Message<NodeMempoolDataUpdate<LDT>>;

pub type MessageBlockHeader<LDT = LoomDataTypesEthereum> = Message<BlockHeader<LDT>>;
pub type MessageBlock<LDT = LoomDataTypesEthereum> = Message<BlockUpdate<LDT>>;
pub type MessageBlockLogs<LDT = LoomDataTypesEthereum> = Message<BlockLogs<LDT>>;
pub type MessageBlockStateUpdate<LDT = LoomDataTypesEthereum> = Message<BlockStateUpdate<LDT>>;

impl BlockHeader<LoomDataTypesEthereum> {
    pub fn new(header: Header) -> Self {
        let next_block_number = header.number + 1;
        let next_block_timestamp = header.timestamp + 12;
        Self { header, next_block_number, next_block_timestamp }
    }
}
