use async_trait::async_trait;
use defi_blockchain::Blockchain;
use eyre::eyre;
use influxdb::{Client, ReadQuery, WriteQuery};
use log::{error, info, warn};
use loom_actors::Consumer;
use loom_actors::{Actor, ActorResult, Broadcaster, WorkerResult};
use loom_actors_macros::Consumer;
use std::collections::HashMap;

pub async fn start_influxdb_worker(
    url: String,
    db_name: String,
    tags: HashMap<String, String>,
    event_receiver: Broadcaster<WriteQuery>,
) -> WorkerResult {
    let client = Client::new(url, db_name.clone());
    let create_db_stmt = format!("CREATE DATABASE {}", db_name);
    let result = client.query(ReadQuery::new(create_db_stmt)).await;
    match result {
        Ok(_) => info!("Database created with name: {}", db_name),
        Err(e) => info!("Database creation failed/exists: {:?}", e),
    }
    let mut event_receiver = event_receiver.subscribe().await;
    loop {
        let event_result = event_receiver.recv().await;
        match event_result {
            Ok(mut event) => {
                for (key, value) in tags.iter() {
                    event = event.add_tag(key, value.clone());
                }
                let write_result = client.query(event).await;
                if write_result.is_err() {
                    info!("Write failed: {:?}", write_result.err().unwrap());
                }
            }
            Err(e) => {
                info!("Receiver failed: {:?}", e);
                match e {
                    tokio::sync::broadcast::error::RecvError::Closed => {
                        error!("InfluxDB channel closed");
                        return Err(eyre!("INFLUXDB_CHANNEL_CLOSED"));
                    }
                    tokio::sync::broadcast::error::RecvError::Lagged(lagged) => {
                        warn!("InfluxDB lagged: {:?}", lagged);
                        continue;
                    }
                }
            }
        }
    }
}

#[derive(Consumer)]
pub struct InfluxDbWriterActor {
    url: String,
    db_name: String,
    tags: HashMap<String, String>,
    #[consumer]
    influxdb_write_channel_rx: Option<Broadcaster<WriteQuery>>,
}

impl InfluxDbWriterActor {
    pub fn new(url: String, db_name: String, tags: HashMap<String, String>) -> Self {
        Self { url, db_name, tags, influxdb_write_channel_rx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { influxdb_write_channel_rx: Some(bc.influxdb_write_channel()), ..self }
    }
}

#[async_trait]
impl Actor for InfluxDbWriterActor {
    fn start(&self) -> ActorResult {
        let influxdb_write_channel_rx = match &self.influxdb_write_channel_rx {
            Some(rx) => rx.clone(),
            None => {
                error!("InfluxDB write channel is not set.");
                return Err(eyre!("INFLUXDB_WRITE_CHANNEL_NOT_SET"));
            }
        };
        let task = tokio::task::spawn(start_influxdb_worker(
            self.url.clone(),
            self.db_name.clone(),
            self.tags.clone(),
            influxdb_write_channel_rx.clone(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "InfluxDbWriterActor"
    }
}
