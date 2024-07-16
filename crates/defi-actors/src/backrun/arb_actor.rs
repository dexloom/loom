use std::marker::PhantomData;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;
use async_trait::async_trait;
use log::info;
use tokio::task::JoinHandle;

use debug_provider::DebugProviderExt;
use defi_entities::{BlockHistory, LatestBlock, Market, MarketState};
use defi_events::{MarketEvents, MempoolEvents, MessageHealthEvent, MessageTxCompose};
use defi_types::Mempool;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

use crate::backrun::block_state_change_processor::BlockStateChangeProcessorActor;

use super::{PendingTxStateChangeProcessorActor, StateChangeArbSearcherActor};

#[derive(Accessor, Consumer, Producer)]
pub struct StateChangeArbActor<P, T, N> {
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
    market_state: Option<SharedState<MarketState>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory>>,
    #[consumer]
    mempool_events_tx: Option<Broadcaster<MempoolEvents>>,
    #[consumer]
    market_events_tx: Option<Broadcaster<MarketEvents>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    pool_health_monitor_tx: Option<Broadcaster<MessageHealthEvent>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> StateChangeArbActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P, use_blocks: bool, use_mempool: bool) -> StateChangeArbActor<P, T, N> {
        StateChangeArbActor {
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

#[async_trait]
impl<P, T, N> Actor for StateChangeArbActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    async fn start(&self) -> ActorResult {
        let searcher_pool_update_channel = Broadcaster::new(100);
        let mut tasks: Vec<JoinHandle<WorkerResult>> = Vec::new();

        let mut state_update_searcher = StateChangeArbSearcherActor::new(true);
        match state_update_searcher
            .access(self.market.clone().unwrap())
            .consume(searcher_pool_update_channel.clone())
            .produce(self.compose_channel_tx.clone().unwrap())
            .produce(self.pool_health_monitor_tx.clone().unwrap())
            .start()
            .await
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
                .await
            {
                Err(e) => {
                    panic!("{e}")
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
                .await
            {
                Err(e) => {
                    panic!("{e}")
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
