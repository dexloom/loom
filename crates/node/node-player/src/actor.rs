use revm::{Database, DatabaseCommit, DatabaseRef};
use std::any::type_name;
use std::marker::PhantomData;

use crate::compose::replayer_compose_worker;
use crate::worker::node_player_worker;
use alloy_network::{Ethereum, Network};
use alloy_primitives::BlockNumber;
use alloy_provider::Provider;
use eyre::ErrReport;
use loom_core_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_evm_db::DatabaseLoomExt;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_blockchain::Mempool;
use loom_types_entities::MarketState;
use loom_types_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate, MessageTxCompose};
use tokio::task::JoinHandle;

#[derive(Producer, Consumer, Accessor)]
pub struct NodeBlockPlayerActor<P, N, DB: Send + Sync + Clone + 'static> {
    client: P,
    start_block: BlockNumber,
    end_block: BlockNumber,
    #[accessor]
    mempool: Option<SharedState<Mempool>>,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    #[consumer]
    compose_channel: Option<Broadcaster<MessageTxCompose<LoomDataTypesEthereum>>>,
    #[producer]
    block_header_channel: Option<Broadcaster<MessageBlockHeader>>,
    #[producer]
    block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
    _n: PhantomData<N>,
}

impl<P, N, DB> NodeBlockPlayerActor<P, N, DB>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: Database<Error = ErrReport> + DatabaseRef<Error = ErrReport> + DatabaseCommit + DatabaseLoomExt + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, start_block: BlockNumber, end_block: BlockNumber) -> NodeBlockPlayerActor<P, N, DB> {
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
            _n: PhantomData,
        }
    }

    pub fn on_bc(self, bc: &Blockchain, state: &BlockchainState<DB>) -> Self {
        Self {
            mempool: Some(bc.mempool()),
            block_header_channel: Some(bc.new_block_headers_channel()),
            block_with_tx_channel: Some(bc.new_block_with_tx_channel()),
            block_logs_channel: Some(bc.new_block_logs_channel()),
            block_state_update_channel: Some(bc.new_block_state_update_channel()),
            market_state: Some(state.market_state_commit()),
            compose_channel: Some(bc.tx_compose_channel()),
            ..self
        }
    }
}

impl<P, N, DB> Actor for NodeBlockPlayerActor<P, N, DB>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
    N: Send + Sync,
    DB: Database<Error = ErrReport> + DatabaseRef<Error = ErrReport> + DatabaseCommit + DatabaseLoomExt + Send + Sync + Clone + 'static,
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
