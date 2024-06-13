use alloy_network::Network;
use alloy_primitives::BlockHash;
use alloy_provider::Provider;
use alloy_rpc_types::BlockId;
use alloy_transport::Transport;
use log::error;
use tokio::sync::broadcast::Receiver;

use debug_provider::DebugProviderExt;
use defi_events::NodeBlockStateUpdate;
use defi_types::debug_trace_block;
use loom_actors::{Broadcaster, WorkerResult};

pub async fn new_node_block_state_worker<P, T, N>(
    client: P,
    mut block_hash_receiver: Receiver<BlockHash>,
    sender: Broadcaster<NodeBlockStateUpdate>,
) -> WorkerResult
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static
{
    loop {
        if let Ok(block_hash) = block_hash_receiver.recv().await {
            let trace_result = debug_trace_block(client.clone(), BlockId::Hash(block_hash.into()), true).await;
            match trace_result {
                Ok((_, post)) => {
                    match sender.send(NodeBlockStateUpdate { block_hash, state_update: post }).await {
                        Err(e) => { error!("Broadcaster error {}", e) }
                        _ => {}
                    }
                }
                Err(e) => {
                    error!("debug_trace_block error : {e}")
                }
            }
        }
    }
}

