use loom_types_entities::{EntityAddress, PoolClass};

#[derive(Clone, Debug)]
pub enum LoomTask {
    FetchAndAddPools(Vec<(EntityAddress, PoolClass)>),
}
