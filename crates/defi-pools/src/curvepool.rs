use std::collections::BTreeMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;

use alloy_primitives::{Address, Bytes, U256};
use alloy_provider::{Network, Provider};
use alloy_sol_types::SolCall;
use alloy_transport::{BoxTransport, Transport};
use async_trait::async_trait;
use eyre::{eyre, Report, Result};
use log::{debug, error};
use revm::db::DatabaseRef;
use revm::InMemoryDB;
use revm::primitives::{Bytes as rBytes, Env, ExecutionResult, Output, TransactTo, U256 as rU256};

use defi_abi::IERC20;
use defi_entities::{AbiSwapEncoder, Pool, PoolClass, PoolProtocol, PreswapRequirement};
use defi_entities::required_state::RequiredState;
use loom_utils::evm::evm_call;

use crate::protocols::{CurveCommonContract, CurveContract, CurveProtocol};

pub struct CurvePool<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    address: Address,
    pool_contract: Arc<CurveContract<P, T, N>>,
    balances: Vec<U256>,
    tokens: Vec<Address>,
    underlying_tokens: Vec<Address>,
    lp_token: Option<Address>,
    abi_encoder: Arc<CurveAbiSwapEncoder<P, T, N>>,
    is_meta: bool,
    is_native: bool,
}

impl<P, T, N> Clone for CurvePool<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    fn clone(&self) -> Self {
        Self {
            address: self.address,
            pool_contract: Arc::clone(&self.pool_contract),
            balances: self.balances.clone(),
            tokens: self.tokens.clone(),
            underlying_tokens: self.underlying_tokens.clone(),
            lp_token: self.lp_token.clone(),
            abi_encoder: Arc::clone(&self.abi_encoder),
            is_meta: self.is_meta,
            is_native: self.is_native,
        }
    }
}

impl<P, T, N> CurvePool<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    pub fn get_meta_coin_idx(&self, address: Address) -> Result<u32> {
        match self.get_coin_idx(address) {
            Ok(i) => {
                Ok(i)
            }
            Err(_) => {
                match self.get_underlying_coin_idx(address) {
                    Ok(i) => { Ok(self.tokens.len() as u32 + i - 1) }
                    Err(e) => { Err(e) }
                }
            }
        }
    }
    pub fn get_coin_idx(&self, address: Address) -> Result<u32> {
        for i in 0..self.tokens.len() {
            if address == self.tokens[i] {
                return Ok(i as u32);
            }
        }
        Err(eyre!("COIN_NOT_FOUND"))
    }
    pub fn get_underlying_coin_idx(&self, address: Address) -> Result<u32> {
        for i in 0..self.underlying_tokens.len() {
            if address == self.underlying_tokens[i] {
                return Ok(i as u32);
            }
        }
        Err(eyre!("COIN_NOT_FOUND"))
    }

    pub async fn fetch_out_amount(&self, token_address_from: Address, token_address_to: Address, amount_in: U256) -> Result<U256> {
        let i = self.get_coin_idx(token_address_from)?;
        let j = self.get_coin_idx(token_address_to)?;

        self.pool_contract.get_dy(i, j, amount_in).await
    }


    pub async fn fetch_pool_data(client: P, pool_contract: CurveContract<P, T, N>) -> Result<Self> {
        let pool_contract = Arc::new(pool_contract);


        let mut tokens = CurveCommonContract::coins(client.clone(), pool_contract.get_address()).await?;
        let mut is_native = false;

        for tkn in tokens.iter_mut() {
            if *tkn == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".parse::<Address>().unwrap() {
                //return Err(eyre!("ETH_CURVE_POOL_NOT_SUPPORTED"));
                *tkn = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
                is_native = true;
            }
        }

        let lp_token = match CurveCommonContract::lp_token(client.clone(), pool_contract.get_address()).await {
            Ok(lp_token_address) => {
                Some(lp_token_address)
            }
            Err(_) => None
        };


        let (underlying_tokens, is_meta) = match pool_contract.as_ref() {
            CurveContract::I128_2_To_Meta(interface) => {
                (CurveProtocol::<P, N, T>::get_underlying_tokens(tokens[1])?, true)
            }
            _ => {
                (vec![], false)
            }
        };


        let balances = CurveCommonContract::balances(client.clone(), pool_contract.get_address()).await?;

        let abi_encoder = Arc::new(CurveAbiSwapEncoder::new(pool_contract.get_address(),
                                                            tokens.clone(),
                                                            if underlying_tokens.len() > 0 { Some(underlying_tokens.clone()) } else { None },
                                                            lp_token,
                                                            is_meta,
                                                            is_native,
                                                            pool_contract.clone()));


        let abi_encoder = Arc::new(CurveAbiSwapEncoder::new(pool_contract.get_address(), Vec::new(), None, None, false, false, pool_contract.clone()));

        Ok(CurvePool {
            address: pool_contract.get_address(),
            abi_encoder,
            pool_contract,
            balances,
            tokens,
            underlying_tokens,
            lp_token,
            is_meta,
            is_native,
        })
    }
}


#[async_trait]
impl<P, T, N> Pool for CurvePool<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    fn get_class(&self) -> PoolClass {
        PoolClass::Curve
    }

    fn get_protocol(&self) -> PoolProtocol {
        PoolProtocol::Curve
    }

    fn get_address(&self) -> Address {
        self.address
    }

    fn get_fee(&self) -> U256 {
        U256::ZERO
    }

    fn get_tokens(&self) -> Vec<Address> {
        self.tokens.clone()
    }

    fn get_swap_directions(&self) -> Vec<(Address, Address)> {
        let mut ret: Vec<(Address, Address)> = Vec::new();
        if self.is_meta {
            ret.push((self.tokens[0], self.tokens[1]));
            ret.push((self.tokens[1], self.tokens[0]));
            for j in 0..self.underlying_tokens.len() {
                ret.push((self.tokens[0], self.underlying_tokens[j]));
                ret.push((self.underlying_tokens[j], self.tokens[0]));
            }
        } else {
            for i in 0..self.tokens.len() {
                for j in 0..self.tokens.len() {
                    if i == j {
                        continue;
                    }
                    ret.push((self.tokens[i], self.tokens[j]));
                }
                if let Some(lp_token_address) = self.lp_token {
                    ret.push((self.tokens[i], lp_token_address));
                    ret.push((lp_token_address, self.tokens[i]));
                }
            }
        }
        ret
    }

    fn calculate_out_amount(&self, state_db: &InMemoryDB, env: Env, token_address_from: &Address, token_address_to: &Address, in_amount: U256) -> Result<(U256, u64)> {
        let mut env = env;
        env.tx.gas_limit = 500_000;


        let call_data = if self.is_meta {
            let i: Result<u32> = self.get_coin_idx(*token_address_from);
            let j: Result<u32> = self.get_coin_idx(*token_address_to);
            if i.is_ok() && j.is_ok() {
                self.pool_contract.get_dy_call_data(i.unwrap(), j.unwrap(), in_amount)?
            } else {
                let i: u32 = self.get_meta_coin_idx(*token_address_from)?;
                let j: u32 = self.get_meta_coin_idx(*token_address_to)?;
                self.pool_contract.get_dy_underlying_call_data(i, j, in_amount)?
            }
        } else {
            if let Some(lp_token) = self.lp_token {
                if *token_address_from == lp_token {
                    let i: u32 = self.get_coin_idx(*token_address_to)?;
                    self.pool_contract.calc_withdraw_one_coin_call_data(i, in_amount)?
                } else if *token_address_to == lp_token {
                    let i: u32 = self.get_coin_idx(*token_address_from)?;
                    self.pool_contract.calc_token_amount_call_data(i, in_amount)?
                } else {
                    let i: u32 = self.get_coin_idx(*token_address_from)?;
                    let j: u32 = self.get_coin_idx(*token_address_to)?;
                    self.pool_contract.get_dy_call_data(i, j, in_amount)?
                }
            } else {
                let i: u32 = self.get_coin_idx(*token_address_from)?;
                let j: u32 = self.get_coin_idx(*token_address_to)?;
                self.pool_contract.get_dy_call_data(i, j, in_amount)?
            }
        };


        let (value, gas_used) = evm_call(state_db, env, self.get_address(), call_data.to_vec())?;


        let ret = if value.len() > 32 {
            U256::from_be_slice(&value[0..32])
        } else {
            U256::from_be_slice(&value[0..])
        };

        if ret.is_zero() {
            Err(eyre!("ZERO_OUT_AMOUNT"))
        } else {
            Ok((ret - U256::from(1), gas_used))
        }
    }

    fn calculate_in_amount(&self, state_db: &InMemoryDB, env: Env, token_address_from: &Address, token_address_to: &Address, out_amount: U256) -> Result<(U256, u64)> {
        if self.pool_contract.can_calculate_in_amount() {
            let mut env = env;
            env.tx.gas_limit = 500_000;


            let i: u32 = self.get_coin_idx(*token_address_from)?;
            let j: u32 = self.get_coin_idx(*token_address_to)?;
            let call_data = self.pool_contract.get_dx_call_data(i, j, out_amount)?;

            let (value, gas_used) = evm_call(state_db, env, self.get_address(), call_data.to_vec())?;


            let ret = if value.len() > 32 {
                U256::from_be_slice(&value[0..32])
            } else {
                U256::from_be_slice(&value[0..])
            };

            if ret.is_zero() {
                Err(eyre!("ZERO_IN_AMOUNT"))
            } else {
                Ok((ret + U256::from(1), gas_used))
            }
        } else {
            Err(eyre!("NOT_SUPPORTED"))
        }
    }

    fn can_flash_swap(&self) -> bool {
        false
    }

    fn can_calculate_in_amount(&self) -> bool {
        self.pool_contract.can_calculate_in_amount()
    }

    fn get_encoder(&self) -> &dyn AbiSwapEncoder {
        self.abi_encoder.as_ref()
    }

    fn get_state_required(&self) -> Result<RequiredState> {
        let mut state_reader = RequiredState::new();

        if self.is_meta {
            match &self.pool_contract.as_ref() {
                CurveContract::I128_2_To_Meta(interface) => {
                    for j in 0..self.underlying_tokens.len() {
                        let value = self.balances[0] / U256::from(10);
                        match self.pool_contract.get_dy_call_data((0 as u32).into(), ((j + self.tokens.len()) as u32).into(), value) {
                            Ok(data) => {
                                state_reader.add_call(self.get_address(), data);
                            }
                            Err(e) => { error!("{}", e); }
                        }
                    }
                }
                _ => { error!("CURVE_META_POOL_NOT_SUPPORTED") }
            }
        } else {
            if let Some(lp_token) = self.lp_token {
                for i in 0..self.tokens.len() {
                    let value = self.balances[i] / U256::from(10);
                    match self.pool_contract.get_add_liquidity_call_data((i as u32).into(), value, Address::ZERO) {
                        Ok(data) => { state_reader.add_call(self.get_address(), data); }
                        Err(e) => { error!("{}", e); }
                    }
                }
            }

            for i in 0..self.tokens.len() {
                for j in 0..self.tokens.len() {
                    if i == j {
                        continue;
                    }
                    let value = self.balances[i] / U256::from(100);
                    match self.pool_contract.get_dy_call_data((i as u32).into(), (j as u32).into(), value) {
                        Ok(data) => { state_reader.add_call(self.get_address(), data); }
                        Err(e) => { error!("{}", e); }
                    }
                }
            }
        }
        state_reader.add_slot_range(self.get_address(), U256::from(0), 0x20);

        for token_address in self.get_tokens() {
            state_reader.add_call(token_address, IERC20::balanceOfCall { account: self.get_address() }.abi_encode());
        }
        Ok(state_reader)
    }
}


#[derive(Clone)]
struct CurveAbiSwapEncoder<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    pool_address: Address,
    tokens: Vec<Address>,
    underlying_tokens: Option<Vec<Address>>,
    lp_token: Option<Address>,
    is_meta: bool,
    is_native: bool,
    curve_contract: Arc<CurveContract<P, T, N>>,
}

impl<P, T, N> CurveAbiSwapEncoder<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    pub fn new(pool_address: Address, tokens: Vec<Address>, underlying_tokens: Option<Vec<Address>>, lp_token: Option<Address>, is_meta: bool, is_native: bool, curve_contract: Arc<CurveContract<P, T, N>>) -> Self {
        Self {
            pool_address,
            tokens,
            underlying_tokens,
            lp_token,
            is_meta,
            is_native,
            curve_contract,
        }
    }

    pub fn get_meta_coin_idx(&self, address: Address) -> Result<u32> {
        match self.get_coin_idx(address) {
            Ok(idx) => Ok(idx),
            _ => {
                match self.get_underlying_coin_idx(address) {
                    Ok(idx) => Ok(idx + self.tokens.len() as u32 - 1),
                    Err(_) => Err(eyre!("TOKEN_NOT_FOUND"))
                }
            }
        }
    }

    pub fn get_coin_idx(&self, address: Address) -> Result<u32> {
        for i in 0..self.tokens.len() {
            if address == self.tokens[i] {
                return Ok(i as u32);
            }
        }
        Err(eyre!("COIN_NOT_FOUND"))
    }

    pub fn get_underlying_coin_idx(&self, address: Address) -> Result<u32> {
        match &self.underlying_tokens {
            Some(underlying_tokens) => {
                for i in 0..underlying_tokens.len() {
                    if address == underlying_tokens[i] {
                        return Ok(i as u32);
                    }
                }
                Err(eyre!("UNDERLYING_COIN_NOT_FOUND"))
            }
            _ => Err(eyre!("UNDERLYING_COIN_NOT_SET"))
        }
    }
}

impl<P, T, N> AbiSwapEncoder for CurveAbiSwapEncoder<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    fn encode_swap_out_amount_provided(&self, token_from_address: Address, token_to_address: Address, amount: U256, recipient: Address, payload: Bytes) -> Result<Bytes> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }

    fn encode_swap_in_amount_provided(&self, token_from_address: Address, token_to_address: Address, amount: U256, recipient: Address, payload: Bytes) -> Result<Bytes> {
        if self.is_meta {
            let i: Result<u32> = self.get_coin_idx(token_from_address);
            let j: Result<u32> = self.get_coin_idx(token_to_address);

            if i.is_ok() && j.is_ok() {
                self.curve_contract.get_exchange_call_data(i.unwrap(), j.unwrap(), amount, U256::ZERO, recipient)
            } else {
                let i: u32 = self.get_meta_coin_idx(token_from_address)?;
                let j: u32 = self.get_meta_coin_idx(token_to_address)?;
                self.curve_contract.get_exchange_underlying_call_data(i, j, amount, U256::ZERO, recipient)
            }
        } else {
            if let Some(lp_token) = self.lp_token {
                if token_from_address == lp_token {
                    let i: u32 = self.get_coin_idx(token_to_address)?;
                    self.curve_contract.get_remove_liquidity_one_coin_call_data(i, amount, recipient)
                } else if token_to_address == lp_token {
                    let i: u32 = self.get_coin_idx(token_from_address)?;
                    self.curve_contract.get_add_liquidity_call_data(i, amount, recipient)
                } else {
                    let i: u32 = self.get_coin_idx(token_from_address)?;
                    let j: u32 = self.get_coin_idx(token_to_address)?;
                    self.curve_contract.get_exchange_call_data(i, j, amount, U256::ZERO, recipient)
                }
            } else {
                let i: u32 = self.get_coin_idx(token_from_address)?;
                let j: u32 = self.get_coin_idx(token_to_address)?;
                self.curve_contract.get_exchange_call_data(i, j, amount, U256::ZERO, recipient)
            }
        }
    }
    fn preswap_requirement(&self) -> PreswapRequirement {
        PreswapRequirement::Allowance
    }

    fn swap_in_amount_offset(&self, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        Some(0x44)
    }
    fn swap_out_amount_offset(&self, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        None
    }

    fn swap_out_amount_return_offset(&self, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        None
    }
    fn swap_in_amount_return_offset(&self, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        None
    }
    fn swap_out_amount_return_script(&self, token_from_address: Address, token_to_address: Address) -> Option<Bytes> {
        None
    }
    fn swap_in_amount_return_script(&self, token_from_address: Address, token_to_address: Address) -> Option<Bytes> {
        None
    }

    fn is_native(&self) -> bool {
        self.is_native
    }
}


#[cfg(test)]
mod tests {
    use alloy_primitives::U256;
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_provider::network::Ethereum;
    use alloy_rpc_client::{ClientBuilder, WsConnect};
    use alloy_rpc_types::BlockNumberOrTag;
    use alloy_transport::BoxTransport;
    use env_logger::Env as EnvLog;
    use log::info;
    use revm::db::EmptyDB;
    use revm::InMemoryDB;
    use revm::primitives::Env;

    use debug_provider::{AnvilControl, AnvilDebugProviderType};
    use defi_entities::{MarketState, Pool};
    use defi_entities::required_state::RequiredStateReader;

    use crate::CurvePool;
    use crate::protocols::CurveProtocol;

    #[tokio::test]
    async fn test_pool() {
        std::env::set_var("RUST_LOG", "debug");
        std::env::set_var("RUST_BACKTRACE", "1");
        env_logger::init_from_env(EnvLog::default().default_filter_or("debug"));

        let node_url = std::env::var("TEST_NODE_URL").unwrap_or("ws://falcon.loop:8008/looper".to_string());
        let ws_connect = WsConnect::new(node_url);
        let client = ClientBuilder::default().ws(ws_connect).await.unwrap();

        let client = ProviderBuilder::new().on_client(client).boxed();


        //let provider = AnvilControl::from_node_on_block("ws://falcon.loop:8008/looper".to_string(), 19109956).await.unwrap();

        //let client = Arc::new(provider);
        //let client = provider;

        let mut market_state = MarketState::new(InMemoryDB::new(EmptyDB::default()));

        //let pool_address : Address = "0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7".parse().unwrap(); //

        //let curve_contract = CurveProtocol::new_I128_3(client.clone(), pool_address);

        let curve_contracts = CurveProtocol::get_contracts_vec(client.clone());

        for curve_contract in curve_contracts.into_iter() {
            info!("Loading Pool : {} {:?}", curve_contract.get_address(), curve_contract);
            let pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await.unwrap();
            let state_required = pool.get_state_required().unwrap();

            let state_required = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, None).await.unwrap();
            info!("Pool state fetched {} {}", pool.address, state_required.len());

            market_state.add_state(&state_required);
            info!("Pool : {} Accs : {} Storage : {}", pool.address, market_state.accounts_len(), market_state.storage_len());

            let mut evm_env = Env::default();

            let block_header = client.get_block_by_number(BlockNumberOrTag::Latest, false).await.unwrap().unwrap().header;
            info!("Block {} {}", block_header.number.unwrap(), block_header.timestamp);

            let mut evm_env = revm::primitives::Env::default();

            //evm_env.block.number = U256::from(block_header.number.unwrap().as_u64() + 1).into();
            evm_env.block.number = U256::from(block_header.number.unwrap_or_default() + 0);

            let timestamp = block_header.timestamp;

            //evm_env.block.timestamp = (block_header.timestamp + U256::from(12)).into();
            evm_env.block.timestamp = U256::from(timestamp);

            let mut pools_vec: Vec<CurvePool<AnvilDebugProviderType, BoxTransport, Ethereum>> = Vec::new();


            let tokens = pool.tokens.clone();
            let balances = pool.balances.clone();
            for i in 0..tokens.len() {
                for j in 0..tokens.len() {
                    if i == j {
                        continue;
                    }
                    let in_amount = balances[i] / U256::from(100);
                    //let in_amount = U256::from(10).pow(U256::from(17));
                    let token_in = tokens[i];
                    let token_out = tokens[j];
                    let (out_amount, gas_used) = pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &token_in, &token_out, in_amount).unwrap_or_default();
                    info!("{:?} {} -> {} : {} -> {} gas : {}", pool.get_address(), tokens[i], tokens[j], in_amount, out_amount, gas_used);

                    let out_amount_fetched = pool.fetch_out_amount(token_in, token_out, in_amount).await.unwrap();
                    info!("Fetched {:?} {} -> {} : {} -> {}", pool.get_address(), tokens[i], tokens[j], in_amount, out_amount_fetched);
                }
            }

            if let Some(lp_token) = pool.lp_token {
                for i in 0..tokens.len() {
                    let in_amount = balances[i] / U256::from(1000);
                    let token_in = tokens[i];
                    let (out_amount, gas_used) = pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &token_in, &lp_token, in_amount).unwrap_or_default();
                    info!("LP {:?} {} -> {} : {} -> {} gas : {}", pool.get_address(), token_in, lp_token, in_amount, out_amount, gas_used);
                    let (out_amount2, gas_used) = pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &lp_token, &token_in, out_amount).unwrap_or_default();
                    info!("LP {:?} {} -> {} : {} -> {} gas : {}", pool.get_address(), lp_token, token_in, out_amount, out_amount2, gas_used);
                }
            }

            if pool.is_meta {
                let underlying_tokens = pool.underlying_tokens.clone();
                for j in 0..underlying_tokens.len() {
                    let in_amount = balances[0] / U256::from(1000);
                    let token_in = tokens[0];
                    let token_out = underlying_tokens[j];
                    let (out_amount, gas_used) = pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &token_in, &token_out, in_amount).unwrap_or_default();
                    info!("Meta {:?} {} -> {} : {} -> {} gas: {}", pool.get_address(), token_in, token_out, in_amount, out_amount, gas_used);
                    let (out_amount2, gas_used) = pool.calculate_out_amount(&market_state.state_db, evm_env.clone(), &token_out, &token_in, out_amount).unwrap_or_default();
                    info!("Meta {:?} {} -> {} : {} -> {} gas : {} ", pool.get_address(), token_out, token_in, out_amount, out_amount2, gas_used);
                }
            }
        }
    }
}
