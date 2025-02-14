use alloy_network::Ethereum;
use alloy_provider::Provider;
use alloy_rpc_types::SyncStatus;
use eyre::eyre;
use loom_core_actors::{Actor, ActorResult, WorkerResult};
use loom_node_debug_provider::DebugProviderExt;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info};

const SYNC_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Wait for the node to sync. This works only for http/ipc/ws providers.
async fn wait_for_node_sync_one_shot_worker<P>(client: P) -> WorkerResult
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
{
    info!("Waiting for node to sync...");
    let mut print_count = 0;
    loop {
        match timeout(CLIENT_TIMEOUT, client.syncing()).await {
            Ok(result) => match result {
                Ok(syncing_status) => match syncing_status {
                    SyncStatus::None => {
                        break;
                    }
                    SyncStatus::Info(sync_progress) => {
                        if print_count == 0 {
                            info!("Sync progress: {:?}", sync_progress);
                        }
                    }
                },
                Err(e) => {
                    error!("Error retrieving syncing status: {:?}", e);
                    break;
                }
            },
            Err(elapsed) => {
                error!("Timeout during get syncing status. Elapsed time: {:?}", elapsed);
                break;
            }
        }
        tokio::time::sleep(SYNC_CHECK_INTERVAL).await;
        print_count = if print_count > 4 { 0 } else { print_count + 1 };
    }
    Ok("Node is sync".to_string())
}

pub struct WaitForNodeSyncOneShotBlockingActor<P> {
    client: P,
}

impl<P> WaitForNodeSyncOneShotBlockingActor<P>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> WaitForNodeSyncOneShotBlockingActor<P> {
        WaitForNodeSyncOneShotBlockingActor { client }
    }
}

impl<P> Actor for WaitForNodeSyncOneShotBlockingActor<P>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
{
    fn start_and_wait(&self) -> eyre::Result<()> {
        let rt = tokio::runtime::Runtime::new()?; // we need a different runtime to wait for the result
        let client_cloned = self.client.clone();
        let handle = rt.spawn(async { wait_for_node_sync_one_shot_worker(client_cloned).await });

        self.wait(Ok(vec![handle]))?;
        rt.shutdown_background();

        Ok(())
    }

    fn start(&self) -> ActorResult {
        Err(eyre!("NEED_TO_BE_WAITED"))
    }

    fn name(&self) -> &'static str {
        "WaitForNodeSyncOneShotBlockingActor"
    }
}
