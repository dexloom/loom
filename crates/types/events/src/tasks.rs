use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::{PoolClass, PoolId};

#[derive(Clone, Debug)]
pub enum Task<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    FetchAndAddPools(Vec<(PoolId<LDT>, PoolClass)>),
    FetchStateAndAddPools(Vec<(PoolId<LDT>, PoolClass)>),
}
