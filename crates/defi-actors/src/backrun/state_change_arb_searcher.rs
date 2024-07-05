use std::collections::BTreeMap;
use std::sync::Arc;

use alloy_primitives::{Address, U256};
use async_trait::async_trait;
use chrono::TimeDelta;
use eyre::{eyre, Result};
use log::{debug, error, warn};
use rayon::{ThreadPool, ThreadPoolBuilder};
use rayon::prelude::*;
use revm::InMemoryDB;
use revm::primitives::Env;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_entities::{GasStation, Market, MarketState, PoolWrapper, Swap, SwapLine, SwapPath};
use defi_events::{BestTxCompose, HealthEvent, Message, MessageHealthEvent, MessageTxCompose, TxComposeData};
use defi_types::SwapError;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

use super::messages::MessageSearcherPoolStateUpdate;

async fn state_change_arb_searcher_task(
    thread_pool: Arc<ThreadPool>,
    smart: bool,
    msg: MessageSearcherPoolStateUpdate,
    market: SharedState<Market>,
    swap_request_tx: Broadcaster<MessageTxCompose>,
    pool_health_monitor_tx: Broadcaster<MessageHealthEvent>,
) -> Result<()> {
    debug!("Message received {} stuffing : {:?}", msg.origin, msg.stuffing_tx_hash() );
    //let msg_time = chrono::Local::now();


    let db = msg.market_state().clone();
    let mut current_market_state = MarketState::new(db);

    current_market_state.apply_state_update(msg.state_update(), true, false);

    let start_time = chrono::Local::now();
    let mut swap_path_vec: Vec<Arc<SwapPath>> = Vec::new();

    let market_guard_read = market.read().await;
    for (pool, v) in msg.directions().iter() {
        let pool_paths: Vec<Arc<SwapPath>> = match market_guard_read.get_pool_paths(&pool.get_address()) {
            Some(paths) => {
                paths.into_iter().filter(|swap_path| !swap_path.pools.iter().any(|pool| !market_guard_read.is_pool_ok(&pool.get_address()))).collect()
            }
            None => {
                let mut pool_direction: BTreeMap<PoolWrapper, Vec<(Address, Address)>> = BTreeMap::new();
                pool_direction.insert(pool.clone(), v.clone());
                market_guard_read.build_swap_path_vec(&pool_direction).unwrap_or_default()
            }
        };

        swap_path_vec.extend(pool_paths)
    }
    drop(market_guard_read);


    if swap_path_vec.len() == 0 {
        warn!("No swap path built for request: {:?} {}", msg.stuffing_txs_hashes().first().unwrap_or_default(), chrono::Local::now() - start_time);
        return Err(eyre!("NO_SWAP_PATHS"));
    }
    warn!("Calculation started {} {}", swap_path_vec.len(), chrono::Local::now() - start_time );

    //let state_db = msg.market_state().clone();
    let env = msg.evm_env();

    //let channel_len = if swap_path_vec.len() > 1000 { 1000 } else {swap_path_vec.len()};

    let channel_len = swap_path_vec.len();
    let (swap_path_tx, mut swap_line_rx) = tokio::sync::mpsc::channel(channel_len);


    let market_state_clone = current_market_state.state_db.clone();
    let swap_path_vec_len = swap_path_vec.len();

    tokio::task::spawn(async move {
        //let pool = ThreadPoolBuilder::new().num_threads(20).build().unwrap();

        thread_pool.install(|| {
            swap_path_vec.into_par_iter().for_each_with((&swap_path_tx, &market_state_clone, &env, &pool_health_monitor_tx), |req, item| {
                let mut mut_item: SwapLine = SwapLine {
                    path: item.as_ref().clone(),
                    ..Default::default()
                };
                let start_time = chrono::Local::now();
                let calc_result = Calculator::calculate(&mut mut_item, req.1, req.2.clone());
                let took_time = chrono::Local::now() - start_time;

                match calc_result {
                    Ok(_) => {
                        if took_time > TimeDelta::new(0, 10 * 1000000).unwrap() {
                            warn!("Took longer than expected {} {}", took_time,  mut_item.clone())
                        }
                        if let Ok(profit) = mut_item.profit() {
                            if profit.is_positive() && msg.gas_fee != 0 && mut_item.abs_profit_eth() > GasStation::calc_gas_cost(200000u128, msg.gas_fee) {
                                match swap_path_tx.try_send(mut_item.clone()) {
                                    Err(e) => { error!("try_send 1 error : {e}") }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if took_time > TimeDelta::new(0, 10 * 1000000).unwrap() {
                            warn!("Took longer than expected {:?} {}", e, mut_item.clone())
                        }

                        let pool_health_tx = req.3;
                        match pool_health_tx.try_send(Message::new(HealthEvent::PoolSwapError(e.clone()))) {
                            Err(ee) => { error!("try_send to pool_health_monitor error : {e:?}") }
                            _ => {}
                        }
                    }
                }
            });
        });
        warn!("Calculation iteration finished {}", chrono::Local::now() - start_time);
    });


    warn!("Calculation results receiver started {}", chrono::Local::now() - start_time);

    let swap_request_tx_clone = swap_request_tx.clone();
    let arc_db = Arc::new(current_market_state.state_db);


    let mut answers = 0;

    let mut best_answers = BestTxCompose::new_with_pct(U256::from(9000));

    while let Some(swap_line) = swap_line_rx.recv().await {
        //let msg  = MessageSwapPathEncodeRequest::new(result.clone(), msg.stuffing_txs(), msg.state_update().clone(), msg.state_required().clone());

        let encode_request = MessageTxCompose::encode(
            TxComposeData {
                block: msg.block,
                block_timestamp: msg.block_timestamp,
                gas_fee: msg.gas_fee,
                gas: swap_line.gas_used.unwrap_or(300000) as u128,
                stuffing_txs: msg.stuffing_txs.clone(),
                stuffing_txs_hashes: msg.stuffing_txs_hashes.clone(),
                swap: Swap::BackrunSwapLine(swap_line),
                origin: Some(msg.origin.clone()),
                tips_pct: Some(msg.tips_pct),
                poststate: Some(arc_db.clone()),
                poststate_update: Some(msg.state_update().clone()),
                ..TxComposeData::default()
            }
        );

        if !smart || best_answers.check(&encode_request) {
            match swap_request_tx_clone.send(encode_request).await {
                Err(e) => { error!("{}",e) }
                _ => {}
            }
        }

        answers += 1;
    }
    warn!("Calculation finished. Origin : {} {} {} {} stuffing hash : {:?}", msg.origin, swap_path_vec_len, answers, chrono::Local::now() - start_time, msg.stuffing_tx_hash());


    Ok(())
}


pub async fn state_change_arb_searcher_worker(
    smart: bool,
    market: SharedState<Market>,
    mut search_request_rx: Receiver<MessageSearcherPoolStateUpdate>,
    swap_request_tx: Broadcaster<MessageTxCompose>,
    pool_health_monitor_tx: Broadcaster<MessageHealthEvent>,
) -> WorkerResult {
    let cpus = num_cpus::get();
    println!("Cpus {cpus}");
    let thread_pool = Arc::new(ThreadPoolBuilder::new().num_threads(cpus - 2).build().unwrap());


    loop {
        tokio::select! {
                msg = search_request_rx.recv() => {
                let pool_update_msg : Result<MessageSearcherPoolStateUpdate, RecvError> = msg;
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

impl Calculator
{
    pub fn calculate<'a>(path: &'a mut SwapLine, state: &InMemoryDB, env: Env) -> Result<&'a mut SwapLine, SwapError> {
        let first_token = path.get_first_token().unwrap();
        let amount_in = first_token.calc_token_value_from_eth(U256::from(10).pow(U256::from(17))).unwrap();
        //trace!("calculate : {} amount in : {}",first_token.get_symbol(), first_token.to_float(amount_in) );
        path.optimize_with_in_amount(state, env, amount_in)
    }
}


#[derive(Accessor, Consumer, Producer)]
pub struct StateChangeArbSearcherActor
{
    smart: bool,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[consumer]
    pool_state_update_rx: Option<Broadcaster<MessageSearcherPoolStateUpdate>>,
    #[producer]
    swap_arb_request_tx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    pool_health_monitor_tx: Option<Broadcaster<MessageHealthEvent>>,
}


impl StateChangeArbSearcherActor
{
    pub fn new(smart: bool) -> StateChangeArbSearcherActor {
        StateChangeArbSearcherActor {
            smart,
            market: None,
            pool_state_update_rx: None,
            swap_arb_request_tx: None,
            pool_health_monitor_tx: None,
        }
    }
}

#[async_trait]
impl Actor for StateChangeArbSearcherActor
{
    async fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(
            state_change_arb_searcher_worker(
                self.smart,
                self.market.clone().unwrap(),
                self.pool_state_update_rx.clone().unwrap().subscribe().await,
                self.swap_arb_request_tx.clone().unwrap(),
                self.pool_health_monitor_tx.clone().unwrap(),
            )
        );
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "StateChangeArbSearcherActor"
    }
}
