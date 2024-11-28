use loom_core_actors::SharedState;
use loom_evm_db::DatabaseLoomExt;
use loom_types_entities::{BlockHistory, BlockHistoryState, MarketState};
use revm::{Database, DatabaseCommit, DatabaseRef};

#[derive(Clone)]
pub struct BlockchainState<DB: Clone + Send + Sync + 'static> {
    market_state: SharedState<MarketState<DB>>,
    block_history_state: SharedState<BlockHistory<DB>>,
}

impl<DB: DatabaseRef + Database + DatabaseCommit + BlockHistoryState + DatabaseLoomExt + Send + Sync + Clone + Default + 'static> Default
    for BlockchainState<DB>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<DB: DatabaseRef + Database + DatabaseCommit + BlockHistoryState + DatabaseLoomExt + Send + Sync + Clone + Default + 'static>
    BlockchainState<DB>
{
    pub fn new() -> Self {
        BlockchainState {
            market_state: SharedState::new(MarketState::new(DB::default())),
            block_history_state: SharedState::new(BlockHistory::new(10)),
        }
    }

    pub fn new_with_market_state(market_state: MarketState<DB>) -> Self {
        Self { market_state: SharedState::new(market_state), block_history_state: SharedState::new(BlockHistory::new(10)) }
    }

    pub fn with_market_state(self, market_state: MarketState<DB>) -> BlockchainState<DB> {
        BlockchainState { market_state: SharedState::new(market_state), ..self.clone() }
    }
}

impl<DB: Clone + Send + Sync> BlockchainState<DB> {
    pub fn market_state_commit(&self) -> SharedState<MarketState<DB>> {
        self.market_state.clone()
    }

    pub fn market_state(&self) -> SharedState<MarketState<DB>> {
        self.market_state.clone()
    }

    pub fn block_history(&self) -> SharedState<BlockHistory<DB>> {
        self.block_history_state.clone()
    }
}
