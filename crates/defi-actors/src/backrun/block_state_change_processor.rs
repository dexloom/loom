use async_trait::async_trait;
use log::{debug, error};
use tokio::sync::broadcast::Receiver;

use defi_entities::{BlockHistory, Market};
use defi_events::MarketEvents;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

use super::affected_pools::get_affected_pools;
use super::messages::MessageSearcherPoolStateUpdate;

pub async fn block_state_change_worker(
    market: SharedState<Market>,
    block_history: SharedState<BlockHistory>,
    mut market_events_rx: Receiver<MarketEvents>,
    state_updates_broadcaster: Broadcaster<MessageSearcherPoolStateUpdate>,
) -> WorkerResult
{
    //let mut block_number;
    //let mut block_hash;
    //let mut timestamp;
    let mut next_block_base_fee = 0;

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


                                            let request = MessageSearcherPoolStateUpdate::new(
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
                                            match state_updates_broadcaster.send(request).await {
                                                Err(e)=>{error!("{}", e)}
                                                _=>{}
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
pub struct BlockStateChangeProcessorActor
{
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
    #[producer]
    state_updates_tx: Option<Broadcaster<MessageSearcherPoolStateUpdate>>,
}

impl BlockStateChangeProcessorActor
{
    pub fn new() -> BlockStateChangeProcessorActor {
        BlockStateChangeProcessorActor {
            //client,
            market: None,
            block_history: None,
            market_events_rx: None,
            state_updates_tx: None,
        }
    }
}

#[async_trait]
impl Actor for BlockStateChangeProcessorActor
{
    async fn start(&mut self) -> ActorResult {
        let task = tokio::task::spawn(
            block_state_change_worker(
                self.market.clone().unwrap(),
                self.block_history.clone().unwrap(),
                self.market_events_rx.clone().unwrap().subscribe().await,
                self.state_updates_tx.clone().unwrap(),
            )
        );
        Ok(vec![task])
    }
}
