use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use alloy_primitives::{Address, U256};
#[cfg(not(debug_assertions))]
use chrono::TimeDelta;
use eyre::{eyre, Result};
use log::{debug, error, info, warn};
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use revm::primitives::Env;
use tokio::sync::broadcast::error::RecvError;

use defi_blockchain::Blockchain;
use defi_entities::{Market, PoolWrapper, Swap, SwapLine, SwapPath};
use defi_events::{BestTxCompose, HealthEvent, Message, MessageHealthEvent, MessageTxCompose, StateUpdateEvent, TxCompose, TxComposeData};
use defi_types::SwapError;
use loom_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_revm_db::LoomInMemoryDB;

async fn state_change_arb_searcher_task(
    thread_pool: Arc<ThreadPool>,
    smart: bool,
    msg: StateUpdateEvent,
    market: SharedState<Market>,
    swap_request_tx: Broadcaster<MessageTxCompose>,
    pool_health_monitor_tx: Broadcaster<MessageHealthEvent>,
) -> Result<()> {
    debug!("Message received {} stuffing : {:?}", msg.origin, msg.stuffing_tx_hash());
    //let msg_time = chrono::Local::now();

    let mut db = msg.market_state().clone();
    db.apply_geth_update_vec(msg.state_update().clone());
    //let mut current_market_state = MarketState::new(db);

    //current_market_state.apply_state_update(msg.state_update(), true, false);

    let start_time = chrono::Local::now();
    let mut swap_path_vec: Vec<SwapPath> = Vec::new();

    let market_guard_read = market.read().await;
    for (pool, v) in msg.directions().iter() {
        let pool_paths: Vec<SwapPath> = match market_guard_read.get_pool_paths(&pool.get_address()) {
            Some(paths) => paths
                .into_iter()
                .filter(|swap_path| !swap_path.pools.iter().any(|pool| !market_guard_read.is_pool_ok(&pool.get_address())))
                .collect(),
            None => {
                let mut pool_direction: BTreeMap<PoolWrapper, Vec<(Address, Address)>> = BTreeMap::new();
                pool_direction.insert(pool.clone(), v.clone());
                market_guard_read.build_swap_path_vec(&pool_direction).unwrap_or_default()
            }
        };

        swap_path_vec.extend(pool_paths)
    }
    drop(market_guard_read);

    if swap_path_vec.is_empty() {
        warn!(
            "No swap path built for request: {:?} {}",
            msg.stuffing_txs_hashes().first().unwrap_or_default(),
            chrono::Local::now() - start_time
        );
        return Err(eyre!("NO_SWAP_PATHS"));
    }
    warn!("Calculation started {} {}", swap_path_vec.len(), chrono::Local::now() - start_time);

    let env = msg.evm_env();

    let channel_len = swap_path_vec.len();
    let (swap_path_tx, mut swap_line_rx) = tokio::sync::mpsc::channel(channel_len);

    let market_state_clone = db.clone();
    let swap_path_vec_len = swap_path_vec.len();

    tokio::task::spawn(async move {
        //let pool = ThreadPoolBuilder::new().num_threads(20).build().unwrap();

        thread_pool.install(|| {
            swap_path_vec.into_par_iter().for_each_with((&swap_path_tx, &market_state_clone, &env), |req, item| {
                let mut mut_item: SwapLine = SwapLine { path: item, ..Default::default() };
                #[cfg(not(debug_assertions))]
                let start_time = chrono::Local::now();
                let calc_result = Calculator::calculate(&mut mut_item, req.1, req.2.clone());
                #[cfg(not(debug_assertions))]
                let took_time = chrono::Local::now() - start_time;

                match calc_result {
                    Ok(_) => {
                        #[cfg(not(debug_assertions))]
                        {
                            if took_time > TimeDelta::new(0, 10 * 1000000).unwrap() {
                                warn!("Took longer than expected {} {}", took_time, mut_item.clone())
                            }
                        }

                        if let Ok(profit) = mut_item.profit() {
                            if profit.is_positive()
                                && msg.next_base_fee != 0
                                && mut_item.abs_profit_eth() > U256::from(200000 * msg.next_base_fee)
                            {
                                if let Err(e) = swap_path_tx.try_send(Ok(mut_item)) {
                                    error!("try_send ok swap_path_tx  error : {e}")
                                }
                            }
                        }
                    }
                    Err(e) => {
                        #[cfg(not(debug_assertions))]
                        {
                            if took_time > TimeDelta::new(0, 10 * 1000000).unwrap() {
                                warn!("Took longer than expected {:?} {}", e, mut_item.clone())
                            }
                        }
                        //error!("Swap error: {:?}", e);

                        if let Err(e) = swap_path_tx.try_send(Err(e)) {
                            error!("try_send error to swap_path_tx error : {e}")
                        }
                    }
                }
            });
        });
        debug!("Calculation iteration finished {}", chrono::Local::now() - start_time);
    });

    debug!("Calculation results receiver started {}", chrono::Local::now() - start_time);

    let swap_request_tx_clone = swap_request_tx.clone();
    let pool_health_monitor_tx_clone = pool_health_monitor_tx.clone();

    let arc_db = Arc::new(db);

    let mut answers = 0;

    let mut best_answers = BestTxCompose::new_with_pct(U256::from(9000));

    let mut failed_pools: HashSet<SwapError> = HashSet::new();

    while let Some(swap_line_result) = swap_line_rx.recv().await {
        match swap_line_result {
            Ok(swap_line) => {
                let encode_request = TxCompose::Encode(TxComposeData {
                    block: msg.next_block,
                    block_timestamp: msg.next_block_timestamp,
                    base_fee: msg.next_base_fee,
                    gas: swap_line.gas_used.unwrap_or(300000),
                    stuffing_txs: msg.stuffing_txs.clone(),
                    stuffing_txs_hashes: msg.stuffing_txs_hashes.clone(),
                    swap: Swap::BackrunSwapLine(swap_line),
                    origin: Some(msg.origin.clone()),
                    tips_pct: Some(msg.tips_pct),
                    poststate: Some(arc_db.clone()),
                    poststate_update: Some(msg.state_update().clone()),
                    ..TxComposeData::default()
                });

                if !smart || best_answers.check(&encode_request) {
                    if let Err(e) = swap_request_tx_clone.send(Message::new(encode_request)).await {
                        error!("swap_request_tx_clone.send {}", e)
                    }
                }
            }
            Err(swap_error) => {
                if failed_pools.insert(swap_error.clone()) {
                    if let Err(e) = pool_health_monitor_tx_clone.send(Message::new(HealthEvent::PoolSwapError(swap_error))).await {
                        error!("try_send to pool_health_monitor error : {:?}", e)
                    }
                }
            }
        }

        answers += 1;
    }
    warn!(
        "Calculation finished. Origin : {} {} {} {} stuffing hash : {:?}",
        msg.origin,
        swap_path_vec_len,
        answers,
        chrono::Local::now() - start_time,
        msg.stuffing_tx_hash()
    );

    Ok(())
}

pub async fn state_change_arb_searcher_worker(
    smart: bool,
    market: SharedState<Market>,
    search_request_rx: Broadcaster<StateUpdateEvent>,
    swap_request_tx: Broadcaster<MessageTxCompose>,
    pool_health_monitor_tx: Broadcaster<MessageHealthEvent>,
) -> WorkerResult {
    subscribe!(search_request_rx);

    let cpus = num_cpus::get();
    info!("Cpus : {cpus}");
    let thread_pool = Arc::new(ThreadPoolBuilder::new().num_threads(cpus - 2).build().unwrap());

    loop {
        tokio::select! {
                msg = search_request_rx.recv() => {
                let pool_update_msg : Result<StateUpdateEvent, RecvError> = msg;
                if let Ok(msg) = pool_update_msg {
                    tokio::task::spawn(
                        state_change_arb_searcher_task(
                            thread_pool.clone(),
                            smart,
                            msg,
                            market.clone(),
                            swap_request_tx.clone(),
                            pool_health_monitor_tx.clone()
                        )
                    );
                }
            }
        }
    }
}

struct Calculator {}

impl Calculator {
    #[inline]
    pub fn calculate<'a>(path: &'a mut SwapLine, state: &LoomInMemoryDB, env: Env) -> Result<&'a mut SwapLine, SwapError> {
        let first_token = path.get_first_token().unwrap();
        if let Some(amount_in) = first_token.calc_token_value_from_eth(U256::from(10).pow(U256::from(17))) {
            //trace!("calculate : {} amount in : {}",first_token.get_symbol(), first_token.to_float(amount_in) );
            path.optimize_with_in_amount(state, env, amount_in)
        } else {
            Err(path.to_error("PRICE_NOT_SET".to_string()))
        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct StateChangeArbSearcherActor {
    smart: bool,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[consumer]
    state_update_rx: Option<Broadcaster<StateUpdateEvent>>,
    #[producer]
    compose_tx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    pool_health_monitor_tx: Option<Broadcaster<MessageHealthEvent>>,
}

impl StateChangeArbSearcherActor {
    pub fn new(smart: bool) -> StateChangeArbSearcherActor {
        StateChangeArbSearcherActor { smart, market: None, state_update_rx: None, compose_tx: None, pool_health_monitor_tx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            market: Some(bc.market()),
            compose_tx: Some(bc.compose_channel()),
            pool_health_monitor_tx: Some(bc.pool_health_monitor_channel()),
            state_update_rx: Some(bc.state_update_channel()),
            ..self
        }
    }
}

impl Actor for StateChangeArbSearcherActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(state_change_arb_searcher_worker(
            self.smart,
            self.market.clone().unwrap(),
            self.state_update_rx.clone().unwrap(),
            self.compose_tx.clone().unwrap(),
            self.pool_health_monitor_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "StateChangeArbSearcherActor"
    }
}
