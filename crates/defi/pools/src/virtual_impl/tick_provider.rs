use crate::db_reader::UniswapV3DBReader;
use alloy_primitives::{Address, U256};
use loom_defi_uniswap_v3_math::tick_provider::TickProvider;
use revm::DatabaseRef;

pub struct TickProviderEVMDB<'a, DB> {
    pub db: &'a DB,
    pub pool_address: Address,
}

impl<'a, DB> TickProviderEVMDB<'a, DB>
where
    DB: DatabaseRef,
{
    pub fn new(db: &'a DB, pool_address: Address) -> Self {
        TickProviderEVMDB { db, pool_address }
    }
}

impl<'a, DB> TickProvider for TickProviderEVMDB<'a, DB>
where
    DB: DatabaseRef,
{
    fn get_tick(&self, tick: i16) -> eyre::Result<U256> {
        UniswapV3DBReader::tick_bitmap(self.db, self.pool_address, tick)
    }
}
