use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use alloy_primitives::{address, Address};
use eyre::{eyre, OptionExt, Result};
use log::debug;

use crate::build_swap_path_vec;
use crate::{PoolClass, PoolWrapper, Token};
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
    // token_from -> token_to -> pool_addresses
    token_token_pools: HashMap<Address, HashMap<Address, Vec<Address>>>,
    // token -> pool
    token_pools: HashMap<Address, Vec<Address>>,
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

    /// Add a new pool to the market if it does not exist or the class is unknown.
    pub fn add_pool<T: Into<PoolWrapper>>(&mut self, pool: T) -> Result<()> {
        let pool_contract = pool.into();
        let pool_address = pool_contract.get_address();

        if let Some(pool) = self.pools.get(&pool_address) {
            return Err(eyre!("Pool already exists {:?}", pool.get_address()));
        }

        debug!("Adding pool {:?}", pool_address);

        for (token_from_address, token_to_address) in pool_contract.get_swap_directions().into_iter() {
            self.token_token_pools.entry(token_from_address).or_default().entry(token_to_address).or_default().push(pool_address);
            self.token_tokens.entry(token_from_address).or_default().push(token_to_address);
            // Swap directions are bidirectional, for that reason we only need to add the token_from_address
            self.token_pools.entry(token_from_address).or_default().push(pool_address);
        }

        self.pools.insert(pool_address, pool_contract);

        Ok(())
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

    /// Get a pool reference by the pool address. If the pool exists but the class is unknown it returns None.
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
        if let Some(token_from_map) = self.token_token_pools.get(token_from_address) {
            if let Some(pool_address_vec) = token_from_map.get(token_to_address) {
                return Some(pool_address_vec.clone());
            }
        }
        None
    }

    /// Get all pool addresses as reference that allow to swap from `token_from_address` to `token_to_address`.
    pub fn get_token_token_pools_ptr(&self, token_from_address: &Address, token_to_address: &Address) -> Option<&Vec<Address>> {
        if let Some(token_from_map) = self.token_token_pools.get(token_from_address) {
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

    /// Get all pool addresses that allow to swap `token_address`.
    pub fn get_token_pools(&self, token_from_address: &Address) -> Option<Vec<Address>> {
        if let Some(token_vec) = self.token_pools.get(token_from_address) {
            return Some(token_vec.clone());
        }
        None
    }

    /// Get all pool addresses as reference that allow to swap `token_address`.
    pub fn get_token_pools_ptr(&self, token_address: &Address) -> Option<&Vec<Address>> {
        if let Some(token_vec) = self.token_pools.get(token_address) {
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
    use crate::{AbiSwapEncoder, Pool, PoolProtocol};
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
        fn get_class(&self) -> PoolClass {
            PoolClass::UniswapV2
        }

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

        assert_eq!(market.get_pool(&pool_address).unwrap().pool.get_address(), pool_address);

        assert_eq!(*market.get_token_token_pools(&token0, &token1).unwrap().get(0).unwrap(), pool_address);
        assert_eq!(*market.get_token_token_pools(&token1, &token0).unwrap().get(0).unwrap(), pool_address);

        assert!(market.get_token_tokens(&token0).unwrap().contains(&token1));
        assert!(market.get_token_tokens(&token1).unwrap().contains(&token0));

        assert!(market.get_token_pools(&token0).unwrap().contains(&pool_address));
        assert!(market.get_token_pools(&token1).unwrap().contains(&pool_address));
    }

    #[test]
    fn test_add_token() {
        let mut market = Market::default();
        let token_address = Address::random();

        let result = market.add_token(Arc::new(Token::new(token_address)));

        assert!(result.is_ok());
        assert_eq!(market.get_token(&token_address).unwrap().get_address(), token_address);
    }

    #[test]
    fn test_get_token_default() {
        let market = Market::default();
        let token_address = Address::random();

        let token = market.get_token_or_default(&token_address);

        assert_eq!(token.get_address(), token_address);
    }

    #[test]
    fn test_get_pool() {
        let mut market = Market::default();
        let pool_address = Address::random();
        let mock_pool = MockPool { address: pool_address, token0: Address::ZERO, token1: Address::ZERO };
        market.add_pool(mock_pool.clone());

        let pool = market.get_pool(&pool_address);

        assert_eq!(pool.unwrap().get_address(), pool_address);
    }

    #[test]
    fn test_is_pool() {
        let mut market = Market::default();
        let pool_address = Address::random();
        let mock_pool = MockPool { address: pool_address, token0: Address::ZERO, token1: Address::ZERO };
        market.add_pool(mock_pool.clone());

        let is_pool = market.is_pool(&pool_address);

        assert!(is_pool);
    }

    #[test]
    fn test_is_pool_not_found() {
        let market = Market::default();
        let pool_address = Address::random();

        let is_pool = market.is_pool(&pool_address);

        assert!(!is_pool);
    }

    #[test]
    fn test_set_pool_ok_to_not_ok() {
        let mut market = Market::default();
        let pool_address = Address::random();
        let mock_pool = MockPool { address: pool_address, token0: Address::ZERO, token1: Address::ZERO };
        market.add_pool(mock_pool.clone());

        market.set_pool_ok(pool_address, false);

        assert!(!market.is_pool_ok(&pool_address));
    }

    #[test]
    fn test_set_pool_ok_to_ok() {
        let mut market = Market::default();
        let pool_address = Address::random();
        let mock_pool = MockPool { address: pool_address, token0: Address::ZERO, token1: Address::ZERO };
        market.add_pool(mock_pool);

        market.set_pool_ok(pool_address, true);

        assert!(market.is_pool_ok(&pool_address));
    }

    #[test]
    fn test_get_token_token_pools() {
        let mut market = Market::default();
        let pool_address = Address::random();
        let token0 = Address::random();
        let token1 = Address::random();
        let mock_pool = MockPool { address: pool_address, token0, token1 };
        market.add_pool(mock_pool);

        let pools = market.get_token_token_pools(&token0, &token1);

        assert_eq!(pools.unwrap().get(0).unwrap(), &pool_address);
    }

    #[test]
    fn test_get_token_tokens() {
        let mut market = Market::default();
        let pool_address = Address::random();
        let token0 = Address::random();
        let token1 = Address::random();
        let mock_pool = MockPool { address: pool_address, token0, token1 };
        market.add_pool(mock_pool);

        let tokens = market.get_token_tokens(&token0);

        assert_eq!(tokens.unwrap().get(0).unwrap(), &token1);
    }

    #[test]
    fn test_get_token_pools() {
        let mut market = Market::default();
        let pool_address = Address::random();
        let token0 = Address::random();
        let token1 = Address::random();
        let mock_pool = MockPool { address: pool_address, token0, token1 };
        market.add_pool(mock_pool);

        let pools = market.get_token_pools(&token0);

        assert_eq!(pools.unwrap().get(0).unwrap(), &pool_address);
    }

    #[test]
    fn test_build_swap_path_vec_one_hop() -> Result<()> {
        let mut market = Market::default();

        // Add basic token for start/end
        let weth_token = Token::new_with_data(WETH_ADDRESS, Some("WETH".to_string()), None, Some(18), true, false);
        market.add_token(weth_token);

        // Swap pool: token weth -> token1
        let pool_address = Address::random();
        let token1 = Address::random();
        let mock_pool = PoolWrapper::new(Arc::new(MockPool { address: pool_address, token0: WETH_ADDRESS, token1 }));
        market.add_pool(mock_pool.clone());

        // Swap pool: token weth -> token1
        let pool_address2 = Address::random();
        let mock_pool2 = PoolWrapper::new(Arc::new(MockPool { address: pool_address2, token0: WETH_ADDRESS, token1 }));
        market.add_pool(mock_pool2.clone());

        // Add test swap paths
        let mut directions = BTreeMap::new();
        directions.insert(mock_pool2.clone(), mock_pool2.get_swap_directions());
        let swap_paths = market.build_swap_path_vec(&directions)?;

        // first path weth -> token1 -> weth
        assert_eq!(swap_paths.get(0).unwrap().pool_count(), 2);
        assert_eq!(swap_paths.get(0).unwrap().tokens_count(), 3);
        let tokens = swap_paths.get(0).unwrap().tokens.iter().map(|token| token.get_address()).collect::<Vec<Address>>();
        assert!(tokens.contains(&WETH_ADDRESS));
        assert!(tokens.contains(&token1));

        // other way around
        assert_eq!(swap_paths.get(1).unwrap().pool_count(), 2);
        assert_eq!(swap_paths.get(0).unwrap().tokens_count(), 3);
        let tokens = swap_paths.get(1).unwrap().tokens.iter().map(|token| token.get_address()).collect::<Vec<Address>>();
        assert!(tokens.contains(&WETH_ADDRESS));
        assert!(tokens.contains(&token1));

        Ok(())
    }
}
