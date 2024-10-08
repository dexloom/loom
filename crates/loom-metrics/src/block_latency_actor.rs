use defi_blockchain::Blockchain;
use defi_events::MessageBlockHeader;
use eyre::eyre;
use influxdb::{Timestamp, WriteQuery};
use loom_actors::Consumer;
use loom_actors::Producer;
use loom_actors::{subscribe, Actor, ActorResult, Broadcaster, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, info};

async fn block_latency_worker(
    block_header_update_rx: Broadcaster<MessageBlockHeader>,
    influx_channel_tx: Broadcaster<WriteQuery>,
) -> WorkerResult {
    let mut last_block_number = 0;
    let mut last_block_hash = Default::default();

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
        let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "block_latency")
            .add_field("value", block_latency)
            .add_field("block_number", block_header.inner.header.number);
        if let Err(e) = influx_channel_tx.send(write_query).await {
            error!("Failed to send block latency to influxdb: {:?}", e);
        }

        // check if we received twice the same block number
        if last_block_number == block_header.inner.header.number {
            // check that we have not received the same block hash
            if last_block_hash != block_header.header.hash {
                let write_query = WriteQuery::new(Timestamp::from(current_timestamp), "reorg_detected")
                    .add_field("block_number", block_header.inner.header.number);
                if let Err(e) = influx_channel_tx.send(write_query).await {
                    error!("Failed to send block reorg to influxdb: {:?}", e);
                }
            }
        }

        last_block_number = block_header.inner.header.number;
        last_block_hash = block_header.header.hash;
    }
}

#[derive(Accessor, Consumer, Producer, Default)]
pub struct BlockLatencyRecorderActor {
    #[consumer]
    block_header_rx: Option<Broadcaster<MessageBlockHeader>>,
    #[producer]
    influxdb_write_channel_tx: Option<Broadcaster<WriteQuery>>,
}

impl BlockLatencyRecorderActor {
    pub fn new() -> Self {
        Self { block_header_rx: None, influxdb_write_channel_tx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { block_header_rx: Some(bc.new_block_headers_channel()), influxdb_write_channel_tx: Some(bc.influxdb_write_channel()) }
    }
}

impl Actor for BlockLatencyRecorderActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(block_latency_worker(
            self.block_header_rx.clone().unwrap(),
            self.influxdb_write_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "BlockLatencyRecorderActor"
    }
}
