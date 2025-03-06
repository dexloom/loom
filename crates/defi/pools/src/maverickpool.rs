use alloy::primitives::{Address, Bytes, U128, U256};
use alloy::providers::{Network, Provider};
use alloy::sol_types::{SolCall, SolInterface};
use eyre::{eyre, ErrReport, OptionExt, Result};
use lazy_static::lazy_static;
use loom_defi_abi::maverick::IMaverickPool::{getStateCall, IMaverickPoolCalls, IMaverickPoolInstance};
use loom_defi_abi::maverick::IMaverickQuoter::{calculateSwapCall, IMaverickQuoterCalls};
use loom_defi_abi::maverick::{IMaverickPool, IMaverickQuoter, State};
use loom_defi_abi::IERC20;
use loom_defi_address_book::PeripheryAddress;
use loom_evm_utils::evm::evm_call;
use loom_types_entities::required_state::RequiredState;
use loom_types_entities::{Pool, PoolAbiEncoder, PoolClass, PoolId, PoolProtocol, PreswapRequirement, SwapDirection};
use revm::primitives::Env;
use revm::DatabaseRef;
use std::any::Any;
use tracing::error;

use crate::state_readers::UniswapV3StateReader;

lazy_static! {
    static ref U256_ONE: U256 = U256::from(1);
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct MaverickPool {
    //contract_storage : ContractStorage,
    address: Address,
    pub token0: Address,
    pub token1: Address,
    liquidity0: U256,
    liquidity1: U256,
    fee: U256,
    spacing: u32,
    slot0: Option<State>,
    factory: Address,
    protocol: PoolProtocol,
    encoder: MaverickAbiSwapEncoder,
}

impl MaverickPool {
    pub fn new(address: Address) -> Self {
        MaverickPool {
            address,
            token0: Address::ZERO,
            token1: Address::ZERO,
            liquidity0: U256::ZERO,
            liquidity1: U256::ZERO,
            fee: U256::ZERO,
            spacing: 0,
            slot0: None,
            factory: Address::ZERO,
            protocol: PoolProtocol::Maverick,
            encoder: MaverickAbiSwapEncoder::new(address),
        }
    }

    pub fn get_tick_bitmap_index(tick: i32, spacing: u32) -> i32 {
        let tick_bitmap_index = tick / (spacing as i32);

        if tick_bitmap_index < 0 {
            ((tick_bitmap_index + 1) / 256) - 1
        } else {
            tick_bitmap_index >> 8
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
        *token_address_from < *token_address_to
    }

    fn get_protocol_by_factory(_factory_address: Address) -> PoolProtocol {
        PoolProtocol::Maverick
    }

    pub async fn fetch_pool_data<N: Network, P: Provider<N> + Send + Sync + Clone + 'static>(client: P, address: Address) -> Result<Self> {
        let pool = IMaverickPoolInstance::new(address, client.clone());

        let token0: Address = pool.tokenA().call().await?._0;
        let token1: Address = pool.tokenB().call().await?._0;
        let fee: U256 = pool.fee().call().await?._0;
        let slot0 = pool.getState().call().await?._0;
        let factory: Address = pool.factory().call().await?._0;
        let spacing: u32 = pool.tickSpacing().call().await?._0.to();

        let token0_erc20 = IERC20::IERC20Instance::new(token0, client.clone());
        let token1_erc20 = IERC20::IERC20Instance::new(token1, client.clone());

        let liquidity0: U256 = token0_erc20.balanceOf(address).call().await?._0;
        let liquidity1: U256 = token1_erc20.balanceOf(address).call().await?._0;

        let protocol = MaverickPool::get_protocol_by_factory(factory);

        let ret = MaverickPool {
            address,
            token0,
            token1,
            fee,
            slot0: Some(slot0),
            liquidity0,
            liquidity1,
            factory,
            protocol,
            spacing,
            encoder: MaverickAbiSwapEncoder { pool_address: address },
        };

        Ok(ret)
    }
    pub fn fetch_pool_data_evm(db: &dyn DatabaseRef<Error = ErrReport>, env: Env, address: Address) -> Result<Self> {
        let token0: Address = UniswapV3StateReader::token0(&db, env.clone(), address)?;
        let token1: Address = UniswapV3StateReader::token1(&db, env.clone(), address)?;
        let fee = UniswapV3StateReader::fee(&db, env.clone(), address)?;
        let factory: Address = UniswapV3StateReader::factory(&db, env.clone(), address)?;
        let spacing: u32 = UniswapV3StateReader::tick_spacing(&db, env.clone(), address)?;

        let protocol = Self::get_protocol_by_factory(factory);

        let ret = MaverickPool {
            address,
            token0,
            token1,
            liquidity0: Default::default(),
            liquidity1: Default::default(),
            fee: U256::from(fee),
            spacing,
            slot0: None,
            factory,
            protocol,
            encoder: MaverickAbiSwapEncoder { pool_address: address },
        };

        Ok(ret)
    }
}

impl Pool for MaverickPool {
    fn as_any<'a>(&self) -> &dyn Any {
        self
    }

    fn get_class(&self) -> PoolClass {
        PoolClass::Maverick
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
        self.fee
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
        if in_amount >= U256::from(U128::MAX) {
            error!("IN_AMOUNT_EXCEEDS_MAX {}", self.get_address().to_checksum(None));
            return Err(eyre!("IN_AMOUNT_EXCEEDS_MAX"));
        }

        let token_a_in = MaverickPool::get_zero_for_one(token_address_from, token_address_to);
        //let sqrt_price_limit = MaverickPool::get_price_limit(token_address_from, token_address_to);

        let mut env = env;
        env.tx.gas_limit = 1_500_000;

        let call_data_vec = IMaverickQuoterCalls::calculateSwap(calculateSwapCall {
            pool: self.address,
            amount: in_amount.to(),
            tokenAIn: token_a_in,
            exactOutput: false,
            sqrtPriceLimit: U256::ZERO,
        })
        .abi_encode();

        let (value, gas_used) = evm_call(state_db, env, PeripheryAddress::MAVERICK_QUOTER, call_data_vec)?;

        let ret = calculateSwapCall::abi_decode_returns(&value, false)?.returnAmount;

        if ret.is_zero() {
            Err(eyre!("ZERO_OUT_AMOUNT"))
        } else {
            Ok((ret.checked_sub(*U256_ONE).ok_or_eyre("SUBTRACTION_OVERFLOWN")?, gas_used))
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
        env.tx.gas_limit = 500_000;

        if out_amount >= U256::from(U128::MAX) {
            error!("OUT_AMOUNT_EXCEEDS_MAX {} ", self.get_address().to_checksum(None));
            return Err(eyre!("OUT_AMOUNT_EXCEEDS_MAX"));
        }

        let token_a_in = MaverickPool::get_zero_for_one(token_address_from, token_address_to);
        //let sqrt_price_limit = MaverickPool::get_price_limit(token_address_from, token_address_to);

        let call_data_vec = IMaverickQuoterCalls::calculateSwap(calculateSwapCall {
            pool: self.address,
            amount: out_amount.to(),
            tokenAIn: token_a_in,
            exactOutput: true,
            sqrtPriceLimit: U256::ZERO,
        })
        .abi_encode();

        let (value, gas_used) = evm_call(state_db, env, PeripheryAddress::MAVERICK_QUOTER, call_data_vec)?;

        let ret = calculateSwapCall::abi_decode_returns(&value, false)?.returnAmount;

        if ret.is_zero() {
            Err(eyre!("ZERO_IN_AMOUNT"))
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
        let tick = self.slot0.clone().unwrap().activeTick;

        let quoter_swap_0_1_call = IMaverickQuoterCalls::calculateSwap(calculateSwapCall {
            pool: self.address,
            amount: (self.liquidity0 / U256::from(100)).to(),
            tokenAIn: true,
            exactOutput: false,
            sqrtPriceLimit: U256::ZERO,
        })
        .abi_encode();

        //let sqrt_price_limit = MaverickPool::get_price_limit(&self.token1, &self.token0);

        let quoter_swap_1_0_call = IMaverickQuoterCalls::calculateSwap(calculateSwapCall {
            pool: self.address,
            amount: (self.liquidity1 / U256::from(100)).to(),
            tokenAIn: false,
            exactOutput: false,
            sqrtPriceLimit: U256::ZERO,
        })
        .abi_encode();

        //let tick_bitmap_index = MaverickPool::get_tick_bitmap_index(tick, self.spacing.as_u32());
        let tick_bitmap_index = tick;

        let pool_address = self.get_address();

        let mut state_required = RequiredState::new();
        state_required
            .add_call(self.get_address(), IMaverickPoolCalls::getState(getStateCall {}).abi_encode())
            .add_call(
                PeripheryAddress::MAVERICK_QUOTER,
                IMaverickQuoterCalls::getBinsAtTick(IMaverickQuoter::getBinsAtTickCall { pool: pool_address, tick: tick_bitmap_index - 4 })
                    .abi_encode(),
            )
            .add_call(
                PeripheryAddress::MAVERICK_QUOTER,
                IMaverickQuoterCalls::getBinsAtTick(IMaverickQuoter::getBinsAtTickCall { pool: pool_address, tick: tick_bitmap_index - 3 })
                    .abi_encode(),
            )
            .add_call(
                PeripheryAddress::MAVERICK_QUOTER,
                IMaverickQuoterCalls::getBinsAtTick(IMaverickQuoter::getBinsAtTickCall { pool: pool_address, tick: tick_bitmap_index - 2 })
                    .abi_encode(),
            )
            .add_call(
                PeripheryAddress::MAVERICK_QUOTER,
                IMaverickQuoterCalls::getBinsAtTick(IMaverickQuoter::getBinsAtTickCall { pool: pool_address, tick: tick_bitmap_index - 1 })
                    .abi_encode(),
            )
            .add_call(
                PeripheryAddress::MAVERICK_QUOTER,
                IMaverickQuoterCalls::getBinsAtTick(IMaverickQuoter::getBinsAtTickCall { pool: pool_address, tick: tick_bitmap_index })
                    .abi_encode(),
            )
            .add_call(
                PeripheryAddress::MAVERICK_QUOTER,
                IMaverickQuoterCalls::getBinsAtTick(IMaverickQuoter::getBinsAtTickCall { pool: pool_address, tick: tick_bitmap_index + 1 })
                    .abi_encode(),
            )
            .add_call(
                PeripheryAddress::MAVERICK_QUOTER,
                IMaverickQuoterCalls::getBinsAtTick(IMaverickQuoter::getBinsAtTickCall { pool: pool_address, tick: tick_bitmap_index + 2 })
                    .abi_encode(),
            )
            .add_call(
                PeripheryAddress::MAVERICK_QUOTER,
                IMaverickQuoterCalls::getBinsAtTick(IMaverickQuoter::getBinsAtTickCall { pool: pool_address, tick: tick_bitmap_index + 3 })
                    .abi_encode(),
            )
            .add_call(
                PeripheryAddress::MAVERICK_QUOTER,
                IMaverickQuoterCalls::getBinsAtTick(IMaverickQuoter::getBinsAtTickCall { pool: pool_address, tick: tick_bitmap_index + 4 })
                    .abi_encode(),
            )
            .add_call(PeripheryAddress::MAVERICK_QUOTER, quoter_swap_0_1_call)
            .add_call(PeripheryAddress::MAVERICK_QUOTER, quoter_swap_1_0_call)
            .add_slot_range(self.get_address(), U256::from(0), 0x20);

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
struct MaverickAbiSwapEncoder {
    pool_address: Address,
}

impl MaverickAbiSwapEncoder {
    pub fn new(pool_address: Address) -> Self {
        Self { pool_address }
    }
}

impl PoolAbiEncoder for MaverickAbiSwapEncoder {
    fn encode_swap_in_amount_provided(
        &self,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> Result<Bytes> {
        //let sqrt_price_limit_x96 = MaverickPool::get_price_limit(&token_from_address, &token_to_address);

        let token_a_in = MaverickPool::get_zero_for_one(&token_from_address, &token_to_address);

        let swap_call = IMaverickPool::swapCall {
            recipient,
            amount,
            tokenAIn: token_a_in,
            exactOutput: false,
            sqrtPriceLimit: U256::ZERO,
            data: payload,
        };

        Ok(Bytes::from(IMaverickPoolCalls::swap(swap_call).abi_encode()))
    }

    fn encode_swap_out_amount_provided(
        &self,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> Result<Bytes> {
        let token_a_in = MaverickPool::get_zero_for_one(&token_from_address, &token_to_address);
        let sqrt_price_limit_x96 = MaverickPool::get_price_limit(&token_from_address, &token_to_address);

        let swap_call = IMaverickPool::swapCall {
            recipient,
            amount,
            tokenAIn: token_a_in,
            exactOutput: true,
            sqrtPriceLimit: sqrt_price_limit_x96,
            data: payload,
        };

        Ok(Bytes::from(IMaverickPoolCalls::swap(swap_call).abi_encode()))
    }

    fn swap_in_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x24)
    }
    fn swap_out_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x24)
    }
    fn swap_out_amount_return_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x20)
    }
    fn swap_in_amount_return_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::providers::network::primitives::BlockTransactionsKind;
    use alloy::rpc::types::BlockNumberOrTag;
    use loom_defi_abi::maverick::IMaverickQuoter::IMaverickQuoterInstance;
    use loom_evm_db::LoomDBType;
    use loom_evm_utils::evm_env::env_for_block;
    use loom_node_debug_provider::AnvilDebugProviderFactory;
    use loom_types_entities::required_state::RequiredStateReader;
    use loom_types_entities::MarketState;
    use std::env;
    use tracing::debug;

    #[tokio::test]
    async fn test_pool() -> Result<()> {
        let _ = env_logger::try_init_from_env(env_logger::Env::default().default_filter_or("info,defi_pools=off"));

        let node_url = env::var("MAINNET_WS")?;

        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, 20045799).await?;

        let pool_address: Address = "0x352B186090068Eb35d532428676cE510E17AB581".parse().unwrap();

        let pool = MaverickPool::fetch_pool_data(client.clone(), pool_address).await.unwrap();

        let state_required = pool.get_state_required()?;

        let state_required = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, None).await?;
        debug!("{:?}", state_required);

        let mut market_state = MarketState::new(LoomDBType::default());
        market_state.state_db.apply_geth_update(state_required);

        let block_number = client.get_block_number().await?;
        let block = client.get_block_by_number(BlockNumberOrTag::Number(block_number), BlockTransactionsKind::Hashes).await?.unwrap();

        let evm_env = env_for_block(block.header.number, block.header.timestamp);

        let amount = U256::from(pool.liquidity1 / U256::from(1000));

        let quoter = IMaverickQuoterInstance::new(PeripheryAddress::MAVERICK_QUOTER, client.clone());

        let resp = quoter.calculateSwap(pool_address, amount.to(), false, false, U256::ZERO).call().await?;
        debug!("Router call : {:?}", resp.returnAmount);
        assert_ne!(resp.returnAmount, U256::ZERO);

        let (out_amount, gas_used) = pool
            .calculate_out_amount(
                &market_state.state_db,
                evm_env.clone(),
                &pool.token1,
                &pool.token0,
                U256::from(pool.liquidity1 / U256::from(1000)),
            )
            .unwrap();
        debug!("{} {} {}", pool.get_protocol(), out_amount, gas_used);
        assert_ne!(out_amount, U256::ZERO);
        assert!(gas_used > 100000);

        let (out_amount, gas_used) = pool
            .calculate_out_amount(
                &market_state.state_db,
                evm_env.clone(),
                &pool.token0,
                &pool.token1,
                U256::from(pool.liquidity0 / U256::from(1000)),
            )
            .unwrap();
        debug!("{} {} {}", pool.get_protocol(), out_amount, gas_used);
        assert_ne!(out_amount, U256::ZERO);
        assert!(gas_used > 100000);

        Ok(())
    }
}
