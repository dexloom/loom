use std::any::Any;
use std::sync::Arc;

use alloy::primitives::{address, Address, Bytes, U256};
use alloy::providers::{Network, Provider};
use alloy::sol_types::SolCall;
use eyre::{eyre, ErrReport, OptionExt, Result};
use lazy_static::lazy_static;
use loom_defi_abi::IERC20;
use loom_defi_address_book::TokenAddressEth;
use loom_evm_utils::evm::evm_call;
use loom_types_entities::required_state::RequiredState;
use loom_types_entities::{Pool, PoolAbiEncoder, PoolClass, PoolId, PoolProtocol, PreswapRequirement, SwapDirection};
use revm::primitives::Env;
use revm::DatabaseRef;
use tracing::error;

use crate::protocols::{CurveCommonContract, CurveContract, CurveProtocol};

lazy_static! {
    static ref U256_ONE: U256 = U256::from(1);
}

pub struct CurvePool<P, N, E = CurvePoolAbiEncoder<P, N>>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    E: PoolAbiEncoder + Send + Sync + 'static,
{
    address: Address,
    pool_contract: Arc<CurveContract<P, N>>,
    balances: Vec<U256>,
    tokens: Vec<Address>,
    underlying_tokens: Vec<Address>,
    lp_token: Option<Address>,
    abi_encoder: Option<Arc<E>>,
    is_meta: bool,
    is_native: bool,
}

impl<P, N, E> Clone for CurvePool<P, N, E>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    E: PoolAbiEncoder + Send + Sync + Clone + 'static,
{
    fn clone(&self) -> Self {
        Self {
            address: self.address,
            pool_contract: Arc::clone(&self.pool_contract),
            balances: self.balances.clone(),
            tokens: self.tokens.clone(),
            underlying_tokens: self.underlying_tokens.clone(),
            lp_token: self.lp_token,
            abi_encoder: self.abi_encoder.clone(),
            is_meta: self.is_meta,
            is_native: self.is_native,
        }
    }
}

impl<P, N, E> CurvePool<P, N, E>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    E: PoolAbiEncoder + Send + Sync + Clone + 'static,
{
    pub fn is_meta(&self) -> bool {
        self.is_meta
    }
    pub fn curve_contract(&self) -> Arc<CurveContract<P, N>> {
        self.pool_contract.clone()
    }

    pub fn lp_token(&self) -> Option<Address> {
        self.lp_token
    }

    pub fn with_encoder(self, e: E) -> Self {
        Self { abi_encoder: Some(Arc::new(e)), ..self }
    }

    pub fn get_meta_coin_idx(&self, address: Address) -> Result<u32> {
        match self.get_coin_idx(address) {
            Ok(i) => Ok(i),
            Err(_) => match self.get_underlying_coin_idx(address) {
                Ok(i) => Ok(self.tokens.len() as u32 + i - 1),
                Err(e) => Err(e),
            },
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

    pub async fn fetch_pool_data(client: P, pool_contract: CurveContract<P, N>) -> Result<Self> {
        let pool_contract = Arc::new(pool_contract);

        let mut tokens = CurveCommonContract::coins(client.clone(), pool_contract.get_address()).await?;
        let mut is_native = false;

        for tkn in tokens.iter_mut() {
            if *tkn == address!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE") {
                //return Err(eyre!("ETH_CURVE_POOL_NOT_SUPPORTED"));
                *tkn = TokenAddressEth::WETH;
                is_native = true;
            }
        }

        let lp_token = match CurveCommonContract::<P, N>::lp_token(pool_contract.get_address()).await {
            Ok(lp_token_address) => Some(lp_token_address),
            Err(_) => None,
        };

        let (underlying_tokens, is_meta) = match pool_contract.as_ref() {
            CurveContract::I128_2ToMeta(_interface) => (CurveProtocol::<P, N>::get_underlying_tokens(tokens[1])?, true),
            _ => (vec![], false),
        };

        let balances = CurveCommonContract::balances(client.clone(), pool_contract.get_address()).await?;

        // let abi_encoder = Arc::new(CurveAbiSwapEncoder::new(
        //     pool_contract.get_address(),
        //     tokens.clone(),
        //     if !underlying_tokens.is_empty() { Some(underlying_tokens.clone()) } else { None },
        //     lp_token,
        //     is_meta,
        //     is_native,
        //     pool_contract.clone(),
        // ));

        Ok(CurvePool {
            address: pool_contract.get_address(),
            abi_encoder: None,
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

impl<P, N> CurvePool<P, N, CurvePoolAbiEncoder<P, N>>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pub async fn fetch_pool_data_with_default_encoder(client: P, pool_contract: CurveContract<P, N>) -> Result<Self> {
        let pool_contract = Arc::new(pool_contract);

        let mut tokens = CurveCommonContract::coins(client.clone(), pool_contract.get_address()).await?;
        let mut is_native = false;

        for tkn in tokens.iter_mut() {
            if *tkn == address!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE") {
                //return Err(eyre!("ETH_CURVE_POOL_NOT_SUPPORTED"));
                *tkn = TokenAddressEth::WETH;
                is_native = true;
            }
        }

        let lp_token = match CurveCommonContract::<P, N>::lp_token(pool_contract.get_address()).await {
            Ok(lp_token_address) => Some(lp_token_address),
            Err(_) => None,
        };

        let (underlying_tokens, is_meta) = match pool_contract.as_ref() {
            CurveContract::I128_2ToMeta(_interface) => (CurveProtocol::<P, N>::get_underlying_tokens(tokens[1])?, true),
            _ => (vec![], false),
        };

        let balances = CurveCommonContract::balances(client.clone(), pool_contract.get_address()).await?;

        let mut pool = CurvePool {
            address: pool_contract.get_address(),
            abi_encoder: None,
            pool_contract,
            balances,
            tokens,
            underlying_tokens,
            lp_token,
            is_meta,
            is_native,
        };

        let abi_encoder = Arc::new(CurvePoolAbiEncoder::new(&pool));

        pool.abi_encoder = Some(abi_encoder);

        Ok(pool)
    }
}

impl<P, N, E> Pool for CurvePool<P, N, E>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    E: PoolAbiEncoder + Send + Sync + Clone + 'static,
{
    fn as_any<'a>(&self) -> &dyn Any {
        self
    }
    fn get_class(&self) -> PoolClass {
        PoolClass::Curve
    }

    fn get_protocol(&self) -> PoolProtocol {
        PoolProtocol::Curve
    }

    fn get_address(&self) -> Address {
        self.address
    }

    fn get_pool_id(&self) -> PoolId {
        PoolId::Address(self.address)
    }

    fn get_fee(&self) -> U256 {
        U256::ZERO
    }

    fn get_tokens(&self) -> Vec<Address> {
        self.tokens.clone()
    }

    fn get_swap_directions(&self) -> Vec<SwapDirection> {
        let mut ret: Vec<SwapDirection> = Vec::new();
        if self.is_meta {
            ret.push((self.tokens[0], self.tokens[1]).into());
            ret.push((self.tokens[1], self.tokens[0]).into());
            for j in 0..self.underlying_tokens.len() {
                ret.push((self.tokens[0], self.underlying_tokens[j]).into());
                ret.push((self.underlying_tokens[j], self.tokens[0]).into());
            }
        } else {
            for i in 0..self.tokens.len() {
                for j in 0..self.tokens.len() {
                    if i == j {
                        continue;
                    }
                    ret.push((self.tokens[i], self.tokens[j]).into());
                }
                if let Some(lp_token_address) = self.lp_token {
                    ret.push((self.tokens[i], lp_token_address).into());
                    ret.push((lp_token_address, self.tokens[i]).into());
                }
            }
        }
        ret
    }

    fn calculate_out_amount(
        &self,
        state_db: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        in_amount: U256,
    ) -> Result<(U256, u64)> {
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
        } else if let Some(lp_token) = self.lp_token {
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
        };

        let (value, gas_used) = evm_call(state_db, env, self.get_address(), call_data.to_vec())?;

        let ret = if value.len() > 32 { U256::from_be_slice(&value[0..32]) } else { U256::from_be_slice(&value[0..]) };

        if ret.is_zero() {
            Err(eyre!("ZERO_OUT_AMOUNT"))
        } else {
            Ok((ret.checked_sub(*U256_ONE).ok_or_eyre("SUB_OVERFLOWN")?, gas_used))
        }
    }

    fn calculate_in_amount(
        &self,
        state_db: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        out_amount: U256,
    ) -> Result<(U256, u64)> {
        if self.pool_contract.can_calculate_in_amount() {
            let mut env = env;
            env.tx.gas_limit = 500_000;

            let i: u32 = self.get_coin_idx(*token_address_from)?;
            let j: u32 = self.get_coin_idx(*token_address_to)?;
            let call_data = self.pool_contract.get_dx_call_data(i, j, out_amount)?;

            let (value, gas_used) = evm_call(state_db, env, self.get_address(), call_data.to_vec())?;

            let ret = if value.len() > 32 { U256::from_be_slice(&value[0..32]) } else { U256::from_be_slice(&value[0..]) };

            if ret.is_zero() {
                Err(eyre!("ZERO_IN_AMOUNT"))
            } else {
                Ok((ret.checked_add(*U256_ONE).ok_or_eyre("ADD_OVERFLOWN")?, gas_used))
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

    fn get_abi_encoder(&self) -> Option<&dyn PoolAbiEncoder> {
        let r = self.abi_encoder.as_ref().unwrap().as_ref();
        Some(r as &dyn PoolAbiEncoder)
    }

    fn get_read_only_cell_vec(&self) -> Vec<U256> {
        Vec::new()
    }

    fn get_state_required(&self) -> Result<RequiredState> {
        let mut state_reader = RequiredState::new();

        if self.is_meta {
            match &self.pool_contract.as_ref() {
                CurveContract::I128_2ToMeta(_interface) => {
                    for j in 0..self.underlying_tokens.len() {
                        let value = self.balances[0] / U256::from(10);
                        match self.pool_contract.get_dy_call_data(0_u32, (j + self.tokens.len()) as u32, value) {
                            Ok(data) => {
                                state_reader.add_call(self.get_address(), data);
                            }
                            Err(e) => {
                                error!("{}", e);
                            }
                        }
                    }
                }
                _ => {
                    error!("CURVE_META_POOL_NOT_SUPPORTED")
                }
            }
        } else {
            if let Some(_lp_token) = self.lp_token {
                for i in 0..self.tokens.len() {
                    let value = self.balances[i] / U256::from(10);
                    match self.pool_contract.get_add_liquidity_call_data(i as u32, value, Address::ZERO) {
                        Ok(data) => {
                            state_reader.add_call(self.get_address(), data);
                        }
                        Err(e) => {
                            error!("{}", e);
                        }
                    }
                }
            }

            for i in 0..self.tokens.len() {
                for j in 0..self.tokens.len() {
                    if i == j {
                        continue;
                    }
                    if let Some(balance) = self.balances.get(i) {
                        let value = balance / U256::from(100);
                        match self.pool_contract.get_dy_call_data(i as u32, j as u32, value) {
                            Ok(data) => {
                                state_reader.add_call(self.get_address(), data);
                            }
                            Err(e) => {
                                error!("{}", e);
                            }
                        }
                    } else {
                        error!("Cannot get curve pool balance {} {}", self.address, i);
                        return Err(eyre!("CANNOT_GET_CURVE_BALANCE"));
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

    fn is_native(&self) -> bool {
        self.is_native
    }

    fn preswap_requirement(&self) -> PreswapRequirement {
        PreswapRequirement::Allowance
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct CurvePoolAbiEncoder<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pool_address: Address,
    tokens: Vec<Address>,
    underlying_tokens: Option<Vec<Address>>,
    lp_token: Option<Address>,
    is_meta: bool,
    is_native: bool,
    curve_contract: Arc<CurveContract<P, N>>,
}

impl<P, N> CurvePoolAbiEncoder<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pub fn new(pool: &CurvePool<P, N>) -> Self {
        Self {
            pool_address: pool.address,
            tokens: pool.tokens.clone(),
            underlying_tokens: if pool.underlying_tokens.is_empty() { None } else { Some(pool.underlying_tokens.clone()) },
            lp_token: pool.lp_token,
            is_meta: pool.is_meta,
            is_native: pool.is_native,
            curve_contract: pool.pool_contract.clone(),
        }
    }

    pub fn get_meta_coin_idx(&self, address: Address) -> Result<u32> {
        match self.get_coin_idx(address) {
            Ok(idx) => Ok(idx),
            _ => match self.get_underlying_coin_idx(address) {
                Ok(idx) => Ok(idx + self.tokens.len() as u32 - 1),
                Err(_) => Err(eyre!("TOKEN_NOT_FOUND")),
            },
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
                for (i, token_address) in underlying_tokens.iter().enumerate() {
                    if address == *token_address {
                        return Ok(i as u32);
                    }
                }
                Err(eyre!("UNDERLYING_COIN_NOT_FOUND"))
            }
            _ => Err(eyre!("UNDERLYING_COIN_NOT_SET")),
        }
    }
}

impl<P, N> PoolAbiEncoder for CurvePoolAbiEncoder<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    fn encode_swap_in_amount_provided(
        &self,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        _payload: Bytes,
    ) -> Result<Bytes> {
        if self.is_meta {
            let i: Result<u32> = self.get_coin_idx(token_from_address);
            let j: Result<u32> = self.get_coin_idx(token_to_address);

            match (i, j) {
                (Ok(i), Ok(j)) => self.curve_contract.get_exchange_call_data(i, j, amount, U256::ZERO, recipient),
                _ => {
                    let meta_i: u32 = self.get_meta_coin_idx(token_from_address)?;
                    let meta_j: u32 = self.get_meta_coin_idx(token_to_address)?;
                    self.curve_contract.get_exchange_underlying_call_data(meta_i, meta_j, amount, U256::ZERO, recipient)
                }
            }
        } else if let Some(lp_token) = self.lp_token {
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

    fn encode_swap_out_amount_provided(
        &self,
        _token_from_address: Address,
        _token_to_address: Address,
        _amount: U256,
        _recipient: Address,
        _payload: Bytes,
    ) -> Result<Bytes> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }

    fn swap_in_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x44)
    }

    fn swap_out_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }
    fn swap_out_amount_return_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }
    fn swap_in_amount_return_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }
    fn swap_out_amount_return_script(&self, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        None
    }

    fn swap_in_amount_return_script(&self, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        None
    }
}

#[cfg(test)]
mod tests {
    use eyre::Result;

    use alloy::primitives::U256;
    use alloy::providers::network::primitives::BlockTransactionsKind;
    use alloy::providers::Provider;
    use alloy::rpc::types::BlockNumberOrTag;
    use env_logger::Env as EnvLog;
    use loom_evm_db::{DatabaseLoomExt, LoomDBType};
    use loom_node_debug_provider::AnvilDebugProviderFactory;
    use loom_types_entities::required_state::RequiredStateReader;
    use loom_types_entities::{MarketState, Pool};
    use tracing::debug;

    use crate::protocols::CurveProtocol;
    use crate::CurvePool;

    #[tokio::test]
    async fn test_pool() -> Result<()> {
        let _ = env_logger::try_init_from_env(EnvLog::default().default_filter_or("info,alloy_rpc_client=off"));

        let node_url = std::env::var("MAINNET_WS")?;

        let client = AnvilDebugProviderFactory::from_node_on_block(node_url, 20045799).await?;

        let mut market_state = MarketState::new(LoomDBType::new());

        let curve_contracts = CurveProtocol::get_contracts_vec(client.clone());

        for curve_contract in curve_contracts.into_iter() {
            debug!("Loading Pool : {} {:?}", curve_contract.get_address(), curve_contract);
            let pool = CurvePool::fetch_pool_data_with_default_encoder(client.clone(), curve_contract).await.unwrap();
            let state_required = pool.get_state_required().unwrap();

            let state_required = RequiredStateReader::fetch_calls_and_slots(client.clone(), state_required, None).await.unwrap();
            debug!("Pool state fetched {} {}", pool.address, state_required.len());

            market_state.state_db.apply_geth_update(state_required);
            debug!(
                "Pool : {} Accs : {} Storage : {}",
                pool.address,
                market_state.state_db.accounts_len(),
                market_state.state_db.storage_len()
            );

            let block_header =
                client.get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes).await.unwrap().unwrap().header;
            debug!("Block {} {}", block_header.number, block_header.timestamp);

            let mut evm_env = revm::primitives::Env::default();

            evm_env.block.number = U256::from(block_header.number);

            let timestamp = block_header.timestamp;

            evm_env.block.timestamp = U256::from(timestamp);

            let tokens = pool.tokens.clone();
            let balances = pool.balances.clone();
            for i in 0..tokens.len() {
                for j in 0..tokens.len() {
                    if i == j {
                        continue;
                    }
                    let in_amount = balances[i] / U256::from(100);
                    let token_in = tokens[i];
                    let token_out = tokens[j];
                    let (out_amount, gas_used) = pool
                        .calculate_out_amount(&market_state.state_db, evm_env.clone(), &token_in, &token_out, in_amount)
                        .unwrap_or_default();
                    debug!(
                        "Calculated : {:?} {} -> {} : {} -> {} gas : {}",
                        pool.get_address(),
                        tokens[i],
                        tokens[j],
                        in_amount,
                        out_amount,
                        gas_used
                    );

                    let out_amount_fetched = pool.fetch_out_amount(token_in, token_out, in_amount).await.unwrap();
                    debug!("Fetched {:?} {} -> {} : {} -> {}", pool.get_address(), tokens[i], tokens[j], in_amount, out_amount_fetched);
                    assert_eq!(out_amount, out_amount_fetched - U256::from(1));
                }
            }

            if let Some(lp_token) = pool.lp_token {
                for i in 0..tokens.len() {
                    let in_amount = balances[i] / U256::from(1000);
                    let token_in = tokens[i];
                    let (out_amount, gas_used) = pool
                        .calculate_out_amount(&market_state.state_db, evm_env.clone(), &token_in, &lp_token, in_amount)
                        .unwrap_or_default();
                    debug!("LP {:?} {} -> {} : {} -> {} gas : {}", pool.get_address(), token_in, lp_token, in_amount, out_amount, gas_used);
                    assert!(gas_used > 50000);

                    let (out_amount2, gas_used) = pool
                        .calculate_out_amount(&market_state.state_db, evm_env.clone(), &lp_token, &token_in, out_amount)
                        .unwrap_or_default();
                    debug!(
                        "LP {:?} {} -> {} : {} -> {} gas : {}",
                        pool.get_address(),
                        lp_token,
                        token_in,
                        out_amount,
                        out_amount2,
                        gas_used
                    );
                    assert!(gas_used > 50000);
                    assert_ne!(out_amount, U256::ZERO);
                    assert_ne!(out_amount2, U256::ZERO);
                }
            }

            if pool.is_meta {
                let underlying_tokens = pool.underlying_tokens.clone();
                for underlying_token in underlying_tokens {
                    let in_amount = balances[0] / U256::from(1000);
                    let token_in = tokens[0];
                    let token_out = underlying_token;
                    let (out_amount, gas_used) = pool
                        .calculate_out_amount(&market_state.state_db, evm_env.clone(), &token_in, &token_out, in_amount)
                        .unwrap_or_default();
                    debug!(
                        "Meta {:?} {} -> {} : {} -> {} gas: {}",
                        pool.get_address(),
                        token_in,
                        token_out,
                        in_amount,
                        out_amount,
                        gas_used
                    );
                    let (out_amount2, gas_used) = pool
                        .calculate_out_amount(&market_state.state_db, evm_env.clone(), &token_out, &token_in, out_amount)
                        .unwrap_or_default();
                    debug!(
                        "Meta {:?} {} -> {} : {} -> {} gas : {} ",
                        pool.get_address(),
                        token_out,
                        token_in,
                        out_amount,
                        out_amount2,
                        gas_used
                    );
                    assert_ne!(out_amount, U256::ZERO);
                    assert_ne!(out_amount2, U256::ZERO);
                }
            }
        }
        Ok(())
    }
}
