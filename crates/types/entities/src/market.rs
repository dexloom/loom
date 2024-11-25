#![allow(clippy::type_complexity)]
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use eyre::{eyre, OptionExt, Result};
use tracing::debug;

use crate::build_swap_path_vec;
use crate::{PoolClass, PoolWrapper, Token};
use crate::{SwapPath, SwapPaths};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};

/// The market struct contains all the pools and tokens.
/// It keeps track if a pool is disabled or not and the swap paths.
#[derive(Default, Clone)]
pub struct Market<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    // pool_address -> pool
    pools: HashMap<LDT::Address, PoolWrapper<LDT>>,
    // pool_address -> is_disabled
    pools_disabled: HashMap<LDT::Address, bool>,
    // token_address -> token
    tokens: HashMap<LDT::Address, Arc<Token<LDT>>>,
    // token_from -> token_to
    token_tokens: HashMap<LDT::Address, Vec<LDT::Address>>,
    // token_from -> token_to -> pool_addresses
    token_token_pools: HashMap<LDT::Address, HashMap<LDT::Address, Vec<LDT::Address>>>,
    // token -> pool
    token_pools: HashMap<LDT::Address, Vec<LDT::Address>>,
    // swap_paths
    swap_paths: SwapPaths<LDT>,
}

impl<LDT: LoomDataTypes> Market<LDT> {
    pub fn is_weth(&self, &address: &LDT::Address) -> bool {
        address.eq(&LDT::WETH)
    }
    /// Add a [`Token`] reference to the market.
    pub fn add_token<T: Into<Arc<Token<LDT>>>>(&mut self, token: T) -> Result<()> {
        let arc_token: Arc<Token<LDT>> = token.into();
        self.tokens.insert(arc_token.get_address(), arc_token);
        Ok(())
    }

    /// Check if the token is a basic token.
    pub fn is_basic_token(&self, address: &LDT::Address) -> bool {
        if let Some(token) = self.tokens.get(address) {
            token.is_basic()
        } else {
            false
        }
    }

    /// Add a new pool to the market if it does not exist or the class is unknown.
    pub fn add_pool<T: Into<PoolWrapper<LDT>>>(&mut self, pool: T) -> Result<()> {
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
    pub fn add_paths<T: Into<SwapPath<LDT>> + Clone>(&mut self, paths: Vec<T>) {
        for path in paths.into_iter() {
            self.swap_paths.add(path);
        }
    }

    /// Get all swap paths from the market by the pool address.
    pub fn get_pool_paths(&self, pool_address: &LDT::Address) -> Option<Vec<SwapPath<LDT>>> {
        self.swap_paths.get_pool_paths_vec(pool_address)
    }

    /// Get a pool reference by the pool address. If the pool exists but the class is unknown it returns None.
    pub fn get_pool(&self, address: &LDT::Address) -> Option<&PoolWrapper<LDT>> {
        self.pools.get(address).filter(|&pool_wrapper| pool_wrapper.get_class() != PoolClass::Unknown)
    }

    /// Check if the pool exists in the market.
    pub fn is_pool(&self, address: &LDT::Address) -> bool {
        self.pools.contains_key(address)
    }

    /// Get a reference to the pools map in the market.
    pub fn pools(&self) -> &HashMap<LDT::Address, PoolWrapper<LDT>> {
        &self.pools
    }

    /// Set the pool status to ok or not ok.
    pub fn set_pool_ok(&mut self, address: LDT::Address, ok: bool) {
        *self.pools_disabled.entry(address).or_insert(false) = ok;

        let pool_contract = match self.pools.get(&address) {
            Some(pool) => pool.pool.clone(),
            None => return,
        };

        for (token_from_address, token_to_address) in pool_contract.get_swap_directions().into_iter() {
            if !ok {
                // remove pool from token_token_pools
                let _ = self
                    .token_token_pools
                    .get_mut(&token_from_address)
                    .and_then(|token_from_map| token_from_map.get_mut(&token_to_address))
                    .map(|pool_addresses| pool_addresses.retain(|&x| x != address));
            } else if self
                .token_token_pools
                .get(&token_from_address)
                .and_then(|token_from_map| token_from_map.get(&token_to_address))
                .map_or(false, |pool_addresses| !pool_addresses.contains(&address))
            {
                // add pool to token_token_pools if it does not exist
                self.token_token_pools.entry(token_from_address).or_default().entry(token_to_address).or_default().push(address);
            }
        }
    }

    /// Check if the pool is ok.
    pub fn is_pool_ok(&self, address: &LDT::Address) -> bool {
        self.pools_disabled.get(address).cloned().unwrap_or(true)
    }

    /// Get a [`Token`] reference from the market by the address of the token or create a new one.
    pub fn get_token_or_default(&self, address: &LDT::Address) -> Arc<Token<LDT>> {
        self.tokens.get(address).map_or(Arc::new(Token::new(*address)), |t| t.clone())
    }

    /// Get a [`Token`] reference from the market by the address of the token.
    pub fn get_token(&self, address: &LDT::Address) -> Option<Arc<Token<LDT>>> {
        self.tokens.get(address).cloned()
    }

    /// Get all pool addresses that allow to swap from `token_from_address` to `token_to_address`.
    pub fn get_token_token_pools(&self, token_from_address: &LDT::Address, token_to_address: &LDT::Address) -> Option<Vec<LDT::Address>> {
        if let Some(token_from_map) = self.token_token_pools.get(token_from_address) {
            if let Some(pool_address_vec) = token_from_map.get(token_to_address) {
                return Some(pool_address_vec.clone());
            }
        }
        None
    }

    /// Get all pool addresses as reference that allow to swap from `token_from_address` to `token_to_address`.
    pub fn get_token_token_pools_ptr(
        &self,
        token_from_address: &LDT::Address,
        token_to_address: &LDT::Address,
    ) -> Option<&Vec<LDT::Address>> {
        if let Some(token_from_map) = self.token_token_pools.get(token_from_address) {
            if let Some(pool_address_vec) = token_from_map.get(token_to_address) {
                return Some(pool_address_vec);
            }
        }
        None
    }

    /// Get all token addresses that allow to swap from `token_from_address`.
    pub fn get_token_tokens(&self, token_from_address: &LDT::Address) -> Option<Vec<LDT::Address>> {
        if let Some(token_vec) = self.token_tokens.get(token_from_address) {
            return Some(token_vec.clone());
        }
        None
    }

    /// Get all token addresses as reference that allow to swap from `token_from_address`.
    pub fn get_token_tokens_ptr(&self, token_from_address: &LDT::Address) -> Option<&Vec<LDT::Address>> {
        if let Some(token_vec) = self.token_tokens.get(token_from_address) {
            return Some(token_vec);
        }
        None
    }

    /// Get all pool addresses that allow to swap `token_address`.
    pub fn get_token_pools(&self, token_from_address: &LDT::Address) -> Option<Vec<LDT::Address>> {
        if let Some(token_vec) = self.token_pools.get(token_from_address) {
            return Some(token_vec.clone());
        }
        None
    }

    /// Get all pool addresses as reference that allow to swap `token_address`.
    pub fn get_token_pools_ptr(&self, token_address: &LDT::Address) -> Option<&Vec<LDT::Address>> {
        if let Some(token_vec) = self.token_pools.get(token_address) {
            return Some(token_vec);
        }
        None
    }

    /// Build a list of swap paths from the given directions.
    pub fn build_swap_path_vec(
        &self,
        directions: &BTreeMap<PoolWrapper<LDT>, Vec<(LDT::Address, LDT::Address)>>,
    ) -> Result<Vec<SwapPath<LDT>>> {
        build_swap_path_vec(self, directions)
    }

    /// get a [`SwapPath`] from the given token and pool addresses.
    pub fn swap_path(&self, token_address_vec: Vec<LDT::Address>, pool_address_vec: Vec<LDT::Address>) -> Result<SwapPath<LDT>> {
        let mut tokens: Vec<Arc<Token<LDT>>> = Vec::new();
        let mut pools: Vec<PoolWrapper<LDT>> = Vec::new();

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
    use crate::mock_pool::MockPool;
    use alloy_primitives::Address;
    use eyre::Result;
    use loom_defi_address_book::TokenAddress;

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
        let mut market = Market::<LoomDataTypesEthereum>::default();
        let token_address = Address::random();

        let result = market.add_token(Arc::new(Token::new(token_address)));

        assert!(result.is_ok());
        assert_eq!(market.get_token(&token_address).unwrap().get_address(), token_address);
    }

    #[test]
    fn test_get_token_default() {
        let market = Market::<LoomDataTypesEthereum>::default();
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
        let market = Market::<LoomDataTypesEthereum>::default();
        let pool_address = Address::random();

        let is_pool = market.is_pool(&pool_address);

        assert!(!is_pool);
    }

    #[test]
    fn test_set_pool_ok() {
        let mut market = Market::default();
        let pool_address = Address::random();
        let token0 = Address::random();
        let token1 = Address::random();
        let mock_pool = MockPool { address: pool_address, token0, token1 };
        market.add_pool(mock_pool.clone());

        assert!(market.is_pool_ok(&pool_address));
        assert_eq!(market.get_token_token_pools(&token0, &token1).unwrap().len(), 1);

        // toggle not ok
        market.set_pool_ok(pool_address, false);
        assert!(!market.is_pool_ok(&pool_address));
        assert_eq!(market.get_token_token_pools(&token0, &token1).unwrap().len(), 0);

        // toggle back
        market.set_pool_ok(pool_address, true);
        assert!(market.is_pool_ok(&pool_address));
        assert_eq!(market.get_token_token_pools(&token0, &token1).unwrap().len(), 1);
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
    fn test_build_swap_path_vec_two_hops() -> Result<()> {
        let mut market = Market::default();

        // Add basic token for start/end
        let weth_token = Token::new_with_data(TokenAddress::WETH, Some("WETH".to_string()), None, Some(18), true, false);
        market.add_token(weth_token);

        // Swap pool: token weth -> token1
        let pool_address1 = Address::random();
        let token1 = Address::random();
        let mock_pool1 = PoolWrapper::new(Arc::new(MockPool { address: pool_address1, token0: TokenAddress::WETH, token1 }));
        market.add_pool(mock_pool1.clone());

        // Swap pool: token weth -> token1
        let pool_address2 = Address::random();
        let mock_pool2 = PoolWrapper::new(Arc::new(MockPool { address: pool_address2, token0: TokenAddress::WETH, token1 }));
        market.add_pool(mock_pool2.clone());

        // Add test swap paths
        let mut directions = BTreeMap::new();
        directions.insert(mock_pool2.clone(), mock_pool2.get_swap_directions());
        let swap_paths = market.build_swap_path_vec(&directions)?;

        // verify that we have to paths, with 2 pools and 3 tokens
        assert_eq!(swap_paths.len(), 2);
        assert_eq!(swap_paths.get(0).unwrap().pool_count(), 2);
        assert_eq!(swap_paths.get(0).unwrap().tokens_count(), 3);
        assert_eq!(swap_paths.get(1).unwrap().pool_count(), 2);
        assert_eq!(swap_paths.get(1).unwrap().tokens_count(), 3);

        // the order of the swap paths is not deterministic
        let (first_path, second_path) = if swap_paths.get(0).unwrap().pools.get(0).unwrap().get_address() == pool_address1 {
            (swap_paths.get(0).unwrap(), swap_paths.get(1).unwrap())
        } else {
            (swap_paths.get(1).unwrap(), swap_paths.get(0).unwrap())
        };

        // first path weth -> token1 -> -> weth
        let tokens = first_path.tokens.iter().map(|token| token.get_address()).collect::<Vec<Address>>();
        assert_eq!(tokens.get(0), Some(&TokenAddress::WETH));
        assert_eq!(tokens.get(1), Some(&token1));
        assert_eq!(tokens.get(2), Some(&TokenAddress::WETH));

        let pools = first_path.pools.iter().map(|pool| pool.get_address()).collect::<Vec<Address>>();
        assert_eq!(pools.get(0), Some(&pool_address1));
        assert_eq!(pools.get(1), Some(&pool_address2));

        // other way around
        let tokens = second_path.tokens.iter().map(|token| token.get_address()).collect::<Vec<Address>>();
        assert_eq!(tokens.get(0), Some(&TokenAddress::WETH));
        assert_eq!(tokens.get(1), Some(&token1));
        assert_eq!(tokens.get(2), Some(&TokenAddress::WETH));

        let pools = second_path.pools.iter().map(|pool| pool.get_address()).collect::<Vec<Address>>();
        assert_eq!(pools.get(0), Some(&pool_address2));
        assert_eq!(pools.get(1), Some(&pool_address1));

        Ok(())
    }

    #[test]
    fn test_build_swap_path_vec_three_hops() -> Result<()> {
        let mut market = Market::default();

        // Add basic token for start/end
        let weth_token = Token::new_with_data(TokenAddress::WETH, Some("WETH".to_string()), None, Some(18), true, false);
        market.add_token(weth_token);

        // tokens
        let token1 = Address::random();
        let token2 = Address::random();

        // Swap pool: weth -> token1
        let pool_address1 = Address::random();
        let mock_pool = PoolWrapper::new(Arc::new(MockPool { address: pool_address1, token0: token1, token1: TokenAddress::WETH }));
        market.add_pool(mock_pool);

        // Swap pool: token1 -> token2
        let pool_address2 = Address::random();
        let mock_pool2 = PoolWrapper::new(Arc::new(MockPool { address: pool_address2, token0: token1, token1: token2 }));
        market.add_pool(mock_pool2);

        // Swap pool: token2 -> weth
        let pool_address3 = Address::random();
        let mock_pool3 = PoolWrapper::new(Arc::new(MockPool { address: pool_address3, token0: token2, token1: TokenAddress::WETH }));
        market.add_pool(mock_pool3.clone());

        // under test
        let mut directions = BTreeMap::new();
        directions.insert(mock_pool3.clone(), mock_pool3.get_swap_directions());
        let swap_paths = market.build_swap_path_vec(&directions)?;

        // verify that we have to paths, with 3 pools and 4 tokens
        assert_eq!(swap_paths.len(), 2);
        assert_eq!(swap_paths.get(0).unwrap().pool_count(), 3);
        assert_eq!(swap_paths.get(0).unwrap().tokens_count(), 4);
        assert_eq!(swap_paths.get(1).unwrap().pool_count(), 3);
        assert_eq!(swap_paths.get(1).unwrap().tokens_count(), 4);

        // the order of the swap paths is not deterministic
        let (first_path, second_path) = if swap_paths.get(0).unwrap().tokens.get(1).unwrap().get_address() == token1 {
            (swap_paths.get(0).unwrap(), swap_paths.get(1).unwrap())
        } else {
            (swap_paths.get(1).unwrap(), swap_paths.get(0).unwrap())
        };

        // first path weth -> token1 -> token2 -> weth
        let tokens = first_path.tokens.iter().map(|token| token.get_address()).collect::<Vec<Address>>();
        assert_eq!(tokens.get(0), Some(&TokenAddress::WETH));
        assert_eq!(tokens.get(1), Some(&token1));
        assert_eq!(tokens.get(2), Some(&token2));
        assert_eq!(tokens.get(3), Some(&TokenAddress::WETH));

        let pools = first_path.pools.iter().map(|pool| pool.get_address()).collect::<Vec<Address>>();
        assert_eq!(pools.get(0), Some(&pool_address1));
        assert_eq!(pools.get(1), Some(&pool_address2));
        assert_eq!(pools.get(2), Some(&pool_address3));

        // other way around
        let tokens = second_path.tokens.iter().map(|token| token.get_address()).collect::<Vec<Address>>();
        assert_eq!(tokens.get(0), Some(&TokenAddress::WETH));
        assert_eq!(tokens.get(1), Some(&token2));
        assert_eq!(tokens.get(2), Some(&token1));
        assert_eq!(tokens.get(3), Some(&TokenAddress::WETH));

        let pools = second_path.pools.iter().map(|pool| pool.get_address()).collect::<Vec<Address>>();
        assert_eq!(pools.get(0), Some(&pool_address3));
        assert_eq!(pools.get(1), Some(&pool_address2));
        assert_eq!(pools.get(2), Some(&pool_address1));

        Ok(())
    }
}
