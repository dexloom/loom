use alloy_primitives::{Address, U256};
use eyre::{eyre, Report};

#[derive(Clone, Debug)]
pub struct SwapError {
    pub msg: String,
    pub pool: Address,
    pub token_from: Address,
    pub token_to: Address,
    pub amount: U256,
}


impl From<SwapError> for Report {
    fn from(value: SwapError) -> Self {
        eyre!(value.msg)
    }
}

