use std::collections::HashMap;

use alloy_eips::BlockNumberOrTag;
use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Local};
use eyre::Result;
use log::{error, info, warn};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_entities::MarketState;
use defi_events::{MarketEvents, MessageTxCompose, TxCompose};
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

async fn verify_pool_state_task<P: Provider + 'static>(
    client: P,
    address: Address,
    market_state: SharedState<MarketState>,
) -> Result<()> {
    info!("Verifying state {address:?}");
    let account = market_state.write().await.state_db.load_account(address).cloned()?;
    let read_only_cell_hash_set = market_state.read().await.read_only_cells.get(&address).cloned().unwrap_or_default();


    for (cell, current_value) in account.storage.iter() {
        if read_only_cell_hash_set.contains(cell) {
            continue;
        }
        match client.get_storage_at(address, *cell).block_id(BlockNumberOrTag::Latest.into()).await {
            Ok(actual_value) => {
                if actual_value.is_zero() {
                    continue;
                }
                let actual_value: U256 = actual_value.into();
                if *current_value != actual_value {
                    warn!("verify : account storage is different : {address:?} {cell:?} {current_value:#32x} -> {actual_value:#32x} storage size : {}", account.storage.len());
                    match market_state.write().await.state_db.insert_account_storage(address, *cell, actual_value) {
                        Err(e) => error!("{e}"),
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("Cannot read storage {:?} {:?} : {}", account, cell, e)
            }
        }
    }

    Ok(())
}

pub async fn state_health_monitor_worker<P: Provider + Clone + 'static>(
    client: P,
    market_state: SharedState<MarketState>,
    mut tx_compose_channel_rx: Receiver<MessageTxCompose>,
    mut market_events_rx: Receiver<MarketEvents>,
) -> WorkerResult
{
    let mut check_time_map: HashMap<Address, DateTime<Local>> = HashMap::new();
    let mut pool_address_to_verify_vec: Vec<Address> = Vec::new();

    loop {
        tokio::select! {
            msg = market_events_rx.recv() => {
                let market_event_msg : Result<MarketEvents, RecvError> = msg;
                match market_event_msg {
                    Ok(market_event)=>{
                        match market_event {
                            MarketEvents::BlockStateUpdate{..}=>{
                                for pool_address in pool_address_to_verify_vec {
                                    tokio::task::spawn(
                                        verify_pool_state_task(
                                            client.clone(),
                                            pool_address,
                                            market_state.clone()
                                        )
                                    );
                                }
                                pool_address_to_verify_vec = Vec::new();
                            }
                            _=>{}
                        }

                    }
                    Err(e)=>{error!("market_event_rx error : {e}")}
                }
            },

            msg = tx_compose_channel_rx.recv() => {
                let tx_compose_update : Result<MessageTxCompose, RecvError>  = msg;
                match tx_compose_update {
                    Ok(tx_compose_msg)=>{
                        match tx_compose_msg.inner {
                            TxCompose::Broadcast(broadcast_data)=>{
                                let pool_address_vec =  broadcast_data.swap.get_pool_address_vec();
                                let now = chrono::Local::now();
                                for pool_address in pool_address_vec {
                                    if now - check_time_map.get(&pool_address).cloned().unwrap_or(DateTime::<Local>::MIN_UTC.into()) > Duration::seconds(60) {
                                        check_time_map.insert(pool_address, Local::now());
                                        if !pool_address_to_verify_vec.contains(&pool_address){
                                            pool_address_to_verify_vec.push(pool_address)
                                        }
                                    }
                                }
                            }
                            _=>{

                            }
                        }

                    }
                    Err(e)=>{
                        error!("tx_compose_channel_rx : {e}")
                    }
                }

            }
        }
    }
}

#[derive(Accessor, Consumer)]
pub struct StateHealthMonitorActor<P>
{
    client: P,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[consumer]
    tx_compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
}

impl<P> StateHealthMonitorActor<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        StateHealthMonitorActor {
            client,
            market_state: None,
            tx_compose_channel_rx: None,
            market_events_rx: None,
        }
    }
}


#[async_trait]
impl<P> Actor for StateHealthMonitorActor<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    async fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(
            state_health_monitor_worker(
                self.client.clone(),
                self.market_state.clone().unwrap(),
                self.tx_compose_channel_rx.clone().unwrap().subscribe().await,
                self.market_events_rx.clone().unwrap().subscribe().await,
            )
        );
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "StateHealthMonitorActor"
    }
}