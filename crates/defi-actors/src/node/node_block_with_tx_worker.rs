use alloy_primitives::BlockHash;
use alloy_provider::Provider;
use alloy_rpc_types::Block;
use log::error;
use tokio::sync::broadcast::Receiver;

use loom_actors::{Broadcaster, WorkerResult};

pub async fn new_block_with_tx_worker<P>(client: P, mut block_hash_receiver: Receiver<BlockHash>, sender: Broadcaster<Block>) -> WorkerResult
    where P: Provider + Send + Sync + 'static,
{
    loop {
        if let Ok(block_hash) = block_hash_receiver.recv().await {
            if let Some(block_with_txes) = client.get_block_by_hash(block_hash, true).await? {
                match sender.send(block_with_txes).await {
                    Err(e) => { error!("Broadcaster error {}", e); }
                    _ => {}
                }
            }
        }
    }
}