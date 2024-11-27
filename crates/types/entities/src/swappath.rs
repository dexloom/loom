use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::{PoolWrapper, Token};
use eyre::Result;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};

#[derive(Clone, Debug)]
pub struct SwapPath<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub tokens: Vec<Arc<Token<LDT>>>,
    pub pools: Vec<PoolWrapper<LDT>>,
    pub disabled: bool,
}

impl<LDT: LoomDataTypes> Default for SwapPath<LDT> {
    #[inline]
    fn default() -> Self {
        SwapPath::<LDT> { tokens: Vec::new(), pools: Vec::new(), disabled: false }
    }
}

impl<LDT: LoomDataTypes> PartialEq for SwapPath<LDT> {
    fn eq(&self, other: &Self) -> bool {
        self.tokens == other.tokens && self.pools == other.pools
    }
}

impl<LDT: LoomDataTypes> Eq for SwapPath<LDT> {}

impl<LDT: LoomDataTypes> Hash for SwapPath<LDT> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tokens.hash(state);
        self.pools.hash(state);
    }
}

impl<LDT: LoomDataTypes> SwapPath<LDT> {
    #[inline]
    pub fn new<T: Into<Arc<Token<LDT>>>, P: Into<PoolWrapper<LDT>>>(tokens: Vec<T>, pools: Vec<P>) -> Self {
        SwapPath {
            tokens: tokens.into_iter().map(|i| i.into()).collect(),
            pools: pools.into_iter().map(|i| i.into()).collect(),
            disabled: false,
        }
    }

    #[inline]
    pub fn is_emply(&self) -> bool {
        self.tokens.is_empty() && self.pools.is_empty()
    }

    #[inline]
    pub fn tokens_count(&self) -> usize {
        self.tokens.len()
    }

    #[inline]
    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    #[inline]
    pub fn new_swap(token_from: Arc<Token<LDT>>, token_to: Arc<Token<LDT>>, pool: PoolWrapper<LDT>) -> Self {
        SwapPath { tokens: vec![token_from, token_to], pools: vec![pool], disabled: false }
    }

    #[inline]
    pub fn push_swap_hope(&mut self, token_from: Arc<Token<LDT>>, token_to: Arc<Token<LDT>>, pool: PoolWrapper<LDT>) -> Result<&mut Self> {
        if self.is_emply() {
            self.tokens = vec![token_from, token_to];
            self.pools = vec![pool];
        } else {
            if token_from.as_ref() != self.tokens.last().map_or(&Token::<LDT>::zero(), |t| t.as_ref()) {
                return Err(eyre::eyre!("NEW_SWAP_NOT_CONNECTED"));
            }
            self.tokens.push(token_to);
            self.pools.push(pool);
        }
        Ok(self)
    }

    #[inline]
    pub fn insert_swap_hope(
        &mut self,
        token_from: Arc<Token<LDT>>,
        token_to: Arc<Token<LDT>>,
        pool: PoolWrapper<LDT>,
    ) -> Result<&mut Self> {
        if self.is_emply() {
            self.tokens = vec![token_from, token_to];
            self.pools = vec![pool];
        } else {
            if token_to.as_ref() != self.tokens.first().map_or(&Token::<LDT>::zero(), |t| t.as_ref()) {
                return Err(eyre::eyre!("NEW_SWAP_NOT_CONNECTED"));
            }
            self.tokens.insert(0, token_from);
            self.pools.insert(0, pool);
        }

        Ok(self)
    }

    #[inline]
    pub fn contains_pool(&self, pool: &PoolWrapper<LDT>) -> bool {
        for p in self.pools.iter() {
            if p.get_address() == pool.get_address() {
                return true;
            }
        }
        false
    }
}

#[derive(Clone, Debug, Default)]
pub struct SwapPaths<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    paths: HashSet<SwapPath<LDT>>,
    pool_paths: HashMap<LDT::Address, HashSet<SwapPath<LDT>>>,
}

impl<LDT: LoomDataTypes> SwapPaths<LDT> {
    pub fn new() -> SwapPaths<LDT> {
        SwapPaths { paths: HashSet::new(), pool_paths: HashMap::new() }
    }
    pub fn from(paths: Vec<SwapPath<LDT>>) -> Self {
        let mut swap_paths_ret = SwapPaths::<LDT>::new();
        for p in paths {
            swap_paths_ret.add(p);
        }
        swap_paths_ret
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }
    pub fn len_max(&self) -> usize {
        self.pool_paths.values().map(|item| item.len()).max().unwrap_or_default()
    }

    #[inline]
    pub fn add(&mut self, path: SwapPath<LDT>) {
        if self.paths.insert(path.clone()) {
            for pool in path.pools.iter() {
                self.pool_paths.entry(pool.get_address()).or_default().insert(path.clone());
            }
        }
    }

    #[inline]
    pub fn replace(&mut self, path: SwapPath<LDT>) {
        self.paths.replace(path.clone());
        for pool in path.pools.iter() {
            self.pool_paths.entry(pool.get_address()).or_default().replace(path.clone());
        }
    }

    pub fn disable_pool(&mut self, pool_address: &LDT::Address, disabled: bool) {
        let Some(pool_paths) = self.pool_paths.get(pool_address).cloned() else { return };

        for path in pool_paths.into_iter() {
            self.replace(SwapPath { disabled, ..path })
        }
    }

    #[inline]
    pub fn get_pool_paths_hashset(&self, pool_address: &LDT::Address) -> Option<&HashSet<SwapPath<LDT>>> {
        self.pool_paths.get(pool_address)
    }

    #[inline]
    pub fn get_pool_paths_vec(&self, pool_address: &LDT::Address) -> Option<Vec<SwapPath<LDT>>> {
        let Some(paths) = self.get_pool_paths_hashset(pool_address) else { return None };

        let paths_vec_ret: Vec<SwapPath<LDT>> =
            paths.iter().filter_map(|path| if !path.disabled { Some(path.clone()) } else { None }).collect();

        paths_vec_ret.is_empty().then(|| paths_vec_ret)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pool::DefaultAbiSwapEncoder;
    use crate::required_state::RequiredState;
    use crate::{AbiSwapEncoder, Pool};
    use alloy_primitives::{Address, U256};
    use eyre::{eyre, ErrReport};
    use revm::primitives::Env;
    use revm::DatabaseRef;
    use tokio::task::JoinHandle;
    use tracing::error;

    #[derive(Clone)]
    pub struct EmptyPool {
        address: Address,
    }

    impl EmptyPool {
        pub fn new(address: Address) -> Self {
            EmptyPool { address }
        }
    }

    impl Pool for EmptyPool {
        fn get_address(&self) -> Address {
            self.address
        }

        fn calculate_out_amount(
            &self,
            _state: &dyn DatabaseRef<Error = ErrReport>,
            _env: Env,
            _token_address_from: &Address,
            _token_address_to: &Address,
            _in_amount: U256,
        ) -> Result<(U256, u64), ErrReport> {
            Err(eyre!("NOT_IMPLEMENTED"))
        }

        fn calculate_in_amount(
            &self,
            _state: &dyn DatabaseRef<Error = ErrReport>,
            _env: Env,
            _token_address_from: &Address,
            _token_address_to: &Address,
            _out_amount: U256,
        ) -> eyre::Result<(U256, u64), ErrReport> {
            Err(eyre!("NOT_IMPLEMENTED"))
        }

        fn can_flash_swap(&self) -> bool {
            false
        }

        fn get_encoder(&self) -> &dyn AbiSwapEncoder {
            &DefaultAbiSwapEncoder {}
        }

        fn get_state_required(&self) -> Result<RequiredState> {
            Ok(RequiredState::new())
        }
    }

    #[test]
    fn test_add_path() {
        let basic_token = Token::new(Address::repeat_byte(0x11));

        let paths_vec: Vec<SwapPath> = (0..10)
            .map(|i| {
                SwapPath::new(
                    vec![basic_token.clone(), Token::new(Address::repeat_byte(i)), basic_token.clone()],
                    vec![
                        PoolWrapper::new(Arc::new(EmptyPool::new(Address::repeat_byte(i + 1)))),
                        PoolWrapper::new(Arc::new(EmptyPool::new(Address::repeat_byte(i + 2)))),
                    ],
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
            .map(|i| {
                (
                    PoolWrapper::new(Arc::new(EmptyPool::new(Address::repeat_byte(i as u8)))),
                    PoolWrapper::new(Arc::new(EmptyPool::new(Address::repeat_byte((i + 1) as u8)))),
                )
            })
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
            paths.add(path);
        }

        let paths_shared = Arc::new(tokio::sync::RwLock::new(paths));

        let mut tasks: Vec<JoinHandle<_>> = Vec::new();

        for (i, pools) in pool_address_vec.into_iter().enumerate() {
            let pool_address = pools.0.get_address();
            let paths_shared_clone = paths_shared.clone();
            tasks.push(tokio::task::spawn(async move {
                let pool = PoolWrapper::new(Arc::new(EmptyPool::new(pool_address)));
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
