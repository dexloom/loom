use async_trait::async_trait;
use log::{error, info};
use tokio::sync::broadcast::Receiver;

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
    mut market_events_receiver: Receiver<MarketEvents>,
    broadcaster: Broadcaster<MarketEvents>,
) -> WorkerResult
{
    loop {
        tokio::select! {
            msg = market_events_receiver.recv() => {
                match msg {
                    Ok(market_event) => {
                        match market_event {
                            BlockTxUpdate{ block_number, block_hash } => {
                                if let Some(entry) = market_history_state.read().await.get_market_history_entry(&block_hash).cloned() {
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
                            _=>{}
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
pub struct GasStationActor
{
    chain_parameters: ChainParameters,
    #[accessor]
    gas_station: Option<SharedState<GasStation>>,
    #[accessor]
    market_history_state: Option<SharedState<BlockHistory>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
    #[producer]
    market_events_tx: Option<Broadcaster<MarketEvents>>,
}

impl Default for GasStationActor
{
    fn default() -> Self {
        GasStationActor {
            chain_parameters: ChainParameters::ethereum(),
            market_events_tx: None,
            market_events_rx: None,
            market_history_state: None,
            gas_station: None,
        }
    }
}

impl GasStationActor
{
    pub fn new(chain_parameters: ChainParameters) -> GasStationActor {
        Self {
            chain_parameters,
            ..Self::default()
        }
    }
}


#[async_trait]
impl Actor for GasStationActor
{
    async fn start(&mut self) -> ActorResult {
        let task = tokio::task::spawn(
            new_gas_worker(
                self.chain_parameters.clone(),
                self.gas_station.clone().unwrap(),
                self.market_history_state.clone().unwrap(),
                self.market_events_rx.clone().unwrap().subscribe().await,
                self.market_events_tx.clone().unwrap(),
            )
        );


        Ok(vec![task])
    }
}
