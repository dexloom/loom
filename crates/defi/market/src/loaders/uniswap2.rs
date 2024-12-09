use alloy_primitives::Bytes;
use loom_defi_pools::protocols::UniswapV2Protocol;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::{PoolClass, PoolId, PoolLoader, PoolWrapper};
use revm::primitives::Env;
use std::marker::PhantomData;

pub struct UniswapV2Loader<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    phantom_data: PhantomData<LDT>,
}

impl<LDT: LoomDataTypes> PoolLoader for UniswapV2Loader<LDT> {
    fn get_pool_class_by_log(&self, log_entry: LoomDataTypesEthereum::Log) -> Option<(PoolId<LoomDataTypesEthereum>, PoolClass)> {
        todo!()
    }

    fn fetch_pool_by_id_from_provider(&self, pool_id: PoolId<LoomDataTypesEthereum>) -> eyre::Result<PoolWrapper<LoomDataTypesEthereum>> {
        todo!()
    }

    fn fetch_pool_by_id_from_evm(
        &self,
        pool_id: PoolId<LoomDataTypesEthereum>,
        env: Env,
    ) -> eyre::Result<PoolWrapper<LoomDataTypesEthereum>> {
        todo!()
    }

    fn is_code(&self, code: &Bytes) -> bool {
        UniswapV2Protocol::is_code(code)
    }
}
