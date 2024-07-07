use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use alloy_eips::BlockNumberOrTag;
use alloy_network::Network;
use alloy_primitives::{Address, TxHash, U256, U64};
use alloy_provider::Provider;
use alloy_rpc_types::{BlockOverrides, Transaction};
use alloy_rpc_types::state::StateOverride;
use alloy_rpc_types_trace::geth::GethDebugTracingCallOptions;
use alloy_transport::Transport;
use async_trait::async_trait;
use eyre::{eyre, Result};
use lazy_static::lazy_static;
use log::{debug, error, info};
use revm::{Evm, InMemoryDB};
use revm::primitives::{BlockEnv, Env, SHANGHAI};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

use debug_provider::DebugProviderExt;
use defi_entities::{AccountNonceAndBalanceState, DataFetcher, FetchState, LatestBlock, MarketState, Swap, TxSigners};
use defi_events::{MarketEvents, MessageTxCompose, TxCompose, TxComposeData};
use defi_types::{debug_trace_call_pre_state, GethStateUpdate, GethStateUpdateVec, Mempool, TRACING_CALL_OPTS};
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_revm::LoomInMemoryDB;
use loom_utils::evm::evm_transact;

lazy_static! {
    static ref COINBASE : Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

fn get_merge_list<'a>(request: &TxComposeData, swap_paths: &'a HashMap<TxHash, Vec<TxComposeData>>) -> Vec<&'a TxComposeData> {
    //let mut ret : Vec<&TxComposeData> = Vec::new();
    let swap_line = if let Swap::BackrunSwapLine(swap_line) = &request.swap {
        swap_line
    } else {
        return Vec::new();
    };

    let swap_stuffing_hash = request.first_stuffing_hash();

    let mut ret: Vec<&TxComposeData> =
        swap_paths.iter().filter(|(k, _)| **k != swap_stuffing_hash).map(|(_k, v)|
        v.iter().find(|a| {
            if let Swap::BackrunSwapLine(a_line) = &a.swap {
                a_line.path == swap_line.path
            } else {
                false
            }
        }).clone()
        ).filter(|x| x.is_some()).map(|x| x.unwrap()).collect();

    ret.sort_by(|a, b| b.swap.abs_profit_eth().cmp(&a.swap.abs_profit_eth()));


    ret
}

async fn same_path_merger_task<P, T, N>
(
    client: P,
    stuffing_txes: Vec<Transaction>,
    pre_states: Arc<RwLock<DataFetcher<TxHash, GethStateUpdate>>>,
    market_state: SharedState<MarketState>,
    call_opts: GethDebugTracingCallOptions,
    request: TxComposeData,
    swap_request_tx: Broadcaster<MessageTxCompose>,
) -> Result<()>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    info!("same_path_merger_task stuffing_txs len {}", stuffing_txes.len() );

    let mut prestate_guard = pre_states.write().await;

    let mut stuffing_state_locks: Vec<(Transaction, FetchState<GethStateUpdate>)> = Vec::new();

    let env = Env {
        block: BlockEnv {
            number: U256::from(request.block),
            timestamp: U256::from(request.block_timestamp),
            ..BlockEnv::default()
        },
        ..Env::default()
    };


    for tx in stuffing_txes.into_iter() {
        let client_clone = client.clone(); //Pin::new(Box::new(client.clone()));
        let tx_clone = tx.clone();
        let tx_hash: TxHash = tx.hash;
        let call_opts_clone = call_opts.clone();

        let lock = prestate_guard.fetch(tx_hash, |_tx_hash| async move {
            debug_trace_call_pre_state(client_clone, tx_clone, BlockNumberOrTag::Latest, Some(call_opts_clone)).await
        }).await;

        stuffing_state_locks.push((tx, lock));
    }

    drop(prestate_guard);

    let mut stuffing_states: Vec<(Transaction, GethStateUpdate)> = Vec::new();

    for (tx, lock) in stuffing_state_locks.into_iter() {
        match lock {
            FetchState::Fetching(lock) => {
                match lock.read().await.deref() {
                    Some(t) => {
                        stuffing_states.push((tx, t.clone()))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    let mut tx_order: Vec<usize> = (0..stuffing_states.len()).into_iter().collect();


    let mut changing: Option<usize> = None;
    let mut counter = 0;
    let rdb: Option<LoomInMemoryDB> = loop {
        counter += 1;
        if counter > 10 {
            break None;
        }

        let mut ok = true;

        let tx_and_state: Vec<&(Transaction, GethStateUpdate)> = tx_order.iter().map(|i| stuffing_states.get(*i).unwrap()).collect();

        let db = market_state.read().await.state_db.clone();

        let states: GethStateUpdateVec = tx_and_state.iter().map(|(_tx, state)| state.clone()).collect();

        let mut market_db = MarketState::new(db);

        market_db.apply_state_update(&states, false, true);

        let db = market_db.state_db;

        let mut evm = Evm::builder().with_spec_id(SHANGHAI).with_db(db).with_env(Box::new(env.clone())).build();

        for (idx, tx_idx) in tx_order.clone().iter().enumerate() {
            let tx = &stuffing_states[*tx_idx].0;

            match evm_transact(&mut evm, tx) {
                Ok(_c) => {
                    info!("Transaction committed successfully {:?}", tx.hash);
                }
                Err(e) => {
                    error!("Transaction {:?} commit error: {}", tx.hash, e );
                    match changing.clone() {
                        Some(changing_idx) => {
                            if (changing_idx == idx && idx == 0) || (changing_idx == idx - 1) {
                                tx_order.remove(changing_idx);
                                debug!("Removing Some {idx} {changing_idx}");
                                changing = None;
                                //TODO : Check idx > 1 condition
                            } else if idx < tx_order.len() && idx > 0 { // Next
                                tx_order.swap(idx, idx - 1);
                                debug!("Swapping Some {idx} {changing_idx}");
                                changing = Some(idx - 1)
                            } /*else {
                                debug!("Removing Some 2 {idx} {changing_idx}");
                                tx_order.remove(idx);
                                ok = true;
                            }*/
                        }
                        None => {
                            if idx > 0 {
                                debug!("Swapping None {idx}");
                                tx_order.swap(idx, idx - 1);
                                changing = Some(idx - 1)
                            } else {
                                debug!("Removing None {idx}");
                                tx_order.remove(0);
                                changing = None
                            }
                        }
                    }
                    ok = false;
                    break;
                }
            }
        }

        if ok {
            info!("Transaction sequence found {tx_order:?}");
            let (db, _) = evm.into_db_and_env_with_handler_cfg();
            break Some(db);
        }
    };

    if tx_order.len() < 2 {
        return Err(eyre!("NOT_MERGED"));
    }

    if let Some(db) = rdb {
        if let Swap::BackrunSwapLine(mut swap_line) = request.swap.clone() {
            let first_token = swap_line.get_first_token().unwrap();
            let amount_in = first_token.calc_token_value_from_eth(U256::from(10).pow(U256::from(17))).unwrap();
            match swap_line.optimize_with_in_amount(&db, env.clone(), amount_in) {
                Ok(_r) => {
                    let arc_db = Arc::new(db);
                    let encode_request = MessageTxCompose::encode(
                        TxComposeData {
                            stuffing_txs_hashes: tx_order.iter().map(|i| stuffing_states[*i].0.hash).collect(),
                            stuffing_txs: tx_order.iter().map(|i| stuffing_states[*i].0.clone()).collect(),
                            swap: Swap::BackrunSwapLine(swap_line.clone()),
                            origin: Some("samepath_merger".to_string()),
                            tips_pct: None,
                            poststate: Some(arc_db.clone()),
                            poststate_update: None,
                            ..request
                        }
                    );

                    match swap_request_tx.send(encode_request).await {
                        Err(e) => { error!("{}",e) }
                        _ => {}
                    }
                    info!("+++ Calculation finished {swap_line}" );
                }
                Err(e) => {
                    error!("optimization error : {e:?}")
                }
            }
        }
    }


    //let (db,_) = evm.into_db_and_env_with_handler_cfg();

    /*for (addr,acc) in db.accounts.iter(){
        debug!("-- {} : {:?} code len: {} storage len: {}", addr, acc.account_state, acc.info.code.as_ref().map_or(0, |c| c.len()), acc.storage.len() )
    }*/

    info!("same_path_merger_task stuffing_states len {}", stuffing_states.len() );


    Ok(())
}


async fn same_path_merger_worker<T: Transport + Clone, N: Network, P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static>(
    client: P,
    //encoder: SwapStepEncoder,
    //signers: SharedState<TxSigners>,
    //account_monitor: SharedState<AccountNonceAndBalanceState>,
    latest_block: SharedState<LatestBlock>,
    //mempool: SharedState<Mempool>,
    market_state: SharedState<MarketState>,
    mut market_events_rx: Receiver<MarketEvents>,
    mut compose_channel_rx: Receiver<MessageTxCompose>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult
{
    let mut swap_paths: HashMap<TxHash, Vec<TxComposeData>> = HashMap::new();

    let prestate = Arc::new(RwLock::new(DataFetcher::<TxHash, GethStateUpdate>::new()));

    //let mut affecting_tx: HashMap<TxHash, bool> = HashMap::new();
    //let mut cur_base_fee: u128 = 0;
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
                        MarketEvents::BlockHeaderUpdate{block_number, block_hash,  base_fee, next_base_fee, timestamp} =>{
                            debug!("Block header update {} {} base_fee {} ", block_number, block_hash, base_fee);
                            cur_block_number = Some( block_number + 1);
                            cur_block_time = Some(timestamp + 12 );
                            cur_next_base_fee = next_base_fee;
                            //cur_base_fee = base_fee;
                            *prestate.write().await = DataFetcher::<TxHash, GethStateUpdate>::new();
                            swap_paths = HashMap::new();

                            for _counter in 0..5  {
                                if let Ok(msg) = market_events_rx.recv().await {
                                    if matches!(msg, MarketEvents::BlockStateUpdate{block_hash}) {
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

                            if sign_request.stuffing_txs_hashes.len() == 1 {
                                if let Swap::BackrunSwapLine( _swap_line ) = &sign_request.swap {
                                    let stuffing_tx_hash = sign_request.first_stuffing_hash();

                                    let requests_vec = get_merge_list(&sign_request, &swap_paths);
                                    if requests_vec.len() > 0 {

                                        let mut stuffing_txs : Vec<Transaction> = vec![sign_request.stuffing_txs[0].clone()];
                                        stuffing_txs.extend( requests_vec.iter().map(|r| r.stuffing_txs[0].clone() ).collect::<Vec<Transaction>>());
                                        let client_clone = client.clone();
                                        let prestate_clone = prestate.clone();

                                        let call_opts : GethDebugTracingCallOptions = GethDebugTracingCallOptions{
                                            block_overrides : Some(BlockOverrides {
                                                number : Some( U256::from(cur_block_number.unwrap_or_default())),
                                                time : Some(U64::from(cur_block_time.unwrap_or_default())),
                                                coinbase : Some(*COINBASE),
                                                base_fee : Some(U256::from(cur_next_base_fee)),
                                                ..Default::default()
                                            }),
                                            state_overrides : Some(cur_state_override.clone()),
                                            ..TRACING_CALL_OPTS.clone()
                                        };

                                        tokio::task::spawn(
                                            same_path_merger_task(
                                                client_clone,
                                                stuffing_txs,
                                                prestate_clone,
                                                market_state.clone(),
                                                call_opts,
                                                sign_request.clone(),
                                                compose_channel_tx.clone()
                                            )
                                        );
                                    }

                                    let e = swap_paths.entry(stuffing_tx_hash).or_insert(Default::default());
                                    e.push( sign_request.clone() );

                                }
                            }
                        }
                    },
                    Err(e)=>{
                        error!("{e}")
                    }
                }
            }
        }
    };
}

#[derive(Consumer, Producer, Accessor)]
pub struct SamePathMergerActor<P, T, N>
{
    client: P,
    //encoder: SwapStepEncoder,
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
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> SamePathMergerActor<P, T, N>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self {
            client,
            //encoder: SwapStepEncoder::new(multicaller),
            mempool: None,
            market_state: None,
            signers: None,
            account_monitor: None,
            latest_block: None,
            market_events: None,
            compose_channel_rx: None,
            compose_channel_tx: None,
            _t: PhantomData::default(),
            _n: PhantomData::default(),
        }
    }
}

#[async_trait]
impl<P, T, N> Actor for SamePathMergerActor<P, T, N>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    async fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(
            same_path_merger_worker(
                self.client.clone(),
                //self.encoder.clone(),
                //self.signers.clone().unwrap(),
                //self.account_monitor.clone().unwrap(),
                self.latest_block.clone().unwrap(),
                //self.mempool.clone().unwrap(),
                self.market_state.clone().unwrap(),
                self.market_events.clone().unwrap().subscribe().await,
                self.compose_channel_rx.clone().unwrap().subscribe().await,
                self.compose_channel_tx.clone().unwrap(),
            )
        );
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "SamePathMergerActor"
    }
}
