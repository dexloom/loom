use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use alloy_primitives::{address, Address};
use eyre::{OptionExt, Result};
use log::debug;

use crate::build_swap_path_vec;
use crate::{EmptyPool, Pool, PoolClass, PoolWrapper, Token};
use crate::{SwapPath, SwapPaths};

/// The market struct contains all the pools and tokens.
/// It keeps track if a pool is disabled or not and the swap paths.
#[derive(Default, Clone)]
pub struct Market {
    // pool_address -> pool
    pools: HashMap<Address, PoolWrapper>,
    // pool_address -> is_disabled
    pools_disabled: HashMap<Address, bool>,
    // token_address -> token
    tokens: HashMap<Address, Arc<Token>>,
    // token_from -> token_to
    token_tokens: HashMap<Address, Vec<Address>>,
    // Shadow to token_tokens for fast lookup token -> token
    token_tokens_lookup: HashMap<Address, HashMap<Address, bool>>,
    // token_from -> token_to -> pool_addresses
    token_pools: HashMap<Address, HashMap<Address, Vec<Address>>>,
    // Shadow to token_pools for fast lookup token_from -> token_to -> pool_addresses
    token_pools_lookup: HashMap<Address, HashMap<Address, HashMap<Address, bool>>>,
    // swap_paths
    swap_paths: SwapPaths,
}

const WETH_ADDRESS: Address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

impl Market {
    /// Add a [`Token`](crate::Token) reference to the market.
    pub fn add_token<T: Into<Arc<Token>>>(&mut self, token: T) -> Result<()> {
        let arc_token: Arc<Token> = token.into();
        self.tokens.insert(arc_token.get_address(), arc_token);
        Ok(())
    }

    /// Check if the token is a basic token.
    pub fn is_basic_token(&self, address: &Address) -> bool {
        if let Some(token) = self.tokens.get(address) {
            token.is_basic()
        } else {
            false
        }
    }

    /// Check if the given address is the WETH address.
    pub fn is_weth(address: &Address) -> bool {
        address.eq(&WETH_ADDRESS)
    }

    /// Add a new empty pool to the market for the given pool address.
    pub fn add_empty_pool(&mut self, address: &Address) -> Result<()> {
        let pool_contract = EmptyPool::new(*address);
        self.pools.insert(pool_contract.get_address(), pool_contract.into());
        Ok(())
    }

    /// Add a new pool to the market.
    pub fn add_pool<T: Into<PoolWrapper>>(&mut self, pool: T) -> Result<()> {
        let pool_contract = pool.into();

        debug!("Adding pool {:?}", pool_contract.get_address());

        let pool_address = pool_contract.get_address();
        let swap_directions = pool_contract.get_swap_directions();

        self.add_token_token_paths(pool_address, swap_directions.clone());

        self.pools.insert(pool_address, pool_contract);

        Ok(())
    }

    /// Add a new token to token_tokens mapping if it does not exist. A lookup map is maintained for faster inserts.
    fn insert_token_tokens(&mut self, token_from: Address, token_to: Address) {
        if let Entry::Vacant(e) = self.token_tokens_lookup.entry(token_from).or_default().entry(token_to) {
            e.insert(true);
            self.token_tokens.entry(token_from).or_default().push(token_to);
        }
    }

    /// Add a new token to token_tokens mapping if it does not exist. A lookup map is maintained for faster inserts.
    fn insert_token_pools(&mut self, token_from: Address, token_to: Address, pool_address: Address) {
        if let Entry::Vacant(e) = self.token_pools_lookup.entry(token_from).or_default().entry(token_to).or_default().entry(pool_address) {
            e.insert(true);
            self.token_pools.entry(token_from).or_default().entry(token_to).or_default().push(pool_address);
        }
    }

    /// Add token->token, token->token->pool paths to the market.
    fn add_token_token_paths(&mut self, pool_address: Address, swap_directions: Vec<(Address, Address)>) {
        for (token_address_from, token_address_to) in swap_directions.iter() {
            self.insert_token_pools(*token_address_from, *token_address_to, pool_address);

            self.insert_token_tokens(*token_address_from, *token_address_to);
            self.insert_token_tokens(*token_address_to, *token_address_from);
        }
    }

    /// Add a swap path to the market.
    pub fn add_paths<T: Into<SwapPath> + Clone>(&mut self, paths: Vec<T>) {
        for path in paths.into_iter() {
            self.swap_paths.add(path);
        }
    }

    /// Get all swap paths from the market by the pool address.
    pub fn get_pool_paths(&self, pool_address: &Address) -> Option<Vec<SwapPath>> {
        self.swap_paths.get_pool_paths_vec(pool_address)
    }

    /// Get a pool reference by the pool address.
    pub fn get_pool(&self, address: &Address) -> Option<&PoolWrapper> {
        self.pools.get(address).filter(|&pool_wrapper| pool_wrapper.get_class() != PoolClass::Unknown)
    }

    /// Check if the pool exists in the market.
    pub fn is_pool(&self, address: &Address) -> bool {
        self.pools.contains_key(address)
    }

    /// Get a reference to the pools map in the market.
    pub fn pools(&self) -> &HashMap<Address, PoolWrapper> {
        &self.pools
    }

    /// Set the pool status to ok or not ok.
    pub fn set_pool_ok(&mut self, address: Address, ok: bool) {
        *self.pools_disabled.entry(address).or_insert(false) = ok
    }

    /// Check if the pool is ok.
    pub fn is_pool_ok(&self, address: &Address) -> bool {
        self.pools_disabled.get(address).cloned().unwrap_or(true)
    }

    /// Get a [`Token`](crate::Token) reference from the market by the address of the token or create a new one.
    pub fn get_token_or_default(&self, address: &Address) -> Arc<Token> {
        self.tokens.get(address).map_or(Arc::new(Token::new(*address)), |t| t.clone())
    }

    /// Get a [`Token`](crate::Token) reference from the market by the address of the token.
    pub fn get_token(&self, address: &Address) -> Option<Arc<Token>> {
        self.tokens.get(address).cloned()
    }

    /// Get all pool addresses that allow to swap from `token_from_address` to `token_to_address`.
    pub fn get_token_token_pools(&self, token_from_address: &Address, token_to_address: &Address) -> Option<Vec<Address>> {
        if let Some(token_from_map) = self.token_pools.get(token_from_address) {
            if let Some(pool_address_vec) = token_from_map.get(token_to_address) {
                return Some(pool_address_vec.clone());
            }
        }
        None
    }

    /// Get all pool addresses as reference that allow to swap from `token_from_address` to `token_to_address`.
    pub fn get_token_token_pools_ptr(&self, token_from_address: &Address, token_to_address: &Address) -> Option<&Vec<Address>> {
        if let Some(token_from_map) = self.token_pools.get(token_from_address) {
            if let Some(pool_address_vec) = token_from_map.get(token_to_address) {
                return Some(pool_address_vec);
            }
        }
        None
    }

    /// Get all token addresses that allow to swap from `token_from_address`.
    pub fn get_token_tokens(&self, token_from_address: &Address) -> Option<Vec<Address>> {
        if let Some(token_vec) = self.token_tokens.get(token_from_address) {
            return Some(token_vec.clone());
        }
        None
    }

    /// Get all token addresses as reference that allow to swap from `token_from_address`.
    pub fn get_token_tokens_ptr(&self, token_from_address: &Address) -> Option<&Vec<Address>> {
        if let Some(token_vec) = self.token_tokens.get(token_from_address) {
            return Some(token_vec);
        }
        None
    }

    /// Build a list of swap paths from the given directions.
    pub fn build_swap_path_vec(&self, directions: &BTreeMap<PoolWrapper, Vec<(Address, Address)>>) -> Result<Vec<SwapPath>> {
        build_swap_path_vec(self, directions)
    }

    /// get a [`SwapPath`](crate::SwapPath) from the given token and pool addresses.
    pub fn swap_path(&self, token_address_vec: Vec<Address>, pool_address_vec: Vec<Address>) -> Result<SwapPath> {
        let mut tokens: Vec<Arc<Token>> = Vec::new();
        let mut pools: Vec<PoolWrapper> = Vec::new();

        for token_address in token_address_vec.iter() {
            tokens.push(self.get_token(token_address).ok_or_eyre("TOKEN_NOT_FOUND")?);
        }
        for pool_address in pool_address_vec.iter() {
            pools.push(self.get_pool(pool_address).cloned().ok_or_eyre("TOKEN_NOT_FOUND")?);
        }

        Ok(SwapPath { tokens, pools })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::required_state::RequiredState;
    use crate::{AbiSwapEncoder, PoolProtocol};
    use alloy_primitives::{Address, U256};
    use eyre::ErrReport;
    use loom_revm_db::LoomInMemoryDB;
    use reth_revm::primitives::Env;

    #[derive(Clone)]
    struct MockPool {
        token0: Address,
        token1: Address,
        address: Address,
    }

    impl Pool for MockPool {
        fn get_protocol(&self) -> PoolProtocol {
            PoolProtocol::UniswapV2
        }

        fn get_address(&self) -> Address {
            self.address
        }

        fn get_tokens(&self) -> Vec<Address> {
            vec![self.token0, self.token1]
        }

        fn get_swap_directions(&self) -> Vec<(Address, Address)> {
            vec![(self.token0, self.token1), (self.token1, self.token0)]
        }

        fn calculate_out_amount(
            &self,
            state: &LoomInMemoryDB,
            env: Env,
            token_address_from: &Address,
            token_address_to: &Address,
            in_amount: U256,
        ) -> Result<(U256, u64), ErrReport> {
            panic!("Not implemented")
        }

        fn calculate_in_amount(
            &self,
            state: &LoomInMemoryDB,
            env: Env,
            token_address_from: &Address,
            token_address_to: &Address,
            out_amount: U256,
        ) -> Result<(U256, u64), ErrReport> {
            panic!("Not implemented")
        }

        fn can_flash_swap(&self) -> bool {
            panic!("Not implemented")
        }

        fn get_encoder(&self) -> &dyn AbiSwapEncoder {
            panic!("Not implemented")
        }

        fn get_state_required(&self) -> Result<RequiredState> {
            panic!("Not implemented")
        }
    }

    #[test]
    fn test_add_pool() {
        let mut market = Market::default();
        let pool_address = Address::random();
        let token0 = Address::random();
        let token1 = Address::random();
        let mock_pool = MockPool { address: pool_address, token0, token1 };

        let result = market.add_pool(mock_pool);

        assert!(result.is_ok());
        assert!(market.pools.contains_key(&pool_address));
        assert_eq!(market.pools.get(&pool_address).unwrap().get_address(), pool_address);

        assert!(market.token_pools.get(&token0).unwrap().get(&token1).unwrap().contains(&pool_address));
        assert!(market.token_pools.get(&token1).unwrap().get(&token0).unwrap().contains(&pool_address));

        assert!(market.token_pools_lookup.contains_key(&token0));
        assert!(market.token_pools_lookup.contains_key(&token1));

        assert!(market.token_tokens.get(&token0).unwrap().contains(&token1));
        assert!(market.token_tokens.get(&token1).unwrap().contains(&token0));

        assert!(market.token_tokens_lookup.get(&token0).unwrap().get(&token1).unwrap());
        assert!(market.token_tokens_lookup.get(&token1).unwrap().get(&token0).unwrap());
    }
}
