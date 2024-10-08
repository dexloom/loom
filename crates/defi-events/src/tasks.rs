use alloy_primitives::Address;
use defi_entities::PoolClass;

#[derive(Clone, Debug)]
pub enum Task {
    FetchAndAddPools(Vec<(Address, PoolClass)>),
    FetchStateAndAddPools(Vec<(Address, PoolClass)>),
}
