use log::{error, info};
use tokio::sync::broadcast::Receiver;

use defi_blockchain::Blockchain;
use defi_entities::{BlockHistory, GasStation};
use defi_events::MarketEvents;
use defi_events::MarketEvents::BlockTxUpdate;
use defi_types::ChainParameters;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

pub async fn new_gas_worker(
    chain_parameters: ChainParameters,
    gas_station: SharedState<GasStation>,
    market_history_state: SharedState<BlockHistory>,
    market_events_rx: Broadcaster<MarketEvents>,
    broadcaster: Broadcaster<MarketEvents>,
) -> WorkerResult {
    let mut market_events_rx: Receiver<MarketEvents> = market_events_rx.subscribe().await;

    loop {
        tokio::select! {
            msg = market_events_rx.recv() => {
                match msg {
                    Ok(market_event) => {
                        if let BlockTxUpdate{ block_number, block_hash } = market_event {
                            if let Some(entry) = market_history_state.read().await.get_entry(&block_hash).cloned() {
                                if let Some(block) = entry.block {
                                    if let Some(cur_base_fee) = block.header.base_fee_per_gas {
                                        let next_block_base_fee : u128 = chain_parameters.calc_next_block_base_fee(block.header.gas_used, block.header.gas_limit, cur_base_fee);
                                        gas_station.write().await.next_block_base_fee = next_block_base_fee;
                                        match broadcaster.send(MarketEvents::GasUpdate{ next_block_base_fee}).await {
                                            Ok(_)=>{
                                                info!("Gas updated block: {} next base fee : {}", block_number, next_block_base_fee)
                                            }
                                            Err(e)=>{
                                                error!("{e}")
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("{}", e);
                    }
                }

            }

        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct GasStationActor {
    chain_parameters: ChainParameters,
    #[accessor]
    gas_station: Option<SharedState<GasStation>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
    #[producer]
    market_events_tx: Option<Broadcaster<MarketEvents>>,
}

impl Default for GasStationActor {
    fn default() -> Self {
        GasStationActor {
            chain_parameters: ChainParameters::ethereum(),
            market_events_tx: None,
            market_events_rx: None,
            block_history: None,
            gas_station: None,
        }
    }
}

impl GasStationActor {
    pub fn new() -> GasStationActor {
        Self { chain_parameters: ChainParameters::ethereum(), ..Self::default() }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            chain_parameters: bc.chain_parameters(),
            gas_station: Some(bc.gas_station()),
            block_history: Some(bc.block_history()),
            market_events_rx: Some(bc.market_events_channel()),
            market_events_tx: Some(bc.market_events_channel()),
        }
    }
}

impl Actor for GasStationActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_gas_worker(
            self.chain_parameters.clone(),
            self.gas_station.clone().unwrap(),
            self.block_history.clone().unwrap(),
            self.market_events_rx.clone().unwrap(),
            self.market_events_tx.clone().unwrap(),
        ));

        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "GasStationActor"
    }
}
