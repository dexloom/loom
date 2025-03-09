use super::affected_pools_state::get_affected_pools_from_state_update;
use eyre::eyre;
use loom_core_actors::{run_sync, subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::{Blockchain, BlockchainState, Strategy};
use loom_types_blockchain::{ChainParameters, LoomDataTypes};
use loom_types_blockchain::{LoomDataTypesEVM, LoomDataTypesEthereum};
use loom_types_entities::{BlockHistory, Market};
use loom_types_events::{MarketEvents, StateUpdateEvent};
use revm::DatabaseRef;
use tokio::sync::broadcast::error::RecvError;
use tracing::error;

pub async fn block_state_change_worker<DB: DatabaseRef + Send + Sync + Clone + 'static, LDT: LoomDataTypesEVM>(
    chain_parameters: ChainParameters,
    market: SharedState<Market>,
    block_history: SharedState<BlockHistory<DB, LDT>>,
    market_events_rx: Broadcaster<MarketEvents<LDT>>,
    state_updates_broadcaster: Broadcaster<StateUpdateEvent<DB, LDT>>,
) -> WorkerResult {
    subscribe!(market_events_rx);

    loop {
        let market_event = match market_events_rx.recv().await {
            Ok(market_event) => market_event,
            Err(e) => match e {
                RecvError::Closed => {
                    error!("Market events txs channel closed");
                    break Err(eyre!("MARKET_EVENTS_RX_CLOSED"));
                }
                RecvError::Lagged(lag) => {
                    error!("Market events txs channel lagged by {} messages", lag);
                    continue;
                }
            },
        };
        let block_hash = match market_event {
            MarketEvents::BlockStateUpdate { block_hash } => block_hash,
            _ => continue,
        };

        let Some(block_history_entry) = block_history.read().await.get_block_history_entry(&block_hash).cloned() else {
            error!("Block history entry not found in block history: {:?}", block_hash);
            continue;
        };

        let Some(block_state_entry) = block_history.read().await.get_block_state(&block_hash).cloned() else {
            error!("Block state not found in block history: {:?}", block_hash);
            continue;
        };

        let Some(state_update) = block_history_entry.state_update.clone() else {
            error!("Block {:?} has no state update", block_hash);
            continue;
        };

        let affected_pools = get_affected_pools_from_state_update(market.clone(), &state_update).await;

        if affected_pools.is_empty() {
            error!("Could not get affected pools for block {:?}", block_hash);
            continue;
        };

        let next_block_number = block_history_entry.number() + 1;
        let next_block_timestamp = block_history_entry.timestamp() + 12;
        let next_base_fee = chain_parameters.calc_next_block_base_fee_from_header(&block_history_entry.header);

        let request = StateUpdateEvent::new(
            next_block_number,
            next_block_timestamp,
            next_base_fee,
            block_state_entry,
            state_update,
            None,
            affected_pools,
            Vec::new(),
            Vec::new(),
            "block_searcher".to_string(),
            90_00,
        );
        run_sync!(state_updates_broadcaster.send(request));
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct BlockStateChangeProcessorActor<DB: Clone + Send + Sync + 'static, LDT: LoomDataTypesEVM + 'static> {
    chain_parameters: ChainParameters,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory<DB, LDT>>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents<LDT>>>,
    #[producer]
    state_updates_tx: Option<Broadcaster<StateUpdateEvent<DB, LDT>>>,
}

impl<DB: DatabaseRef + Send + Sync + Clone + 'static, LDT: LoomDataTypesEVM> BlockStateChangeProcessorActor<DB, LDT> {
    pub fn new() -> BlockStateChangeProcessorActor<DB, LDT> {
        BlockStateChangeProcessorActor {
            chain_parameters: ChainParameters::ethereum(),
            market: None,
            block_history: None,
            market_events_rx: None,
            state_updates_tx: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain<LDT>, state: &BlockchainState<DB, LDT>, strategy: &Strategy<DB, LDT>) -> Self {
        Self {
            chain_parameters: bc.chain_parameters(),
            market: Some(bc.market()),
            market_events_rx: Some(bc.market_events_channel()),
            state_updates_tx: Some(strategy.state_update_channel()),
            block_history: Some(state.block_history()),
        }
    }
}

impl<DB: DatabaseRef + Send + Sync + Clone + 'static, LDT: LoomDataTypesEVM> Default for BlockStateChangeProcessorActor<DB, LDT> {
    fn default() -> Self {
        Self::new()
    }
}

impl<DB: DatabaseRef + Send + Sync + Clone + 'static, LDT: LoomDataTypesEVM> Actor for BlockStateChangeProcessorActor<DB, LDT> {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(block_state_change_worker(
            self.chain_parameters.clone(),
            self.market.clone().unwrap(),
            self.block_history.clone().unwrap(),
            self.market_events_rx.clone().unwrap(),
            self.state_updates_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "BlockStateChangeProcessorActor"
    }
}
