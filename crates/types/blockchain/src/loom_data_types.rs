use crate::GethStateUpdate;
use alloy_primitives::{Address, BlockHash, TxHash};
use alloy_rpc_types_eth::{Block, Header, Log, Transaction};
use std::fmt::Debug;
use std::hash::Hash;

pub trait LoomDataTypes: Debug + Clone + Send + Sync {
    type Transaction: Debug + Clone + Send + Sync;
    type Block: Default + Debug + Clone + Send + Sync;
    type Header: Default + Debug + Clone + Send + Sync;
    type Log: Default + Debug + Clone + Send + Sync;
    type StateUpdate: Default + Debug + Clone + Send + Sync;
    type BlockHash: Eq + Hash + Default + Debug + Clone + Send + Sync;
    type TxHash: Eq + Hash + Default + Debug + Clone + Send + Sync;
    type Address: Default + Debug + Clone + Send + Sync;
}

#[derive(Clone, Debug, Default)]
pub struct LoomDataTypesEthereum {
    _private: (),
}

impl LoomDataTypes for LoomDataTypesEthereum {
    type Transaction = Transaction;
    type Block = Block;
    type Header = Header;
    type Log = Log;
    type StateUpdate = GethStateUpdate;

    type BlockHash = BlockHash;
    type TxHash = TxHash;

    type Address = Address;
}
