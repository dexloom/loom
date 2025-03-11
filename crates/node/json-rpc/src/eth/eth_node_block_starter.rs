use crate::eth::node_block_header_worker::new_node_block_header_worker;
use crate::eth::node_block_logs_worker::new_node_block_logs_worker;
use crate::eth::node_block_state_worker::new_node_block_state_worker;
use crate::eth::node_block_with_tx_worker::new_block_with_tx_worker;
use alloy_json_rpc::RpcRecv;
use alloy_network::{BlockResponse, Ethereum, Network};
use alloy_provider::Provider;
use alloy_rpc_types::Header;
use loom_core_actors::{ActorResult, Broadcaster, WorkerResult};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::LoomDataTypesEVM;
use loom_types_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate};
use tokio::task::JoinHandle;

pub fn new_eth_node_block_workers_starter<P, N, LDT>(
    client: P,
    new_block_headers_channel: Option<Broadcaster<MessageBlockHeader<LDT>>>,
    new_block_with_tx_channel: Option<Broadcaster<MessageBlock<LDT>>>,
    new_block_logs_channel: Option<Broadcaster<MessageBlockLogs<LDT>>>,
    new_block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate<LDT>>>,
) -> ActorResult
where
    N: Network<HeaderResponse = LDT::Header, BlockResponse = LDT::Block>,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    LDT: LoomDataTypesEVM,
    LDT::Block: RpcRecv + BlockResponse,
{
    let new_header_internal_channel: Broadcaster<Header> = Broadcaster::new(10);
    let mut tasks: Vec<JoinHandle<WorkerResult>> = Vec::new();

    if let Some(channel) = new_block_with_tx_channel {
        tasks.push(tokio::task::spawn(new_block_with_tx_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    }

    if let Some(channel) = new_block_headers_channel {
        tasks.push(tokio::task::spawn(new_node_block_header_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    }

    if let Some(channel) = new_block_logs_channel {
        tasks.push(tokio::task::spawn(new_node_block_logs_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    }

    if let Some(channel) = new_block_state_update_channel {
        tasks.push(tokio::task::spawn(new_node_block_state_worker(client.clone(), new_header_internal_channel.clone(), channel)));
    }

    Ok(tasks)
}
