use std::ops::{BitAnd, Shl, Shr};

use crate::protocols::get_uniswap3pool_address;
use alloy_primitives::aliases::{U176, U24, U80};
use alloy_primitives::{address, b256, keccak256, Address, Signed, Uint, B256, I256};
use alloy_primitives::{U160, U256};
use alloy_sol_types::SolValue;
use defi_abi::uniswap3::IUniswapV3Pool::slot0Return;
use eyre::{eyre, Result};
use lazy_static::lazy_static;
use log::{debug, trace};
use loom_revm_db::LoomInMemoryDB;
use loom_utils::remv_db_direct_access::{try_read_cell, try_read_hashmap_cell};
use reth_storage_api::StateProvider;

#[derive(Debug)]
pub struct Pool {
    address: Address,
    token0: Address,
    token1: Address,
    fee: U24,
}

#[derive(Debug)]
pub struct PoolKey {
    token0: Address,
    token1: Address,
    fee: U24,
}

pub struct UniswapV3DBReader {}

const POOL_INIT_CODE_HASH: B256 = b256!("e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54");
const NEXT_POOL_ID: B256 = b256!("000000000000000000000000000000000000000000000000000000000000000d");
const POOL_ID_TO_POOL_KEY: B256 = b256!("000000000000000000000000000000000000000000000000000000000000000b");

const UNIV3_POSITION_MNG: Address = address!("c36442b4a4522e871399cd717abdd847ab11fe88");
const UNIV3_FACTORY: Address = address!("1F98431c8aD98523631AE4a59f267346ea31F984");

lazy_static! {
    static ref BITS160MASK: U256 = U256::from(1).shl(160) - U256::from(1);
    static ref BITS128MASK: U256 = U256::from(1).shl(128) - U256::from(1);
    static ref BITS24MASK: U256 = U256::from(1).shl(24) - U256::from(1);
    static ref BITS16MASK: U256 = U256::from(1).shl(16) - U256::from(1);
    static ref BITS8MASK: U256 = U256::from(1).shl(8) - U256::from(1);
    static ref BITS1MASK: U256 = U256::from(1);
}
impl UniswapV3DBReader {
    pub fn fee_growth_global0_x128(db: &LoomInMemoryDB, address: Address) -> Result<U256> {
        let cell = try_read_cell(db, &address, &U256::from(1))?;
        Ok(cell)
    }

    pub fn fee_growth_global1_x128(db: &LoomInMemoryDB, address: Address) -> Result<U256> {
        let cell = try_read_cell(db, &address, &U256::from(2))?;
        Ok(cell)
    }

    pub fn protocol_fees(db: &LoomInMemoryDB, address: Address) -> Result<U256> {
        let cell = try_read_cell(db, &address, &U256::from(3))?;
        Ok(cell)
    }

    pub fn liquidity(db: &LoomInMemoryDB, address: Address) -> Result<u128> {
        let cell = try_read_cell(db, &address, &U256::from(4))?;
        let cell: u128 = cell.saturating_to();
        Ok(cell)
    }

    pub fn ticks_liquidity_net(db: &LoomInMemoryDB, address: Address, tick: i32) -> Result<i128> {
        //i24
        let cell = try_read_hashmap_cell(db, &address, &U256::from(5), &U256::from_be_bytes(I256::try_from(tick)?.to_be_bytes::<32>()))?;
        let unsigned_liqudity: Uint<128, 2> = cell.shr(U256::from(128)).to();
        let signed_liquidity: Signed<128, 2> = Signed::<128, 2>::from_raw(unsigned_liqudity);
        let lu128: u128 = unsigned_liqudity.to();
        let li128: i128 = lu128 as i128;
        trace!("ticks_liquidity_net {address} {tick} {cell} -> {signed_liquidity}");

        Ok(li128)
    }
    pub fn tick_bitmap(db: &LoomInMemoryDB, address: Address, tick: i16) -> Result<U256> {
        //i16
        let cell = try_read_hashmap_cell(db, &address, &U256::from(6), &U256::from_be_bytes(I256::try_from(tick)?.to_be_bytes::<32>()))?;
        trace!("tickBitmap {address} {tick} {cell}");
        Ok(cell)
    }

    pub fn position_info(db: &LoomInMemoryDB, address: Address, position: B256) -> Result<U256> {
        //i16
        let position: U256 = position.into();
        let cell = try_read_hashmap_cell(db, &address, &U256::from(7), &position)?;
        Ok(cell)
    }

    pub fn observations(db: &LoomInMemoryDB, address: Address, idx: u32) -> Result<U256> {
        //i16
        let cell = try_read_hashmap_cell(db, &address, &U256::from(7), &U256::from(idx))?;
        Ok(cell)
    }

    pub fn slot0(db: &LoomInMemoryDB, address: Address) -> Result<slot0Return> {
        let cell = try_read_cell(db, &address, &U256::from(0))?;
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

    pub fn read_univ3_position_pools<T: StateProvider>(provider: T) -> Result<Vec<Pool>> {
        let (next_pool_id, next_position_id) = match provider.storage(UNIV3_POSITION_MNG, NEXT_POOL_ID)? {
            None => return Err(eyre!("Invalid pair length")),
            Some(value) => {
                let bytes = value.to_be_bytes_vec();
                let next_pool_id = U80::from_be_slice(&bytes[0..10]);
                let next_position_id = U176::from_be_slice(&bytes[10..32]);
                (next_pool_id, next_position_id)
            }
        };
        debug!("Next pool id: {}, Next position id: {}", next_pool_id, next_position_id);

        let mut pool_addresses = vec![];

        for pool_id in 1..next_pool_id.to::<u64>() {
            // mapping(uint80 => PoolAddress.PoolKey)
            let storage_key0 = keccak256((U80::from(pool_id), POOL_ID_TO_POOL_KEY).abi_encode());
            let storage_key1 = B256::from(U256::from_be_slice(storage_key0.0.as_slice()) + U256::from(1));

            let pool_key = match provider.storage(UNIV3_POSITION_MNG, storage_key0)? {
                None => return Err(eyre!("Invalid pool id")),
                Some(value) => {
                    let bytes = value.to_be_bytes_vec();
                    let token0 = Address::from_slice(&bytes[12..32]);

                    // read second slot
                    let (fee, token1) = match provider.storage(UNIV3_POSITION_MNG, storage_key1)? {
                        None => return Err(eyre!("Invalid pool id second slot")),
                        Some(value) => {
                            let bytes = value.to_be_bytes_vec();
                            let fee = U24::from_be_slice(&bytes[9..12]);
                            let token1 = Address::from_slice(&bytes[12..32]);
                            (fee, token1)
                        }
                    };

                    PoolKey { token0, token1, fee }
                }
            };

            let pool_address =
                get_uniswap3pool_address(pool_key.token0, pool_key.token1, pool_key.fee.to::<u32>(), UNIV3_FACTORY, POOL_INIT_CODE_HASH);
            pool_addresses.push(Pool { address: pool_address, token0: pool_key.token0, token1: pool_key.token1, fee: pool_key.fee });
        }

        Ok(pool_addresses)
    }
}

#[cfg(test)]
mod test {
    use alloy_primitives::Address;
    use eyre::Result;
    use log::debug;
    use revm::primitives::Env;
    use std::env;

    use debug_provider::AnvilDebugProviderFactory;
    use defi_entities::required_state::RequiredStateReader;
    use defi_entities::{MarketState, Pool};
    use loom_revm_db::LoomInMemoryDB;

    use crate::db_reader::UniswapV3DBReader;
    use crate::protocols::UniswapV3Protocol;
    use crate::state_readers::UniswapV3StateReader;
    use crate::UniswapV3Pool;

    #[tokio::test]
    async fn test_reader() -> Result<()> {
        let _ = env_logger::try_init_from_env(env_logger::Env::default().default_filter_or(
            "info,defi_entities::required_state=off,defi_types::state_update=off,alloy_rpc_client::call=off,tungstenite=off",
        ));

        let node_url = env::var("MAINNET_WS")?;

        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, 20038285).await?;

        let mut market_state = MarketState::new(LoomInMemoryDB::default());

        market_state.add_state(&UniswapV3Protocol::get_quoter_v3_state());

        let pool_address: Address = "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640".parse().unwrap();

        let pool = UniswapV3Pool::fetch_pool_data(client.clone(), pool_address).await?;

        let state_required = pool.get_state_required()?;

        let state_required = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, None).await?;

        market_state.add_state(&state_required);

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
