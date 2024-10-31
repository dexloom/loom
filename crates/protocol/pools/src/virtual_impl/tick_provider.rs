use crate::db_reader::UniswapV3DBReader;
use alloy_primitives::{Address, U256};
use loom_evm_db::LoomDBType;
use loom_protocol_uniswap_v3_math::tick_provider::TickProvider;

pub struct TickProviderLoomDB<'a> {
    pub db: &'a LoomDBType,
    pub pool_address: Address,
}

impl<'a> TickProviderLoomDB<'a> {
    pub fn new(db: &'a LoomDBType, pool_address: Address) -> Self {
        TickProviderLoomDB { db, pool_address }
    }
}

impl<'a> TickProvider for TickProviderLoomDB<'a> {
    fn get_tick(&self, tick: i16) -> eyre::Result<U256> {
        UniswapV3DBReader::tick_bitmap(self.db, self.pool_address, tick)
    }
}
