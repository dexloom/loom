use alloy_primitives::{Address, U256};
use eyre::{eyre, Report};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug)]
pub struct SwapError {
    pub msg: String,
    pub pool: Address,
    pub token_from: Address,
    pub token_to: Address,
    pub is_in_amount: bool,
    pub amount: U256,
}

impl From<SwapError> for Report {
    fn from(value: SwapError) -> Self {
        eyre!(value.msg)
    }
}

impl Hash for SwapError {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pool.hash(state);
        self.token_from.hash(state);
        self.token_to.hash(state);
    }
}

impl PartialEq<Self> for SwapError {
    fn eq(&self, other: &Self) -> bool {
        self.pool == other.pool && self.token_to == other.token_to && self.token_from == other.token_from
    }
}

impl Eq for SwapError {}
