use crate::GethStateUpdate;
use alloy_primitives::{Address, BlockHash, TxHash};
use alloy_rpc_types_eth::{Block, Header, Log, Transaction, TransactionReceipt, TransactionRequest};
use std::fmt::{Debug, Display};
use std::hash::Hash;

pub trait LoomDataTypes: Debug + Clone + Send + Sync {
    type Transaction: Debug + Clone + Send + Sync;
    type TransactionRequest: Debug + Clone + Send + Sync;
    type TransactionReceipt: Debug + Clone + Send + Sync;
    type Block: Default + Debug + Clone + Send + Sync;
    type Header: Default + Debug + Clone + Send + Sync;
    type Log: Default + Debug + Clone + Send + Sync;
    type StateUpdate: Default + Debug + Clone + Send + Sync;
    type BlockHash: Eq + Copy + Hash + Default + Display + Debug + Clone + Send + Sync;
    type TxHash: Eq + Copy + Hash + Default + Display + Debug + Clone + Send + Sync;
    type Address: Eq + Copy + Hash + Ord + Default + Display + Debug + Clone + Send + Sync;
    const WETH: Self::Address;

    fn is_weth(address: &Self::Address) -> bool;
}

#[derive(Clone, Debug, Default)]
pub struct LoomDataTypesEthereum {
    _private: (),
}

impl LoomDataTypes for LoomDataTypesEthereum {
    type Transaction = Transaction;
    type TransactionRequest = TransactionRequest;
    type TransactionReceipt = TransactionReceipt;
    type Block = Block;
    type Header = Header;
    type Log = Log;
    type StateUpdate = GethStateUpdate;

    type BlockHash = BlockHash;
    type TxHash = TxHash;

    type Address = Address;

    const WETH: Self::Address = Address::ZERO;

    fn is_weth(address: &Self::Address) -> bool {
        address.eq(&Self::WETH)
    }
}
