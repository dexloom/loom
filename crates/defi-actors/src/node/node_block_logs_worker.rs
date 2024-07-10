use alloy_network::Network;
use alloy_primitives::BlockHash;
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use alloy_transport::Transport;
use log::error;
use reth_provider::{
    AccountReader, BlockReader, BlockSource, HeaderProvider, ProviderFactory,
    providers::StaticFileProvider, ReceiptProvider, StateProvider, TransactionsProvider,
};
use tokio::sync::broadcast::Receiver;

use defi_events::BlockLogs;
use loom_actors::{Broadcaster, WorkerResult};

pub async fn new_node_block_logs_worker<T: Transport + Clone, N: Network, P: Provider<T, N> + Send + Sync + 'static>(
    client: P,
    mut block_hash_receiver: Receiver<BlockHash>,
    sender: Broadcaster<BlockLogs>) -> WorkerResult
{
    loop {
        if let Ok(block_hash) = block_hash_receiver.recv().await {
            let filter = Filter::new().at_block_hash(block_hash);

            let logs = client.get_logs(&filter).await?;
            match sender.send(BlockLogs { block_hash, logs }).await {
                Err(e) => { error!("Broadcaster error {}", e); }
                _ => {}
            }
        }
    }
}

pub async fn new_node_block_logs_worker_reth<T: Transport + Clone, N: Network, P: Provider<T, N> + Send + Sync + 'static>(
    client: P,
    mut block_hash_receiver: Receiver<BlockHash>,
    sender: Broadcaster<BlockLogs>) -> WorkerResult
{
    loop {
        if let Ok(block_hash) = block_hash_receiver.recv().await {
            let filter = Filter::new().at_block_hash(block_hash);

            let logs = client.get_logs(&filter).await?;
            match sender.send(BlockLogs { block_hash, logs }).await {
                Err(e) => { error!("Broadcaster error {}", e); }
                _ => {}
            }
        }
    }
}
