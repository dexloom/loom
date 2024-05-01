use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;

use alloy_primitives::{Address, TxHash, U256};
use alloy_rpc_types::state::StateOverride;
use alloy_rpc_types::Transaction;
use async_trait::async_trait;
use eyre::{eyre, OptionExt, Result};
use lazy_static::lazy_static;
use log::{debug, error, info};
use revm::{Context, Evm, EvmContext, Handler, InMemoryDB};
use revm::db::WrapDatabaseRef;
use revm::primitives::{BlockEnv, Env, SHANGHAI, ShanghaiSpec};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

use defi_entities::{AccountNonceAndBalanceState, LatestBlock, MarketState, NWETH, TxSigners};
use defi_events::{MarketEvents, MessageTxCompose, SwapType, TxCompose, TxComposeData};
use defi_types::Mempool;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_multicaller::SwapStepEncoder;

lazy_static! {
    static ref COINBASE : Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

fn get_merge_list<'a>(request: &TxComposeData, swap_paths: &'a Vec<TxComposeData>) -> Vec<&'a TxComposeData> {
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
    encoder: SwapStepEncoder,
    signers: SharedState<TxSigners>,
    account_monitor: SharedState<AccountNonceAndBalanceState>,
    latest_block: SharedState<LatestBlock>,
    mempool: SharedState<Mempool>,
    market_state: SharedState<MarketState>,
    mut market_events_rx: Receiver<MarketEvents>,
    mut compose_channel_rx: Receiver<MessageTxCompose>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult
{
    let mut swap_paths: Vec<TxComposeData> = Vec::new();

    let mut affecting_tx: HashMap<TxHash, bool> = HashMap::new();
    let mut cur_base_fee: u128 = 0;
    let mut cur_next_base_fee: u128 = 0;
    let mut cur_block_number: Option<alloy_primitives::BlockNumber> = None;
    let mut cur_block_time: Option<u64> = None;
    let mut cur_state_override: StateOverride = StateOverride::default();


    loop {
        tokio::select! {
            msg = market_events_rx.recv() => {
                if let Ok(msg) = msg {
                    let market_event_msg : MarketEvents = msg;
                    match market_event_msg {
                        MarketEvents::BlockHeaderUpdate{block_number, block_hash, timestamp, base_fee, next_base_fee} =>{
                            debug!("Block header update {} {} base_fee {} ", block_number, block_hash, base_fee);
                            cur_block_number = Some( block_number + 1);
                            cur_block_time = Some(timestamp + 12 );
                            cur_next_base_fee = next_base_fee;
                            cur_base_fee = base_fee;
                            swap_paths = Vec::new();

                            let mut counter = 0;
                            for counter in 0..5  {
                                if let Ok(msg) = market_events_rx.recv().await {
                                    if matches!(msg, MarketEvents::BlockStateUpdate{ block_hash } ) {
                                        cur_state_override = latest_block.read().await.node_state_override();
                                        debug!("Block state update received {} {}", block_number, block_hash);
                                        break;
                                    }
                                }
                            }
                        }
                        _=>{}
                    }
                }
            }


            msg = compose_channel_rx.recv() => {
                let msg : Result<MessageTxCompose, RecvError> = msg;
                match msg {
                    Ok(compose_request)=>{
                        if let TxCompose::Sign(sign_request) = compose_request.inner() {
                            if matches!( sign_request.swap, SwapType::BackrunSwapLine(_)) || matches!( sign_request.swap, SwapType::BackrunSwapSteps(_)) {
                                let mut merge_list = get_merge_list(sign_request, &swap_paths);

                                if merge_list.len() > 0 {

                                    let mut state = MarketState::new(sign_request.poststate.clone().unwrap().as_ref().clone());

                                    for dbs in merge_list.iter() {
                                        state.apply_state_update( dbs.poststate_update.as_ref().ok_or_eyre("NO_STATE_UPDATE")?, false, false );
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

                                    let encode_request = MessageTxCompose::encode(
                                        TxComposeData {
                                            stuffing_txs_hashes,
                                            stuffing_txs,
                                            swap : SwapType::Multiple( merge_list.iter().map(|i| i.swap.clone()  ).collect()) ,
                                            origin : Some("diffpath_merger".to_string()),
                                            tips_pct : Some(5000),
                                            poststate : Some(arc_db),
                                            ..sign_request.clone()
                                        }
                                    );
                                    info!("+++ Calculation finished. Merge list : {} profit : {}",merge_list.len(), NWETH::to_float(encode_request.swap.abs_profit_eth())  );

                                    match compose_channel_tx.send(encode_request).await {
                                        Err(e)=>{error!("{}",e)}
                                        _=>{}
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
    };
    Err(eyre!("Finished"))
}

#[derive(Consumer, Producer, Accessor)]
pub struct DiffPathMergerActor
{
    encoder: SwapStepEncoder,
    #[accessor]
    mempool: Option<SharedState<Mempool>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[accessor]
    signers: Option<SharedState<TxSigners>>,
    #[accessor]
    account_monitor: Option<SharedState<AccountNonceAndBalanceState>>,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[consumer]
    market_events: Option<Broadcaster<MarketEvents>>,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl DiffPathMergerActor
{
    pub fn new(multicaller: Address) -> Self {
        Self {
            encoder: SwapStepEncoder::new(multicaller),
            mempool: None,
            market_state: None,
            signers: None,
            account_monitor: None,
            latest_block: None,
            market_events: None,
            compose_channel_rx: None,
            compose_channel_tx: None,
        }
    }
}

#[async_trait]
impl Actor for DiffPathMergerActor
{
    async fn start(&mut self) -> ActorResult {
        let task = tokio::task::spawn(
            diff_path_merger_worker(
                self.encoder.clone(),
                self.signers.clone().unwrap(),
                self.account_monitor.clone().unwrap(),
                self.latest_block.clone().unwrap(),
                self.mempool.clone().unwrap(),
                self.market_state.clone().unwrap(),
                self.market_events.clone().unwrap().subscribe().await,
                self.compose_channel_rx.clone().unwrap().subscribe().await,
                self.compose_channel_tx.clone().unwrap(),
            )
        );
        Ok(vec![task])
    }
}
