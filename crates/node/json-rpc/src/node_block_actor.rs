use alloy_network::{Ethereum, Network};
use alloy_provider::Provider;
use alloy_rpc_types::Header;
use op_alloy::network::Optimism;
use std::marker::PhantomData;
use tokio::task::JoinHandle;

use crate::eth::new_eth_node_block_workers_starter;
use crate::op::new_op_node_block_workers_starter;
use loom_core_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_core_actors_macros::Producer;
use loom_core_blockchain::Blockchain;
use loom_node_actor_config::NodeBlockActorConfig;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum, LoomDataTypesOptimism};
use loom_types_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate};

#[derive(Producer)]
pub struct NodeBlockActor<P, N, LDT: LoomDataTypes + 'static> {
    client: P,
    config: NodeBlockActorConfig,
    #[producer]
    block_header_channel: Option<Broadcaster<MessageBlockHeader<LDT>>>,
    #[producer]
    block_with_tx_channel: Option<Broadcaster<MessageBlock<LDT>>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<MessageBlockLogs<LDT>>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate<LDT>>>,
    _n: PhantomData<N>,
}

impl<P, N, LDT> NodeBlockActor<P, N, LDT>
where
    LDT: LoomDataTypes,
    N: Network,
    P: Provider<N> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
{
    fn name(&self) -> &'static str {
        "NodeBlockActor"
    }

    pub fn new(client: P, config: NodeBlockActorConfig) -> NodeBlockActor<P, Ethereum, LoomDataTypesEthereum> {
        NodeBlockActor {
            client,
            config,
            block_header_channel: None,
            block_with_tx_channel: None,
            block_logs_channel: None,
            block_state_update_channel: None,
            _n: PhantomData,
        }
    }

    pub fn on_bc(self, bc: &Blockchain<LDT>) -> Self {
        Self {
            block_header_channel: if self.config.block_header { Some(bc.new_block_headers_channel()) } else { None },
            block_with_tx_channel: if self.config.block_with_tx { Some(bc.new_block_with_tx_channel()) } else { None },
            block_logs_channel: if self.config.block_logs { Some(bc.new_block_logs_channel()) } else { None },
            block_state_update_channel: if self.config.block_state_update { Some(bc.new_block_state_update_channel()) } else { None },
            ..self
        }
    }
}

impl<P> Actor for NodeBlockActor<P, Ethereum, LoomDataTypesEthereum>
where
    P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        new_eth_node_block_workers_starter(
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

impl<P> Actor for NodeBlockActor<P, Optimism, LoomDataTypesOptimism>
where
    P: Provider<Optimism> + DebugProviderExt + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        new_op_node_block_workers_starter(
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
