use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::Network;
use alloy_provider::Provider;
use tracing::error;

use loom_core_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_core_actors_macros::Producer;
use loom_core_blockchain::Blockchain;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_entities::PoolLoaders;
use loom_types_events::LoomTask;
use tokio_stream::StreamExt;

async fn protocol_pool_loader_worker<P, N>(
    _client: P,
    pool_loaders: Arc<PoolLoaders<P, N>>,
    tasks_tx: Broadcaster<LoomTask>,
) -> WorkerResult
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
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
pub struct ProtocolPoolLoaderOneShotActor<P, N>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
{
    client: P,
    pool_loaders: Arc<PoolLoaders<P, N>>,
    #[producer]
    tasks_tx: Option<Broadcaster<LoomTask>>,
    _n: PhantomData<N>,
}

impl<P, N> ProtocolPoolLoaderOneShotActor<P, N>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, pool_loaders: Arc<PoolLoaders<P, N>>) -> Self {
        Self { client, pool_loaders, tasks_tx: None, _n: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { tasks_tx: Some(bc.tasks_channel()), ..self }
    }
}

impl<P, N> Actor for ProtocolPoolLoaderOneShotActor<P, N>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
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
