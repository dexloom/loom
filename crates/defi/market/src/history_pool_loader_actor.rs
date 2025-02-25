use alloy_network::Network;
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use std::marker::PhantomData;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::logs_parser::process_log_entries;
use loom_core_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_core_actors_macros::Producer;
use loom_core_blockchain::Blockchain;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_entities::PoolLoaders;
use loom_types_events::LoomTask;

async fn history_pool_loader_one_shot_worker<P, PL, N>(
    client: P,
    pool_loaders: Arc<PoolLoaders<PL, N, LoomDataTypesEthereum>>,
    tasks_tx: Broadcaster<LoomTask>,
) -> WorkerResult
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    PL: Provider<N> + Send + Sync + Clone + 'static,
{
    let mut current_block = client.get_block_number().await?;

    let block_size: u64 = 5;

    for _ in 1..10000 {
        if current_block < block_size + 1 {
            break;
        }
        current_block -= block_size;
        debug!("Loading blocks {} {}", current_block, current_block + block_size);
        let filter = Filter::new().from_block(current_block).to_block(current_block + block_size - 1);
        match client.get_logs(&filter).await {
            Ok(logs) => {
                process_log_entries(logs, pool_loaders.as_ref(), tasks_tx.clone()).await?;
            }
            Err(e) => {
                error!("{}", e)
            }
        }
    }
    info!("history_pool_loader_worker finished");

    Ok("history_pool_loader_worker".to_string())
}

#[derive(Producer)]
pub struct HistoryPoolLoaderOneShotActor<P, PL, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    PL: Provider<N> + Send + Sync + Clone + 'static,
{
    client: P,
    pool_loaders: Arc<PoolLoaders<PL, N>>,
    #[producer]
    tasks_tx: Option<Broadcaster<LoomTask>>,
    _n: PhantomData<N>,
}

impl<P, PL, N> HistoryPoolLoaderOneShotActor<P, PL, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    PL: Provider<N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, pool_loaders: Arc<PoolLoaders<PL, N>>) -> Self {
        Self { client, pool_loaders, tasks_tx: None, _n: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { tasks_tx: Some(bc.tasks_channel()), ..self }
    }
}

impl<P, PL, N> Actor for HistoryPoolLoaderOneShotActor<P, PL, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
    PL: Provider<N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(history_pool_loader_one_shot_worker(
            self.client.clone(),
            self.pool_loaders.clone(),
            self.tasks_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "HistoryPoolLoaderOneShotActor"
    }
}
