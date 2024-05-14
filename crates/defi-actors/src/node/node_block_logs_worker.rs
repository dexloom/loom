use alloy_primitives::BlockHash;
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use log::error;
use tokio::sync::broadcast::Receiver;

use defi_events::BlockLogsUpdate;
use loom_actors::{Broadcaster, WorkerResult};

pub async fn new_node_block_logs_worker<P>(
    client: P,
    mut block_hash_receiver: Receiver<BlockHash>,
    sender: Broadcaster<BlockLogsUpdate>) -> WorkerResult
    where P: Provider + Send + Sync + 'static
{
    loop {
        if let Ok(block_hash) = block_hash_receiver.recv().await {
            let filter = Filter::new().at_block_hash(block_hash);

            let logs = client.get_logs(&filter).await?;
            match sender.send(BlockLogsUpdate { block_hash, logs }).await {
                Err(e) => { error!("Broadcaster error {}", e); }
                _ => {}
            }
        }
    }
}
