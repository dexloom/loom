use std::any::Any;
use std::fmt::Debug;
use std::ops::Sub;

use crate::state_readers::UniswapV3StateReader;
use alloy::primitives::aliases::{I24, U24};
use alloy::primitives::{Address, Bytes, I256, U160, U256};
use alloy::providers::{Network, Provider};
use alloy::sol_types::{SolCall, SolInterface};
use eyre::{eyre, ErrReport, OptionExt, Result};
use loom_defi_abi::pancake::IPancakeQuoterV2::IPancakeQuoterV2Calls;
use loom_defi_abi::pancake::IPancakeV3Pool::slot0Return;
use loom_defi_abi::pancake::{IPancakeQuoterV2, IPancakeV3Pool};
use loom_defi_abi::uniswap3::IUniswapV3Pool;
use loom_defi_abi::uniswap_periphery::ITickLens;
use loom_defi_abi::IERC20;
use loom_defi_address_book::PeripheryAddress;
use loom_evm_utils::evm::evm_call;
use loom_types_entities::required_state::RequiredState;
use loom_types_entities::{Pool, PoolAbiEncoder, PoolClass, PoolId, PoolProtocol, PreswapRequirement, SwapDirection};
use revm::primitives::Env;
use revm::DatabaseRef;

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct Slot0 {
    pub tick: I24,
    pub fee_protocol: u32,
    pub sqrt_price_x96: U160,
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

#[allow(dead_code)]
#[derive(Clone)]
pub struct PancakeV3Pool {
    //contract_storage : ContractStorage,
    address: Address,
    pub token0: Address,
    pub token1: Address,
    liquidity0: U256,
    liquidity1: U256,
    fee: U24,
    fee_u32: u32,
    slot0: Option<Slot0>,
    factory: Address,
    protocol: PoolProtocol,
    encoder: PancakeV3AbiSwapEncoder,
}

impl PancakeV3Pool {
    pub fn new(address: Address) -> Self {
        PancakeV3Pool {
            address,
            token0: Address::ZERO,
            token1: Address::ZERO,
            liquidity0: U256::ZERO,
            liquidity1: U256::ZERO,
            fee: U24::ZERO,
            fee_u32: 0,
            slot0: None,
            factory: Address::ZERO,
            protocol: PoolProtocol::PancakeV3,
            encoder: PancakeV3AbiSwapEncoder::new(address),
        }
    }

    pub fn get_price_step(fee: u32) -> u32 {
        match fee {
            10000 => 200,
            2500 => 50,
            500 => 10,
            100 => 1,
            _ => 0,
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

    pub fn get_price_limit(token_address_from: &Address, token_address_to: &Address) -> U160 {
        if *token_address_from < *token_address_to {
            U160::from(4295128740u64)
        } else {
            U160::from_str_radix("1461446703485210103287273052203988822378723970341", 10).unwrap()
        }
    }

    pub fn get_zero_for_one(token_address_from: &Address, token_address_to: &Address) -> bool {
        *token_address_from < *token_address_to
    }

    fn get_protocol_by_factory(factory_address: Address) -> PoolProtocol {
        let pancake3_factory: Address = "0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865".parse().unwrap();
        if factory_address == pancake3_factory {
            PoolProtocol::PancakeV3
        } else {
            PoolProtocol::UniswapV3Like
        }
    }
    pub fn fetch_pool_data_evm(db: &dyn DatabaseRef<Error = ErrReport>, env: Env, address: Address) -> Result<Self> {
        let token0: Address = UniswapV3StateReader::token0(&db, env.clone(), address)?;
        let token1: Address = UniswapV3StateReader::token1(&db, env.clone(), address)?;
        let fee = UniswapV3StateReader::fee(&db, env.clone(), address)?;
        let fee_u32: u32 = fee.to();
        let factory = UniswapV3StateReader::factory(&db, env.clone(), address)?;
        let protocol = Self::get_protocol_by_factory(factory);

        let ret = PancakeV3Pool {
            address,
            token0,
            token1,
            liquidity0: Default::default(),
            liquidity1: Default::default(),
            fee,
            fee_u32,
            slot0: None,
            factory,
            protocol,
            encoder: PancakeV3AbiSwapEncoder::new(address),
        };

        Ok(ret)
    }

    pub async fn fetch_pool_data<N: Network, P: Provider<N> + Send + Sync + Clone + 'static>(client: P, address: Address) -> Result<Self> {
        let uni3_pool = IPancakeV3Pool::IPancakeV3PoolInstance::new(address, client.clone());

        let token0: Address = uni3_pool.token0().call().await?._0;
        let token1: Address = uni3_pool.token1().call().await?._0;
        let fee = uni3_pool.fee().call().await?._0;
        let fee_u32: u32 = fee.to();
        let slot0 = uni3_pool.slot0().call().await?;
        let factory: Address = uni3_pool.factory().call().await?._0;

        let token0_erc20 = IERC20::IERC20Instance::new(token0, client.clone());
        let token1_erc20 = IERC20::IERC20Instance::new(token1, client.clone());

        let liquidity0: U256 = token0_erc20.balanceOf(address).call().await?._0;
        let liquidity1: U256 = token1_erc20.balanceOf(address).call().await?._0;

        let protocol = PancakeV3Pool::get_protocol_by_factory(factory);

        let ret = PancakeV3Pool {
            address,
            token0,
            token1,
            fee,
            fee_u32,
            slot0: Some(slot0.into()),
            liquidity0,
            liquidity1,
            factory,
            protocol,
            encoder: PancakeV3AbiSwapEncoder::new(address),
        };

        Ok(ret)
    }
}

impl Pool for PancakeV3Pool {
    fn as_any<'a>(&self) -> &dyn Any {
        self
    }
    fn get_class(&self) -> PoolClass {
        PoolClass::PancakeV3
    }

    fn get_protocol(&self) -> PoolProtocol {
        self.protocol
    }

    fn get_address(&self) -> Address {
        self.address
    }

    fn get_pool_id(&self) -> PoolId {
        PoolId::Address(self.address)
    }

    fn get_fee(&self) -> U256 {
        U256::from(self.fee)
    }

    fn get_tokens(&self) -> Vec<Address> {
        vec![self.token0, self.token1]
    }

    fn get_swap_directions(&self) -> Vec<SwapDirection> {
        vec![(self.token0, self.token1).into(), (self.token1, self.token0).into()]
    }

    fn calculate_out_amount(
        &self,
        state_db: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        in_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        let mut env = env;
        env.tx.gas_limit = 1_000_000;

        let call_data = IPancakeQuoterV2Calls::quoteExactInputSingle(IPancakeQuoterV2::quoteExactInputSingleCall {
            params: IPancakeQuoterV2::QuoteExactInputSingleParams {
                tokenIn: *token_address_from,
                tokenOut: *token_address_to,
                amountIn: in_amount,
                fee: self.fee,
                sqrtPriceLimitX96: PancakeV3Pool::get_price_limit(token_address_from, token_address_to),
            },
        })
        .abi_encode();

        let (value, gas_used) = evm_call(state_db, env, PeripheryAddress::PANCAKE_V3_QUOTER, call_data)?;

        let ret = IPancakeQuoterV2::quoteExactInputSingleCall::abi_decode_returns(&value, false)?;

        if ret.amountOut.is_zero() {
            Err(eyre!("ZERO_OUT_AMOUNT"))
        } else {
            Ok((ret.amountOut - U256::from(1), gas_used))
        }
    }

    fn calculate_in_amount(
        &self,
        state_db: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        out_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        let mut env = env;
        env.tx.gas_limit = 1_000_000;

        let call_data = IPancakeQuoterV2Calls::quoteExactOutputSingle(IPancakeQuoterV2::quoteExactOutputSingleCall {
            params: IPancakeQuoterV2::QuoteExactOutputSingleParams {
                tokenIn: *token_address_from,
                tokenOut: *token_address_to,
                amount: out_amount,
                fee: self.fee,
                sqrtPriceLimitX96: PancakeV3Pool::get_price_limit(token_address_from, token_address_to),
            },
        })
        .abi_encode();

        let (value, gas_used) = evm_call(state_db, env, PeripheryAddress::PANCAKE_V3_QUOTER, call_data)?;

        let ret = IPancakeQuoterV2::quoteExactOutputSingleCall::abi_decode_returns(&value, false)?;

        if ret.amountIn.is_zero() {
            Err(eyre!("ZERO_IN_AMOUNT"))
        } else {
            Ok((ret.amountIn + U256::from(1), gas_used))
        }
    }

    fn can_flash_swap(&self) -> bool {
        true
    }

    fn can_calculate_in_amount(&self) -> bool {
        true
    }

    fn get_abi_encoder(&self) -> Option<&dyn PoolAbiEncoder> {
        Some(&self.encoder)
    }

    fn get_read_only_cell_vec(&self) -> Vec<U256> {
        vec![U256::from(0x10008)]
    }

    fn get_state_required(&self) -> Result<RequiredState> {
        let tick = self.slot0.as_ref().ok_or_eyre("SLOT0_NOT_SET")?.tick;
        let price_step = PancakeV3Pool::get_price_step(self.fee_u32);
        if price_step == 0 {
            return Err(eyre!("BAD_PRICE_STEP"));
        }
        let tick_bitmap_index = PancakeV3Pool::get_tick_bitmap_index(tick.as_i32(), PancakeV3Pool::get_price_step(self.fee_u32));

        let quoter_swap_0_1_call = IPancakeQuoterV2Calls::quoteExactInputSingle(IPancakeQuoterV2::quoteExactInputSingleCall {
            params: IPancakeQuoterV2::QuoteExactInputSingleParams {
                tokenIn: self.token0,
                tokenOut: self.token1,
                amountIn: self.liquidity0 / U256::from(100),
                fee: self.fee,
                sqrtPriceLimitX96: PancakeV3Pool::get_price_limit(&self.token0, &self.token1),
            },
        })
        .abi_encode();

        let quoter_swap_1_0_call = IPancakeQuoterV2Calls::quoteExactInputSingle(IPancakeQuoterV2::quoteExactInputSingleCall {
            params: IPancakeQuoterV2::QuoteExactInputSingleParams {
                tokenIn: self.token1,
                tokenOut: self.token0,
                amountIn: self.liquidity1 / U256::from(100),
                fee: self.fee,
                sqrtPriceLimitX96: PancakeV3Pool::get_price_limit(&self.token1, &self.token0),
            },
        })
        .abi_encode();

        let pool_address = self.get_address();

        let mut state_required = RequiredState::new();
        state_required
            .add_call(self.get_address(), IUniswapV3Pool::IUniswapV3PoolCalls::slot0(IUniswapV3Pool::slot0Call {}).abi_encode())
            .add_call(self.get_address(), IUniswapV3Pool::IUniswapV3PoolCalls::liquidity(IUniswapV3Pool::liquidityCall {}).abi_encode())
            .add_call(
                PeripheryAddress::PANCAKE_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index - 4,
                })
                .abi_encode(),
            )
            .add_call(
                PeripheryAddress::PANCAKE_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index - 3,
                })
                .abi_encode(),
            )
            .add_call(
                PeripheryAddress::PANCAKE_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index - 2,
                })
                .abi_encode(),
            )
            .add_call(
                PeripheryAddress::PANCAKE_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index - 1,
                })
                .abi_encode(),
            )
            .add_call(
                PeripheryAddress::PANCAKE_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index,
                })
                .abi_encode(),
            )
            .add_call(
                PeripheryAddress::PANCAKE_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index + 1,
                })
                .abi_encode(),
            )
            .add_call(
                PeripheryAddress::PANCAKE_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index + 2,
                })
                .abi_encode(),
            )
            .add_call(
                PeripheryAddress::PANCAKE_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index + 3,
                })
                .abi_encode(),
            )
            .add_call(
                PeripheryAddress::PANCAKE_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index + 4,
                })
                .abi_encode(),
            )
            .add_call(PeripheryAddress::PANCAKE_V3_QUOTER, quoter_swap_0_1_call)
            .add_call(PeripheryAddress::PANCAKE_V3_QUOTER, quoter_swap_1_0_call)
            .add_slot_range(self.get_address(), U256::from(0), 0x20)
            .add_empty_slot_range(self.get_address(), U256::from(0x10000), 0x20);

        for token_address in self.get_tokens() {
            state_required.add_call(token_address, IERC20::balanceOfCall { account: pool_address }.abi_encode());
        }
        Ok(state_required)
    }

    fn is_native(&self) -> bool {
        false
    }

    fn preswap_requirement(&self) -> PreswapRequirement {
        PreswapRequirement::Callback
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct PancakeV3AbiSwapEncoder {
    pool_address: Address,
}

impl PancakeV3AbiSwapEncoder {
    pub fn new(pool_address: Address) -> Self {
        Self { pool_address }
    }
}

impl PoolAbiEncoder for PancakeV3AbiSwapEncoder {
    fn encode_swap_in_amount_provided(
        &self,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> Result<Bytes> {
        let zero_for_one = PancakeV3Pool::get_zero_for_one(&token_from_address, &token_to_address);
        let sqrt_price_limit_x96 = PancakeV3Pool::get_price_limit(&token_from_address, &token_to_address);
        let swap_call = IUniswapV3Pool::swapCall {
            recipient,
            zeroForOne: zero_for_one,
            sqrtPriceLimitX96: sqrt_price_limit_x96,
            amountSpecified: I256::from_raw(amount),
            data: payload,
        };

        Ok(Bytes::from(IUniswapV3Pool::IUniswapV3PoolCalls::swap(swap_call).abi_encode()))
    }

    fn encode_swap_out_amount_provided(
        &self,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> Result<Bytes> {
        let zero_for_one = PancakeV3Pool::get_zero_for_one(&token_from_address, &token_to_address);
        let sqrt_price_limit_x96 = PancakeV3Pool::get_price_limit(&token_from_address, &token_to_address);
        let swap_call = IUniswapV3Pool::swapCall {
            recipient,
            zeroForOne: zero_for_one,
            sqrtPriceLimitX96: sqrt_price_limit_x96,
            amountSpecified: I256::ZERO.sub(I256::from_raw(amount)),
            data: payload,
        };

        Ok(Bytes::from(IUniswapV3Pool::IUniswapV3PoolCalls::swap(swap_call).abi_encode()))
    }

    fn swap_in_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x44)
    }

    fn swap_out_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x44)
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
    use env_logger::Env as EnvLog;
    use loom_evm_db::LoomDBType;
    use std::env;
    use tracing::debug;

    use loom_node_debug_provider::AnvilDebugProviderFactory;
    use loom_types_entities::required_state::RequiredStateReader;
    use loom_types_entities::MarketState;

    use super::*;

    #[tokio::test]
    async fn test_pool() {
        let _ = env_logger::try_init_from_env(EnvLog::default().default_filter_or("info"));
        let node_url = env::var("MAINNET_WS").unwrap();

        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, 19931897).await.unwrap();

        //let weth_address : Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        //let usdc_address : Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
        //let pool_address : Address = "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640".parse().unwrap();

        //let pool_address : Address = "0xCAD4b51069a150a77D3a1d381d2D768769F7D195".parse().unwrap();
        //let pool_address : Address = "0x1ac1A8FEaAEa1900C4166dEeed0C11cC10669D36".parse().unwrap();
        //let pool_address : Address = "0x7ca3EdB2c8fb3e657E282e67F4008d658aA161D2".parse().unwrap();
        let pool_address: Address = "0x9b5699d18dff51fc65fb8ad6f70d93287c36349f".parse().unwrap();

        let pool = PancakeV3Pool::fetch_pool_data(client.clone(), pool_address).await.unwrap();

        let state_required = pool.get_state_required().unwrap();
        debug!("{:?}", state_required);

        let state_update = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, None).await.unwrap();

        let mut market_state = MarketState::new(LoomDBType::default());

        market_state.state_db.apply_geth_update(state_update);

        let evm_env = Env::default();

        let (out_amount, gas_used) = pool
            .calculate_out_amount(
                &market_state.state_db,
                evm_env.clone(),
                &pool.token0,
                &pool.token1,
                U256::from(pool.liquidity0 / U256::from(100)),
            )
            .unwrap();
        debug!("{} {} ", out_amount, gas_used);
        assert_ne!(out_amount, U256::ZERO);
        assert!(gas_used > 100_000, "gas used check failed");

        let (out_amount, gas_used) = pool
            .calculate_out_amount(
                &market_state.state_db,
                evm_env.clone(),
                &pool.token1,
                &pool.token0,
                U256::from(pool.liquidity1 / U256::from(100)),
            )
            .unwrap();
        debug!("{} {} ", out_amount, gas_used);
        assert_ne!(out_amount, U256::ZERO);
        assert!(gas_used > 100_000, "gas used check failed");
    }
}
