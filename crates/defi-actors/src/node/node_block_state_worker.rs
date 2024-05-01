use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use alloy_primitives::{Address, BlockHash, TxHash};
use alloy_provider::Provider;
use alloy_rpc_types::{BlockId, RpcBlockHash};
use eyre::Result;
use log::error;
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

use defi_events::BlockStateUpdate;
use defi_types::debug_trace_block;
use loom_actors::{Broadcaster, WorkerResult};

pub async fn new_node_block_state_worker<P>(
    client: P,
    mut block_hash_receiver: Receiver<BlockHash>,
    sender: Broadcaster<BlockStateUpdate>,
) -> WorkerResult
    where P: Provider + Send + Sync + Clone + 'static
{
    loop {
        if let Ok(block_hash) = block_hash_receiver.recv().await {
            let trace_result = debug_trace_block(client.clone(), BlockId::Hash(block_hash.into()), true).await;
            match trace_result {
                Ok((pre, post)) => {
                    match sender.send(BlockStateUpdate { block_hash, state_update: post }).await {
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

