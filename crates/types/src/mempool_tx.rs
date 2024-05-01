use alloy_primitives::{BlockNumber, TxHash};
use alloy_rpc_types::{Log, Transaction};
use chrono::{DateTime, Utc};

use crate::{FetchState, GethStateUpdate};

#[derive(Clone, Debug)]
pub struct MempoolTx {
    pub tx_hash: TxHash,
    pub time: DateTime<Utc>,
    pub tx: Option<Transaction>,
    pub logs: Option<Vec<Log>>,
    pub mined: Option<BlockNumber>,
    pub failed: Option<bool>,
    pub state_update: Option<GethStateUpdate>,
    pub pre_state: Option<FetchState<GethStateUpdate>>,
}

impl MempoolTx {
    pub fn new() -> MempoolTx {
        MempoolTx {
            ..MempoolTx::default()
        }
    }
    pub fn new_with_hash(tx_hash: TxHash) -> MempoolTx {
        MempoolTx {
            tx_hash,
            ..MempoolTx::default()
        }
    }
}

impl Default for MempoolTx {
    fn default() -> Self {
        MempoolTx {
            tx_hash: TxHash::repeat_byte(0),
            time: chrono::Utc::now(),
            tx: None,
            state_update: None,
            logs: None,
            mined: None,
            failed: None,
            pre_state: None,
        }
    }
}

