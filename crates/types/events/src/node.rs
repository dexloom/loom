use alloy_rpc_types::Header;

use crate::Message;
use loom_types_blockchain::{GethStateUpdateVec, MempoolTx};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};

#[derive(Clone, Debug)]
pub struct NodeMempoolDataUpdate<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub tx_hash: LDT::TxHash,
    pub mempool_tx: MempoolTx<LDT>,
}

#[derive(Clone, Debug)]
pub struct BlockUpdate<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub block: LDT::Block,
}

#[derive(Clone, Debug)]
pub struct BlockStateUpdate<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub block_header: LDT::Header,
    pub state_update: GethStateUpdateVec,
}

#[derive(Clone, Debug)]
pub struct BlockLogs<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub block_header: LDT::Header,
    pub logs: Vec<LDT::Log>,
}

#[derive(Clone, Debug, Default)]
pub struct BlockHeader<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub header: LDT::Header,
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
