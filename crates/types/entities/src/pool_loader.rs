use crate::{PoolClass, PoolId, PoolWrapper};
use alloy_primitives::Bytes;
use eyre::Result;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use reth_revm::primitives::Env;
use std::collections::HashMap;
use std::sync::Arc;

pub trait PoolLoader<LDT: LoomDataTypes = LoomDataTypesEthereum>: Send + Sync {
    fn get_pool_class_by_log(&self, log_entry: LDT::Log) -> Option<(PoolId<LDT>, PoolClass)>;
    fn fetch_pool_by_id_from_provider(&self, pool_id: PoolId<LDT>) -> Result<PoolWrapper<LDT>>;
    fn fetch_pool_by_id_from_evm(&self, pool_id: PoolId<LDT>, env: Env) -> Result<PoolWrapper<LDT>>;
    fn is_code(&self, code: &Bytes) -> bool;
}

pub struct PoolLoaders<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    map: HashMap<PoolClass, Arc<dyn PoolLoader<LDT>>>,
}
