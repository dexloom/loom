use std::ops::{BitAnd, Shl, Shr};

use alloy::primitives::{Address, Signed, Uint, B256, I256};
use alloy::primitives::{U160, U256};
use eyre::Result;
use lazy_static::lazy_static;
use revm::DatabaseRef;
use tracing::trace;

use loom_defi_abi::uniswap3::IUniswapV3Pool::slot0Return;
use loom_evm_db::LoomDBType;
use loom_evm_utils::remv_db_direct_access::{try_read_cell, try_read_hashmap_cell};

pub struct UniswapV3DBReader {}

lazy_static! {
    static ref BITS160MASK: U256 = U256::from(1).shl(160) - U256::from(1);
    static ref BITS128MASK: U256 = U256::from(1).shl(128) - U256::from(1);
    static ref BITS24MASK: U256 = U256::from(1).shl(24) - U256::from(1);
    static ref BITS16MASK: U256 = U256::from(1).shl(16) - U256::from(1);
    static ref BITS8MASK: U256 = U256::from(1).shl(8) - U256::from(1);
    static ref BITS1MASK: U256 = U256::from(1);
}
impl UniswapV3DBReader {
    pub fn fee_growth_global0_x128(db: &LoomDBType, address: Address) -> Result<U256> {
        let cell = try_read_cell(db, &address, &U256::from(1))?;
        Ok(cell)
    }

    pub fn fee_growth_global1_x128(db: &LoomDBType, address: Address) -> Result<U256> {
        let cell = try_read_cell(db, &address, &U256::from(2))?;
        Ok(cell)
    }

    pub fn protocol_fees(db: &LoomDBType, address: Address) -> Result<U256> {
        let cell = try_read_cell(db, &address, &U256::from(3))?;
        Ok(cell)
    }

    pub fn liquidity<DB: DatabaseRef>(db: &DB, address: Address) -> Result<u128> {
        let cell = try_read_cell(&db, &address, &U256::from(4))?;
        let cell: u128 = cell.saturating_to();
        Ok(cell)
    }

    pub fn ticks_liquidity_net<DB: DatabaseRef>(db: &DB, address: Address, tick: i32) -> Result<i128> {
        //i24
        let cell = try_read_hashmap_cell(&db, &address, &U256::from(5), &U256::from_be_bytes(I256::try_from(tick)?.to_be_bytes::<32>()))?;
        let unsigned_liqudity: Uint<128, 2> = cell.shr(U256::from(128)).to();
        let signed_liquidity: Signed<128, 2> = Signed::<128, 2>::from_raw(unsigned_liqudity);
        let lu128: u128 = unsigned_liqudity.to();
        let li128: i128 = lu128 as i128;
        trace!("ticks_liquidity_net {address} {tick} {cell} -> {signed_liquidity}");

        Ok(li128)
    }
    pub fn tick_bitmap<DB: DatabaseRef>(db: &DB, address: Address, tick: i16) -> Result<U256> {
        //i16
        let cell = try_read_hashmap_cell(&db, &address, &U256::from(6), &U256::from_be_bytes(I256::try_from(tick)?.to_be_bytes::<32>()))?;
        trace!("tickBitmap {address} {tick} {cell}");
        Ok(cell)
    }

    pub fn position_info<DB: DatabaseRef>(db: &DB, address: Address, position: B256) -> Result<U256> {
        //i16
        let position: U256 = position.into();
        let cell = try_read_hashmap_cell(db, &address, &U256::from(7), &position)?;
        Ok(cell)
    }

    pub fn observations<DB: DatabaseRef>(db: &DB, address: Address, idx: u32) -> Result<U256> {
        //i16
        let cell = try_read_hashmap_cell(&db, &address, &U256::from(7), &U256::from(idx))?;
        Ok(cell)
    }

    pub fn slot0<DB: DatabaseRef>(db: &DB, address: Address) -> Result<slot0Return> {
        let cell = try_read_cell(&db, &address, &U256::from(0))?;
        let tick: Uint<24, 1> = ((Shr::<U256>::shr(cell, U256::from(160))) & *BITS24MASK).to();
        let tick: Signed<24, 1> = Signed::<24, 1>::from_raw(tick);
        let tick: i32 = tick.as_i32();

        let sqrt_price_x96: U160 = cell.bitand(*BITS160MASK).to();

        Ok(slot0Return {
            sqrtPriceX96: sqrt_price_x96,
            tick: tick.try_into()?,
            observationIndex: ((Shr::<U256>::shr(cell, U256::from(160 + 24))) & *BITS16MASK).to(),
            observationCardinality: ((Shr::<U256>::shr(cell, U256::from(160 + 24 + 16))) & *BITS16MASK).to(),
            observationCardinalityNext: ((Shr::<U256>::shr(cell, U256::from(160 + 24 + 16 + 16))) & *BITS16MASK).to(),
            feeProtocol: ((Shr::<U256>::shr(cell, U256::from(160 + 24 + 16 + 16 + 16))) & *BITS8MASK).to(),
            unlocked: ((Shr::<U256>::shr(cell, U256::from(160 + 24 + 16 + 16 + 16 + 8))) & *BITS1MASK).to(),
        })
    }
}

#[cfg(test)]
mod test {
    use alloy::primitives::Address;
    use eyre::Result;
    use revm::primitives::Env;
    use std::env;
    use tracing::debug;

    use loom_defi_address_book::UniswapV3PoolAddress;
    use loom_evm_db::LoomDBType;
    use loom_node_debug_provider::AnvilDebugProviderFactory;
    use loom_types_entities::required_state::RequiredStateReader;
    use loom_types_entities::{MarketState, Pool};

    use crate::db_reader::UniswapV3DBReader;
    use crate::state_readers::UniswapV3StateReader;
    use crate::UniswapV3Pool;

    #[tokio::test]
    async fn test_reader() -> Result<()> {
        let _ = env_logger::try_init_from_env(env_logger::Env::default().default_filter_or(
            "info,loom_types_entities::required_state=off,loom_types_blockchain::state_update=off,alloy_rpc_client::call=off,tungstenite=off",
        ));

        let node_url = env::var("MAINNET_WS")?;

        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, 20038285).await?;

        let mut market_state = MarketState::new(LoomDBType::default());

        let pool_address: Address = UniswapV3PoolAddress::USDC_WETH_500;

        let pool = UniswapV3Pool::fetch_pool_data(client.clone(), pool_address).await?;

        let state_required = pool.get_state_required()?;

        let state_required = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, None).await?;

        market_state.state_db.apply_geth_update(state_required);

        let evm_env = Env::default();

        let factory_evm = UniswapV3StateReader::factory(&market_state.state_db, evm_env.clone(), pool_address)?;
        let token0_evm = UniswapV3StateReader::token0(&market_state.state_db, evm_env.clone(), pool_address)?;
        let token1_evm = UniswapV3StateReader::token1(&market_state.state_db, evm_env.clone(), pool_address)?;

        debug!("{factory_evm:?} {token0_evm:?} {token1_evm:?}");

        let slot0_evm = UniswapV3StateReader::slot0(&market_state.state_db, evm_env.clone(), pool_address)?;

        let slot0_db = UniswapV3DBReader::slot0(&market_state.state_db, pool_address)?;

        debug!("evm : {slot0_evm:?}");
        debug!("db  : {slot0_db:?}");

        assert_eq!(slot0_evm, slot0_db);

        Ok(())
    }
}
