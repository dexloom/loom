use crate::pool_config::PoolsConfig;
use crate::{PoolClass, PoolId, PoolWrapper};
use alloy_network::Network;
use alloy_primitives::Bytes;
use alloy_provider::Provider;
use alloy_transport::Transport;
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
pub trait PoolLoader<P, T, N, LDT = LoomDataTypesEthereum>: Send + Sync + 'static
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N>,
    LDT: Send + Sync + LoomDataTypes,
{
    fn get_pool_class_by_log(&self, log_entry: &LDT::Log) -> Option<(PoolId<LDT>, PoolClass)>;
    fn fetch_pool_by_id<'a>(&'a self, pool_id: PoolId<LDT>) -> Pin<Box<dyn Future<Output = Result<PoolWrapper<LDT>>> + 'a>>;
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

pub struct PoolLoaders<P, T, N, LDT = LoomDataTypesEthereum>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + 'static,
    LDT: LoomDataTypes,
{
    provider: Option<P>,
    config: Option<PoolsConfig>,
    pub map: HashMap<PoolClass, Arc<dyn PoolLoader<P, T, N, LDT>>>,
}

impl<P, T, N, LDT> PoolLoaders<P, T, N, LDT>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + 'static,
    LDT: LoomDataTypes,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(self, config: PoolsConfig) -> Self {
        Self { config: Some(config), ..self }
    }

    pub fn with_provider(self, provider: P) -> Self {
        Self { provider: Some(provider), ..self }
    }

    pub fn add_loader(self, pool_class: PoolClass, loader: Arc<dyn PoolLoader<P, T, N, LDT>>) -> Self {
        let mut map = self.map;
        map.insert(pool_class, loader);
        Self { map, ..self }
    }
}

impl<P, T, N, LDT> Default for PoolLoaders<P, T, N, LDT>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + 'static,
    LDT: LoomDataTypes,
{
    fn default() -> Self {
        Self { provider: None, map: Default::default(), config: None }
    }
}

impl<P, T, N> PoolLoaders<P, T, N>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + 'static,
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

    pub fn load_pool_with_provider<'a>(
        &'a self,
        provider: P,
        pool_id: PoolId<LoomDataTypesEthereum>,
        pool_class: &'a PoolClass,
    ) -> Pin<Box<dyn Future<Output = Result<PoolWrapper>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(pool_loader) = self.map.get(pool_class).cloned() {
                pool_loader.fetch_pool_by_id_from_provider(pool_id, provider).await
            } else {
                Err(eyre!("POOL_CLASS_NOT_FOUND"))
            }
        })
    }
}
