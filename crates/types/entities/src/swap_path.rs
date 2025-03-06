use crate::pool_id::PoolId;
use crate::{PoolWrapper, SwapDirection, Token};
use alloy_primitives::map::HashMap;
use eyre::Result;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use std::fmt;
use std::fmt::Display;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use tracing::debug;

#[derive(Clone, Debug)]
pub struct SwapPath<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub tokens: Vec<Arc<Token<LDT>>>,
    pub pools: Vec<PoolWrapper<LDT>>,
    pub disabled: bool,
    pub disabled_pool: Vec<PoolId<LDT>>,
    pub score: Option<f64>,
}

impl<LDT: LoomDataTypes> Display for SwapPath<LDT> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tokens = self.tokens.iter().map(|token| token.get_symbol()).collect::<Vec<String>>().join(", ");
        let pools =
            self.pools.iter().map(|pool| format!("{}@{}", pool.get_protocol(), pool.get_pool_id())).collect::<Vec<String>>().join(", ");

        write!(f, "SwapPath [tokens=[{}], pools=[{}] disabled={}]", tokens, pools, self.disabled)
    }
}

impl<LDT: LoomDataTypes> Default for SwapPath<LDT> {
    #[inline]
    fn default() -> Self {
        SwapPath::<LDT> { tokens: Vec::new(), pools: Vec::new(), disabled: false, disabled_pool: Default::default(), score: None }
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
            disabled_pool: Default::default(),
            score: None,
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
        SwapPath { tokens: vec![token_from, token_to], pools: vec![pool], disabled: false, disabled_pool: Default::default(), score: None }
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
            if p == pool {
                return true;
            }
        }
        false
    }

    #[inline]
    pub fn get_hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        let hash = self.hash(&mut h);
        h.finish()
    }
}

#[derive(Clone, Debug, Default)]
pub struct SwapPaths<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub paths: Vec<SwapPath<LDT>>,
    pub pool_paths: HashMap<PoolId<LDT>, Vec<usize>>,
    pub path_hash_map: HashMap<u64, usize>,
    pub disabled_directions: HashMap<u64, bool>,
}

impl<LDT: LoomDataTypes> SwapPaths<LDT> {
    pub fn new() -> SwapPaths<LDT> {
        SwapPaths {
            paths: Vec::new(),
            pool_paths: HashMap::default(),
            path_hash_map: HashMap::default(),
            disabled_directions: HashMap::default(),
        }
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

    pub fn disabled_len(&self) -> usize {
        self.paths.iter().filter(|p| p.disabled).count()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn len_max(&self) -> usize {
        self.pool_paths.values().map(|item| item.len()).max().unwrap_or_default()
    }

    #[inline]
    pub fn add(&mut self, path: SwapPath<LDT>) -> Option<usize> {
        let path_hash = path.get_hash();
        let path_idx = self.paths.len();

        match self.path_hash_map.entry(path_hash) {
            std::collections::hash_map::Entry::Occupied(_) => {
                //debug!("Path already exists hash={}, path={}", path.get_hash(), path);
                None
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                //debug!("Path added hash={}, path={}", path.get_hash(), path);
                e.insert(path_idx);

                for pool in &path.pools {
                    self.pool_paths.entry(pool.get_pool_id()).or_default().push(path_idx);
                }

                self.paths.push(path);
                Some(path_idx)
            }
        }
    }

    pub fn disable_path(&mut self, swap_path: &SwapPath<LDT>, disable: bool) -> bool {
        if let Some(swap_path_idx) = self.path_hash_map.get(&swap_path.get_hash()) {
            if let Some(swap_path) = self.paths.get_mut(*swap_path_idx) {
                debug!("Path disabled hash={}, path={}", swap_path.get_hash(), swap_path);
                swap_path.disabled = disable;
                return true;
            }
        }
        debug!("Path not disabled hash={}, path={}", swap_path.get_hash(), swap_path);
        false
    }

    pub fn disable_pool_paths(
        &mut self,
        pool_id: &PoolId<LDT>,
        token_from_address: &LDT::Address,
        token_to_address: &LDT::Address,
        disabled: bool,
    ) {
        let Some(pool_paths) = self.pool_paths.get(pool_id).cloned() else { return };

        for path_idx in pool_paths.iter() {
            if let Some(entry) = self.paths.get_mut(*path_idx) {
                if let Some(idx) = entry.pools.iter().position(|item| item.get_pool_id().eq(pool_id)) {
                    if let (Some(token_from), Some(token_to)) = (entry.tokens.get(idx), entry.tokens.get(idx + 1)) {
                        if token_from.get_address().eq(token_from_address) && token_to.get_address().eq(token_to_address) {
                            entry.disabled = disabled;
                            if !entry.disabled_pool.contains(pool_id) {
                                entry.disabled_pool.push(pool_id.clone());
                            }
                            self.disabled_directions
                                .insert(SwapDirection::new(*token_from_address, *token_to_address).get_hash_with_pool(&pool_id), disabled);
                        }
                    }
                } else {
                    //debug!("All path disabled by pool hash={}, path={}", entry.get_hash(), entry);
                    entry.disabled = disabled;
                    if !entry.disabled_pool.contains(pool_id) {
                        entry.disabled_pool.push(pool_id.clone());
                    }
                }
            }
        }
    }
    //
    // #[inline]
    // pub fn get_pool_paths_vec(&self, pool_address: &PoolId<LDT>) -> Option<&HashSet<SwapPath<LDT>>> {
    //     self.pool_paths.get(pool_address)
    // }
    #[inline]
    pub fn get_pool_paths_enabled_vec(&self, pool_id: &PoolId<LDT>) -> Option<Vec<SwapPath<LDT>>> {
        let paths = self.pool_paths.get(pool_id)?;
        let paths_vec_ret: Vec<SwapPath<LDT>> = paths
            .iter()
            .filter_map(|a| {
                self.paths
                    .get(*a)
                    .filter(|a| a.disabled_pool.len() == 0 || (a.disabled_pool.len() == 1 && a.disabled_pool.contains(pool_id)))
            })
            .cloned()
            .collect();
        (!paths_vec_ret.is_empty()).then_some(paths_vec_ret)
    }

    #[inline]
    pub fn get_path_by_idx(&self, idx: usize) -> Option<&SwapPath<LDT>> {
        self.paths.get(idx)
    }

    #[inline]
    pub fn get_path_by_idx_mut(&mut self, idx: usize) -> Option<&mut SwapPath<LDT>> {
        self.paths.get_mut(idx)
    }

    #[inline]
    pub fn get_path_by_hash(&self, idx: u64) -> Option<&SwapPath<LDT>> {
        self.path_hash_map.get(&idx).and_then(|i| self.paths.get(*i))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pool::DefaultAbiSwapEncoder;
    use crate::required_state::RequiredState;
    use crate::{Pool, PoolAbiEncoder, PoolClass, PoolProtocol, PreswapRequirement, SwapDirection};
    use alloy_primitives::{Address, U256};
    use eyre::{eyre, ErrReport};
    use revm::primitives::Env;
    use revm::DatabaseRef;
    use std::any::Any;
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
        fn as_any<'a>(&self) -> &dyn Any {
            self
        }

        fn is_native(&self) -> bool {
            false
        }
        fn get_address(&self) -> Address {
            self.address
        }

        fn get_pool_id(&self) -> PoolId<LoomDataTypesEthereum> {
            PoolId::Address(self.address)
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

        fn get_abi_encoder(&self) -> Option<&dyn PoolAbiEncoder> {
            Some(&DefaultAbiSwapEncoder {})
        }

        fn get_state_required(&self) -> Result<RequiredState> {
            Ok(RequiredState::new())
        }

        fn get_class(&self) -> PoolClass {
            PoolClass::Unknown
        }

        fn get_protocol(&self) -> PoolProtocol {
            PoolProtocol::Unknown
        }

        fn get_fee(&self) -> U256 {
            U256::ZERO
        }

        fn get_tokens(&self) -> Vec<<LoomDataTypesEthereum as LoomDataTypes>::Address> {
            vec![]
        }

        fn get_swap_directions(&self) -> Vec<SwapDirection> {
            vec![]
        }

        fn can_calculate_in_amount(&self) -> bool {
            true
        }

        fn get_read_only_cell_vec(&self) -> Vec<U256> {
            vec![]
        }

        fn preswap_requirement(&self) -> PreswapRequirement {
            PreswapRequirement::Base
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
                let pool_paths = path_guard.get_pool_paths_enabled_vec(&PoolId::Address(pool.get_address()));
                println!("{i} {pool_address}: {pool_paths:?}");
            }));
        }

        for t in tasks {
            if let Err(e) = t.await {
                error!("{}", e)
            }
        }
    }

    #[test]
    fn test_disable_path() {
        let basic_token = Token::new(Address::repeat_byte(0x11));

        let paths_vec: Vec<SwapPath> = (0..10)
            .map(|i| {
                SwapPath::new(
                    vec![basic_token.clone(), Token::new(Address::repeat_byte(i)), basic_token.clone()],
                    vec![
                        PoolWrapper::new(Arc::new(EmptyPool::new(Address::repeat_byte(1)))),
                        PoolWrapper::new(Arc::new(EmptyPool::new(Address::repeat_byte(i + 2)))),
                    ],
                )
            })
            .collect();
        let disabled_paths = paths_vec[0].clone();
        let mut paths = SwapPaths::from(paths_vec);
        println!("Paths : {paths:?}");

        paths.disable_path(&disabled_paths, true);

        let pool_paths = paths.get_pool_paths_enabled_vec(&disabled_paths.pools[0].get_pool_id());

        println!("Pool paths : {pool_paths:?}");
    }
}
