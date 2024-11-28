use alloy_primitives::{Address, Bytes, U256};

#[derive(Clone, Debug)]
pub struct CallbackSequence {
    pub pre_swap_calls: Vec<(Address, Bytes, Option<U256>)>,
    pub post_swap_calls: Vec<(Address, Bytes, Option<U256>)>,
}

#[derive(Clone, Debug)]
pub enum CallSequence {
    Standard { 
        pre_calls: Vec<(Address, Bytes, Option<U256>)>,
        post_calls: Vec<(Address, Bytes, Option<U256>)>,
    },
    FlashLoan { 
        pre_flashloan: Vec<(Address, Bytes, Option<U256>)>,
        flashloan_params: FlashLoanParams,
        callback_sequence: CallbackSequence,
        post_flashloan: Vec<(Address, Bytes, Option<U256>)>,
    }
}

#[derive(Clone, Debug)]
pub struct FlashLoanParams {
    pub token: Address,
    pub amount: U256,
    pub recipient: Address,
}