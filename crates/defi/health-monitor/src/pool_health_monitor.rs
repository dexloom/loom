use eyre::Result;
use influxdb::{Timestamp, WriteQuery};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info};

use loom_core_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::Blockchain;
use loom_types_entities::{Market, PoolId};
use loom_types_events::{HealthEvent, MessageHealthEvent};

pub async fn pool_health_monitor_worker(
    market: SharedState<Market>,
    pool_health_monitor_rx: Broadcaster<MessageHealthEvent>,
    influx_channel_tx: Broadcaster<WriteQuery>,
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


                                if !market_guard.is_pool_disabled(&swap_error.pool) {
                                    market_guard.set_pool_disabled(swap_error.pool, true);


                                    match market_guard.get_pool(&swap_error.pool) {
                                        Some(pool)=>{
                                            info!("Disabling pool: protocol={}, address={:?}, msg={} amount={}", pool.get_protocol(),swap_error.pool, swap_error.msg, swap_error.amount);

                                            let amount_f64 = if let Some(token_in) = market_guard.get_token(&swap_error.token_from) {
                                                token_in.to_float(swap_error.amount)
                                            } else {
                                                -1.0f64
                                            };

                                            let pool_protocol = pool.get_protocol().to_string();
                                            let pool_id = pool.get_pool_id().to_string();
                                            let influx_channel_clone = influx_channel_tx.clone();

                                            if let Err(e) = tokio::time::timeout(
                                                Duration::from_secs(1),
                                                async move {
                                                    let start_time_utc =   chrono::Utc::now();

                                                    let write_query = WriteQuery::new(Timestamp::from(start_time_utc), "pool_disabled")
                                                        .add_field("message", swap_error.msg)
                                                        .add_field("amount", amount_f64)
                                                        .add_tag("id", pool_id)
                                                        .add_tag("protocol", pool_protocol)
                                                        .add_tag("token_from", swap_error.token_from.to_checksum(None))
                                                        .add_tag("token_to", swap_error.token_to.to_checksum(None));

                                                    if let Err(e) = influx_channel_clone.send(write_query).await {
                                                       error!("Failed to failed pool to influxdb: {:?}", e);
                                                    }
                                                }
                                            ).await {
                                                error!("Failed to send failed pool info to influxdb: {:?}", e);
                                            }



                                        }
                                        _=>{
                                            error!("Disabled pool missing in market: address={:?}, msg={} amount={}", swap_error.pool, swap_error.msg, swap_error.amount);
                                        }
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

#[derive(Accessor, Consumer, Producer, Default)]
pub struct PoolHealthMonitorActor {
    #[accessor]
    market: Option<SharedState<Market>>,
    #[consumer]
    pool_health_update_rx: Option<Broadcaster<MessageHealthEvent>>,
    #[producer]
    influxdb_tx: Option<Broadcaster<WriteQuery>>,
}

impl PoolHealthMonitorActor {
    pub fn new() -> Self {
        PoolHealthMonitorActor::default()
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            market: Some(bc.market()),
            pool_health_update_rx: Some(bc.pool_health_monitor_channel()),
            influxdb_tx: Some(bc.influxdb_write_channel()),
        }
    }
}

impl Actor for PoolHealthMonitorActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(pool_health_monitor_worker(
            self.market.clone().unwrap(),
            self.pool_health_update_rx.clone().unwrap(),
            self.influxdb_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "PoolHealthMonitorActor"
    }
}
