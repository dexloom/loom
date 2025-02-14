use alloy_network::Ethereum;
use alloy_provider::Provider;
use tokio::task::JoinHandle;

use crate::node_block_hash_worker::new_node_block_header_worker;
use crate::node_block_logs_worker::new_node_block_logs_worker;
use crate::node_block_state_worker::new_node_block_state_worker;
use crate::node_block_with_tx_worker::new_block_with_tx_worker;
use loom_core_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_core_actors_macros::Producer;
use loom_core_blockchain::Blockchain;
use loom_node_actor_config::NodeBlockActorConfig;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate};

pub fn new_node_block_workers_starter<P>(
    client: P,
    new_block_headers_channel: Option<Broadcaster<MessageBlockHeader>>,
    new_block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    new_block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    new_block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
) -> ActorResult
where
    P: Provider<Ethereum> + DebugProviderExt + Send + Sync + Clone + 'static,
{
    let new_header_internal_channel = Broadcaster::new(10);
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

#[derive(Producer)]
pub struct NodeBlockActor<P> {
    client: P,
    config: NodeBlockActorConfig,
    #[producer]
    block_header_channel: Option<Broadcaster<MessageBlockHeader>>,
    #[producer]
    block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
}

impl<P> NodeBlockActor<P>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
{
    fn name(&self) -> &'static str {
        "NodeBlockActor"
    }

    pub fn new(client: P, config: NodeBlockActorConfig) -> NodeBlockActor<P> {
        NodeBlockActor {
            client,
            config,
            block_header_channel: None,
            block_with_tx_channel: None,
            block_logs_channel: None,
            block_state_update_channel: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain<LoomDataTypesEthereum>) -> Self {
        Self {
            block_header_channel: if self.config.block_header { Some(bc.new_block_headers_channel()) } else { None },
            block_with_tx_channel: if self.config.block_with_tx { Some(bc.new_block_with_tx_channel()) } else { None },
            block_logs_channel: if self.config.block_logs { Some(bc.new_block_logs_channel()) } else { None },
            block_state_update_channel: if self.config.block_state_update { Some(bc.new_block_state_update_channel()) } else { None },
            ..self
        }
    }
}

impl<P> Actor for NodeBlockActor<P>
where
    P: Provider<Ethereum> + DebugProviderExt + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        new_node_block_workers_starter(
            self.client.clone(),
            self.block_header_channel.clone(),
            self.block_with_tx_channel.clone(),
            self.block_logs_channel.clone(),
            self.block_state_update_channel.clone(),
        )
    }
    fn name(&self) -> &'static str {
        self.name()
    }
}
