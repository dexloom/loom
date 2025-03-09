use crate::{EntityAddress, SwapPath};
use alloy_primitives::U256;
use eyre::{eyre, Report};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug)]
pub struct EstimationError {
    pub msg: String,
    pub swap_path: SwapPath,
}

impl PartialEq<Self> for EstimationError {
    fn eq(&self, other: &Self) -> bool {
        self.swap_path == other.swap_path
    }
}

impl Eq for EstimationError {}

#[derive(Clone, Debug)]
pub struct SwapError {
    pub msg: String,
    pub pool: EntityAddress,
    pub token_from: EntityAddress,
    pub token_to: EntityAddress,
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
