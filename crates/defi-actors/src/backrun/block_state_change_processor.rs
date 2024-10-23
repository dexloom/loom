use super::affected_pools::get_affected_pools;
use defi_blockchain::Blockchain;
use defi_entities::{BlockHistory, Market};
use defi_events::{MarketEvents, StateUpdateEvent};
use defi_types::ChainParameters;
use eyre::eyre;
use loom_actors::{run_async, subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use tokio::sync::broadcast::error::RecvError;
use tracing::error;

pub async fn block_state_change_worker(
    chain_parameters: ChainParameters,
    market: SharedState<Market>,
    block_history: SharedState<BlockHistory>,
    market_events_rx: Broadcaster<MarketEvents>,
    state_updates_broadcaster: Broadcaster<StateUpdateEvent>,
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

        let Some(block_history_entry) = block_history.read().await.get_entry(&block_hash).cloned() else {
            error!("Block not found in block history: {:?}", block_hash);
            continue;
        };
        let Some(state_update) = block_history_entry.state_update.clone() else {
            error!("Block {:?} has no state update", block_hash);
            continue;
        };

        let affected_pools = match get_affected_pools(market.clone(), &state_update).await {
            Ok(affected_pools) => affected_pools,
            Err(e) => {
                error!("Could not get affected pools for block {:?}: {}", block_hash, e);
                continue;
            }
        };

        let Some(cur_state) = block_history_entry.state_db.clone() else {
            error!("Block {:?} has no state db", block_hash);
            continue;
        };

        let next_block = block_history_entry.number() + 1;
        let next_block_timestamp = block_history_entry.timestamp() + 12;
        let next_base_fee = chain_parameters.calc_next_block_base_fee_from_header(&block_history_entry.header);

        let request = StateUpdateEvent::new(
            next_block,
            next_block_timestamp,
            next_base_fee,
            cur_state,
            state_update,
            None,
            affected_pools,
            Vec::new(),
            Vec::new(),
            "block_searcher".to_string(),
            9000,
        );
        run_async!(state_updates_broadcaster.send(request));
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct BlockStateChangeProcessorActor {
    chain_parameters: ChainParameters,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
    #[producer]
    state_updates_tx: Option<Broadcaster<StateUpdateEvent>>,
}

impl BlockStateChangeProcessorActor {
    pub fn new() -> BlockStateChangeProcessorActor {
        BlockStateChangeProcessorActor {
            chain_parameters: ChainParameters::ethereum(),
            market: None,
            block_history: None,
            market_events_rx: None,
            state_updates_tx: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            chain_parameters: bc.chain_parameters(),
            market: Some(bc.market()),
            block_history: Some(bc.block_history()),
            market_events_rx: Some(bc.market_events_channel()),
            state_updates_tx: Some(bc.state_update_channel()),
        }
    }
}

impl Default for BlockStateChangeProcessorActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for BlockStateChangeProcessorActor {
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
