use std::any::type_name;
use std::marker::PhantomData;

use crate::node_player::compose::replayer_compose_worker;
use crate::node_player::worker::node_player_worker;
use alloy_network::{Ethereum, Network};
use alloy_primitives::BlockNumber;
use alloy_provider::Provider;
use alloy_rpc_types::Block;
use alloy_transport::Transport;
use debug_provider::{DebugProviderExt, HttpCachedTransport};
use defi_blockchain::Blockchain;
use defi_entities::MarketState;
use defi_events::{BlockLogs, BlockStateUpdate, MessageBlockHeader, MessageTxCompose};
use defi_types::{ChainParameters, Mempool};
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use tokio::task::JoinHandle;

#[derive(Producer, Consumer, Accessor)]
pub struct NodeBlockPlayerActor<P, T, N> {
    client: P,
    chain_parameters: ChainParameters,
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
    block_with_tx_channel: Option<Broadcaster<Block>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<BlockLogs>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<BlockStateUpdate>>,
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
            chain_parameters: ChainParameters::ethereum(),
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
            chain_parameters: bc.chain_parameters(),
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
            self.chain_parameters.clone(),
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
