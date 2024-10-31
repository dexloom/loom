use alloy_primitives::{Address, Bytes, U256};
use alloy_provider::{Network, Provider};
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_types::SolInterface;
use alloy_transport::Transport;
use eyre::{eyre, ErrReport, Result};
use lazy_static::lazy_static;
use loom_defi_entities::required_state::RequiredState;
use loom_defi_entities::{AbiSwapEncoder, Pool, PoolClass, PoolProtocol, PreswapRequirement};
use loom_evm_db::LoomDBType;
use loom_protocol_abi::uniswap2::IUniswapV2Pair;
use loom_protocol_abi::IERC20;
use loom_protocol_address_book::FactoryAddress;
use revm::primitives::Env;
use revm::DatabaseRef;
use std::ops::Div;
use tracing::debug;

use crate::state_readers::UniswapV2StateReader;

lazy_static! {
    static ref U112_MASK: U256 = (U256::from(1) << 112) - U256::from(1);
}
#[allow(dead_code)]
#[derive(Clone)]
pub struct UniswapV2Pool {
    address: Address,
    token0: Address,
    token1: Address,
    factory: Address,
    protocol: PoolProtocol,
    fee: U256,
    encoder: UniswapV2AbiSwapEncoder,
    reserves_cell: Option<U256>,
    liquidity0: U256,
    liquidity1: U256,
}

impl UniswapV2Pool {
    pub fn new(address: Address) -> UniswapV2Pool {
        UniswapV2Pool {
            address,
            token0: Address::ZERO,
            token1: Address::ZERO,
            factory: Address::ZERO,
            protocol: PoolProtocol::UniswapV2Like,
            fee: U256::from(9970),
            encoder: UniswapV2AbiSwapEncoder::new(address),
            reserves_cell: None,
            liquidity0: U256::ZERO,
            liquidity1: U256::ZERO,
        }
    }

    pub fn new_with_data(
        address: Address,
        token0: Address,
        token1: Address,
        factory: Address,
        liquidity0: U256,
        liquidity1: U256,
    ) -> UniswapV2Pool {
        UniswapV2Pool {
            address,
            token0,
            token1,
            factory,
            protocol: PoolProtocol::UniswapV2Like,
            fee: U256::from(9970),
            encoder: UniswapV2AbiSwapEncoder::new(address),
            reserves_cell: None,
            liquidity0,
            liquidity1,
        }
    }

    pub fn set_fee(self, fee: U256) -> Self {
        Self { fee, ..self }
    }

    pub fn get_zero_for_one(token_address_from: Address, token_address_to: Address) -> bool {
        token_address_from < token_address_to
    }

    fn get_protocol_by_factory(factory_address: Address) -> PoolProtocol {
        if factory_address == FactoryAddress::UNISWAP_V2 {
            PoolProtocol::UniswapV2
        } else if factory_address == FactoryAddress::SUSHISWAP_V2 {
            PoolProtocol::Sushiswap
        } else if factory_address == FactoryAddress::NOMISWAP {
            PoolProtocol::NomiswapStable
        } else if factory_address == FactoryAddress::DOOARSWAP {
            PoolProtocol::DooarSwap
        } else if factory_address == FactoryAddress::SAFESWAP {
            PoolProtocol::Safeswap
        } else if factory_address == FactoryAddress::MINISWAP {
            PoolProtocol::Miniswap
        } else if factory_address == FactoryAddress::SHIBASWAP {
            PoolProtocol::Shibaswap
        } else if factory_address == FactoryAddress::OG_PEPE {
            PoolProtocol::OgPepe
        } else {
            PoolProtocol::UniswapV2Like
        }
    }

    fn get_fee_by_protocol(protocol: PoolProtocol) -> U256 {
        match protocol {
            PoolProtocol::DooarSwap | PoolProtocol::OgPepe => U256::from(9900),
            _ => U256::from(9970),
        }
    }

    fn storage_to_reserves(value: U256) -> (U256, U256) {
        //let uvalue : U256 = value.convert();
        ((value >> 0) & *U112_MASK, (value >> (112)) & *U112_MASK)
    }

    pub fn fetch_pool_data_evm(db: &LoomDBType, env: Env, address: Address) -> Result<Self> {
        let token0 = UniswapV2StateReader::token0(db, env.clone(), address)?;
        let token1 = UniswapV2StateReader::token1(db, env.clone(), address)?;
        let factory = UniswapV2StateReader::factory(db, env.clone(), address)?;
        let protocol = Self::get_protocol_by_factory(factory);

        let fee = Self::get_fee_by_protocol(protocol);

        let ret = UniswapV2Pool {
            address,
            token0,
            token1,
            fee,
            factory,
            protocol,
            encoder: UniswapV2AbiSwapEncoder { pool_address: address },
            reserves_cell: None,
            liquidity0: Default::default(),
            liquidity1: Default::default(),
        };
        debug!("fetch_pool_data_evm {:?} {:?} {} {:?} {}", token0, token1, fee, factory, protocol);

        Ok(ret)
    }

    pub async fn fetch_pool_data<T: Transport + Clone, N: Network, P: Provider<T, N> + Send + Sync + Clone + 'static>(
        client: P,
        address: Address,
    ) -> Result<Self> {
        let uni2_pool = IUniswapV2Pair::IUniswapV2PairInstance::new(address, client.clone());

        let token0: Address = uni2_pool.token0().call().await?._0;
        let token1: Address = uni2_pool.token1().call().await?._0;
        let factory: Address = uni2_pool.factory().call().await?._0;
        let reserves = uni2_pool.getReserves().call().await?.clone();

        let storage_reserves_cell = client.get_storage_at(address, U256::from(8)).block_id(BlockNumberOrTag::Latest.into()).await.unwrap();

        let storage_reserves = Self::storage_to_reserves(storage_reserves_cell);

        let reserves_cell: Option<U256> =
            if storage_reserves.0 == U256::from(reserves.reserve0) && storage_reserves.1 == U256::from(reserves.reserve1) {
                Some(U256::from(8))
            } else {
                debug!("{storage_reserves:?} {reserves:?}");
                None
            };

        let protocol = UniswapV2Pool::get_protocol_by_factory(factory);

        let fee = Self::get_fee_by_protocol(protocol);

        let ret = UniswapV2Pool {
            address,
            token0,
            token1,
            factory,
            protocol,
            fee,
            reserves_cell,
            liquidity0: U256::from(reserves.reserve0),
            liquidity1: U256::from(reserves.reserve1),
            encoder: UniswapV2AbiSwapEncoder::new(address),
        };
        Ok(ret)
    }

    pub fn fetch_reserves(&self, state_db: &LoomDBType, env: Env) -> Result<(U256, U256)> {
        let (reserve_0, reserve_1) = match self.reserves_cell {
            Some(cell) => {
                if let Ok(storage_value) = state_db.storage_ref(self.get_address(), cell) {
                    Self::storage_to_reserves(storage_value)
                } else {
                    return Err(eyre!("ERROR_READING_STATE_DB"));
                }
            }
            None => UniswapV2StateReader::get_reserves(state_db, env, self.get_address())?,
        };
        Ok((reserve_0, reserve_1))
    }
}

impl Pool for UniswapV2Pool {
    fn get_class(&self) -> PoolClass {
        PoolClass::UniswapV2
    }

    fn get_protocol(&self) -> PoolProtocol {
        self.protocol
    }

    fn get_address(&self) -> Address {
        self.address
    }

    fn get_fee(&self) -> U256 {
        self.fee
    }

    fn get_tokens(&self) -> Vec<Address> {
        vec![self.token0, self.token1]
    }

    fn get_swap_directions(&self) -> Vec<(Address, Address)> {
        vec![(self.token0, self.token1), (self.token1, self.token0)]
    }

    fn calculate_out_amount(
        &self,
        state_db: &LoomDBType,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        in_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        let (reserves_0, reserves_1) = self.fetch_reserves(state_db, env)?;

        let (reserve_in, reserve_out) = match token_address_from < token_address_to {
            true => (reserves_0, reserves_1),
            false => (reserves_1, reserves_0),
        };

        let amount_in_with_fee = in_amount.checked_mul(self.fee).ok_or(eyre!("AMOUNT_IN_WITH_FEE_OVERFLOW"))?;
        let numerator = amount_in_with_fee.checked_mul(reserve_out).ok_or(eyre!("NUMERATOR_OVERFLOW"))?;
        let denominator = reserve_in.checked_mul(U256::from(10000)).ok_or(eyre!("DENOMINATOR_OVERFLOW"))?;
        let denominator = denominator.checked_add(amount_in_with_fee).ok_or(eyre!("DENOMINATOR_OVERFLOW_FEE"))?;

        let out_amount = numerator.checked_div(denominator).ok_or(eyre!("CANNOT_CALCULATE_ZERO_RESERVE"))?;
        if out_amount > reserve_out {
            Err(eyre!("RESERVE_EXCEEDED"))
        } else if out_amount.is_zero() {
            Err(eyre!("OUT_AMOUNT_IS_ZERO"))
        } else {
            Ok((out_amount, 100_000))
        }
    }

    fn calculate_in_amount(
        &self,
        state_db: &LoomDBType,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        out_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        let (reserves_0, reserves_1) = self.fetch_reserves(state_db, env)?;

        let (reserve_in, reserve_out) = match token_address_from < token_address_to {
            true => (reserves_0, reserves_1),
            false => (reserves_1, reserves_0),
        };

        if out_amount > reserve_out {
            return Err(eyre!("RESERVE_OUT_EXCEEDED"));
        }
        let numerator = reserve_in.checked_mul(out_amount).ok_or(eyre!("NUMERATOR_OVERFLOW"))?;
        let numerator = numerator.checked_mul(U256::from(10000)).ok_or(eyre!("NUMERATOR_OVERFLOW_FEE"))?;
        let denominator = reserve_out.checked_sub(out_amount).ok_or(eyre!("DENOMINATOR_UNDERFLOW"))?;
        let denominator = denominator.checked_mul(self.fee).ok_or(eyre!("DENOMINATOR_OVERFLOW_FEE"))?;

        if denominator.is_zero() {
            Err(eyre!("CANNOT_CALCULATE_ZERO_RESERVE"))
        } else {
            let in_amount = numerator.div(denominator); // We assure before that denominator is not zero
            if in_amount.is_zero() {
                Err(eyre!("IN_AMOUNT_IS_ZERO"))
            } else {
                Ok((in_amount + U256::from(1), 100_000))
            }
        }
    }

    fn can_flash_swap(&self) -> bool {
        true
    }

    fn get_encoder(&self) -> &dyn AbiSwapEncoder {
        &self.encoder
    }

    fn get_state_required(&self) -> Result<RequiredState> {
        let mut state_required = RequiredState::new();

        let reserves_call_data_vec = IUniswapV2Pair::IUniswapV2PairCalls::factory(IUniswapV2Pair::factoryCall {}).abi_encode();

        state_required.add_call(self.get_address(), reserves_call_data_vec).add_slot_range(self.get_address(), U256::from(0), 0x20);

        for token_address in self.get_tokens() {
            state_required.add_call(
                token_address,
                IERC20::IERC20Calls::balanceOf(IERC20::balanceOfCall { account: self.get_address() }).abi_encode(),
            );
        }

        Ok(state_required)
    }
}

#[derive(Clone, Copy)]
struct UniswapV2AbiSwapEncoder {
    pool_address: Address,
}

impl UniswapV2AbiSwapEncoder {
    pub fn new(pool_address: Address) -> Self {
        Self { pool_address }
    }
}

impl AbiSwapEncoder for UniswapV2AbiSwapEncoder {
    fn encode_swap_out_amount_provided(
        &self,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> Result<Bytes> {
        let swap_call = if token_from_address < token_to_address {
            IUniswapV2Pair::swapCall { amount0Out: U256::ZERO, amount1Out: amount, to: recipient, data: payload }
        } else {
            IUniswapV2Pair::swapCall { amount0Out: amount, amount1Out: U256::ZERO, to: recipient, data: payload }
        };

        Ok(Bytes::from(IUniswapV2Pair::IUniswapV2PairCalls::swap(swap_call).abi_encode()))
    }

    fn preswap_requirement(&self) -> PreswapRequirement {
        PreswapRequirement::Transfer(self.pool_address)
    }

    fn swap_out_amount_offset(&self, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        if token_from_address < token_to_address {
            Some(0x24)
        } else {
            Some(0x04)
        }
    }

    fn swap_out_amount_return_offset(&self, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        if token_from_address < token_to_address {
            Some(0x20)
        } else {
            Some(0x00)
        }
    }
}

// The test are using the deployed contracts for comparison to allow to adjust the test easily
#[cfg(test)]
mod test {
    use super::*;
    use alloy_primitives::{address, BlockNumber};
    use alloy_rpc_types::BlockId;
    use loom_defi_entities::required_state::RequiredStateReader;
    use loom_node_debug_provider::{AnvilDebugProviderFactory, AnvilDebugProviderType};
    use loom_protocol_abi::uniswap2::IUniswapV2Router;
    use loom_protocol_address_book::PeripheryAddress;
    use rand::Rng;
    use std::env;

    const POOL_ADDRESSES: [Address; 4] = [
        address!("322BBA387c825180ebfB62bD8E6969EBe5b5e52d"), // ITO/WETH pool
        address!("b4e16d0168e52d35cacd2c6185b44281ec28c9dc"), // USDC/WETH pool
        address!("0d4a11d5eeaac28ec3f61d100daf4d40471f1852"), // WETH/USDT pool
        address!("ddd23787a6b80a794d952f5fb036d0b31a8e6aff"), // PEPE/WETH pool
    ];

    #[tokio::test]
    async fn test_fetch_reserves() -> Result<()> {
        let block_number = 20935488u64;

        let node_url = env::var("MAINNET_WS")?;
        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, BlockNumber::from(block_number)).await?;

        for pool_address in POOL_ADDRESSES {
            let pool_contract = IUniswapV2Pair::new(pool_address, client.clone());
            let contract_reserves = pool_contract.getReserves().call().block(BlockId::from(block_number)).await?;
            let reserves_0_original = U256::from(contract_reserves.reserve0);
            let reserves_1_original = U256::from(contract_reserves.reserve1);

            let pool = UniswapV2Pool::fetch_pool_data(client.clone(), pool_address).await?;
            let state_required = pool.get_state_required()?;
            let state_update = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, Some(block_number)).await?;

            let mut state_db = LoomDBType::default();
            state_db.apply_geth_update(state_update);

            // under test
            let (reserves_0, reserves_1) = pool.fetch_reserves(&state_db, Env::default())?;

            assert_eq!(reserves_0, reserves_0_original, "{}", format!("Missmatch for pool={:?}", pool_address));
            assert_eq!(reserves_1, reserves_1_original, "{}", format!("Missmatch for pool={:?}", pool_address));
        }
        Ok(())
    }

    async fn fetch_original_contract_amounts(
        client: AnvilDebugProviderType,
        pool_address: Address,
        amount: U256,
        block_number: u64,
        amount_out: bool,
    ) -> Result<U256> {
        let router_contract = IUniswapV2Router::new(PeripheryAddress::UNISWAP_V2_ROUTER, client.clone());

        // get reserves
        let pool_contract = IUniswapV2Pair::new(pool_address, client.clone());
        let contract_reserves = pool_contract.getReserves().call().block(BlockId::from(block_number)).await?;

        let token0 = pool_contract.token0().call().await?._0;
        let token1 = pool_contract.token1().call().await?._0;

        let (reserve_in, reserve_out) = match token0 < token1 {
            true => (U256::from(contract_reserves.reserve0), U256::from(contract_reserves.reserve1)),
            false => (U256::from(contract_reserves.reserve1), U256::from(contract_reserves.reserve0)),
        };

        if amount_out {
            let contract_amount_out =
                router_contract.getAmountOut(amount, reserve_in, reserve_out).call().block(BlockId::from(block_number)).await?;
            Ok(contract_amount_out.amountOut)
        } else {
            let contract_amount_in =
                router_contract.getAmountIn(amount, reserve_in, reserve_out).call().block(BlockId::from(block_number)).await?;
            Ok(contract_amount_in.amountIn)
        }
    }

    #[tokio::test]
    async fn test_calculate_out_amount() -> Result<()> {
        // Verify that the calculated out amount is the same as the contract's out amount
        let block_number = 20935488u64;

        let node_url = env::var("MAINNET_WS")?;
        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, BlockNumber::from(block_number)).await?;

        let amount_in = U256::from(133_333_333_333u128) + U256::from(rand::thread_rng().gen_range(0..100_000_000_000u64));
        for pool_address in POOL_ADDRESSES {
            let pool = UniswapV2Pool::fetch_pool_data(client.clone(), pool_address).await?;
            let state_required = pool.get_state_required()?;
            let state_update = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, Some(block_number)).await?;

            let mut state_db = LoomDBType::default();
            state_db.apply_geth_update(state_update);

            // fetch original
            let contract_amount_out = fetch_original_contract_amounts(client.clone(), pool_address, amount_in, block_number, true).await?;

            // under test
            let evm_env = Env::default();
            let (amount_out, gas_used) = pool.calculate_out_amount(&state_db, evm_env.clone(), &pool.token0, &pool.token1, amount_in)?;

            assert_eq!(amount_out, contract_amount_out, "{}", format!("Missmatch for pool={:?}, amount_in={}", pool_address, amount_in));
            assert_eq!(gas_used, 100_000);
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_in_amount() -> Result<()> {
        // Verify that the calculated in amount is the same as the contract's in amount
        let block_number = 20935488u64;

        let node_url = env::var("MAINNET_WS")?;
        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, BlockNumber::from(block_number)).await?;

        let amount_out = U256::from(133_333_333_333u128) + U256::from(rand::thread_rng().gen_range(0..100_000_000_000u64));
        for pool_address in POOL_ADDRESSES {
            let pool = UniswapV2Pool::fetch_pool_data(client.clone(), pool_address).await?;
            let state_required = pool.get_state_required()?;
            let state_update = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, Some(block_number)).await?;

            let mut state_db = LoomDBType::default();
            state_db.apply_geth_update(state_update);

            // fetch original
            let contract_amount_in = fetch_original_contract_amounts(client.clone(), pool_address, amount_out, block_number, false).await?;

            // under test
            let (amount_in, gas_used) = pool.calculate_in_amount(&state_db, Env::default(), &pool.token0, &pool.token1, amount_out)?;

            assert_eq!(amount_in, contract_amount_in, "{}", format!("Missmatch for pool={:?}, amount_out={}", pool_address, amount_out));
            assert_eq!(gas_used, 100_000);
        }
        Ok(())
    }
}
