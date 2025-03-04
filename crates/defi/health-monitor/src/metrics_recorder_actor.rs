use eyre::eyre;
use influxdb::{Timestamp, WriteQuery};
use loom_core_actors::Producer;
use loom_core_actors::{subscribe, Actor, ActorResult, Broadcaster, WorkerResult};
use loom_core_actors::{Accessor, Consumer, SharedState};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_evm_db::DatabaseLoomExt;
use loom_types_entities::{Market, MarketState};
use loom_types_events::MessageBlockHeader;
use revm::DatabaseRef;
use std::time::Duration;
use tikv_jemalloc_ctl::stats;
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, info};

async fn metrics_recorder_worker<DB: DatabaseLoomExt + DatabaseRef + Send + Sync + 'static>(
    market: SharedState<Market>,
    market_state: SharedState<MarketState<DB>>,
    block_header_update_rx: Broadcaster<MessageBlockHeader>,
    influx_channel_tx: Broadcaster<WriteQuery>,
) -> WorkerResult {
    subscribe!(block_header_update_rx);
    loop {
        let block_header = match block_header_update_rx.recv().await {
            Ok(block) => block,
            Err(e) => match e {
                RecvError::Closed => {
                    error!("Block header channel closed");
                    return Err(eyre!("Block header channel closed".to_string()));
                }
                RecvError::Lagged(lag) => {
                    info!("Block header channel lagged: {}", lag);
                    continue;
                }
            },
        };

        let current_timestamp = chrono::Utc::now();
        let block_latency = current_timestamp.timestamp() as f64 - block_header.inner.header.timestamp as f64;

        // check if we received twice the same block number

        let allocated = stats::allocated::read().unwrap_or_default();

        let market_state_guard = market_state.read().await;
        let accounts = market_state_guard.state_db.accounts_len();
        let contracts = market_state_guard.state_db.contracts_len();

        drop(market_state_guard);

        let market_guard = market.read().await;
        let pools_disabled = market_guard.disabled_pools_count();
        let paths = market_guard.swap_paths().len();
        let paths_disabled = market_guard.swap_paths().disabled_len();
        drop(market_guard);

        let influx_channel_clone = influx_channel_tx.clone();

        if let Err(e) = tokio::time::timeout(Duration::from_secs(2), async move {
            let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "state_accounts")
                .add_field("value", accounts as f32)
                .add_field("block_number", block_header.inner.header.number);
            if let Err(e) = influx_channel_clone.send(write_query) {
                error!("Failed to send block latency to influxdb: {:?}", e);
            }

            let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "state_contracts")
                .add_field("value", contracts as f32)
                .add_field("block_number", block_header.inner.header.number);
            if let Err(e) = influx_channel_clone.send(write_query) {
                error!("Failed to send block latency to influxdb: {:?}", e);
            }

            let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "pools_disabled")
                .add_field("value", pools_disabled as f32)
                .add_field("block_number", block_header.inner.header.number);
            if let Err(e) = influx_channel_clone.send(write_query) {
                error!("Failed to send pools_disabled to influxdb: {:?}", e);
            }

            let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "paths")
                .add_field("value", paths as f32)
                .add_field("block_number", block_header.inner.header.number);
            if let Err(e) = influx_channel_clone.send(write_query) {
                error!("Failed to send pools_disabled to influxdb: {:?}", e);
            }

            let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "paths_disabled")
                .add_field("value", paths_disabled as f32)
                .add_field("block_number", block_header.inner.header.number);
            if let Err(e) = influx_channel_clone.send(write_query) {
                error!("Failed to send pools_disabled to influxdb: {:?}", e);
            }

            let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "pools_disabled")
                .add_field("value", pools_disabled as f32)
                .add_field("block_number", block_header.inner.header.number);
            if let Err(e) = influx_channel_clone.send(write_query) {
                error!("Failed to send pools_disabled to influxdb: {:?}", e);
            }

            let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "jemalloc_allocated")
                .add_field("value", (allocated >> 20) as f32)
                .add_field("block_number", block_header.inner.header.number);
            if let Err(e) = influx_channel_clone.send(write_query) {
                error!("Failed to send jemalloc_allocator latency to influxdb: {:?}", e);
            }

            let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "block_latency")
                .add_field("value", block_latency)
                .add_field("block_number", block_header.inner.header.number);
            if let Err(e) = influx_channel_clone.send(write_query) {
                error!("Failed to send block latency to influxdb: {:?}", e);
            }
        })
        .await
        {
            error!("Failed to send data to influxdb: {:?}", e);
        }
    }
}

#[derive(Accessor, Consumer, Producer, Default)]
pub struct MetricsRecorderActor<DB: Clone + Send + Sync + 'static> {
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    #[consumer]
    block_header_rx: Option<Broadcaster<MessageBlockHeader>>,
    #[producer]
    influxdb_write_channel_tx: Option<Broadcaster<WriteQuery>>,
}

impl<DB> MetricsRecorderActor<DB>
where
    DB: DatabaseRef + DatabaseLoomExt + Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self { market: None, market_state: None, block_header_rx: None, influxdb_write_channel_tx: None }
    }

    pub fn on_bc(self, bc: &Blockchain, bc_state: &BlockchainState<DB>) -> Self {
        Self {
            market: Some(bc.market()),
            market_state: Some(bc_state.market_state()),
            block_header_rx: Some(bc.new_block_headers_channel()),
            influxdb_write_channel_tx: Some(bc.influxdb_write_channel()),
        }
    }
}

impl<DB> Actor for MetricsRecorderActor<DB>
where
    DB: DatabaseRef + DatabaseLoomExt + Clone + Send + Sync + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(metrics_recorder_worker(
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.block_header_rx.clone().unwrap(),
            self.influxdb_write_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "BlockLatencyRecorderActor"
    }
}
