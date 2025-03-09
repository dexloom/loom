use alloy_network::{primitives::HeaderResponse, Network};
use std::time::Duration;

use alloy_provider::Provider;
use alloy_rpc_types::{Filter, Header};
use tokio::sync::broadcast::Receiver;
use tracing::{debug, error};

use loom_core_actors::{subscribe, Broadcaster, WorkerResult};
use loom_types_events::{BlockLogs, Message, MessageBlockLogs};

pub async fn new_node_block_logs_worker<N: Network, P: Provider<N> + Send + Sync + 'static>(
    client: P,
    block_header_receiver: Broadcaster<Header>,
    sender: Broadcaster<MessageBlockLogs>,
) -> WorkerResult {
    subscribe!(block_header_receiver);

    loop {
        if let Ok(block_header) = block_header_receiver.recv().await {
            let (block_number, block_hash) = (block_header.number, block_header.hash);
            debug!("BlockLogs header received {} {}", block_number, block_hash);
            let filter = Filter::new().at_block_hash(block_header.hash());

            let mut err_counter = 0;

            while err_counter < 3 {
                match client.get_logs(&filter).await {
                    Ok(logs) => {
                        if let Err(e) = sender.send(Message::new_with_time(BlockLogs { block_header, logs })) {
                            error!("Broadcaster error {}", e);
                        }
                        break;
                    }
                    Err(e) => {
                        error!("client.get_logs error: {}", e);
                        err_counter += 1;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }

            debug!("BlockLogs processing finished {} {}", block_number, block_hash);
        }
    }
}

#[allow(dead_code)]
pub async fn new_node_block_logs_worker_reth<N: Network, P: Provider<N> + Send + Sync + 'static>(
    client: P,
    mut block_header_receiver: Receiver<Header>,
    sender: Broadcaster<MessageBlockLogs>,
) -> WorkerResult {
    loop {
        if let Ok(block_header) = block_header_receiver.recv().await {
            let filter = Filter::new().at_block_hash(block_header.hash());

            let logs = client.get_logs(&filter).await?;
            if let Err(e) = sender.send(Message::new_with_time(BlockLogs { block_header, logs })) {
                error!("Broadcaster error {}", e);
            }
        }
    }
}
