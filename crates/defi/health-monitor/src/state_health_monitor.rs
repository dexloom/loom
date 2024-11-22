use std::collections::HashMap;
use std::marker::PhantomData;

use alloy_eips::BlockNumberOrTag;
use alloy_network::Ethereum;
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_transport::Transport;
use chrono::{DateTime, Duration, Local};
use eyre::Result;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tracing::{error, info, warn};

use loom_core_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer};
use loom_core_blockchain::Blockchain;
use loom_evm_db::DatabaseLoomExt;
use loom_types_entities::MarketState;
use loom_types_events::{MarketEvents, MessageSwapCompose, SwapComposeMessage};
use revm::DatabaseRef;

async fn verify_pool_state_task<T: Transport + Clone, P: Provider<T, Ethereum> + 'static, DB: DatabaseLoomExt>(
    client: P,
    address: Address,
    market_state: SharedState<MarketState<DB>>,
) -> Result<()> {
    info!("Verifying state {address:?}");
    let account = market_state.write().await.state_db.load_account(address).cloned()?;
    let read_only_cell_hash_set = market_state.read().await.config.read_only_cells.get(&address).cloned().unwrap_or_default();

    for (cell, current_value) in account.storage.iter() {
        if read_only_cell_hash_set.contains(cell) {
            continue;
        }
        match client.get_storage_at(address, *cell).block_id(BlockNumberOrTag::Latest.into()).await {
            Ok(actual_value) => {
                if actual_value.is_zero() {
                    continue;
                }
                if *current_value != actual_value {
                    warn!("verify : account storage is different : {address:?} {cell:?} {current_value:#32x} -> {actual_value:#32x} storage size : {}", account.storage.len());
                    if let Err(e) = market_state.write().await.state_db.insert_account_storage(address, *cell, actual_value) {
                        error!("{e}");
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

pub async fn state_health_monitor_worker<
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Clone + 'static,
    DB: DatabaseRef + DatabaseLoomExt + Send + Sync + Clone + 'static,
>(
    client: P,
    market_state: SharedState<MarketState<DB>>,
    tx_compose_channel_rx: Broadcaster<MessageSwapCompose<DB>>,
    market_events_rx: Broadcaster<MarketEvents>,
) -> WorkerResult {
    let mut tx_compose_channel_rx: Receiver<MessageSwapCompose<DB>> = tx_compose_channel_rx.subscribe().await;
    let mut market_events_rx: Receiver<MarketEvents> = market_events_rx.subscribe().await;

    let mut check_time_map: HashMap<Address, DateTime<Local>> = HashMap::new();
    let mut pool_address_to_verify_vec: Vec<Address> = Vec::new();

    loop {
        tokio::select! {
            msg = market_events_rx.recv() => {
                let market_event_msg : Result<MarketEvents, RecvError> = msg;
                match market_event_msg {
                    Ok(market_event)=>{
                        if matches!(market_event, MarketEvents::BlockStateUpdate{..}) {
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
                    }
                    Err(e)=>{error!("market_event_rx error : {e}")}
                }
            },

            msg = tx_compose_channel_rx.recv() => {
                let tx_compose_update : Result<MessageSwapCompose<DB>, RecvError>  = msg;
                match tx_compose_update {
                    Ok(tx_compose_msg)=>{
                        if let SwapComposeMessage::Broadcast(broadcast_data)= tx_compose_msg.inner {
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
pub struct StateHealthMonitorActor<P, T, DB: Clone + Send + Sync + 'static> {
    client: P,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    #[consumer]
    tx_compose_channel_rx: Option<Broadcaster<MessageSwapCompose<DB>>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
    _t: PhantomData<T>,
}

impl<P, T, DB> StateHealthMonitorActor<P, T, DB>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + DatabaseLoomExt + Send + Sync + Clone + Default + 'static,
{
    pub fn new(client: P) -> Self {
        StateHealthMonitorActor { client, market_state: None, tx_compose_channel_rx: None, market_events_rx: None, _t: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            market_state: Some(bc.market_state()),
            tx_compose_channel_rx: Some(bc.compose_channel()),
            market_events_rx: Some(bc.market_events_channel()),
            ..self
        }
    }
}

impl<P, T, DB> Actor for StateHealthMonitorActor<P, T, DB>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + DatabaseLoomExt + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(state_health_monitor_worker(
            self.client.clone(),
            self.market_state.clone().unwrap(),
            self.tx_compose_channel_rx.clone().unwrap(),
            self.market_events_rx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "StateHealthMonitorActor"
    }
}
