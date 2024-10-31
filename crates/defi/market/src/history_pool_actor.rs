use std::marker::PhantomData;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use alloy_transport::Transport;
use tracing::{debug, error, info};

use crate::logs_parser::process_log_entries;
use loom_core_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_core_actors_macros::Producer;
use loom_core_blockchain::Blockchain;
use loom_defi_pools::PoolsConfig;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_events::Task;

async fn history_pool_loader_one_shot_worker<P, T, N>(client: P, pools_config: PoolsConfig, tasks_tx: Broadcaster<Task>) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
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
                process_log_entries(logs, &pools_config, tasks_tx.clone()).await?;
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
pub struct HistoryPoolLoaderOneShotActor<P, T, N> {
    client: P,
    pools_config: PoolsConfig,
    #[producer]
    tasks_tx: Option<Broadcaster<Task>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> HistoryPoolLoaderOneShotActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, pools_config: PoolsConfig) -> Self {
        Self { client, pools_config, tasks_tx: None, _t: PhantomData, _n: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { tasks_tx: Some(bc.tasks_channel()), ..self }
    }
}

impl<P, T, N> Actor for HistoryPoolLoaderOneShotActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(history_pool_loader_one_shot_worker(
            self.client.clone(),
            self.pools_config.clone(),
            self.tasks_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "HistoryPoolLoaderOneShotActor"
    }
}
