use crate::pool_config::PoolsLoadingConfig;
use crate::{PoolClass, PoolId, PoolWrapper};
use alloy_network::{Ethereum, Network};
use alloy_primitives::Bytes;
use alloy_provider::Provider;
use eyre::{eyre, ErrReport, Result};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use reth_revm::primitives::Env;
use revm::DatabaseRef;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::Stream;

#[allow(clippy::type_complexity)]
pub trait PoolLoader<P, N, LDT = LoomDataTypesEthereum>: Send + Sync + 'static
where
    N: Network,
    P: Provider<N>,
    LDT: Send + Sync + LoomDataTypes,
{
    fn get_pool_class_by_log(&self, log_entry: &LDT::Log) -> Option<(PoolId<LDT>, PoolClass)>;
    fn fetch_pool_by_id<'a>(&'a self, pool_id: PoolId<LDT>) -> Pin<Box<dyn Future<Output = Result<PoolWrapper<LDT>>> + Send + 'a>>;
    fn fetch_pool_by_id_from_provider<'a>(
        &'a self,
        pool_id: PoolId<LDT>,
        provider: P,
    ) -> Pin<Box<dyn Future<Output = Result<PoolWrapper<LDT>>> + Send + 'a>>;
    fn fetch_pool_by_id_from_evm(
        &self,
        pool_id: PoolId<LDT>,
        db: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
    ) -> Result<PoolWrapper<LDT>>;
    fn is_code(&self, code: &Bytes) -> bool;
    fn protocol_loader(&self) -> Result<Pin<Box<dyn Stream<Item = (PoolId, PoolClass)> + Send>>>;
}

pub struct PoolLoaders<P, N = Ethereum, LDT = LoomDataTypesEthereum>
where
    N: Network,
    P: Provider<N> + 'static,
    LDT: LoomDataTypes,
{
    provider: Option<P>,
    config: Option<PoolsLoadingConfig>,
    pub map: HashMap<PoolClass, Arc<dyn PoolLoader<P, N, LDT>>>,
}

impl<P, N, LDT> PoolLoaders<P, N, LDT>
where
    N: Network,
    P: Provider<N> + 'static,
    LDT: LoomDataTypes,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(self, config: PoolsLoadingConfig) -> Self {
        Self { config: Some(config), ..self }
    }

    pub fn with_provider<NP: Provider<N>>(self, provider: NP) -> PoolLoaders<NP, N, LDT> {
        PoolLoaders { provider: Some(provider), map: HashMap::new(), config: self.config }
    }

    pub fn add_loader<L: PoolLoader<P, N, LDT> + Send + Sync + Clone + 'static>(self, pool_class: PoolClass, loader: L) -> Self {
        let mut map = self.map;
        map.insert(pool_class, Arc::new(loader));
        Self { map, ..self }
    }
}

impl<P, N, LDT> Default for PoolLoaders<P, N, LDT>
where
    N: Network,
    P: Provider<N> + 'static,
    LDT: LoomDataTypes,
{
    fn default() -> Self {
        Self { provider: None, map: Default::default(), config: None }
    }
}

impl<P, N> PoolLoaders<P, N>
where
    N: Network,
    P: Provider<N> + 'static,
{
    pub fn determine_pool_class(
        &self,
        log_entry: &<LoomDataTypesEthereum as LoomDataTypes>::Log,
    ) -> Option<(PoolId<LoomDataTypesEthereum>, PoolClass)> {
        for (pool_class, pool_loader) in self.map.iter() {
            if let Some((pool_id, pool_class)) = pool_loader.get_pool_class_by_log(log_entry) {
                return Some((pool_id, pool_class));
            }
        }
        None
    }

    /*pub fn load_pool_with_provider<'a>(
        &'a self,
        provider: P,
        pool_id: PoolId<LoomDataTypesEthereum>,
        pool_class: &'a PoolClass,
    ) -> Pin<Box<dyn Future<Output = Result<PoolWrapper>> + Send + 'a>>
    where
        P: Provider<N>,
    {
        Box::pin(async move {
            if let Some(pool_loader) = self.map.get(pool_class).cloned() {
                pool_loader.fetch_pool_by_id_from_provider(provider, pool_id).await
            } else {
                Err(eyre!("POOL_CLASS_NOT_FOUND"))
            }
        })
    }
     */

    pub fn load_pool_without_provider<'a>(
        &'a self,
        pool_id: PoolId<LoomDataTypesEthereum>,
        pool_class: &'a PoolClass,
    ) -> Pin<Box<dyn Future<Output = Result<PoolWrapper>> + Send + 'a>>
    where
        P: Provider<N>,
    {
        Box::pin(async move {
            if let Some(pool_loader) = self.map.get(pool_class).cloned() {
                pool_loader.fetch_pool_by_id(pool_id).await
            } else {
                Err(eyre!("POOL_CLASS_NOT_FOUND"))
            }
        })
    }
}
