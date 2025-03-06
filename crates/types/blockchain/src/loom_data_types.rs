use crate::ChainParameters;
use std::fmt::{Debug, Display};
use std::hash::Hash;
pub trait LoomTx<LDT: LoomDataTypes> {
    fn gas_price(&self) -> u128;
    fn gas_limit(&self) -> u64;

    fn tx_hash(&self) -> LDT::TxHash;

    fn nonce(&self) -> u64;
    fn from(&self) -> LDT::Address;

    fn encode(&self) -> Vec<u8>;
}

pub trait LoomHeader<LDT: LoomDataTypes> {
    fn number(&self) -> u64;

    fn hash(&self) -> LDT::BlockHash;

    fn base_fee(&self) -> Option<u128>;

    fn next_base_fee(&self, params: &ChainParameters) -> u128;
}

pub trait LoomBlock<LDT: LoomDataTypes> {
    fn transactions(&self) -> Vec<LDT::Transaction>;

    fn number(&self) -> u64;
}

pub trait LoomDataTypes: Debug + Clone + Send + Sync {
    type Transaction: Debug + Clone + Send + Sync + LoomTx<Self>;
    type TransactionRequest: Debug + Clone + Send + Sync;
    type TransactionReceipt: Debug + Clone + Send + Sync;
    type Block: Default + Debug + Clone + Send + Sync + LoomBlock<Self>;
    type Header: Default + Debug + Clone + Send + Sync + LoomHeader<Self>;
    type Log: Default + Debug + Clone + Send + Sync;
    type StateUpdate: Default + Debug + Clone + Send + Sync;
    type BlockHash: Eq + Copy + Hash + Default + Display + Debug + Clone + Send + Sync;
    type TxHash: Eq + Copy + Hash + Default + Display + Debug + Clone + Send + Sync;
    type Address: Eq + Copy + Hash + Ord + Default + Display + Debug + Clone + Send + Sync;
    const WETH: Self::Address;
    fn is_weth(address: &Self::Address) -> bool;
}
