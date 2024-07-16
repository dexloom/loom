use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use alloy_primitives::Address;
use eyre::Result;

use crate::{PoolWrapper, Token};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SwapPath {
    pub tokens: Vec<Arc<Token>>,
    pub pools: Vec<PoolWrapper>,
}

impl SwapPath {
    pub fn new<T: Into<Arc<Token>>, P: Into<PoolWrapper>>(tokens: Vec<T>, pools: Vec<P>) -> Self {
        SwapPath { tokens: tokens.into_iter().map(|i| i.into()).collect(), pools: pools.into_iter().map(|i| i.into()).collect() }
    }

    pub fn is_emply(&self) -> bool {
        self.tokens.is_empty() && self.pools.is_empty()
    }

    pub fn tokens_count(&self) -> usize {
        self.tokens.len()
    }

    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    pub fn new_swap(token_from: Arc<Token>, token_to: Arc<Token>, pool: PoolWrapper) -> Self {
        SwapPath { tokens: vec![token_from, token_to], pools: vec![pool] }
    }

    pub fn push_swap_hope(&mut self, token_from: Arc<Token>, token_to: Arc<Token>, pool: PoolWrapper) -> Result<&mut Self> {
        if self.is_emply() {
            self.tokens = vec![token_from, token_to];
            self.pools = vec![pool];
        } else {
            if token_from.as_ref() != self.tokens.last().map_or(&Default::default(), |t| t.as_ref()) {
                return Err(eyre::eyre!("NEW_SWAP_NOT_CONNECTED"));
            }
            self.tokens.push(token_to);
            self.pools.push(pool);
        }
        Ok(self)
    }

    pub fn insert_swap_hope(&mut self, token_from: Arc<Token>, token_to: Arc<Token>, pool: PoolWrapper) -> Result<&mut Self> {
        if self.is_emply() {
            self.tokens = vec![token_from, token_to];
            self.pools = vec![pool];
        } else {
            if token_to.as_ref() != self.tokens.first().map_or(&Default::default(), |t| t.as_ref()) {
                return Err(eyre::eyre!("NEW_SWAP_NOT_CONNECTED"));
            }
            self.tokens.insert(0, token_from);
            self.pools.insert(0, pool);
        }

        Ok(self)
    }

    pub fn contains_pool(&self, pool: &PoolWrapper) -> bool {
        for p in self.pools.iter() {
            if p.get_address() == pool.get_address() {
                return true;
            }
        }
        false
    }
}

impl Hash for SwapPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tokens.hash(state);
        self.pools.hash(state);
    }
}

#[derive(Clone, Debug, Default)]
pub struct SwapPaths {
    paths: HashSet<Arc<SwapPath>>,
    pool_paths: HashMap<Address, Arc<HashSet<Arc<SwapPath>>>>,
}

impl SwapPaths {
    pub fn new() -> SwapPaths {
        SwapPaths { paths: HashSet::new(), pool_paths: HashMap::new() }
    }
    pub fn from(paths: Vec<SwapPath>) -> Self {
        let mut ret = Self::default();
        for p in paths {
            ret.add(p);
        }
        ret
    }

    pub fn add_mut(&mut self, path: SwapPath) -> bool {
        let rc_path = Arc::new(path);

        if self.paths.insert(rc_path.clone()) {
            for pool in rc_path.pools.iter() {
                let e = self.pool_paths.entry(pool.get_address()).or_insert(Arc::new(HashSet::new()));
                let mut v = e.clone().deref().clone();
                v.insert(rc_path.clone());
                *e = Arc::new(v);
            }
            true
        } else {
            false
        }
    }

    pub fn add<T: Into<Arc<SwapPath>>>(&mut self, path: T) {
        let rc_path: Arc<SwapPath> = path.into();

        if self.paths.insert(rc_path.clone()) {
            for pool in rc_path.pools.iter() {
                let e = self.pool_paths.entry(pool.get_address()).or_insert(Arc::new(HashSet::new()));
                let mut v = e.clone().deref().clone();
                v.insert(rc_path.clone());
                *e = Arc::new(v);
            }
        }
    }

    pub fn get_pool_paths_hashset(&self, pool_address: &Address) -> Option<&Arc<HashSet<Arc<SwapPath>>>> {
        self.pool_paths.get(pool_address)
    }

    pub fn get_pool_paths_vec(&self, pool_address: &Address) -> Option<Vec<Arc<SwapPath>>> {
        self.get_pool_paths_hashset(pool_address).map(|set| set.iter().cloned().collect())
    }
}

#[cfg(test)]
mod test {
    use log::error;
    use tokio::task::JoinHandle;

    use super::*;

    #[test]
    fn test_add_path() {
        let basic_token = Token::new(Address::repeat_byte(0x11));

        let paths_vec: Vec<SwapPath> = (0..10)
            .map(|i| {
                SwapPath::new(
                    vec![basic_token.clone(), Token::new(Address::repeat_byte(i)), basic_token.clone()],
                    vec![PoolWrapper::empty(Address::repeat_byte(i + 1)), PoolWrapper::empty(Address::repeat_byte(i + 2))],
                )
            })
            .collect();
        let paths = SwapPaths::from(paths_vec);

        println!("{paths:?}")
    }

    #[tokio::test]
    async fn async_test() {
        let basic_token = Token::new(Address::repeat_byte(0x11));

        const PATHS_COUNT: usize = 10;

        let pool_address_vec: Vec<(PoolWrapper, PoolWrapper)> = (0..PATHS_COUNT)
            .map(|i| (PoolWrapper::empty(Address::repeat_byte(i as u8)), PoolWrapper::empty(Address::repeat_byte((i + 1) as u8))))
            .collect();

        let paths_vec: Vec<SwapPath> = pool_address_vec
            .iter()
            .map(|p| {
                SwapPath::new(
                    vec![basic_token.clone(), Token::new(Address::repeat_byte(1)), basic_token.clone()],
                    vec![p.0.clone(), p.1.clone()],
                )
            })
            .collect();

        let mut paths = SwapPaths::from(paths_vec.clone());
        for path in paths_vec.clone() {
            println!("{}", paths.add_mut(path));
        }

        let paths_shared = Arc::new(tokio::sync::RwLock::new(paths));

        let mut tasks: Vec<JoinHandle<_>> = Vec::new();

        for i in 0..PATHS_COUNT {
            let pool_address = pool_address_vec[i].0.get_address();
            let paths_shared_clone = paths_shared.clone();
            tasks.push(tokio::task::spawn(async move {
                let pool = PoolWrapper::empty(pool_address);
                let path_guard = paths_shared_clone.read().await;
                let pool_paths = path_guard.get_pool_paths_hashset(&pool.get_address());
                println!("{i} {pool_address}: {pool_paths:?}");
            }));
        }

        for t in tasks {
            if let Err(e) = t.await {
                error!("{}", e)
            }
        }
    }
}
