use std::sync::Arc;

use alloy_primitives::{Address, TxHash};
use alloy_rpc_types::Transaction;
use eyre::{OptionExt, Result};
use lazy_static::lazy_static;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tracing::{debug, error, info};

use loom_core_actors::{Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::Blockchain;
use loom_defi_entities::{MarketState, Swap};
use loom_defi_events::{MarketEvents, MessageTxCompose, TxCompose, TxComposeData};
use loom_evm_utils::NWETH;

lazy_static! {
    static ref COINBASE: Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

fn get_merge_list<'a>(request: &TxComposeData, swap_paths: &'a [TxComposeData]) -> Vec<&'a TxComposeData> {
    let mut ret: Vec<&TxComposeData> = Vec::new();
    let mut pools = request.swap.get_pool_address_vec();
    for p in swap_paths.iter() {
        if !p.cross_pools(&pools) {
            pools.extend(p.swap.get_pool_address_vec());
            ret.push(p);
        }
    }
    ret
}

async fn diff_path_merger_worker(
    market_events_rx: Broadcaster<MarketEvents>,
    compose_channel_rx: Broadcaster<MessageTxCompose>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult {
    let mut market_events_rx: Receiver<MarketEvents> = market_events_rx.subscribe().await;

    let mut compose_channel_rx: Receiver<MessageTxCompose> = compose_channel_rx.subscribe().await;

    let mut swap_paths: Vec<TxComposeData> = Vec::new();

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
                let msg : Result<MessageTxCompose, RecvError> = msg;
                match msg {
                    Ok(compose_request)=>{
                        if let TxCompose::Sign(sign_request) = compose_request.inner() {
                            if matches!( sign_request.swap, Swap::BackrunSwapLine(_)) || matches!( sign_request.swap, Swap::BackrunSwapSteps(_)) {
                                let mut merge_list = get_merge_list(sign_request, &swap_paths);

                                if !merge_list.is_empty() {
                                    let swap_vec : Vec<Swap> = merge_list.iter().map(|x|x.swap.clone()).collect();
                                    info!("Merging started {:?}", swap_vec );

                                    let mut state = MarketState::new(sign_request.poststate.clone().unwrap().as_ref().clone());

                                    for dbs in merge_list.iter() {
                                        state.state_db.apply_geth_state_update( dbs.poststate_update.as_ref().ok_or_eyre("NO_STATE_UPDATE")?, false, false );
                                    }
                                    let arc_db = Arc::new(state.state_db);

                                    merge_list.push(sign_request);

                                    let mut stuffing_txs_hashes : Vec<TxHash> = Vec::new();
                                    let mut stuffing_txs : Vec<Transaction> = Vec::new();

                                    for req in merge_list.iter() {
                                        for tx in req.stuffing_txs.iter() {
                                            if !stuffing_txs_hashes.contains(&tx.hash) {
                                                stuffing_txs_hashes.push(tx.hash);
                                                stuffing_txs.push(tx.clone());
                                            }
                                        }
                                    }

                                    let encode_request = MessageTxCompose::route(
                                        TxComposeData {
                                            stuffing_txs_hashes,
                                            stuffing_txs,
                                            swap : Swap::Multiple( merge_list.iter().map(|i| i.swap.clone()  ).collect()) ,
                                            origin : Some("diffpath_merger".to_string()),
                                            tips_pct : Some(5000),
                                            poststate : Some(arc_db),
                                            ..sign_request.clone()
                                        }
                                    );
                                    info!("+++ Calculation finished. Merge list : {} profit : {}",merge_list.len(), NWETH::to_float(encode_request.swap.abs_profit_eth())  );

                                    if let Err(e) = compose_channel_tx.send(encode_request).await {
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
pub struct DiffPathMergerActor {
    #[consumer]
    market_events: Option<Broadcaster<MarketEvents>>,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl DiffPathMergerActor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            market_events: Some(bc.market_events_channel()),
            compose_channel_tx: Some(bc.compose_channel()),
            compose_channel_rx: Some(bc.compose_channel()),
        }
    }
}

impl Actor for DiffPathMergerActor {
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
