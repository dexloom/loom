use crate::pool_id::PoolId;
use crate::required_state::RequiredState;
use crate::{Pool, PoolAbiEncoder, PoolClass, PoolProtocol, PreswapRequirement, SwapDirection};
use alloy_primitives::{Address, U256};
use eyre::ErrReport;
use eyre::Result;
use revm::primitives::Env;
use revm::DatabaseRef;
use std::any::Any;

#[derive(Clone)]
pub struct MockPool {
    pub(crate) token0: Address,
    pub(crate) token1: Address,
    pub(crate) address: Address,
}

impl MockPool {
    pub fn new(token0: Address, token1: Address, address: Address) -> Self {
        Self { token0, token1, address }
    }
}

impl Pool for MockPool {
    fn as_any<'a>(&self) -> &dyn Any {
        self
    }

    fn get_class(&self) -> PoolClass {
        PoolClass::UniswapV2
    }

    fn get_protocol(&self) -> PoolProtocol {
        PoolProtocol::UniswapV2
    }

    fn get_address(&self) -> Address {
        self.address
    }

    fn get_pool_id(&self) -> PoolId {
        PoolId::Address(self.address)
    }

    fn get_fee(&self) -> U256 {
        U256::ZERO
    }

    fn get_tokens(&self) -> Vec<Address> {
        vec![self.token0, self.token1]
    }

    fn get_swap_directions(&self) -> Vec<SwapDirection> {
        vec![(self.token0, self.token1).into(), (self.token1, self.token0).into()]
    }

    fn calculate_out_amount(
        &self,
        state: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        in_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        panic!("Not implemented")
    }

    fn calculate_in_amount(
        &self,
        state: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        out_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        panic!("Not implemented")
    }

    fn can_flash_swap(&self) -> bool {
        panic!("Not implemented")
    }

    fn can_calculate_in_amount(&self) -> bool {
        true
    }

    fn get_abi_encoder(&self) -> Option<&dyn PoolAbiEncoder> {
        panic!("Not implemented")
    }

    fn get_read_only_cell_vec(&self) -> Vec<U256> {
        Vec::new()
    }

    fn get_state_required(&self) -> Result<RequiredState> {
        panic!("Not implemented")
    }

    fn is_native(&self) -> bool {
        false
    }

    fn preswap_requirement(&self) -> PreswapRequirement {
        PreswapRequirement::Base
    }
}
