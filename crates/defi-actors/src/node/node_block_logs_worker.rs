use alloy_network::{HeaderResponse, Network};

use alloy_provider::Provider;
use alloy_rpc_types::{Filter, Header};
use alloy_transport::Transport;
use log::error;
use tokio::sync::broadcast::Receiver;

use defi_events::{BlockLogs, Message, MessageBlockLogs};
use loom_actors::{subscribe, Broadcaster, WorkerResult};

pub async fn new_node_block_logs_worker<T: Transport + Clone, N: Network, P: Provider<T, N> + Send + Sync + 'static>(
    client: P,
    block_header_receiver: Broadcaster<Header>,
    sender: Broadcaster<MessageBlockLogs>,
) -> WorkerResult {
    subscribe!(block_header_receiver);

    loop {
        if let Ok(block_header) = block_header_receiver.recv().await {
            let filter = Filter::new().at_block_hash(block_header.hash());

            let logs = client.get_logs(&filter).await?;
            if let Err(e) = sender.send(Message::new_with_time(BlockLogs { block_header, logs })).await {
                error!("Broadcaster error {}", e);
            }
        }
    }
}

#[allow(dead_code)]
pub async fn new_node_block_logs_worker_reth<T: Transport + Clone, N: Network, P: Provider<T, N> + Send + Sync + 'static>(
    client: P,
    mut block_header_receiver: Receiver<Header>,
    sender: Broadcaster<MessageBlockLogs>,
) -> WorkerResult {
    loop {
        if let Ok(block_header) = block_header_receiver.recv().await {
            let filter = Filter::new().at_block_hash(block_header.hash());

            let logs = client.get_logs(&filter).await?;
            if let Err(e) = sender.send(Message::new_with_time(BlockLogs { block_header, logs })).await {
                error!("Broadcaster error {}", e);
            }
        }
    }
}
