use crate::db_reader::UniswapV3DBReader;
use alloy_primitives::{Address, U256};
use loom_revm_db::LoomDBType;
use uniswap_v3_math::tick_provider::TickProvider;

pub struct TickProviderLoomDB {
    pub db: LoomDBType,
    pub pool_address: Address,
}

impl TickProviderLoomDB {
    pub fn new(db: LoomDBType, pool_address: Address) -> Self {
        TickProviderLoomDB { db, pool_address }
    }
}

impl TickProvider for TickProviderLoomDB {
    fn get_tick(&self, tick: i16) -> eyre::Result<U256> {
        UniswapV3DBReader::tick_bitmap(&self.db, self.pool_address, tick)
    }
}
