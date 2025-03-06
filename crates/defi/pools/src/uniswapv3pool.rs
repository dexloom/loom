use std::any::Any;
use std::fmt::Debug;
use std::ops::Sub;

use crate::state_readers::UniswapV3QuoterV2StateReader;
use crate::state_readers::{UniswapV3QuoterV2Encoder, UniswapV3StateReader};
use crate::virtual_impl::UniswapV3PoolVirtual;
use alloy::primitives::{Address, Bytes, I256, U160, U256};
use alloy::providers::{Network, Provider};
use alloy::sol_types::{SolCall, SolInterface};
use eyre::{eyre, ErrReport, OptionExt, Result};
use lazy_static::lazy_static;
use loom_defi_abi::uniswap3::IUniswapV3Pool;
use loom_defi_abi::uniswap3::IUniswapV3Pool::slot0Return;
use loom_defi_abi::uniswap_periphery::ITickLens;
use loom_defi_abi::IERC20;
use loom_defi_address_book::{FactoryAddress, PeripheryAddress};
use loom_types_entities::required_state::RequiredState;
use loom_types_entities::{Pool, PoolAbiEncoder, PoolClass, PoolId, PoolProtocol, PreswapRequirement, SwapDirection};
use revm::primitives::Env;
use revm::DatabaseRef;
use tracing::debug;
#[cfg(feature = "debug-calculation")]
use tracing::error;

lazy_static! {
    static ref U256_ONE: U256 = U256::from(1);
    static ref LOWER_LIMIT: U160 = U160::from(4295128740u64);
    static ref UPPER_LIMIT: U160 = U160::from_str_radix("1461446703485210103287273052203988822378723970341", 10).unwrap();
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct Slot0 {
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
            tick: value.tick.try_into().unwrap_or_default(),
            fee_protocol: value.feeProtocol,
            observation_cardinality: value.observationCardinality,
            observation_cardinality_next: value.observationCardinalityNext,
            sqrt_price_x96: value.sqrtPriceX96.to(),
            unlocked: value.unlocked,
            observation_index: value.observationIndex,
        }
    }
}
#[allow(dead_code)]
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
            address,
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

    pub fn new_with_data(
        address: Address,
        token0: Address,
        token1: Address,
        liquidity: u128,
        fee: u32,
        slot0: Option<Slot0>,
        factory: Address,
    ) -> Self {
        UniswapV3Pool {
            address,
            token0,
            token1,
            liquidity,
            liquidity0: U256::ZERO,
            liquidity1: U256::ZERO,
            fee,
            slot0,
            factory,
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
        if token_address_from.lt(token_address_to) {
            *LOWER_LIMIT
        } else {
            *UPPER_LIMIT
        }
    }

    pub fn get_zero_for_one(token_address_from: &Address, token_address_to: &Address) -> bool {
        token_address_from.lt(token_address_to)
    }

    fn get_protocol_by_factory(factory_address: Address) -> PoolProtocol {
        if factory_address == FactoryAddress::UNISWAP_V3 {
            PoolProtocol::UniswapV3
        } else if factory_address == FactoryAddress::SUSHISWAP_V3 {
            PoolProtocol::SushiswapV3
        } else {
            PoolProtocol::UniswapV3Like
        }
    }

    pub fn fetch_pool_data_evm(db: &dyn DatabaseRef<Error = ErrReport>, env: Env, address: Address) -> Result<Self> {
        let token0 = UniswapV3StateReader::token0(&db, env.clone(), address)?;
        let token1 = UniswapV3StateReader::token1(&db, env.clone(), address)?;
        let fee: u32 = UniswapV3StateReader::fee(&db, env.clone(), address)?.to();
        let liquidity = UniswapV3StateReader::liquidity(&db, env.clone(), address)?;
        let factory = UniswapV3StateReader::factory(&db, env.clone(), address).unwrap_or_default();
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

    pub async fn fetch_pool_data<N: Network, P: Provider<N> + Send + Sync + Clone + 'static>(client: P, address: Address) -> Result<Self> {
        let uni3_pool = IUniswapV3Pool::IUniswapV3PoolInstance::new(address, client.clone());

        let token0: Address = uni3_pool.token0().call().await?._0;
        let token1: Address = uni3_pool.token1().call().await?._0;
        let fee: u32 = uni3_pool.fee().call().await?._0.try_into()?;
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

impl Pool for UniswapV3Pool {
    fn as_any<'a>(&self) -> &dyn Any {
        self
    }
    fn get_class(&self) -> PoolClass {
        PoolClass::UniswapV3
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
        _env: Env,
        token_address_from: &Address,
        _token_address_to: &Address,
        in_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        let (ret, gas_used) = if self.get_protocol() == PoolProtocol::UniswapV3 {
            let ret_virtual = UniswapV3PoolVirtual::simulate_swap_in_amount_provider(&state_db, self, *token_address_from, in_amount)?;

            #[cfg(feature = "debug-calculation")]
            {
                let mut env = _env;
                env.tx.gas_limit = 1_000_000;
                let (ret_evm, gas_used) = UniswapV3QuoterV2StateReader::quote_exact_input(
                    &state_db,
                    env,
                    PeripheryAddress::UNISWAP_V3_QUOTER_V2,
                    *token_address_from,
                    *_token_address_to,
                    self.fee.try_into()?,
                    in_amount,
                )?;
                println!("calculate_out_amount ret_evm: {:?} ret: {:?} gas_used: {:?}", ret_evm, ret_virtual, gas_used);
                if ret_virtual != ret_evm {
                    error!(%ret_virtual, %ret_evm, "calculate_out_amount RETURN_RESULT_IS_INCORRECT");
                };
            }
            (ret_virtual, 150_000)
        } else {
            let mut env = _env;
            env.tx.gas_limit = 1_000_000;
            let (ret_evm, gas_used) = UniswapV3QuoterV2StateReader::quote_exact_input(
                &state_db,
                env,
                PeripheryAddress::UNISWAP_V3_QUOTER_V2,
                *token_address_from,
                *_token_address_to,
                self.fee.try_into()?,
                in_amount,
            )?;
            (ret_evm, gas_used)
        };

        if ret.is_zero() {
            Err(eyre!("RETURN_RESULT_IS_ZERO"))
        } else {
            Ok((ret.checked_sub(*U256_ONE).ok_or_eyre("SUB_OVERFLOWN")?, gas_used))
            // value, gas_used
        }
    }

    fn calculate_in_amount(
        &self,
        state_db: &dyn DatabaseRef<Error = ErrReport>,
        _env: Env,
        token_address_from: &Address,
        _token_address_to: &Address,
        out_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        let (ret, gas_used) = if self.get_protocol() == PoolProtocol::UniswapV3 {
            let ret_virtual = UniswapV3PoolVirtual::simulate_swap_out_amount_provided(&state_db, self, *token_address_from, out_amount)?;

            #[cfg(feature = "debug-calculation")]
            {
                let mut env = _env;
                env.tx.gas_limit = 1_000_000;
                let (ret_evm, gas_used) = UniswapV3QuoterV2StateReader::quote_exact_output(
                    &state_db,
                    env,
                    PeripheryAddress::UNISWAP_V3_QUOTER_V2,
                    *token_address_from,
                    *_token_address_to,
                    self.fee.try_into()?,
                    out_amount,
                )?;
                println!("calculate_out_amount ret_evm: {:?} ret: {:?} gas_used: {:?}", ret_evm, ret_virtual, gas_used);

                if ret_virtual != ret_evm {
                    error!(%ret_virtual, %ret_evm,"calculate_in_amount RETURN_RESULT_IS_INCORRECT");
                }
            }
            (ret_virtual, 150000)
        } else {
            let mut env = _env;
            env.tx.gas_limit = 1_000_000;
            let (ret_evm, gas_used) = UniswapV3QuoterV2StateReader::quote_exact_output(
                &state_db,
                env,
                PeripheryAddress::UNISWAP_V3_QUOTER_V2,
                *token_address_from,
                *_token_address_to,
                self.fee.try_into()?,
                out_amount,
            )?;
            (ret_evm, gas_used)
        };

        if ret.is_zero() {
            Err(eyre!("RETURN_RESULT_IS_ZERO"))
        } else {
            Ok((ret.checked_add(*U256_ONE).ok_or_eyre("ADD_OVERFLOWN")?, gas_used))
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
        Vec::new()
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

        let balance_call_data = IERC20::IERC20Calls::balanceOf(IERC20::balanceOfCall { account: self.get_address() }).abi_encode();

        let pool_address = self.get_address();

        state_required
            .add_call(self.get_address(), IUniswapV3Pool::IUniswapV3PoolCalls::slot0(IUniswapV3Pool::slot0Call {}).abi_encode())
            .add_call(self.get_address(), IUniswapV3Pool::IUniswapV3PoolCalls::liquidity(IUniswapV3Pool::liquidityCall {}).abi_encode());

        for i in -4..=3 {
            state_required.add_call(
                PeripheryAddress::UNISWAP_V3_TICK_LENS,
                ITickLens::ITickLensCalls::getPopulatedTicksInWord(ITickLens::getPopulatedTicksInWordCall {
                    pool: pool_address,
                    tickBitmapIndex: tick_bitmap_index + i,
                })
                .abi_encode(),
            );
        }
        state_required
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
            let quoter_swap_0_1_call =
                UniswapV3QuoterV2Encoder::quote_exact_input_encode(self.token0, self.token1, self.fee.try_into()?, price_limit, amount);

            let price_limit = UniswapV3Pool::get_price_limit(&self.token1, &self.token0);
            let amount = self.liquidity1 / U256::from(100);

            let quoter_swap_1_0_call =
                UniswapV3QuoterV2Encoder::quote_exact_input_encode(self.token1, self.token0, self.fee.try_into()?, price_limit, amount);

            // TODO: How about Sushiswap?
            state_required
                .add_call(PeripheryAddress::UNISWAP_V3_QUOTER_V2, quoter_swap_0_1_call)
                .add_call(PeripheryAddress::UNISWAP_V3_QUOTER_V2, quoter_swap_1_0_call);
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
struct UniswapV3AbiSwapEncoder {
    pool_address: Address,
}

impl UniswapV3AbiSwapEncoder {
    pub fn new(pool_address: Address) -> Self {
        Self { pool_address }
    }
}

impl PoolAbiEncoder for UniswapV3AbiSwapEncoder {
    fn encode_swap_in_amount_provided(
        &self,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> Result<Bytes> {
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

    fn encode_swap_out_amount_provided(
        &self,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> Result<Bytes> {
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

// The test are using the deployed contracts for comparison to allow to adjust the test easily
#[cfg(test)]
mod test {
    use super::*;
    use alloy::primitives::{address, BlockNumber};
    use alloy::rpc::types::{BlockId, BlockNumberOrTag};
    use loom_defi_abi::uniswap_periphery::IQuoterV2;
    use loom_defi_abi::uniswap_periphery::IQuoterV2::{QuoteExactInputSingleParams, QuoteExactOutputSingleParams};
    use loom_defi_address_book::{PeripheryAddress, UniswapV3PoolAddress};
    use loom_evm_db::LoomDBType;
    use loom_evm_db::{AlloyDB, LoomDB};
    use loom_node_debug_provider::{AnvilDebugProviderFactory, AnvilDebugProviderType};
    use loom_types_entities::required_state::RequiredStateReader;
    use revm::db::EmptyDBTyped;
    use std::env;

    const POOL_ADDRESSES: [Address; 4] = [
        address!("15153da0e9e13cfc167b3d417d3721bf545479bb"), // Neiro/WETH pool 3000
        UniswapV3PoolAddress::USDC_WETH_3000,                 // USDC/WETH pool
        UniswapV3PoolAddress::WETH_USDT_3000,                 // WETH/USDT pool
        address!("11950d141ecb863f01007add7d1a342041227b58"), // PEPE/WETH pool 3000
    ];
    const BLOCK_NUMBER: u64 = 20935488u64;

    #[tokio::test]
    async fn test_pool_tokens() -> Result<()> {
        let node_url = env::var("MAINNET_WS")?;
        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, BlockNumber::from(BLOCK_NUMBER)).await?;

        for pool_address in POOL_ADDRESSES {
            let pool_contract = IUniswapV3Pool::new(pool_address, client.clone());
            let token0 = pool_contract.token0().call().await?._0;
            let token1 = pool_contract.token1().call().await?._0;

            let pool = UniswapV3Pool::fetch_pool_data(client.clone(), pool_address).await?;

            assert_eq!(token0, pool.token0);
            assert_eq!(token1, pool.token1);
        }

        Ok(())
    }

    async fn fetch_original_contract_amounts(
        client: AnvilDebugProviderType,
        pool_address: Address,
        token_in: Address,
        token_out: Address,
        amount: U256,
        block_number: u64,
        is_amount_out: bool,
    ) -> Result<U256> {
        let router_contract = IQuoterV2::new(PeripheryAddress::UNISWAP_V3_QUOTER_V2, client.clone());
        let pool_contract = IUniswapV3Pool::new(pool_address, client.clone());
        let pool_fee = pool_contract.fee().call().block(BlockId::from(block_number)).await?._0;

        if is_amount_out {
            let contract_amount_out = router_contract
                .quoteExactInputSingle(QuoteExactInputSingleParams {
                    tokenIn: token_in,
                    tokenOut: token_out,
                    amountIn: amount,
                    fee: pool_fee,
                    sqrtPriceLimitX96: U160::ZERO,
                })
                .call()
                .block(BlockId::from(block_number))
                .await?;
            Ok(contract_amount_out.amountOut)
        } else {
            let contract_amount_in = router_contract
                .quoteExactOutputSingle(QuoteExactOutputSingleParams {
                    tokenIn: token_in,
                    tokenOut: token_out,
                    amount,
                    fee: pool_fee,
                    sqrtPriceLimitX96: U160::ZERO,
                })
                .call()
                .block(BlockId::from(block_number))
                .await?;
            Ok(contract_amount_in.amountIn)
        }
    }

    #[tokio::test]
    async fn test_calculate_out_amount() -> Result<()> {
        // Verify that the calculated out amount is the same as the contract's out amount
        let node_url = env::var("MAINNET_WS")?;
        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, BlockNumber::from(BLOCK_NUMBER)).await?;

        for pool_address in POOL_ADDRESSES {
            let pool = UniswapV3Pool::fetch_pool_data(client.clone(), pool_address).await?;
            let state_required = pool.get_state_required()?;
            let state_update = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, Some(BLOCK_NUMBER)).await?;

            let mut state_db = LoomDBType::default();
            state_db.apply_geth_update(state_update);

            let token0_decimals = IERC20::new(pool.token0, client.clone()).decimals().call().block(BlockId::from(BLOCK_NUMBER)).await?._0;
            let token1_decimals = IERC20::new(pool.token1, client.clone()).decimals().call().block(BlockId::from(BLOCK_NUMBER)).await?._0;

            //// CASE: token0 -> token1
            let amount_in = U256::from(10u64).pow(token0_decimals);
            let contract_amount_out =
                fetch_original_contract_amounts(client.clone(), pool_address, pool.token0, pool.token1, amount_in, BLOCK_NUMBER, true)
                    .await?;

            // under test
            let (amount_out, gas_used) = match pool.calculate_out_amount(&state_db, Env::default(), &pool.token0, &pool.token1, amount_in) {
                Ok((amount_out, gas_used)) => (amount_out, gas_used),
                Err(e) => {
                    panic!("Calculation error for pool={:?}, amount_in={}, e={:?}", pool_address, amount_in, e);
                }
            };
            assert_eq!(
                amount_out, contract_amount_out,
                "Mismatch for pool={:?}, token_out={}, amount_in={}",
                pool_address, &pool.token1, amount_in
            );
            assert_eq!(gas_used, 150_000);

            //// CASE: token1 -> token0
            let amount_in = U256::from(10u64).pow(token1_decimals);
            let contract_amount_out =
                fetch_original_contract_amounts(client.clone(), pool_address, pool.token1, pool.token0, amount_in, BLOCK_NUMBER, true)
                    .await?;

            // under test
            let (amount_out, gas_used) = match pool.calculate_out_amount(&state_db, Env::default(), &pool.token1, &pool.token0, amount_in) {
                Ok((amount_out, gas_used)) => (amount_out, gas_used),
                Err(e) => {
                    panic!("Calculation error for pool={:?}, amount_in={}, e={:?}", pool_address, amount_in, e);
                }
            };
            assert_eq!(
                amount_out, contract_amount_out,
                "Mismatch for pool={:?}, token_out={}, amount_in={}",
                pool_address, &pool.token0, amount_in
            );
            assert_eq!(gas_used, 150_000);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_in_amount() -> Result<()> {
        // Verify that the calculated out amount is the same as the contract's out amount
        let node_url = env::var("MAINNET_WS")?;
        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, BlockNumber::from(BLOCK_NUMBER)).await?;

        for pool_address in POOL_ADDRESSES {
            let pool = UniswapV3Pool::fetch_pool_data(client.clone(), pool_address).await?;
            let state_required = pool.get_state_required()?;
            let state_update = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, Some(BLOCK_NUMBER)).await?;

            let mut state_db = LoomDBType::default().with_ext_db(EmptyDBTyped::<ErrReport>::new());
            state_db.apply_geth_update(state_update);

            let token0_decimals = IERC20::new(pool.token0, client.clone()).decimals().call().block(BlockId::from(BLOCK_NUMBER)).await?._0;
            let token1_decimals = IERC20::new(pool.token1, client.clone()).decimals().call().block(BlockId::from(BLOCK_NUMBER)).await?._0;

            //// CASE: token0 -> token1
            let amount_out = U256::from(10u64).pow(token1_decimals);
            let contract_amount_in =
                fetch_original_contract_amounts(client.clone(), pool_address, pool.token0, pool.token1, amount_out, BLOCK_NUMBER, false)
                    .await?;

            // under test
            let (amount_in, gas_used) = match pool.calculate_in_amount(&state_db, Env::default(), &pool.token0, &pool.token1, amount_out) {
                Ok((amount_in, gas_used)) => (amount_in, gas_used),
                Err(e) => {
                    panic!("Calculation error for pool={:?}, amount_out={}, e={:?}", pool_address, amount_out, e);
                }
            };
            assert_eq!(
                amount_in, contract_amount_in,
                "Mismatch for pool={:?}, token_in={:?}, amount_out={}",
                pool_address, &pool.token0, amount_out
            );
            assert_eq!(gas_used, 150_000);

            //// CASE: token1 -> token0
            let amount_out = U256::from(10u64).pow(token0_decimals);
            let contract_amount_in =
                fetch_original_contract_amounts(client.clone(), pool_address, pool.token1, pool.token0, amount_out, BLOCK_NUMBER, false)
                    .await?;

            // under test
            let (amount_in, gas_used) = match pool.calculate_in_amount(&state_db, Env::default(), &pool.token1, &pool.token0, amount_out) {
                Ok((amount_in, gas_used)) => (amount_in, gas_used),
                Err(e) => {
                    panic!("Calculation error for pool={:?}, amount_out={}, e={:?}", pool_address, amount_out, e);
                }
            };
            assert_eq!(
                amount_in, contract_amount_in,
                "Mismatch for pool={:?}, token_in={:?}, amount_out={}",
                pool_address, &pool.token1, amount_out
            );
            assert_eq!(gas_used, 150_000);
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_calculate_in_amount_with_ext_db() -> Result<()> {
        // Verify that the calculated out amount is the same as the contract's out amount
        let node_url = env::var("MAINNET_WS")?;
        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, BlockNumber::from(BLOCK_NUMBER)).await?;

        for pool_address in POOL_ADDRESSES {
            let pool = UniswapV3Pool::fetch_pool_data(client.clone(), pool_address).await?;

            let alloy_db = AlloyDB::new(client.clone(), BlockNumberOrTag::Number(BLOCK_NUMBER).into()).unwrap();

            let state_db = LoomDB::new().with_ext_db(alloy_db);

            let token0_decimals = IERC20::new(pool.token0, client.clone()).decimals().call().block(BlockId::from(BLOCK_NUMBER)).await?._0;
            let token1_decimals = IERC20::new(pool.token1, client.clone()).decimals().call().block(BlockId::from(BLOCK_NUMBER)).await?._0;

            //// CASE: token0 -> token1
            let amount_out = U256::from(10u64).pow(token1_decimals);
            let contract_amount_in =
                fetch_original_contract_amounts(client.clone(), pool_address, pool.token0, pool.token1, amount_out, BLOCK_NUMBER, false)
                    .await?;

            // under test
            let (amount_in, gas_used) = match pool.calculate_in_amount(&state_db, Env::default(), &pool.token0, &pool.token1, amount_out) {
                Ok((amount_in, gas_used)) => (amount_in, gas_used),
                Err(e) => {
                    panic!("Calculation error for pool={:?}, amount_out={}, e={:?}", pool_address, amount_out, e);
                }
            };
            assert_eq!(
                amount_in, contract_amount_in,
                "Mismatch for pool={:?}, token_in={:?}, amount_out={}",
                pool_address, &pool.token0, amount_out
            );
            assert_eq!(gas_used, 150_000);

            //// CASE: token1 -> token0
            let amount_out = U256::from(10u64).pow(token0_decimals);
            let contract_amount_in =
                fetch_original_contract_amounts(client.clone(), pool_address, pool.token1, pool.token0, amount_out, BLOCK_NUMBER, false)
                    .await?;

            // under test
            let (amount_in, gas_used) = match pool.calculate_in_amount(&state_db, Env::default(), &pool.token1, &pool.token0, amount_out) {
                Ok((amount_in, gas_used)) => (amount_in, gas_used),
                Err(e) => {
                    panic!("Calculation error for pool={:?}, amount_out={}, e={:?}", pool_address, amount_out, e);
                }
            };
            assert_eq!(
                amount_in, contract_amount_in,
                "Mismatch for pool={:?}, token_in={:?}, amount_out={}",
                pool_address, &pool.token1, amount_out
            );
            assert_eq!(gas_used, 150_000);
        }

        Ok(())
    }
}
