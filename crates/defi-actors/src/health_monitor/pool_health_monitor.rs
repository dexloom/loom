use std::collections::HashMap;

use alloy_primitives::Address;
use async_trait::async_trait;
use eyre::Result;
use log::{debug, error};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_blockchain::Blockchain;
use defi_entities::Market;
use defi_events::{HealthEvent, MessageHealthEvent};
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

pub async fn pool_health_monitor_worker(
    market: SharedState<Market>,
    mut pool_health_monitor_rx: Receiver<MessageHealthEvent>,
) -> WorkerResult {
    let mut pool_errors_map: HashMap<Address, u32> = HashMap::new();
    //let mut watch_txs : HashMap<H256, u64>;

    loop {
        tokio::select! {
            msg = pool_health_monitor_rx.recv() => {

                let pool_health_update : Result<MessageHealthEvent, RecvError>  = msg;
                match pool_health_update {
                    Ok(pool_health_message)=>{
                        if let HealthEvent::PoolSwapError(swap_error) = pool_health_message.inner {
                            debug!("Pool health_monitor message update: {:?} {} {} ", swap_error.pool, swap_error.msg, swap_error.amount);
                            let entry = pool_errors_map.entry(swap_error.pool).or_insert(0);
                            *entry += 1;
                            if *entry == 100 {
                                let mut market_guard = market.write().await;
                                market_guard.set_pool_ok(swap_error.pool, false);
                                match market_guard.get_pool(&swap_error.pool) {
                                    Some(pool)=>{
                                        error!("Disabling pool  {}@{:?} {} {}", pool.get_protocol(),swap_error.pool, swap_error.msg, swap_error.amount);
                                    }
                                    _=>{
                                        error!("Disabling pool NOT_FOUND@{:?} {} {}", swap_error.pool, swap_error.msg, swap_error.amount);
                                    }
                                }
                            }

                            if *entry > 100  {
                                error!("Pool disabled : {:?} {} {}", swap_error.pool, swap_error.msg, swap_error.amount);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("pool_health_update error {}", e)
                    }
                }

            }
        }
    }
}

#[derive(Accessor, Consumer, Default)]
pub struct PoolHealthMonitorActor {
    #[accessor]
    market: Option<SharedState<Market>>,
    #[consumer]
    pool_health_update_rx: Option<Broadcaster<MessageHealthEvent>>,
}

impl PoolHealthMonitorActor {
    pub fn new() -> Self {
        PoolHealthMonitorActor::default()
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market: Some(bc.market()), pool_health_update_rx: Some(bc.pool_health_monitor_channel()) }
    }
}

#[async_trait]
impl Actor for PoolHealthMonitorActor {
    async fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(pool_health_monitor_worker(
            self.market.clone().unwrap(),
            self.pool_health_update_rx.clone().unwrap().subscribe().await,
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "PoolHealthMonitorActor"
    }
}
