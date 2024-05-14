use alloy_provider::Provider;
use async_trait::async_trait;
use eyre::Result;
use log::{debug, error, info};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use debug_provider::DebugProviderExt;
use defi_entities::{Market, MarketState};
use defi_events::BlockLogsUpdate;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

use crate::market::logs_parser::process_log_entries;

pub async fn new_pool_worker<P>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
    mut log_update_rx: Receiver<BlockLogsUpdate>,
) -> WorkerResult
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    loop {
        tokio::select! {
            msg = log_update_rx.recv() => {
                debug!("Log update");

                let log_update : Result<BlockLogsUpdate, RecvError>  = msg;
                match log_update {
                    Ok(log_update_msg)=>{
                        if let Ok(pool_address_vec) = process_log_entries(
                                client.clone(),
                                market.clone(),
                                market_state.clone(),
                                log_update_msg.logs,
                        ).await {
                            info!("Pools added : {:?}", pool_address_vec)
                        }
                    }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }

            }
        }
    }
}

#[derive(Accessor, Consumer)]
pub struct NewPoolLoaderActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[consumer]
    log_update_rx: Option<Broadcaster<BlockLogsUpdate>>,
}

impl<P> NewPoolLoaderActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        NewPoolLoaderActor {
            client,
            market: None,
            market_state: None,
            log_update_rx: None,
        }
    }
}


#[async_trait]
impl<P> Actor for NewPoolLoaderActor<P>
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    async fn start(&mut self) -> ActorResult {
        let task = tokio::task::spawn(
            new_pool_worker(
                self.client.clone(),
                self.market.clone().unwrap(),
                self.market_state.clone().unwrap(),
                self.log_update_rx.clone().unwrap().subscribe().await,
            )
        );
        Ok(vec![task])
    }
}
