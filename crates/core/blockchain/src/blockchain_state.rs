use loom_core_actors::SharedState;
use loom_evm_db::DatabaseLoomExt;
use loom_types_blockchain::LoomDataTypes;
use loom_types_entities::{BlockHistory, BlockHistoryState, MarketState};
use revm::{Database, DatabaseCommit, DatabaseRef};

#[derive(Clone)]
pub struct BlockchainState<DB: Clone + Send + Sync + 'static, LDT: LoomDataTypes> {
    market_state: SharedState<MarketState<DB>>,
    block_history_state: SharedState<BlockHistory<DB, LDT>>,
}

impl<
        DB: DatabaseRef + Database + DatabaseCommit + BlockHistoryState<LDT> + DatabaseLoomExt + Send + Sync + Clone + Default + 'static,
        LDT: LoomDataTypes,
    > Default for BlockchainState<DB, LDT>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<
        DB: DatabaseRef + Database + DatabaseCommit + BlockHistoryState<LDT> + DatabaseLoomExt + Send + Sync + Clone + Default + 'static,
        LDT: LoomDataTypes,
    > BlockchainState<DB, LDT>
{
    pub fn new() -> Self {
        BlockchainState {
            market_state: SharedState::new(MarketState::new(DB::default())),
            block_history_state: SharedState::new(BlockHistory::<DB, LDT>::new(10)),
        }
    }

    pub fn new_with_market_state(market_state: MarketState<DB>) -> Self {
        Self { market_state: SharedState::new(market_state), block_history_state: SharedState::new(BlockHistory::new(10)) }
    }

    pub fn with_market_state(self, market_state: MarketState<DB>) -> BlockchainState<DB, LDT> {
        BlockchainState { market_state: SharedState::new(market_state), ..self.clone() }
    }
}

impl<DB: Clone + Send + Sync, LDT: LoomDataTypes> BlockchainState<DB, LDT> {
    pub fn market_state_commit(&self) -> SharedState<MarketState<DB>> {
        self.market_state.clone()
    }

    pub fn market_state(&self) -> SharedState<MarketState<DB>> {
        self.market_state.clone()
    }

    pub fn block_history(&self) -> SharedState<BlockHistory<DB, LDT>> {
        self.block_history_state.clone()
    }
}
