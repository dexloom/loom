use std::marker::PhantomData;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;
use async_trait::async_trait;
use eyre::Result;
use log::{debug, error, info};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::{Market, MarketState};
use defi_events::BlockLogs;
use loom_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

use crate::market::logs_parser::process_log_entries;

pub async fn new_pool_worker<P, T, N>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
    log_update_rx: Broadcaster<BlockLogs>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    subscribe!(log_update_rx);

    loop {
        tokio::select! {
            msg = log_update_rx.recv() => {
                debug!("Log update");

                let log_update : Result<BlockLogs, RecvError>  = msg;
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
pub struct NewPoolLoaderActor<P, T, N> {
    client: P,
    #[accessor]
    market: Option<SharedState<Market>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[consumer]
    log_update_rx: Option<Broadcaster<BlockLogs>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> NewPoolLoaderActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        NewPoolLoaderActor { client, market: None, market_state: None, log_update_rx: None, _t: PhantomData, _n: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market: Some(bc.market()), market_state: Some(bc.market_state()), log_update_rx: Some(bc.new_block_logs_channel()), ..self }
    }
}

#[async_trait]
impl<P, T, N> Actor for NewPoolLoaderActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_pool_worker(
            self.client.clone(),
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.log_update_rx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "NewPoolLoaderActor"
    }
}
