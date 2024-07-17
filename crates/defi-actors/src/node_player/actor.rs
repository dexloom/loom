use std::any::type_name;
use std::marker::PhantomData;

use alloy_network::{Ethereum, Network};
use alloy_primitives::BlockNumber;
use alloy_provider::Provider;
use alloy_rpc_types::{Block, Header};
use alloy_transport::Transport;
use async_trait::async_trait;

use debug_provider::{DebugProviderExt, HttpCachedTransport};
use defi_blockchain::Blockchain;
use defi_events::{BlockLogs, BlockStateUpdate};
use loom_actors::{Actor, ActorResult, Broadcaster, Producer};
use loom_actors_macros::Producer;

use crate::node_player::worker::node_player_worker;

#[derive(Producer)]
pub struct NodeBlockPlayerActor<P, T, N> {
    client: P,
    start_block: BlockNumber,
    end_block: BlockNumber,
    #[producer]
    block_header_channel: Option<Broadcaster<Header>>,
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
            start_block,
            end_block,
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
            block_header_channel: Some(bc.new_block_headers_channel()),
            block_with_tx_channel: Some(bc.new_block_with_tx_channel()),
            block_logs_channel: Some(bc.new_block_logs_channel()),
            block_state_update_channel: Some(bc.new_block_state_update_channel()),

            ..self
        }
    }
}

#[async_trait]
impl<P, T, N> Actor for NodeBlockPlayerActor<P, T, N>
where
    P: Provider<HttpCachedTransport, Ethereum> + DebugProviderExt<HttpCachedTransport, Ethereum> + Send + Sync + Clone + 'static,
    T: Send + Sync,
    N: Send + Sync,
{
    async fn start(&self) -> ActorResult {
        let handler = tokio::task::spawn(node_player_worker(
            self.client.clone(),
            self.start_block,
            self.end_block,
            self.block_header_channel.clone(),
            self.block_with_tx_channel.clone(),
            self.block_logs_channel.clone(),
            self.block_state_update_channel.clone(),
        ));
        Ok(vec![handler])
    }

    fn name(&self) -> &'static str {
        type_name::<Self>().rsplit("::").next().unwrap_or(type_name::<Self>())
    }
}
