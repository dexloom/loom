use alloy_primitives::{BlockHash, TxHash};
use alloy_rpc_types::Log;

use defi_types::{GethStateUpdateVec, MempoolTx};

use crate::Message;

#[derive(Clone, Debug)]
pub struct MempoolDataUpdate {
    pub tx_hash: TxHash,
    pub mempool_tx: MempoolTx,
}

pub type MessageMempoolDataUpdate = Message<MempoolDataUpdate>;


#[derive(Clone, Debug)]
pub struct BlockStateUpdate {
    pub block_hash: BlockHash,
    pub state_update: GethStateUpdateVec,
}

#[derive(Clone, Debug)]
pub struct BlockLogsUpdate {
    pub block_hash: BlockHash,
    pub logs: Vec<Log>,
}
