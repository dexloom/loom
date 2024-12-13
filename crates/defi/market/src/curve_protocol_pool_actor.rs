use revm::{Database, DatabaseCommit};
use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;
use tracing::{debug, error};

use crate::pool_loader_actor::fetch_state_and_add_pool;
use loom_core_actors::{Accessor, Actor, ActorResult, Broadcaster, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_defi_pools::protocols::CurveProtocol;
use loom_defi_pools::{CurvePool, CurvePoolAbiEncoder};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_entities::{Market, MarketState, PoolId, PoolLoaders, PoolWrapper};
use loom_types_events::LoomTask;
use revm::DatabaseRef;
use tokio_stream::StreamExt;

async fn protocol_pool_loader_worker<P, T, N>(
    client: P,
    pool_loaders: Arc<PoolLoaders<P, T, N>>,
    tasks_tx: Broadcaster<LoomTask>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    for (pool_class, pool_loader) in pool_loaders.map.iter() {
        let tasks_tx_clone = tasks_tx.clone();
        if let Ok(mut proto_loader) = pool_loader.clone().protocol_loader() {
            tokio::task::spawn(async move {
                while let Some((pool_id, pool_class)) = proto_loader.next().await {
                    if let Err(error) = tasks_tx_clone.send(LoomTask::FetchAndAddPools(vec![(pool_id, pool_class)])).await {
                        error!(%error, "tasks_tx.send");
                    }
                }
            });
        } else {
            error!("Protocol loader unavailable for {}", pool_class);
        }
    }

    Ok("curve_protocol_loader_worker".to_string())
}

#[derive(Producer)]
pub struct ProtocolPoolLoaderOneShotActor<P, T, N>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    client: P,
    pool_loaders: Arc<PoolLoaders<P, T, N>>,
    #[producer]
    tasks_tx: Option<Broadcaster<LoomTask>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> ProtocolPoolLoaderOneShotActor<P, T, N>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, pool_loaders: Arc<PoolLoaders<P, T, N>>) -> Self {
        Self { client, pool_loaders, tasks_tx: None, _n: PhantomData, _t: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { tasks_tx: Some(bc.tasks_channel()), ..self }
    }
}

impl<P, T, N> Actor for ProtocolPoolLoaderOneShotActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task =
            tokio::task::spawn(protocol_pool_loader_worker(self.client.clone(), self.pool_loaders.clone(), self.tasks_tx.clone().unwrap()));

        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "CurvePoolLoaderOneShotActor"
    }
}
