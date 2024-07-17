use alloy_primitives::{Address, Bytes, U256};
use alloy_provider::{Network, Provider};
use alloy_rpc_types::BlockNumberOrTag;
use alloy_sol_types::SolInterface;
use alloy_transport::Transport;
use eyre::{eyre, ErrReport, Result};
use lazy_static::lazy_static;
use log::debug;
use revm::primitives::Env;
use revm::DatabaseRef;

use defi_abi::uniswap2::IUniswapV2Pair;
use defi_abi::IERC20;
use defi_entities::required_state::RequiredState;
use defi_entities::{AbiSwapEncoder, Pool, PoolClass, PoolProtocol, PreswapRequirement};
use loom_revm_db::LoomInMemoryDB;

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

    pub fn set_fee(self, fee: U256) -> Self {
        Self { fee, ..self }
    }

    pub fn get_zero_for_one(token_address_from: Address, token_address_to: Address) -> bool {
        token_address_from < token_address_to
    }

    fn get_protocol_by_factory(factory_address: Address) -> PoolProtocol {
        let uni2_factory: Address = "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse().unwrap();
        let nomiswap_stable_factory: Address = "0x818339b4E536E707f14980219037c5046b049dD4".parse().unwrap();
        let sushiswap_factory: Address = "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".parse().unwrap();
        let dooarswap_factory: Address = "0x1e895bFe59E3A5103e8B7dA3897d1F2391476f3c".parse().unwrap();
        let safeswap_factory: Address = "0x7F09d4bE6bbF4b0fF0C97ca5c486a166198aEAeE".parse().unwrap();
        let miniswap_factory: Address = "0x2294577031F113DF4782B881cF0b140e94209a6F".parse().unwrap();
        let shibaswap_factory: Address = "0x115934131916C8b277DD010Ee02de363c09d037c".parse().unwrap();

        if factory_address == uni2_factory {
            PoolProtocol::UniswapV2
        } else if factory_address == sushiswap_factory {
            PoolProtocol::Sushiswap
        } else if factory_address == nomiswap_stable_factory {
            PoolProtocol::NomiswapStable
        } else if factory_address == dooarswap_factory {
            PoolProtocol::DooarSwap
        } else if factory_address == safeswap_factory {
            PoolProtocol::Safeswap
        } else if factory_address == miniswap_factory {
            PoolProtocol::Miniswap
        } else if factory_address == shibaswap_factory {
            PoolProtocol::Shibaswap
        } else {
            PoolProtocol::UniswapV2Like
        }
    }

    fn storage_to_reserves(value: U256) -> (U256, U256) {
        //let uvalue : U256 = value.convert();
        ((value >> 0) & *U112_MASK, (value >> (112)) & *U112_MASK)
    }

    pub fn fetch_pool_data_evm(db: &LoomInMemoryDB, env: Env, address: Address) -> Result<Self> {
        let token0 = UniswapV2StateReader::token0(db, env.clone(), address)?;
        let token1 = UniswapV2StateReader::token1(db, env.clone(), address)?;
        let factory = UniswapV2StateReader::factory(db, env.clone(), address)?;
        let protocol = Self::get_protocol_by_factory(factory);

        let fee = if protocol == PoolProtocol::DooarSwap { U256::from(9900) } else { U256::from(9970) };

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

        //let mut h = [0u8;32] = U256::from(8).to_be_bytes();

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

        let fee = if protocol == PoolProtocol::DooarSwap { U256::from(9900) } else { U256::from(9970) };

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

    /*
    fn clone_box(&self) -> Box<dyn Pool> {
        Box::new(self.clone())
    }

     */

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
        state_db: &LoomInMemoryDB,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        in_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        let (reserve_in, reserve_out) = match self.reserves_cell {
            Some(cell) => match state_db.storage_ref(self.get_address(), cell) {
                Ok(c) => {
                    let reserves = Self::storage_to_reserves(c);
                    if token_address_from < token_address_to {
                        (reserves.0, reserves.1)
                    } else {
                        (reserves.1, reserves.0)
                    }
                }
                Err(_) => {
                    return Err(eyre!("FAILED_GETTING_STORAGE_CELL"));
                }
            },
            None => {
                let (reserve_0, reserve_1) = UniswapV2StateReader::get_reserves(state_db, env, self.get_address())?;

                if token_address_from < token_address_to {
                    (reserve_0, reserve_1)
                } else {
                    (reserve_1, reserve_0)
                }
            }
        };

        let amount_in_with_fee = in_amount * self.fee;
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(10000) + amount_in_with_fee;
        if denominator.is_zero() {
            Err(eyre!("CANNOT_CALCULATE_ZERO_RESERVE"))
        } else {
            let out_amount = numerator / denominator;
            if out_amount > reserve_out {
                Err(eyre!("RESERVE_EXCEEDED"))
            } else if out_amount.is_zero() {
                Err(eyre!("OUT_AMOUNT_IS_ZERO"))
            } else {
                Ok((out_amount - U256::from(1), 100000))
            }
        }
    }

    fn calculate_in_amount(
        &self,
        state_db: &LoomInMemoryDB,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        out_amount: U256,
    ) -> Result<(U256, u64), ErrReport> {
        let (reserve_in, reserve_out) = match self.reserves_cell {
            Some(cell) => match state_db.storage_ref(self.get_address(), cell) {
                Ok(c) => {
                    let reserves = Self::storage_to_reserves(c);
                    if token_address_from < token_address_to {
                        (reserves.0, reserves.1)
                    } else {
                        (reserves.1, reserves.0)
                    }
                }
                Err(_) => {
                    return Err(eyre!("FAILED_GETTING_STORAGE_CELL"));
                }
            },
            None => {
                let (reserve_0, reserve_1) = UniswapV2StateReader::get_reserves(state_db, env, self.get_address())?;

                if token_address_from < token_address_to {
                    (reserve_0, reserve_1)
                } else {
                    (reserve_1, reserve_0)
                }
            }
        };

        if out_amount > reserve_out {
            return Err(eyre!("RESERVE_OUT_EXCEEDED"));
        }

        let numerator = reserve_in * U256::from(10000) * (out_amount + U256::from(10));
        let denominator = (reserve_out - (out_amount + U256::from(10))) * self.fee;

        if denominator.is_zero() {
            Err(eyre!("CANNOT_CALCULATE_ZERO_RESERVE"))
        } else {
            let in_amount = numerator / denominator;
            if in_amount.is_zero() {
                Err(eyre!("IN_AMOUNT_IS_ZERO"))
            } else {
                Ok((in_amount + U256::from(1), 100000))
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

#[cfg(test)]
mod tests {
    use debug_provider::AnvilDebugProviderFactory;
    use defi_entities::required_state::RequiredStateReader;
    use defi_entities::MarketState;
    use std::env;

    use crate::protocols::UniswapV2Protocol;

    use super::*;

    #[tokio::test]
    async fn test_pool() -> Result<()> {
        let _ = env_logger::try_init_from_env(env_logger::Env::default().default_filter_or("info,defi_pools=off"));

        let node_url = env::var("MAINNET_WS")?;

        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, 20045799).await?;

        let mut market_state = MarketState::new(LoomInMemoryDB::default());

        let weth_address: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        let usdc_address: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
        let pool_address: Address = UniswapV2Protocol::get_pool_address_for_tokens(weth_address, usdc_address);

        let pool = UniswapV2Pool::fetch_pool_data(client.clone(), pool_address).await?;

        let state_required = pool.get_state_required()?;

        let state_required = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, None).await?;

        market_state.add_state(&state_required);

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
        debug!("out {} -> {} gas : {}", U256::from(pool.liquidity0 / U256::from(100)), out_amount, gas_used);
        assert_ne!(out_amount, U256::ZERO);
        assert!(gas_used > 50000);

        let (in_amount, gas_used) =
            pool.calculate_in_amount(&market_state.state_db, evm_env.clone(), &pool.token0, &pool.token1, out_amount).unwrap();
        debug!("in {} -> {} gas : {}", out_amount, in_amount, gas_used);
        assert_ne!(in_amount, U256::ZERO);
        assert!(gas_used > 50000);

        let a = U256::from(161429016704477229850u128);

        let (out_amount, gas_used) =
            pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &pool.token1, &pool.token0, a).unwrap();
        debug!("out {} -> {}", a, out_amount);
        assert_ne!(out_amount, U256::ZERO);
        assert!(gas_used > 50000);

        let (in_amount, gas_used) =
            pool.calculate_in_amount(&market_state.state_db, evm_env.clone(), &pool.token1, &pool.token0, out_amount).unwrap();
        debug!("in {} -> {} {}", out_amount, in_amount, in_amount >= a);
        assert_ne!(in_amount, U256::ZERO);
        assert!(gas_used > 50000);

        let (out_amount, gas_used) = pool
            .calculate_out_amount(
                &market_state.state_db,
                evm_env.clone(),
                &pool.token0,
                &pool.token1,
                U256::from(pool.liquidity0 / U256::from(100)),
            )
            .unwrap();
        debug!("{}", out_amount);
        assert_ne!(out_amount, U256::ZERO);
        assert!(gas_used > 50000);

        let (out_amount, gas_used) = pool
            .calculate_out_amount(
                &market_state.state_db,
                evm_env.clone(),
                &pool.token1,
                &pool.token0,
                U256::from(pool.liquidity1 / U256::from(100)),
            )
            .unwrap();
        debug!("{}", out_amount);
        assert_ne!(out_amount, U256::ZERO);
        assert!(gas_used > 50000);
        Ok(())
    }
}
