use std::collections::HashMap;

use eyre::Result;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info};

use loom_core_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer};
use loom_core_blockchain::Blockchain;
use loom_types_entities::{Market, PoolId};
use loom_types_events::{HealthEvent, MessageHealthEvent};

pub async fn pool_health_monitor_worker(
    market: SharedState<Market>,
    pool_health_monitor_rx: Broadcaster<MessageHealthEvent>,
) -> WorkerResult {
    subscribe!(pool_health_monitor_rx);

    let mut pool_errors_map: HashMap<PoolId, u32> = HashMap::new();

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
                            if *entry >= 10 {
                                let start_time=std::time::Instant::now();
                                let mut market_guard = market.write().await;
                                debug!(elapsed = start_time.elapsed().as_micros(), "market_guard market.write acquired");

                                market_guard.set_pool_disabled(swap_error.pool, true);
                                match market_guard.get_pool(&swap_error.pool) {
                                    Some(pool)=>{
                                        info!("Disabling pool: protocol={}, address={:?}, msg={} amount={}", pool.get_protocol(),swap_error.pool, swap_error.msg, swap_error.amount);
                                    }
                                    _=>{
                                        error!("Disabled pool missing in market: address={:?}, msg={} amount={}", swap_error.pool, swap_error.msg, swap_error.amount);
                                    }
                                }
                                drop(market_guard);
                                debug!(elapsed = start_time.elapsed().as_micros(), "market_guard market.write released");

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

impl Actor for PoolHealthMonitorActor {
    fn start(&self) -> ActorResult {
        let task =
            tokio::task::spawn(pool_health_monitor_worker(self.market.clone().unwrap(), self.pool_health_update_rx.clone().unwrap()));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "PoolHealthMonitorActor"
    }
}
