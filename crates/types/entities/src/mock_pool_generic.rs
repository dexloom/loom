use crate::pool_id::PoolId;
use crate::required_state::RequiredState;
use crate::{Pool, PoolAbiEncoder, PoolClass, PoolProtocol, PreswapRequirement, SwapDirection};
use alloy_network::Ethereum;
use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use alloy_rpc_types::BlockNumberOrTag;
use eyre::Result;
use eyre::{eyre, ErrReport};
use loom_evm_db::{AlloyDB, LoomDBType};
use revm::primitives::Env;
use revm::DatabaseRef;
use std::any::Any;

#[derive(Clone)]
pub struct MockPoolGeneric<P> {
    pub(crate) client: P,
    pub(crate) token0: Address,
    pub(crate) token1: Address,
    pub(crate) address: Address,
}

impl<P> Pool for MockPoolGeneric<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
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
        let alloy_db = AlloyDB::new(self.client.clone(), BlockNumberOrTag::Latest.into()).ok_or(eyre!("ALLOY_DB_NOT_CREATED"))?;
        let state = LoomDBType::new().with_ext_db(alloy_db);

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
