use async_trait::async_trait;
use log::{debug, error};
use tokio::sync::broadcast::Receiver;

use defi_blockchain::Blockchain;
use defi_entities::{BlockHistory, Market};
use defi_events::{MarketEvents, StateUpdateEvent};
use loom_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

use super::affected_pools::get_affected_pools;

pub async fn block_state_change_worker(
    market: SharedState<Market>,
    block_history: SharedState<BlockHistory>,
    market_events_rx: Broadcaster<MarketEvents>,
    state_updates_broadcaster: Broadcaster<StateUpdateEvent>,
) -> WorkerResult {
    let mut next_block_base_fee = 0;
    subscribe!(market_events_rx);

    loop {
        tokio::select! {
            market_event_msg = market_events_rx.recv() => {
                match market_event_msg {
                    Ok(market_event) =>{
                        match market_event {
                            MarketEvents::BlockHeaderUpdate{block_number, block_hash, timestamp, base_fee, next_base_fee } =>{
                                debug!("Block header update {} {} ts {} base_fee {} next {} ", block_number, block_hash, timestamp, base_fee, next_base_fee);
                                /*block_number = block_number;
                                block_hash = block_hash;
                                block_time = new_block_time;
                                base_fee = new_base_fee;

                                 */
                                next_block_base_fee = next_base_fee;
                            }




                            MarketEvents::BlockStateUpdate{ block_hash } => {
                                if let Some(block_history_update) = block_history.read().await.get_market_history_entry(&block_hash).cloned() {
                                    if let Some(state_update) = block_history_update.state_update {
                                        if let Some(block_header) = block_history_update.header {
                                            let affected_pools = get_affected_pools(market.clone(), &state_update).await?;

                                            let cur_state = block_history_update.state_db.unwrap().clone();


                                            let block = block_header.number.unwrap() + 1;

                                            let block_timestamp = block_header.timestamp + 12;
                                            let next_base_fee= next_block_base_fee;
                                            //let next_base_fee : U256 = block_header.next_block_base_fee().unwrap_or_default().convert();


                                            let request = StateUpdateEvent::new(
                                                block,
                                                block_timestamp,
                                                next_base_fee,
                                                cur_state,
                                                state_update,
                                                None,
                                                affected_pools,
                                                Vec::new(),
                                                Vec::new(),
                                                "block_searcher".to_string(),
                                                9000
                                            );
                                            if let Err(e) = state_updates_broadcaster.send(request).await {
                                                error!("{}", e)
                                            }
                                        }
                                    }
                                }
                            }
                            _=>{}
                        }
                    }
                    Err(e)=>{
                        error!("{}",e)
                    }
                }
            }
        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct BlockStateChangeProcessorActor {
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
        BlockStateChangeProcessorActor { market: None, block_history: None, market_events_rx: None, state_updates_tx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            market: Some(bc.market()),
            block_history: Some(bc.block_history()),
            market_events_rx: Some(bc.market_events_channel()),
            state_updates_tx: Some(bc.state_update_channel()),
        }
    }
}

#[async_trait]
impl Actor for BlockStateChangeProcessorActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(block_state_change_worker(
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
