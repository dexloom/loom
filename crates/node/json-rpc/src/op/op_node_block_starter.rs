use crate::op::node_block_header_worker::new_op_node_block_header_worker;
use alloy_provider::Provider;
use alloy_rpc_types::Header;
use loom_core_actors::{ActorResult, Broadcaster, WorkerResult};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::LoomDataTypesOptimism;
use loom_types_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate};
use op_alloy::network::Optimism;
use tokio::task::JoinHandle;

pub fn new_op_node_block_workers_starter<P>(
    client: P,
    new_block_headers_channel: Option<Broadcaster<MessageBlockHeader<LoomDataTypesOptimism>>>,
    new_block_with_tx_channel: Option<Broadcaster<MessageBlock<LoomDataTypesOptimism>>>,
    new_block_logs_channel: Option<Broadcaster<MessageBlockLogs<LoomDataTypesOptimism>>>,
    new_block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate<LoomDataTypesOptimism>>>,
) -> ActorResult
where
    P: Provider<Optimism> + DebugProviderExt + Send + Sync + Clone + 'static,
{
    let new_header_internal_channel: Broadcaster<Header> = Broadcaster::new(10);

    let mut tasks: Vec<JoinHandle<WorkerResult>> = Vec::new();

    // if let Some(channel) = new_block_with_tx_channel {
    //     tasks.push(tokio::task::spawn(crate::eth::node_block_with_tx_worker::new_block_with_tx_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    // }
    //
    if let Some(channel) = new_block_headers_channel {
        tasks.push(tokio::task::spawn(new_op_node_block_header_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    }
    //
    // if let Some(channel) = new_block_logs_channel {
    //     tasks.push(tokio::task::spawn(crate::eth::node_block_logs_worker::new_node_block_logs_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    // }
    //
    // if let Some(channel) = new_block_state_update_channel {
    //     tasks.push(tokio::task::spawn(crate::eth::node_block_state_worker::new_node_block_state_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    // }

    Ok(tasks)
}
