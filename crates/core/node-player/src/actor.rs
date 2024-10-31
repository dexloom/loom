use std::any::type_name;
use std::marker::PhantomData;

use crate::compose::replayer_compose_worker;
use crate::worker::node_player_worker;
use alloy_network::{Ethereum, Network};
use alloy_primitives::BlockNumber;
use alloy_provider::Provider;
use alloy_transport::Transport;
use loom_core_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::Blockchain;
use loom_defi_entities::MarketState;
use loom_defi_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate, MessageTxCompose};
use loom_defi_types::Mempool;
use loom_node_debug_provider::{DebugProviderExt, HttpCachedTransport};
use tokio::task::JoinHandle;

#[derive(Producer, Consumer, Accessor)]
pub struct NodeBlockPlayerActor<P, T, N> {
    client: P,
    start_block: BlockNumber,
    end_block: BlockNumber,
    #[accessor]
    mempool: Option<SharedState<Mempool>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[consumer]
    compose_channel: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    block_header_channel: Option<Broadcaster<MessageBlockHeader>>,
    #[producer]
    block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> NodeBlockPlayerActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, start_block: BlockNumber, end_block: BlockNumber) -> NodeBlockPlayerActor<P, T, N> {
        NodeBlockPlayerActor {
            client,
            start_block,
            end_block,
            mempool: None,
            market_state: None,
            compose_channel: None,
            block_header_channel: None,
            block_with_tx_channel: None,
            block_logs_channel: None,
            block_state_update_channel: None,
            _t: PhantomData,
            _n: PhantomData,
        }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            mempool: Some(bc.mempool()),
            market_state: Some(bc.market_state()),
            compose_channel: Some(bc.compose_channel()),
            block_header_channel: Some(bc.new_block_headers_channel()),
            block_with_tx_channel: Some(bc.new_block_with_tx_channel()),
            block_logs_channel: Some(bc.new_block_logs_channel()),
            block_state_update_channel: Some(bc.new_block_state_update_channel()),
            ..self
        }
    }
}

impl<P, T, N> Actor for NodeBlockPlayerActor<P, T, N>
where
    P: Provider<HttpCachedTransport, Ethereum> + DebugProviderExt<HttpCachedTransport, Ethereum> + Send + Sync + Clone + 'static,
    T: Send + Sync,
    N: Send + Sync,
{
    fn start(&self) -> ActorResult {
        let mut handles: Vec<JoinHandle<WorkerResult>> = Vec::new();
        if let Some(mempool) = self.mempool.clone() {
            if let Some(compose_channel) = self.compose_channel.clone() {
                let handle = tokio::task::spawn(replayer_compose_worker(mempool, compose_channel));
                handles.push(handle);
            }
        }

        let handle = tokio::task::spawn(node_player_worker(
            self.client.clone(),
            self.start_block,
            self.end_block,
            self.mempool.clone(),
            self.market_state.clone(),
            self.block_header_channel.clone(),
            self.block_with_tx_channel.clone(),
            self.block_logs_channel.clone(),
            self.block_state_update_channel.clone(),
        ));
        handles.push(handle);
        Ok(handles)
    }

    fn name(&self) -> &'static str {
        type_name::<Self>().rsplit("::").next().unwrap_or(type_name::<Self>())
    }
}
