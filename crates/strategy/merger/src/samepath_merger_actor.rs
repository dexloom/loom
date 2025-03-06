use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use alloy_eips::BlockNumberOrTag;
use alloy_network::{Network, TransactionResponse};
use alloy_primitives::{Address, TxHash, U256};
use alloy_provider::Provider;
use alloy_rpc_types::state::StateOverride;
use alloy_rpc_types::{BlockOverrides, Transaction};
use alloy_rpc_types_trace::geth::GethDebugTracingCallOptions;
use eyre::{eyre, ErrReport, Result};
use lazy_static::lazy_static;
use revm::primitives::{BlockEnv, Env, CANCUN};
use revm::{Database, DatabaseCommit, DatabaseRef, Evm};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace};

use loom_core_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::{Blockchain, BlockchainState, Strategy};
use loom_evm_db::DatabaseHelpers;
use loom_evm_utils::evm::evm_transact;
use loom_evm_utils::evm_tx_env::tx_to_evm_tx;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::{debug_trace_call_pre_state, GethStateUpdate, GethStateUpdateVec, TRACING_CALL_OPTS};
use loom_types_entities::{DataFetcher, FetchState, LatestBlock, MarketState, Swap};
use loom_types_events::{MarketEvents, MessageSwapCompose, SwapComposeData, SwapComposeMessage, TxComposeData};

lazy_static! {
    static ref COINBASE: Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

fn get_merge_list<'a, DB: Clone + 'static>(
    request: &SwapComposeData<DB>,
    swap_paths: &'a HashMap<TxHash, Vec<SwapComposeData<DB>>>,
) -> Vec<&'a SwapComposeData<DB>> {
    //let mut ret : Vec<&TxComposeData> = Vec::new();
    let swap_line = if let Swap::BackrunSwapLine(swap_line) = &request.swap {
        swap_line
    } else {
        return Vec::new();
    };

    let swap_stuffing_hash = request.first_stuffing_hash();

    let mut ret: Vec<&SwapComposeData<DB>> = swap_paths
        .iter()
        .filter_map(|(k, v)| {
            if *k != swap_stuffing_hash {
                v.iter().find(|a| if let Swap::BackrunSwapLine(a_line) = &a.swap { a_line.path == swap_line.path } else { false })
            } else {
                None
            }
        })
        .collect();

    ret.sort_by(|a, b| b.swap.abs_profit_eth().cmp(&a.swap.abs_profit_eth()));

    ret
}

async fn same_path_merger_task<P, N, DB>(
    client: P,
    stuffing_txes: Vec<Transaction>,
    pre_states: Arc<RwLock<DataFetcher<TxHash, GethStateUpdate>>>,
    market_state: SharedState<MarketState<DB>>,
    call_opts: GethDebugTracingCallOptions,
    request: SwapComposeData<DB>,
    swap_request_tx: Broadcaster<MessageSwapCompose<DB>>,
) -> Result<()>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: Database<Error = ErrReport> + DatabaseRef<Error = ErrReport> + DatabaseCommit + Send + Sync + Clone + 'static,
{
    debug!("same_path_merger_task stuffing_txs len {}", stuffing_txes.len());

    let mut prestate_guard = pre_states.write().await;

    let mut stuffing_state_locks: Vec<(Transaction, FetchState<GethStateUpdate>)> = Vec::new();

    let env = Env {
        block: BlockEnv {
            number: U256::from(request.tx_compose.next_block_number),
            timestamp: U256::from(request.tx_compose.next_block_timestamp),
            basefee: U256::from(request.tx_compose.next_block_base_fee),
            ..BlockEnv::default()
        },
        ..Env::default()
    };

    for tx in stuffing_txes.into_iter() {
        let client_clone = client.clone(); //Pin::new(Box::new(client.clone()));
        let tx_clone = tx.clone();
        let tx_hash: TxHash = tx.tx_hash();
        let call_opts_clone = call_opts.clone();

        let lock = prestate_guard
            .fetch(tx_hash, |_tx_hash| async move {
                debug_trace_call_pre_state(client_clone, tx_clone, BlockNumberOrTag::Latest.into(), Some(call_opts_clone)).await
            })
            .await;

        stuffing_state_locks.push((tx, lock));
    }

    drop(prestate_guard);

    let mut stuffing_states: Vec<(Transaction, GethStateUpdate)> = Vec::new();

    for (tx, lock) in stuffing_state_locks.into_iter() {
        if let FetchState::Fetching(lock) = lock {
            if let Some(t) = lock.read().await.deref() {
                stuffing_states.push((tx, t.clone()));
            }
        }
    }

    let mut tx_order: Vec<usize> = (0..stuffing_states.len()).collect();

    let mut changing: Option<usize> = None;
    let mut counter = 0;

    let db_org = market_state.read().await.state_db.clone();

    let rdb: Option<DB> = loop {
        counter += 1;
        if counter > 10 {
            break None;
        }

        let mut ok = true;

        let tx_and_state: Vec<&(Transaction, GethStateUpdate)> = tx_order.iter().map(|i| stuffing_states.get(*i).unwrap()).collect();

        let states: GethStateUpdateVec = tx_and_state.iter().map(|(_tx, state)| state.clone()).collect();

        let mut db = db_org.clone();

        DatabaseHelpers::apply_geth_state_update_vec(&mut db, states);

        let mut evm = Evm::builder().with_spec_id(CANCUN).with_db(db).with_env(Box::new(env.clone())).build();

        for (idx, tx_idx) in tx_order.clone().iter().enumerate() {
            // set tx context for evm
            let tx = &stuffing_states[*tx_idx].0;
            let tx_env = tx_to_evm_tx(tx);
            evm.context.evm.env.tx = tx_env;

            match evm_transact(&mut evm) {
                Ok(_c) => {
                    trace!("Transaction {} committed successfully {:?}", idx, tx.tx_hash());
                }
                Err(e) => {
                    error!("Transaction {} {:?} commit error: {}", idx, tx.tx_hash(), e);
                    match changing {
                        Some(changing_idx) => {
                            if (changing_idx == idx && idx == 0) || (changing_idx == idx - 1) {
                                tx_order.remove(changing_idx);
                                trace!("Removing Some {idx} {changing_idx}");
                                changing = None;
                            } else if idx < tx_order.len() && idx > 0 {
                                tx_order.swap(idx, idx - 1);
                                trace!("Swapping Some {idx} {changing_idx}");
                                changing = Some(idx - 1)
                            }
                        }
                        None => {
                            if idx > 0 {
                                trace!("Swapping None {idx}");
                                tx_order.swap(idx, idx - 1);
                                changing = Some(idx - 1)
                            } else {
                                trace!("Removing None {idx}");
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
            debug!("Transaction sequence found {tx_order:?}");
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
                    let encode_request = MessageSwapCompose::prepare(SwapComposeData {
                        tx_compose: TxComposeData {
                            stuffing_txs_hashes: tx_order.iter().map(|i| stuffing_states[*i].0.tx_hash()).collect(),
                            stuffing_txs: tx_order.iter().map(|i| stuffing_states[*i].0.clone()).collect(),
                            ..request.tx_compose
                        },
                        swap: Swap::BackrunSwapLine(swap_line.clone()),
                        origin: Some("samepath_merger".to_string()),
                        tips_pct: None,
                        poststate: Some(db),
                        poststate_update: None,
                        ..request
                    });

                    if let Err(e) = swap_request_tx.send(encode_request) {
                        error!("{}", e)
                    }
                    info!("+++ Calculation finished {swap_line}");
                }
                Err(e) => {
                    error!("optimization error : {e:?}")
                }
            }
        }
    }

    trace!("same_path_merger_task stuffing_states len {}", stuffing_states.len());

    Ok(())
}

async fn same_path_merger_worker<
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef<Error = ErrReport> + Database<Error = ErrReport> + DatabaseCommit + Send + Sync + Clone + 'static,
>(
    client: P,
    latest_block: SharedState<LatestBlock>,
    market_state: SharedState<MarketState<DB>>,
    market_events_rx: Broadcaster<MarketEvents>,
    compose_channel_rx: Broadcaster<MessageSwapCompose<DB>>,
    compose_channel_tx: Broadcaster<MessageSwapCompose<DB>>,
) -> WorkerResult {
    subscribe!(market_events_rx);
    subscribe!(compose_channel_rx);

    let mut swap_paths: HashMap<TxHash, Vec<SwapComposeData<DB>>> = HashMap::new();

    let prestate = Arc::new(RwLock::new(DataFetcher::<TxHash, GethStateUpdate>::new()));

    //let mut affecting_tx: HashMap<TxHash, bool> = HashMap::new();
    //let mut cur_base_fee: u128 = 0;
    let mut cur_next_base_fee: u64 = 0;
    let mut cur_block_number: Option<alloy_primitives::BlockNumber> = None;
    let mut cur_block_time: Option<u64> = None;
    let mut cur_state_override: StateOverride = StateOverride::default();

    loop {
        tokio::select! {
            msg = market_events_rx.recv() => {
                if let Ok(msg) = msg {
                    let market_event_msg : MarketEvents = msg;
                    if let MarketEvents::BlockHeaderUpdate{block_number, block_hash,  base_fee, next_base_fee, timestamp} =  market_event_msg {
                        debug!("Block header update {} {} base_fee {} ", block_number, block_hash, base_fee);
                        cur_block_number = Some( block_number + 1);
                        cur_block_time = Some(timestamp + 12 );
                        cur_next_base_fee = next_base_fee;
                        //cur_base_fee = base_fee;
                        *prestate.write().await = DataFetcher::<TxHash, GethStateUpdate>::new();
                        swap_paths = HashMap::new();

                        let new_block_hash = block_hash;

                        for _counter in 0..5  {
                            if let Ok(MarketEvents::BlockStateUpdate{block_hash}) = market_events_rx.recv().await {
                                if new_block_hash == block_hash {
                                    cur_state_override = latest_block.read().await.node_state_override();
                                    debug!("Block state update received {} {}", block_number, block_hash);
                                    break;
                                }
                            }
                        }
                    }
                }
            }


            msg = compose_channel_rx.recv() => {
                let msg : Result<MessageSwapCompose<DB>, RecvError> = msg;
                match msg {
                    Ok(compose_request)=>{
                        if let SwapComposeMessage::Ready(sign_request) = compose_request.inner() {

                            if sign_request.tx_compose.stuffing_txs_hashes.len() == 1 {
                                if let Swap::BackrunSwapLine( _swap_line ) = &sign_request.swap {
                                    let stuffing_tx_hash = sign_request.first_stuffing_hash();

                                    let requests_vec = get_merge_list(sign_request, &swap_paths);
                                    if !requests_vec.is_empty() {

                                        let mut stuffing_txs : Vec<Transaction> = vec![sign_request.tx_compose.stuffing_txs[0].clone()];
                                        stuffing_txs.extend( requests_vec.iter().map(|r| r.tx_compose.stuffing_txs[0].clone() ).collect::<Vec<Transaction>>());
                                        let client_clone = client.clone();
                                        let prestate_clone = prestate.clone();

                                        let call_opts : GethDebugTracingCallOptions = GethDebugTracingCallOptions{
                                            block_overrides : Some(BlockOverrides {
                                                number : Some( U256::from(cur_block_number.unwrap_or_default())),
                                                time : Some(cur_block_time.unwrap_or_default()),
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

                                    let e = swap_paths.entry(stuffing_tx_hash).or_default();
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
    }
}

#[derive(Consumer, Producer, Accessor)]
pub struct SamePathMergerActor<P, N, DB: Send + Sync + Clone + 'static> {
    client: P,
    //encoder: SwapStepEncoder,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[consumer]
    market_events: Option<Broadcaster<MarketEvents>>,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageSwapCompose<DB>>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageSwapCompose<DB>>>,
    _n: PhantomData<N>,
}

impl<P, N, DB> SamePathMergerActor<P, N, DB>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self {
            client,
            market_state: None,
            latest_block: None,
            market_events: None,
            compose_channel_rx: None,
            compose_channel_tx: None,
            _n: PhantomData,
        }
    }

    pub fn on_bc(self, bc: &Blockchain, state: &BlockchainState<DB>, strategy: &Strategy<DB>) -> Self {
        Self {
            market_state: Some(state.market_state_commit()),
            latest_block: Some(bc.latest_block()),
            market_events: Some(bc.market_events_channel()),
            compose_channel_tx: Some(strategy.swap_compose_channel()),
            compose_channel_rx: Some(strategy.swap_compose_channel()),
            ..self
        }
    }
}

impl<P, N, DB> Actor for SamePathMergerActor<P, N, DB>
where
    N: Network,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    DB: DatabaseRef<Error = ErrReport> + Database<Error = ErrReport> + DatabaseCommit + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(same_path_merger_worker(
            self.client.clone(),
            self.latest_block.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.market_events.clone().unwrap(),
            self.compose_channel_rx.clone().unwrap(),
            self.compose_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "SamePathMergerActor"
    }
}
