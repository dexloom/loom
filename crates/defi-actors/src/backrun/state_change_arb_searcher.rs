use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use alloy_primitives::{Address, U256};
#[cfg(not(debug_assertions))]
use chrono::TimeDelta;
use eyre::{eyre, Result};
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use tokio::sync::broadcast::error::RecvError;
#[cfg(not(debug_assertions))]
use tracing::warn;
use tracing::{debug, error, info, trace};

use crate::backrun::SwapCalculator;
use alloy_primitives::utils::parse_units;
use defi_blockchain::Blockchain;
use defi_entities::{Market, PoolWrapper, Swap, SwapLine, SwapPath};
use defi_events::{BestTxCompose, HealthEvent, Message, MessageHealthEvent, MessageTxCompose, StateUpdateEvent, TxCompose, TxComposeData};
use defi_types::SwapError;
use lazy_static::lazy_static;
use loom_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

async fn state_change_arb_searcher_task(
    thread_pool: Arc<ThreadPool>,
    smart: bool,
    state_update_event: StateUpdateEvent,
    market: SharedState<Market>,
    swap_request_tx: Broadcaster<MessageTxCompose>,
    pool_health_monitor_tx: Broadcaster<MessageHealthEvent>,
) -> Result<()> {
    debug!("Message received {} stuffing : {:?}", state_update_event.origin, state_update_event.stuffing_tx_hash());

    let mut db = state_update_event.market_state().clone();
    db.apply_geth_update_vec(state_update_event.state_update().clone());

    let start_time = chrono::Local::now();
    let mut swap_path_vec: Vec<SwapPath> = Vec::new();

    let market_guard_read = market.read().await;
    for (pool, v) in state_update_event.directions().iter() {
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
        debug!(
            "No swap path built for request: {:?} {}",
            state_update_event.stuffing_txs_hashes().first().unwrap_or_default(),
            chrono::Local::now() - start_time
        );
        return Err(eyre!("NO_SWAP_PATHS"));
    }
    info!("Calculation started: swap_path_vec_len={} elapsed={}", swap_path_vec.len(), chrono::Local::now() - start_time);

    let env = state_update_event.evm_env();

    let channel_len = swap_path_vec.len();
    let (swap_path_tx, mut swap_line_rx) = tokio::sync::mpsc::channel(channel_len);

    let market_state_clone = db.clone();
    let swap_path_vec_len = swap_path_vec.len();

    tokio::task::spawn(async move {
        thread_pool.install(|| {
            swap_path_vec.into_par_iter().for_each_with((&swap_path_tx, &market_state_clone, &env), |req, item| {
                let mut mut_item: SwapLine = SwapLine { path: item, ..Default::default() };
                #[cfg(not(debug_assertions))]
                let start_time = chrono::Local::now();
                let calc_result = SwapCalculator::calculate(&mut mut_item, req.1, req.2.clone());
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
                        trace!("Calc result received: {}", mut_item);

                        if let Ok(profit) = mut_item.profit() {
                            if profit.is_positive() && mut_item.abs_profit_eth() > U256::from(state_update_event.next_base_fee * 200_000) {
                                if let Err(error) = swap_path_tx.try_send(Ok(mut_item)) {
                                    error!(%error, "swap_path_tx.try_send")
                                }
                            } else {
                                trace!("profit is not enough")
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
                        trace!("Swap error: {:?}", e);

                        if let Err(error) = swap_path_tx.try_send(Err(e)) {
                            error!(%error, "try_send to swap_path_tx")
                        }
                    }
                }
            });
        });
        debug!(elapsed = %(chrono::Local::now() - start_time), "Calculation iteration finished");
    });

    debug!(elapsed = %(chrono::Local::now() - start_time), "Calculation results receiver started" );

    let swap_request_tx_clone = swap_request_tx.clone();
    let pool_health_monitor_tx_clone = pool_health_monitor_tx.clone();

    let arc_db = Arc::new(db);

    let mut answers = 0;

    let mut best_answers = BestTxCompose::new_with_pct(U256::from(9000));

    let mut failed_pools: HashSet<SwapError> = HashSet::new();

    while let Some(swap_line_result) = swap_line_rx.recv().await {
        match swap_line_result {
            Ok(swap_line) => {
                let encode_request = TxCompose::Route(TxComposeData {
                    next_block_number: state_update_event.next_block_number,
                    next_block_timestamp: state_update_event.next_block_timestamp,
                    next_block_base_fee: state_update_event.next_base_fee,
                    gas: swap_line.gas_used.unwrap_or(300000),
                    stuffing_txs: state_update_event.stuffing_txs.clone(),
                    stuffing_txs_hashes: state_update_event.stuffing_txs_hashes.clone(),
                    swap: Swap::BackrunSwapLine(swap_line),
                    origin: Some(state_update_event.origin.clone()),
                    tips_pct: Some(state_update_event.tips_pct),
                    poststate: Some(arc_db.clone()),
                    poststate_update: Some(state_update_event.state_update().clone()),
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
    info!(
        origin = %state_update_event.origin,
        swap_path_vec_len,
        answers,
        elapsed = %(chrono::Local::now() - start_time),
        stuffing_hash = %state_update_event.stuffing_tx_hash(),
        "Calculation finished"
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
    info!("Starting state arb searcher cpus={cpus}, tasks={}", cpus / 2);
    let thread_pool = Arc::new(ThreadPoolBuilder::new().num_threads(cpus / 2).build()?);

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

lazy_static! {
    static ref START_OPTIMIZE_INPUT: U256 = parse_units("0.01", "ether").unwrap().get_absolute();
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
