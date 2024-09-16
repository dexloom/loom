use async_trait::async_trait;
use eyre::eyre;
use influxdb::{Client, ReadQuery, WriteQuery};
use log::{error, info, warn};
use loom_actors::Consumer;
use loom_actors::{Actor, ActorResult, Broadcaster, WorkerResult};
use loom_actors_macros::Consumer;

pub async fn start_influxdb_worker(
    url: String,
    db_name: String,
    bot_name: String,
    event_receiver: Broadcaster<WriteQuery>,
) -> WorkerResult {
    info!("Starting influx writer...");

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
                event = event.add_tag("bot_name", bot_name.clone());
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
pub struct InfluxDbActor {
    url: String,
    db_name: String,
    bot_name: String,
    #[consumer]
    influxdb_write_channel_rx: Option<Broadcaster<WriteQuery>>,
}

impl InfluxDbActor {
    pub fn new(url: String, db_name: String, bot_name: String) -> Self {
        Self { url, db_name, bot_name, influxdb_write_channel_rx: None }
    }
}

#[async_trait]
impl Actor for InfluxDbActor {
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
            self.bot_name.clone(),
            influxdb_write_channel_rx.clone(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "InfluxDbActor"
    }
}
