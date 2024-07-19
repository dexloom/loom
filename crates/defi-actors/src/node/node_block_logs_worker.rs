use alloy_network::Network;
use alloy_primitives::BlockHash;
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use alloy_transport::Transport;
use log::error;
use tokio::sync::broadcast::Receiver;

use defi_events::BlockLogs;
use loom_actors::{subscribe, Broadcaster, WorkerResult};

pub async fn new_node_block_logs_worker<T: Transport + Clone, N: Network, P: Provider<T, N> + Send + Sync + 'static>(
    client: P,
    block_hash_receiver: Broadcaster<BlockHash>,
    sender: Broadcaster<BlockLogs>,
) -> WorkerResult {
    subscribe!(block_hash_receiver);

    loop {
        if let Ok(block_hash) = block_hash_receiver.recv().await {
            let filter = Filter::new().at_block_hash(block_hash);

            let logs = client.get_logs(&filter).await?;
            if let Err(e) = sender.send(BlockLogs { block_hash, logs }).await {
                error!("Broadcaster error {}", e);
            }
        }
    }
}

#[allow(dead_code)]
pub async fn new_node_block_logs_worker_reth<T: Transport + Clone, N: Network, P: Provider<T, N> + Send + Sync + 'static>(
    client: P,
    mut block_hash_receiver: Receiver<BlockHash>,
    sender: Broadcaster<BlockLogs>,
) -> WorkerResult {
    loop {
        if let Ok(block_hash) = block_hash_receiver.recv().await {
            let filter = Filter::new().at_block_hash(block_hash);

            let logs = client.get_logs(&filter).await?;
            if let Err(e) = sender.send(BlockLogs { block_hash, logs }).await {
                error!("Broadcaster error {}", e);
            }
        }
    }
}
