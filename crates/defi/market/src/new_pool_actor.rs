use eyre::Result;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error};

use loom_core_actors::{subscribe, Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_core_actors_macros::{Consumer, Producer};
use loom_core_blockchain::Blockchain;
use loom_defi_pools::PoolsConfig;
use loom_types_events::{MessageBlockLogs, Task};

use crate::logs_parser::process_log_entries;

pub async fn new_pool_worker(
    log_update_rx: Broadcaster<MessageBlockLogs>,
    pools_config: PoolsConfig,
    tasks_tx: Broadcaster<Task>,
) -> WorkerResult {
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
                                &pools_config,
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
pub struct NewPoolLoaderActor {
    #[consumer]
    log_update_rx: Option<Broadcaster<MessageBlockLogs>>,
    pools_config: PoolsConfig,
    #[producer]
    tasks_tx: Option<Broadcaster<Task>>,
}

impl NewPoolLoaderActor {
    pub fn new(pools_config: PoolsConfig) -> Self {
        NewPoolLoaderActor { log_update_rx: None, pools_config, tasks_tx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { log_update_rx: Some(bc.new_block_logs_channel()), tasks_tx: Some(bc.tasks_channel()), ..self }
    }
}

impl Actor for NewPoolLoaderActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_pool_worker(
            self.log_update_rx.clone().unwrap(),
            self.pools_config.clone(),
            self.tasks_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "NewPoolLoaderActor"
    }
}
