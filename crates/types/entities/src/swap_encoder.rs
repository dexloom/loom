use crate::tips::Tips;
use crate::Swap;
use alloy_primitives::{Address, BlockNumber, Bytes, U256};
use eyre::Result;
use std::ops::Deref;
use std::sync::Arc;

pub trait SwapEncoder {
    /// Encodes Swap
    ///
    /// - next_block_number - number of the next block
    /// - next_block_gas_price - base_fee + priority fee for transaction
    /// - sender_address - EOA of of the transaction
    /// - sender_eth_balance - balance of EOA
    ///
    /// returns (to. value, call_data) for transaction
    #[allow(clippy::too_many_arguments)]
    fn encode(
        &self,
        swap: Swap,
        tips_pct: Option<u32>,
        next_block_number: Option<BlockNumber>,
        gas_cost: Option<U256>,
        sender_address: Option<Address>,
        sender_eth_balance: Option<U256>,
    ) -> Result<(Address, Option<U256>, Bytes, Vec<Tips>)>
    where
        Self: Sized;

    fn set_address(&mut self, address: Address);

    fn address(&self) -> Address;
}

#[derive(Clone)]
pub struct SwapEncoderWrapper {
    pub inner: Arc<dyn SwapEncoder>,
}

impl SwapEncoderWrapper {
    pub fn new(encoder: Arc<dyn SwapEncoder>) -> Self {
        SwapEncoderWrapper { inner: encoder }
    }
}

impl<T: 'static + SwapEncoder + Clone> From<T> for SwapEncoderWrapper {
    fn from(pool: T) -> Self {
        Self { inner: Arc::new(pool) }
    }
}

impl Deref for SwapEncoderWrapper {
    type Target = dyn SwapEncoder;
    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}
