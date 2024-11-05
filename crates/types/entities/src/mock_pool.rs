use crate::required_state::RequiredState;
use crate::{AbiSwapEncoder, Pool, PoolClass, PoolProtocol};
use alloy_primitives::{Address, U256};
use eyre::ErrReport;
use eyre::Result;
use revm::primitives::Env;
use revm::DatabaseRef;

#[derive(Clone)]
pub struct MockPool {
    pub(crate) token0: Address,
    pub(crate) token1: Address,
    pub(crate) address: Address,
}

impl Pool for MockPool {
    fn get_class(&self) -> PoolClass {
        PoolClass::UniswapV2
    }

    fn get_protocol(&self) -> PoolProtocol {
        PoolProtocol::UniswapV2
    }

    fn get_address(&self) -> Address {
        self.address
    }

    fn get_tokens(&self) -> Vec<Address> {
        vec![self.token0, self.token1]
    }

    fn get_swap_directions(&self) -> Vec<(Address, Address)> {
        vec![(self.token0, self.token1), (self.token1, self.token0)]
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

    fn get_encoder(&self) -> &dyn AbiSwapEncoder {
        panic!("Not implemented")
    }

    fn get_state_required(&self) -> Result<RequiredState> {
        panic!("Not implemented")
    }
}
