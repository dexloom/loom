use std::marker::PhantomData;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;
use eyre::ErrReport;
use revm::{Database, DatabaseCommit, DatabaseRef};
use tokio::task::JoinHandle;
use tracing::info;

use loom_core_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::Mempool;
use loom_types_entities::{BlockHistory, LatestBlock, Market, MarketState};
use loom_types_events::{MarketEvents, MempoolEvents, MessageBackrunTxCompose, MessageHealthEvent};

use super::{PendingTxStateChangeProcessorActor, StateChangeArbSearcherActor};
use crate::block_state_change_processor::BlockStateChangeProcessorActor;
use crate::BackrunConfig;

#[derive(Accessor, Consumer, Producer)]
pub struct StateChangeArbActor<P, T, N, DB: Clone + Send + Sync + 'static> {
    backrun_config: BackrunConfig,
    client: P,
    use_blocks: bool,
    use_mempool: bool,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    mempool: Option<SharedState<Mempool>>,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory<DB>>>,
    #[consumer]
    mempool_events_tx: Option<Broadcaster<MempoolEvents>>,
    #[consumer]
    market_events_tx: Option<Broadcaster<MarketEvents>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageBackrunTxCompose<DB>>>,
    #[producer]
    pool_health_monitor_tx: Option<Broadcaster<MessageHealthEvent>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N, DB> StateChangeArbActor<P, T, N, DB>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, use_blocks: bool, use_mempool: bool, backrun_config: BackrunConfig) -> StateChangeArbActor<P, T, N, DB> {
        StateChangeArbActor {
            backrun_config,
            client,
            use_blocks,
            use_mempool,
            market: None,
            mempool: None,
            latest_block: None,
            block_history: None,
            market_state: None,
            mempool_events_tx: None,
            market_events_tx: None,
            compose_channel_tx: None,
            pool_health_monitor_tx: None,
            _t: PhantomData,
            _n: PhantomData,
        }
    }
}

impl<P, T, N, DB> Actor for StateChangeArbActor<P, T, N, DB>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef<Error = ErrReport> + Database<Error = ErrReport> + DatabaseCommit + Send + Sync + Clone + Default + 'static,
{
    fn start(&self) -> ActorResult {
        let searcher_pool_update_channel = Broadcaster::new(100);
        let mut tasks: Vec<JoinHandle<WorkerResult>> = Vec::new();

        let mut state_update_searcher = StateChangeArbSearcherActor::new(self.backrun_config.clone());
        match state_update_searcher
            .access(self.market.clone().unwrap())
            .consume(searcher_pool_update_channel.clone())
            .produce(self.compose_channel_tx.clone().unwrap())
            .produce(self.pool_health_monitor_tx.clone().unwrap())
            .start()
        {
            Err(e) => {
                panic!("{}", e)
            }
            Ok(r) => {
                tasks.extend(r);
                info!("State change searcher actor started successfully")
            }
        }

        if self.mempool_events_tx.is_some() && self.use_mempool {
            let mut pending_tx_state_processor = PendingTxStateChangeProcessorActor::new(self.client.clone());
            match pending_tx_state_processor
                .access(self.mempool.clone().unwrap())
                .access(self.latest_block.clone().unwrap())
                .access(self.market.clone().unwrap())
                .access(self.market_state.clone().unwrap())
                .consume(self.mempool_events_tx.clone().unwrap())
                .consume(self.market_events_tx.clone().unwrap())
                .produce(searcher_pool_update_channel.clone())
                .start()
            {
                Err(e) => {
                    panic!("{}", e)
                }
                Ok(r) => {
                    tasks.extend(r);
                    info!("Pending tx state actor started successfully")
                }
            }
        }

        if self.market_events_tx.is_some() && self.use_blocks {
            let mut block_state_processor = BlockStateChangeProcessorActor::new();
            match block_state_processor
                .access(self.market.clone().unwrap())
                .access(self.block_history.clone().unwrap())
                .consume(self.market_events_tx.clone().unwrap())
                .produce(searcher_pool_update_channel.clone())
                .start()
            {
                Err(e) => {
                    panic!("{}", e)
                }
                Ok(r) => {
                    tasks.extend(r);
                    info!("Block change state actor started successfully")
                }
            }
        }

        Ok(tasks)
    }

    fn name(&self) -> &'static str {
        "StateChangeArbActor"
    }
}
