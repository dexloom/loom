use crate::loom_data_types::{LoomDataTypes, LoomDataTypesEthereum};
use alloy_primitives::{Address, U256};
use eyre::{eyre, Report};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug)]
pub struct SwapError<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub msg: String,
    pub pool: LDT::Address,
    pub token_from: LDT::Address,
    pub token_to: LDT::Address,
    pub is_in_amount: bool,
    pub amount: U256,
}

impl<LDT: LoomDataTypes> From<SwapError<LDT>> for Report {
    fn from(value: SwapError<LDT>) -> Self {
        eyre!(value.msg)
    }
}

impl<LDT: LoomDataTypes> Hash for SwapError<LDT> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pool.hash(state);
        self.token_from.hash(state);
        self.token_to.hash(state);
    }
}

impl<LDT: LoomDataTypes> PartialEq<Self> for SwapError<LDT> {
    fn eq(&self, other: &Self) -> bool {
        self.pool == other.pool && self.token_to == other.token_to && self.token_from == other.token_from
    }
}

impl<LDT: LoomDataTypes> Eq for SwapError<LDT> {}
