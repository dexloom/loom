use alloy_network::Network;
use alloy_provider::Provider;
use eyre::Result;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error};

use loom_core_actors::{subscribe, Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_core_actors_macros::{Consumer, Producer};
use loom_core_blockchain::Blockchain;
use loom_types_entities::PoolLoaders;
use loom_types_events::{LoomTask, MessageBlockLogs};

use crate::logs_parser::process_log_entries;

pub async fn new_pool_worker<P, N>(
    log_update_rx: Broadcaster<MessageBlockLogs>,
    pools_loaders: Arc<PoolLoaders<P, N>>,
    tasks_tx: Broadcaster<LoomTask>,
) -> WorkerResult
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    subscribe!(log_update_rx);

    loop {
        tokio::select! {
            msg = log_update_rx.recv() => {
                debug!("Log update");

                let log_update : Result<MessageBlockLogs, RecvError>  = msg;
                match log_update {
                    Ok(log_update_msg)=>{
                        process_log_entries(
                                log_update_msg.inner.logs,
                                &pools_loaders,
                                tasks_tx.clone(),
                        ).await?
                    }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }

            }
        }
    }
}

#[derive(Consumer, Producer)]
pub struct NewPoolLoaderActor<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pool_loaders: Arc<PoolLoaders<P, N>>,
    #[consumer]
    log_update_rx: Option<Broadcaster<MessageBlockLogs>>,
    #[producer]
    tasks_tx: Option<Broadcaster<LoomTask>>,
}

impl<P, N> NewPoolLoaderActor<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pub fn new(pool_loaders: Arc<PoolLoaders<P, N>>) -> Self {
        NewPoolLoaderActor { log_update_rx: None, pool_loaders, tasks_tx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { log_update_rx: Some(bc.new_block_logs_channel()), tasks_tx: Some(bc.tasks_channel()), ..self }
    }
}

impl<P, N> Actor for NewPoolLoaderActor<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_pool_worker(
            self.log_update_rx.clone().unwrap(),
            self.pool_loaders.clone(),
            self.tasks_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "NewPoolLoaderActor"
    }
}
