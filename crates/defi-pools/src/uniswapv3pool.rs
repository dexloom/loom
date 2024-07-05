use std::fmt::Debug;
use std::ops::Sub;

use alloy_primitives::{Address, Bytes, I256, U256};
use alloy_provider::{Network, Provider};
use alloy_sol_types::{SolCall, SolInterface};
use alloy_transport::Transport;
use eyre::{ErrReport, eyre, OptionExt, Result};
use lazy_static::lazy_static;
use log::{debug, error};
use revm::InMemoryDB;
use revm::primitives::Env;
use uniswap_v3_math::tick_math::{MAX_SQRT_RATIO, MAX_TICK, MIN_SQRT_RATIO, MIN_TICK};

use defi_abi::IERC20;
use defi_abi::uniswap3::IUniswapV3Pool;
use defi_abi::uniswap3::IUniswapV3Pool::slot0Return;
use defi_abi::uniswap_periphery::ITickLens;
use defi_entities::{AbiSwapEncoder, Pool, PoolClass, PoolProtocol, PreswapRequirement};
use defi_entities::required_state::RequiredState;

use crate::protocols::UniswapV3Protocol;
use crate::state_readers::{UniswapCustomQuoterStateReader, UniswapV3QuoterEncoder, UniswapV3StateReader};
use crate::virtual_impl::UniswapV3PoolVirtual;

lazy_static! {
    //pub static ref CUSTOM_QUOTER_ADDRESS : Address = "0x0000000000000000000000000000000000003333".parse().unwrap();

    pub static ref QUOTER_ADDRESS : Address = "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".parse().unwrap();
    pub static ref TICK_LENS_ADDRESS : Address = "0xbfd8137f7d1516D3ea5cA83523914859ec47F573".parse().unwrap();

    pub static ref UNI3_FACTORY_ADDRESS : Address =  "0x1F98431c8aD98523631AE4a59f267346ea31F984".parse().unwrap();
    pub static ref SUSHI3_FACTORY_ADDRESS : Address =  "0xbACEB8eC6b9355Dfc0269C18bac9d6E2Bdc29C4F".parse().unwrap();

}

#[derive(Clone, Debug, Default)]
struct Slot0 {
    pub tick: i32,
    pub fee_protocol: u8,
    pub sqrt_price_x96: U256,
    pub unlocked: bool,
    pub observation_index: u16,
    pub observation_cardinality: u16,
    pub observation_cardinality_next: u16,

}


impl From<slot0Return> for Slot0 {
    fn from(value: slot0Return) -> Self {
        Self {
            tick: value.tick,
            fee_protocol: value.feeProtocol,
            observation_cardinality: value.observationCardinality,
            observation_cardinality_next: value.observationCardinalityNext,
            sqrt_price_x96: value.sqrtPriceX96,
            unlocked: value.unlocked,
            observation_index: value.observationIndex,
        }
    }
}

#[derive(Clone)]
pub struct UniswapV3Pool {
    //contract_storage : ContractStorage,
    address: Address,
    pub token0: Address,
    pub token1: Address,
    pub liquidity: u128,
    pub fee: u32,
    pub slot0: Option<Slot0>,
    liquidity0: U256,
    liquidity1: U256,
    factory: Address,
    protocol: PoolProtocol,
    encoder: UniswapV3AbiSwapEncoder,

}

impl UniswapV3Pool {
    pub fn new(address: Address) -> Self {
        UniswapV3Pool {
            address: address,
            token0: Address::ZERO,
            token1: Address::ZERO,
            liquidity: 0,
            liquidity0: U256::ZERO,
            liquidity1: U256::ZERO,
            fee: 0,
            slot0: None,
            factory: Address::ZERO,
            protocol: PoolProtocol::UniswapV3Like,
            encoder: UniswapV3AbiSwapEncoder::new(address),
        }
    }

    pub fn tick_spacing(&self) -> u32 {
        Self::get_price_step(self.fee)
    }


    pub fn get_price_step(fee: u32) -> u32 {
        match fee {
            10000 => 200,
            3000 => 60,
            500 => 10,
            100 => 1,
            _ => 0
        }
    }

    pub fn get_tick_bitmap_index(tick: i32, spacing: u32) -> i16 {
        let tick_bitmap_index = tick / (spacing as i32);

        if tick_bitmap_index < 0 {
            (((tick_bitmap_index + 1) / 256) - 1) as i16
        } else {
            (tick_bitmap_index >> 8) as i16
        }
    }

    pub fn get_price_limit(token_address_from: &Address, token_address_to: &Address) -> U256 {
        if *token_address_from < *token_address_to {
            U256::from(4295128740u64)
        } else {
            U256::from_str_radix("1461446703485210103287273052203988822378723970341", 10).unwrap()
        }
    }

    pub fn get_zero_for_one(token_address_from: &Address, token_address_to: &Address) -> bool {
        if *token_address_from < *token_address_to {
            true
        } else {
            false
        }
    }


    fn get_protocol_by_factory(factory_address: Address) -> PoolProtocol {
        if factory_address == *UNI3_FACTORY_ADDRESS {
            PoolProtocol::UniswapV3
        } else if factory_address == *SUSHI3_FACTORY_ADDRESS {
            PoolProtocol::SushiswapV3
        } else {
            PoolProtocol::UniswapV3Like
        }
    }


    pub fn fetch_pool_data_evm(db: &InMemoryDB, env: Env, address: Address) -> Result<Self>
    {
        let token0 = UniswapV3StateReader::token0(db, env.clone(), address)?;
        let token1 = UniswapV3StateReader::token1(db, env.clone(), address)?;
        let fee = UniswapV3StateReader::fee(db, env.clone(), address)?;
        let liquidity = UniswapV3StateReader::liquidity(db, env.clone(), address)?;
        let factory = UniswapV3StateReader::factory(db, env.clone(), address).unwrap_or_default();
        let protocol = UniswapV3Pool::get_protocol_by_factory(factory);


        let ret = UniswapV3Pool {
            address,
            token0,
            token1,
            liquidity,
            liquidity0: Default::default(),
            liquidity1: Default::default(),
            fee,
            slot0: None,
            factory,
            protocol,
            encoder: UniswapV3AbiSwapEncoder { pool_address: address },
        };
        debug!("fetch_pool_data_evm {:?} {:?} {} {:?} {}", token0, token1, fee, factory, protocol);

        Ok(ret)
    }

    pub async fn fetch_pool_data<T: Transport + Clone, N: Network, P: Provider<T, N> + Send + Sync + Clone + 'static>(client: P, address: Address) -> Result<Self> {
        let uni3_pool = IUniswapV3Pool::IUniswapV3PoolInstance::new(address, client.clone());

        let token0: Address = uni3_pool.token0().call().await?._0;
        let token1: Address = uni3_pool.token1().call().await?._0;
        let fee: u32 = uni3_pool.fee().call().await?._0;
        let liquidity: u128 = uni3_pool.liquidity().call().await?._0;
        let slot0 = uni3_pool.slot0().call().await?;
        let factory: Address = uni3_pool.factory().call().await?._0;


        let token0_erc20 = IERC20::IERC20Instance::new(token0, client.clone());
        let token1_erc20 = IERC20::IERC20Instance::new(token1, client.clone());

        let liquidity0: U256 = token0_erc20.balanceOf(address).call().await?._0;
        let liquidity1: U256 = token1_erc20.balanceOf(address).call().await?._0;

        let protocol = UniswapV3Pool::get_protocol_by_factory(factory);

        let ret = UniswapV3Pool {
            address,
            token0,
            token1,
            fee,
            liquidity,
            slot0: Some(slot0.into()),
            liquidity0,
            liquidity1,
            factory,
            protocol,
            encoder: UniswapV3AbiSwapEncoder::new(address),
        };

        Ok(ret)
    }
}


impl Pool for UniswapV3Pool
{
    fn get_address(&self) -> Address {
        self.address
    }

    fn get_class(&self) -> PoolClass {
        PoolClass::UniswapV3
    }

    fn get_protocol(&self) -> PoolProtocol {
        self.protocol
    }

    fn get_encoder(&self) -> &dyn AbiSwapEncoder {
        &self.encoder
    }

    fn get_tokens(&self) -> Vec<Address> {
        vec![self.token0, self.token1]
    }

    fn get_swap_directions(&self) -> Vec<(Address, Address)> {
        vec![(self.token0, self.token1), (self.token1, self.token0)]
    }

    fn calculate_out_amount(&self, state_db: &InMemoryDB, env: Env, token_address_from: &Address, token_address_to: &Address, in_amount: U256) -> Result<(U256, u64), ErrReport> {
        let mut env = env;
        env.tx.gas_limit = 1_000_000;


        let gas_used = 150000;

        let ret = UniswapV3PoolVirtual::simulate_swap_in_amount(
            state_db, self, *token_address_from, in_amount,
        )?;

        #[cfg(any(test, debug_assertions))] {
            let (ret_evm, gas_used) = UniswapCustomQuoterStateReader::quote_exact_input(state_db,
                                                                                        env,
                                                                                        UniswapV3Protocol::get_custom_quoter_address(),
                                                                                        self.get_address(),
                                                                                        *token_address_from,
                                                                                        *token_address_to,
                                                                                        self.fee,
                                                                                        in_amount)?;


            if ret != ret_evm {
                error!("calculate_out_amount RETURN_RESULT_IS_INCORRECT : {ret} need {ret_evm}");
                return Err(eyre!("RETURN_RESULT_IS_INCORRECT"));
            }
        }


        if ret.is_zero() {
            return Err(eyre!("RETURN_RESULT_IS_ZERO"));
        } else {
            Ok((ret - U256::from(1), gas_used))
        }
    }


    fn calculate_in_amount(&self, state_db: &InMemoryDB, env: Env, token_address_from: &Address, token_address_to: &Address, out_amount: U256) -> Result<(U256, u64), ErrReport> {
        let mut env = env;
        env.tx.gas_limit = 1_000_000;

        let gas_used = 150000;

        let ret = UniswapV3PoolVirtual::simulate_swap_out_amount(
            state_db, self, *token_address_from, out_amount + U256::from(0),
        )?;


        #[cfg(any(test, debug_assertions))] {
            let (ret_evm, gas_used) = UniswapCustomQuoterStateReader::quote_exact_output(state_db,
                                                                                         env,
                                                                                         UniswapV3Protocol::get_custom_quoter_address(),
                                                                                         self.get_address(),
                                                                                         *token_address_from,
                                                                                         *token_address_to,
                                                                                         self.fee,
                                                                                         out_amount + U256::from(0))?;


            if ret != ret_evm {
                error!("calculate_in_amount RETURN_RESULT_IS_INCORRECT : {ret} need {ret_evm}");
                return Err(eyre!("RETURN_RESULT_IS_INCORRECT"));
            }
        }


        if ret.is_zero() {
            return Err(eyre!("RETURN_RESULT_IS_ZERO"));
        } else {
            Ok((ret + U256::from(1), gas_used))
        }
    }

    fn can_flash_swap(&self) -> bool {
        true
    }

    fn get_state_required(&self) -> Result<RequiredState> {
        let tick = self.slot0.as_ref().ok_or_eyre("SLOT0_NOT_SET")?.tick;
        let price_step = UniswapV3Pool::get_price_step(self.fee);
        let mut state_required = RequiredState::new();
        if price_step == 0 {
            return Err(eyre!("BAD_PRICE_STEP"));
        }
        let tick_bitmap_index = UniswapV3Pool::get_tick_bitmap_index(tick, price_step);

        //debug!("Fetching state {:?} tick {} tick bitmap index {}", self.address, tick, tick_bitmap_index);

        let balance_call_data = IERC20::IERC20Calls::balanceOf(
            IERC20::balanceOfCall {
                account: self.get_address()
            }).abi_encode();


        let pool_address = self.get_address();

        state_required
            .add_call(self.get_address(), IUniswapV3Pool::IUniswapV3PoolCalls::slot0(IUniswapV3Pool::slot0Call {}).abi_encode())
            .add_call(self.get_address(), IUniswapV3Pool::IUniswapV3PoolCalls::liquidity(IUniswapV3Pool::liquidityCall {}).abi_encode())
            .add_call(*TICK_LENS_ADDRESS, ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall { pool: pool_address, tickBitmapIndex: tick_bitmap_index - 4 }).abi_encode())
            .add_call(*TICK_LENS_ADDRESS, ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall { pool: pool_address, tickBitmapIndex: tick_bitmap_index - 3 }).abi_encode())
            .add_call(*TICK_LENS_ADDRESS, ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall { pool: pool_address, tickBitmapIndex: tick_bitmap_index - 2 }).abi_encode())
            .add_call(*TICK_LENS_ADDRESS, ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall { pool: pool_address, tickBitmapIndex: tick_bitmap_index - 1 }).abi_encode())
            .add_call(*TICK_LENS_ADDRESS, ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall { pool: pool_address, tickBitmapIndex: tick_bitmap_index }).abi_encode())
            .add_call(*TICK_LENS_ADDRESS, ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall { pool: pool_address, tickBitmapIndex: tick_bitmap_index + 1 }).abi_encode())
            .add_call(*TICK_LENS_ADDRESS, ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall { pool: pool_address, tickBitmapIndex: tick_bitmap_index + 2 }).abi_encode())
            .add_call(*TICK_LENS_ADDRESS, ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall { pool: pool_address, tickBitmapIndex: tick_bitmap_index + 3 }).abi_encode())
            .add_call(*TICK_LENS_ADDRESS, ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall { pool: pool_address, tickBitmapIndex: tick_bitmap_index + 4 }).abi_encode())


            .add_call(self.token0, balance_call_data.clone())
            .add_call(self.token1, balance_call_data)
            .add_slot_range(self.get_address(), U256::from(0), 0x20)
            .add_empty_slot_range(self.get_address(), U256::from(0x10000), 0x20);

        for token_address in self.get_tokens() {
            state_required.add_call(token_address, IERC20::balanceOfCall { account: pool_address }.abi_encode());
        }


        if self.protocol == PoolProtocol::UniswapV3 {
            let amount = self.liquidity0 / U256::from(100);
            let price_limit = UniswapV3Pool::get_price_limit(&self.token0, &self.token1);
            let quoter_swap_0_1_call = UniswapV3QuoterEncoder::quote_exact_input_encode(self.token0, self.token1, self.fee, price_limit, amount);


            let price_limit = UniswapV3Pool::get_price_limit(&self.token1, &self.token0);
            let amount = self.liquidity1 / U256::from(100);

            let quoter_swap_1_0_call = UniswapV3QuoterEncoder::quote_exact_input_encode(self.token1, self.token0, self.fee, price_limit, amount);

            state_required
                .add_call(*QUOTER_ADDRESS, quoter_swap_0_1_call)
                .add_call(*QUOTER_ADDRESS, quoter_swap_1_0_call);
        }

        Ok(state_required)
    }
}

#[derive(Clone, Copy)]
struct UniswapV3AbiSwapEncoder {
    pool_address: Address,
}

impl UniswapV3AbiSwapEncoder {
    pub fn new(pool_address: Address) -> Self {
        Self {
            pool_address
        }
    }
}

impl AbiSwapEncoder for UniswapV3AbiSwapEncoder {
    fn encode_swap_out_amount_provided(&self, token_from_address: Address, token_to_address: Address, amount: U256, recipient: Address, payload: Bytes) -> Result<Bytes> {
        let zero_for_one = UniswapV3Pool::get_zero_for_one(&token_from_address, &token_to_address);
        let sqrt_price_limit_x96 = UniswapV3Pool::get_price_limit(&token_from_address, &token_to_address);
        let swap_call = IUniswapV3Pool::swapCall {
            recipient,
            zeroForOne: zero_for_one,
            amountSpecified: I256::ZERO.sub(I256::from_raw(amount)),
            sqrtPriceLimitX96: sqrt_price_limit_x96,
            data: payload,
        };

        Ok(Bytes::from(IUniswapV3Pool::IUniswapV3PoolCalls::swap(swap_call).abi_encode()))
    }

    fn encode_swap_in_amount_provided(&self, token_from_address: Address, token_to_address: Address, amount: U256, recipient: Address, payload: Bytes) -> Result<Bytes> {
        let zero_for_one = UniswapV3Pool::get_zero_for_one(&token_from_address, &token_to_address);
        let sqrt_price_limit_x96 = UniswapV3Pool::get_price_limit(&token_from_address, &token_to_address);
        let swap_call = IUniswapV3Pool::swapCall {
            recipient,
            zeroForOne: zero_for_one,
            amountSpecified: I256::from_raw(amount),
            sqrtPriceLimitX96: sqrt_price_limit_x96,
            data: payload,
        };

        Ok(Bytes::from(IUniswapV3Pool::IUniswapV3PoolCalls::swap(swap_call).abi_encode()))
    }

    fn swap_out_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x44)
    }

    fn swap_in_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x44)
    }

    fn preswap_requirement(&self) -> PreswapRequirement {
        PreswapRequirement::Callback
    }

    fn swap_in_amount_return_offset(&self, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        if token_from_address < token_to_address {
            Some(0x20)
        } else {
            Some(0x0)
        }
    }

    fn swap_in_amount_return_script(&self, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        Some(Bytes::from(vec![0x8, 0x2A, 0x00]))
    }
}


#[cfg(test)]
mod tests {
    use alloy_primitives::U256;
    use env_logger::Env as EnvLog;
    use revm::InMemoryDB;
    use revm::primitives::Env;

    use debug_provider::AnvilDebugProviderFactory;
    use defi_entities::{MarketState, Pool};
    use defi_entities::required_state::RequiredStateReader;

    use crate::UniswapV3Pool;

    use super::*;

    #[tokio::test]
    async fn test_pool() -> Result<()> {
        std::env::set_var("RUST_BACKTRACE", "1");
        env_logger::init_from_env(EnvLog::default().default_filter_or("debug,defi_pools=trace"));

        //let client = AnvilControl::from_node_on_block("ws://falcon.loop:8008/looper".to_string(), 19931897).await?;
        let client = AnvilDebugProviderFactory::from_node_on_block("ws://falcon.loop:8008/looper".to_string(), 20045799).await?;

        let mut market_state = MarketState::new(InMemoryDB::default());

        market_state.add_state(&UniswapV3Protocol::get_quoter_v3_state());

        //let pool_address: Address = "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640".parse().unwrap();
        let pool_address: Address = "0x48DA0965ab2d2cbf1C17C09cFB5Cbe67Ad5B1406".parse().unwrap();

        let mut pool = UniswapV3Pool::fetch_pool_data(client.clone(), pool_address).await?;

        let state_required = pool.get_state_required()?;

        let state_required = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, None).await?;

        market_state.add_state(&state_required);

        let evm_env = Env::default();


        let in_amount = U256::from(pool.liquidity0 / U256::from(10));
        let (out_amount, gas_used) = pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &pool.token0, &pool.token1, in_amount).unwrap();
        println!("out {} -> {} {}", in_amount, out_amount, gas_used);
        let (in_amount2, gas_used) = pool.calculate_in_amount(&market_state.state_db, evm_env.clone(), &pool.token0, &pool.token1, out_amount).unwrap();
        println!("in {} -> {} {} {} ", out_amount, in_amount2, in_amount2 >= in_amount, gas_used);

        let in_amount = U256::from(pool.liquidity1 / U256::from(10));
        let (out_amount, gas_used) = pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &pool.token1, &pool.token0, in_amount).unwrap();
        println!("out {} -> {} {}", in_amount, out_amount, gas_used);
        let (in_amount2, gas_used) = pool.calculate_in_amount(&market_state.state_db, evm_env.clone(), &pool.token1, &pool.token0, out_amount).unwrap();
        println!("in {} -> {} {} {}", out_amount, in_amount2, in_amount2 >= in_amount, gas_used);


        //market_state.fetch_state(pool.get_address(), client.clone()).await;
        //market_state.fetch_state(pool.get_address(), client.clone()).await;

        let (out_amount, gas_used) = pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &pool.token0, &pool.token1, U256::from(pool.liquidity0 / U256::from(10))).unwrap();
        println!("{} {}", out_amount, gas_used);
        let (out_amount, gas_used) = pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &pool.token1, &pool.token0, U256::from(pool.liquidity1 / U256::from(10))).unwrap();
        println!("{} {}", out_amount, gas_used);
        Ok(())
    }
}
