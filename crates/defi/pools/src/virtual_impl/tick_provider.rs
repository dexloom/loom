use crate::db_reader::UniswapV3DBReader;
use alloy_primitives::{Address, U256};
use eyre::ErrReport;
use loom_defi_uniswap_v3_math::tick_provider::TickProvider;
use revm::DatabaseRef;

pub struct TickProviderLoomDB<'a, DB> {
    pub db: &'a DB,
    pub pool_address: Address,
}

impl<'a, DB> TickProviderLoomDB<'a, DB>
where
    DB: DatabaseRef<Error = ErrReport>,
{
    pub fn new(db: &'a DB, pool_address: Address) -> Self {
        TickProviderLoomDB { db, pool_address }
    }
}

impl<'a, DB> TickProvider for TickProviderLoomDB<'a, DB>
where
    DB: DatabaseRef<Error = ErrReport>,
{
    fn get_tick(&self, tick: i16) -> eyre::Result<U256> {
        UniswapV3DBReader::tick_bitmap(self.db, self.pool_address, tick)
    }
}
