use alloy_network::Network;

use alloy_provider::Provider;
use alloy_rpc_types::{BlockId, Header};
use tracing::{debug, error};

use loom_core_actors::{subscribe, Broadcaster, WorkerResult};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::debug_trace_block;
use loom_types_events::{BlockStateUpdate, Message, MessageBlockStateUpdate};

pub async fn new_node_block_state_worker<P, N>(
    client: P,
    block_header_receiver: Broadcaster<Header>,
    sender: Broadcaster<MessageBlockStateUpdate>,
) -> WorkerResult
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
{
    subscribe!(block_header_receiver);

    loop {
        if let Ok(block_header) = block_header_receiver.recv().await {
            let (block_number, block_hash) = (block_header.number, block_header.hash);
            debug!("BlockState header received {} {}", block_number, block_hash);

            match debug_trace_block(client.clone(), BlockId::Hash(block_header.hash.into()), true).await {
                Ok((_, post)) => {
                    if let Err(e) = sender.send(Message::new_with_time(BlockStateUpdate { block_header, state_update: post })) {
                        error!("Broadcaster error {}", e)
                    }
                }
                Err(e) => {
                    error!("debug_trace_block error : {e}")
                }
            }
            debug!("BlockState processing finished {} {}", block_number, block_hash);
        }
    }
}
