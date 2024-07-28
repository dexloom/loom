use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use alloy_primitives::Address;
use eyre::{OptionExt, Result};
use lazy_static::lazy_static;
use log::debug;

use crate::build_swap_path_vec;
use crate::{EmptyPool, Pool, PoolClass, PoolWrapper, Token};
use crate::{SwapPath, SwapPaths};

#[derive(Default, Clone)]
pub struct Market {
    pools: HashMap<Address, PoolWrapper>,
    pools_disabled: HashMap<Address, bool>,
    tokens: HashMap<Address, Arc<Token>>,
    token_tokens: HashMap<Address, Vec<Address>>,
    token_pools: HashMap<Address, HashMap<Address, Vec<Address>>>,
    swap_paths: SwapPaths,
}

lazy_static! {
    static ref WETH_ADDRESS: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
}

impl Market {
    pub fn add_token<T: Into<Arc<Token>>>(&mut self, token: T) -> Result<()> {
        let arc_token: Arc<Token> = token.into();
        self.tokens.insert(arc_token.get_address(), arc_token);
        Ok(())
    }

    pub fn is_basic_token(&self, address: &Address) -> bool {
        if let Some(token) = self.tokens.get(address) {
            token.is_basic()
        } else {
            false
        }
    }

    pub fn is_weth(address: &Address) -> bool {
        *address == *WETH_ADDRESS
    }

    pub fn add_empty_pool(&mut self, address: &Address) -> Result<()> {
        let pool_contract = EmptyPool::new(*address);
        self.pools.insert(pool_contract.get_address(), pool_contract.into());
        Ok(())
    }

    pub fn add_pool<T: Into<PoolWrapper>>(&mut self, pool: T) -> Result<()> {
        let pool_contract = pool.into();

        debug!("Adding pool {:?}", pool_contract.get_address());

        let pool_address = pool_contract.get_address();

        let mut token_from_entry_len = 0;
        let mut token_to_entry_len = 0;
        let mut token_from_len = 0;
        let mut token_to_len = 0;

        for (token_address_from, token_address_to) in pool_contract.get_swap_directions().iter() {
            let token_from_entry = self.token_pools.entry(*token_address_from).or_default();
            let token_to_entry = token_from_entry.entry(*token_address_to).or_default();
            if !token_to_entry.contains(&pool_address) {
                token_to_entry.push(pool_address);
            }
            token_to_entry_len = token_to_entry.len();
            token_from_entry_len = token_from_entry.len();

            let token_token_entry = self.token_tokens.entry(*token_address_from).or_default();
            if !token_token_entry.contains(token_address_to) {
                token_token_entry.push(*token_address_to);
            }
            token_from_len = token_token_entry.len();
            let token_token_entry = self.token_tokens.entry(*token_address_to).or_default();
            if !token_token_entry.contains(token_address_from) {
                token_token_entry.push(*token_address_from);
            }
            token_to_len = token_token_entry.len();
        }

        debug!("Added pool {:?} {} TokenFromPools {token_from_entry_len} TokenToPools {token_to_entry_len} TokenFromToken {token_from_len} TokenToToken {token_to_len}", pool_contract.get_address(), pool_contract.get_protocol());
        self.pools.insert(pool_address, pool_contract);

        Ok(())
    }

    pub fn add_paths<T: Into<Arc<SwapPath>>>(&mut self, paths: Vec<T>) {
        for path in paths.into_iter() {
            self.swap_paths.add(path);
        }
    }

    pub fn get_pool_paths(&self, pool_address: &Address) -> Option<Vec<Arc<SwapPath>>> {
        self.swap_paths.get_pool_paths_vec(pool_address)
    }

    pub fn get_pool(&self, address: &Address) -> Option<&PoolWrapper> {
        self.pools.get(address).filter(|&pool_wrapper| pool_wrapper.get_class() != PoolClass::Unknown)
    }

    pub fn is_pool(&self, address: &Address) -> bool {
        self.pools.contains_key(address)
    }

    pub fn pools(&self) -> &HashMap<Address, PoolWrapper> {
        &self.pools
    }

    /*pub fn add_error(&mut self, address: Address) {
        match self.pool_errors_map.entry(address) {
            Entry::Vacant(e)=>{
                e.insert(1);
            }
            Entry::Occupied(mut e)=>{
                e.insert(*e.get() + 1);
            }
        }
    }

     */
    pub fn set_pool_ok(&mut self, address: Address, ok: bool) {
        *self.pools_disabled.entry(address).or_insert(false) = ok
    }

    pub fn is_pool_ok(&self, address: &Address) -> bool {
        self.pools_disabled.get(address).cloned().unwrap_or(true)
    }

    pub fn get_token_or_default(&self, address: &Address) -> Arc<Token> {
        self.tokens.get(address).map_or(Arc::new(Token::new(*address)), |t| t.clone())
    }

    pub fn get_token(&self, address: &Address) -> Option<Arc<Token>> {
        self.tokens.get(address).cloned()
    }

    /*pub fn get_token_mut(&mut self, address : &Address) -> Option<&mut Token> {
        self.tokens.get_mut(address)
    }
     */

    pub fn get_token_token_pools(&self, token_from_address: &Address, token_to_address: &Address) -> Option<Vec<Address>> {
        if let Some(token_from_map) = self.token_pools.get(token_from_address) {
            if let Some(pool_address_vec) = token_from_map.get(token_to_address) {
                return Some(pool_address_vec.clone());
            }
        }
        None
    }

    pub fn get_token_token_pools_ptr(&self, token_from_address: &Address, token_to_address: &Address) -> Option<&Vec<Address>> {
        if let Some(token_from_map) = self.token_pools.get(token_from_address) {
            if let Some(pool_address_vec) = token_from_map.get(token_to_address) {
                return Some(pool_address_vec);
            }
        }
        None
    }

    pub fn get_token_tokens(&self, token_from_address: &Address) -> Option<Vec<Address>> {
        if let Some(token_vec) = self.token_tokens.get(token_from_address) {
            return Some(token_vec.clone());
        }
        None
    }

    pub fn get_token_tokens_ptr(&self, token_from_address: &Address) -> Option<&Vec<Address>> {
        if let Some(token_vec) = self.token_tokens.get(token_from_address) {
            return Some(token_vec);
        }
        None
    }

    pub fn build_swap_path_vec(&self, directions: &BTreeMap<PoolWrapper, Vec<(Address, Address)>>) -> Result<Vec<Arc<SwapPath>>> {
        build_swap_path_vec(self, directions)
    }

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
