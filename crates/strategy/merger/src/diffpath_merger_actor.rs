use alloy_network::TransactionResponse;
use alloy_primitives::{Address, TxHash};
use alloy_rpc_types::Transaction;
use eyre::{OptionExt, Result};
use lazy_static::lazy_static;
use revm::{Database, DatabaseCommit, DatabaseRef};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tracing::{debug, error, info};

use loom_core_actors::{Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::{Blockchain, Strategy};
use loom_evm_utils::NWETH;
use loom_types_entities::{MarketState, Swap};
use loom_types_events::{MarketEvents, MessageSwapCompose, SwapComposeData, SwapComposeMessage, TxComposeData};

lazy_static! {
    static ref COINBASE: Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

fn get_merge_list<'a, DB: Clone + Send + Sync + 'static>(
    request: &SwapComposeData<DB>,
    swap_paths: &'a [SwapComposeData<DB>],
) -> Vec<&'a SwapComposeData<DB>> {
    let mut ret: Vec<&SwapComposeData<DB>> = Vec::new();
    let mut pools = request.swap.get_pool_id_vec();
    for p in swap_paths.iter() {
        if !p.cross_pools(&pools) {
            pools.extend(p.swap.get_pool_id_vec());
            ret.push(p);
        }
    }
    ret
}

async fn diff_path_merger_worker<DB>(
    market_events_rx: Broadcaster<MarketEvents>,
    compose_channel_rx: Broadcaster<MessageSwapCompose<DB>>,
    compose_channel_tx: Broadcaster<MessageSwapCompose<DB>>,
) -> WorkerResult
where
    DB: DatabaseRef + Database + DatabaseCommit + Send + Sync + Clone + 'static,
{
    let mut market_events_rx: Receiver<MarketEvents> = market_events_rx.subscribe();

    let mut compose_channel_rx: Receiver<MessageSwapCompose<DB>> = compose_channel_rx.subscribe();

    let mut swap_paths: Vec<SwapComposeData<DB>> = Vec::new();

    loop {
        tokio::select! {
            msg = market_events_rx.recv() => {
                if let Ok(msg) = msg {
                    let market_event_msg : MarketEvents = msg;
                    if let MarketEvents::BlockHeaderUpdate{block_number, block_hash, timestamp, base_fee, next_base_fee} =  market_event_msg {
                        debug!("Block header update {} {} ts {} base_fee {} next {} ", block_number, block_hash, timestamp, base_fee, next_base_fee);
                        //cur_block_number = Some( block_number + 1);
                        //cur_block_time = Some(timestamp + 12 );
                        //cur_next_base_fee = next_base_fee;
                        //cur_base_fee = base_fee;
                        swap_paths = Vec::new();

                        // for _counter in 0..5  {
                        //     if let Ok(msg) = market_events_rx.recv().await {
                        //         if matches!(msg, MarketEvents::BlockStateUpdate{ block_hash } ) {
                        //             cur_state_override = latest_block.read().await.node_state_override();
                        //             debug!("Block state update received {} {}", block_number, block_hash);
                        //             break;
                        //         }
                        //     }
                        // }
                    }
                }
            }


            msg = compose_channel_rx.recv() => {
                let msg : Result<MessageSwapCompose<DB>, RecvError> = msg;
                match msg {
                    Ok(compose_request)=>{
                        if let SwapComposeMessage::Ready(sign_request) = compose_request.inner() {
                            if matches!( sign_request.swap, Swap::BackrunSwapLine(_)) || matches!( sign_request.swap, Swap::BackrunSwapSteps(_)) {
                                let mut merge_list = get_merge_list(sign_request, &swap_paths);

                                if !merge_list.is_empty() {
                                    let swap_vec : Vec<Swap> = merge_list.iter().map(|x|x.swap.clone()).collect();
                                    info!("Merging started {:?}", swap_vec );

                                    let mut state = MarketState::new(sign_request.poststate.clone().unwrap().clone());

                                    for dbs in merge_list.iter() {
                                        state.apply_geth_update_vec( dbs.poststate_update.clone().ok_or_eyre("NO_STATE_UPDATE")?);
                                    }

                                    merge_list.push(sign_request);

                                    let mut stuffing_txs_hashes : Vec<TxHash> = Vec::new();
                                    let mut stuffing_txs : Vec<Transaction> = Vec::new();

                                    for req in merge_list.iter() {
                                        for tx in req.tx_compose.stuffing_txs.iter() {
                                            if !stuffing_txs_hashes.contains(&tx.tx_hash()) {
                                                stuffing_txs_hashes.push(tx.tx_hash());
                                                stuffing_txs.push(tx.clone());
                                            }
                                        }
                                    }

                                    let encode_request = MessageSwapCompose::prepare(
                                        SwapComposeData {
                                            tx_compose : TxComposeData {
                                                stuffing_txs_hashes,
                                                stuffing_txs,
                                                ..sign_request.tx_compose.clone()
                                            },
                                            swap : Swap::Multiple( merge_list.iter().map(|i| i.swap.clone()  ).collect()) ,
                                            origin : Some("diffpath_merger".to_string()),
                                            tips_pct : Some(9000),
                                            poststate : Some(state.state_db),
                                            ..sign_request.clone()
                                        }
                                    );
                                    info!("+++ Calculation finished. Merge list : {} profit : {}",merge_list.len(), NWETH::to_float(encode_request.inner.swap.abs_profit_eth())  );

                                    if let Err(e) = compose_channel_tx.send(encode_request) {
                                       error!("{}",e)
                                    }
                                }

                                swap_paths.push(sign_request.clone());
                                swap_paths.sort_by(|a, b| b.swap.abs_profit_eth().cmp(&a.swap.abs_profit_eth() ) )
                            }
                        }
                    }
                    Err(e)=>{error!("{e}")}
                }

            }


        }
    }
}

#[derive(Consumer, Producer, Accessor, Default)]
pub struct DiffPathMergerActor<DB: Clone + Send + Sync + 'static> {
    #[consumer]
    market_events: Option<Broadcaster<MarketEvents>>,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageSwapCompose<DB>>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageSwapCompose<DB>>>,
}

impl<DB> DiffPathMergerActor<DB>
where
    DB: DatabaseRef + Database + DatabaseCommit + Send + Sync + Clone + Default + 'static,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market_events: Some(bc.market_events_channel()), ..self }
    }

    pub fn on_strategy(self, strategy: &Strategy<DB>) -> Self {
        Self {
            compose_channel_tx: Some(strategy.swap_compose_channel()),
            compose_channel_rx: Some(strategy.swap_compose_channel()),
            ..self
        }
    }
}

impl<DB> Actor for DiffPathMergerActor<DB>
where
    DB: DatabaseRef + Database + DatabaseCommit + Send + Sync + Clone + Default + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(diff_path_merger_worker(
            self.market_events.clone().unwrap(),
            self.compose_channel_rx.clone().unwrap(),
            self.compose_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "DiffPathMergerActor"
    }
}
