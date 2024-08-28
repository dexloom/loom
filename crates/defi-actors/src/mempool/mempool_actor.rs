use alloy_primitives::BlockNumber;
use alloy_rpc_types::BlockTransactions;
use chrono::{Duration, Utc};
use eyre::{eyre, Result};
use log::{debug, error, info, trace};
use tokio::sync::broadcast::error::RecvError;

use defi_blockchain::Blockchain;
use defi_entities::BlockHistory;
use defi_events::{MarketEvents, MempoolEvents, MessageMempoolDataUpdate};
use defi_types::{ChainParameters, Mempool, MempoolTx};
use loom_actors::{run_async, subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

pub async fn new_mempool_worker(
    chain_parameters: ChainParameters,
    mempool: SharedState<Mempool>,
    market_history: SharedState<BlockHistory>,
    mempool_update_rx: Broadcaster<MessageMempoolDataUpdate>,
    market_events_rx: Broadcaster<MarketEvents>,
    broadcaster: Broadcaster<MempoolEvents>,
) -> WorkerResult {
    subscribe!(mempool_update_rx);
    subscribe!(market_events_rx);

    let mut current_gas_price: Option<u128> = None;
    let mut last_cleaning_block: Option<BlockNumber> = None;

    loop {
        tokio::select! {
            msg = mempool_update_rx.recv() => {
                let mempool_update_msg : Result<MessageMempoolDataUpdate,RecvError> = msg;
                match mempool_update_msg {
                    Ok(mempool_update_msg) => {
                        let mut mempool_guard = mempool.write().await;
                        let tx_hash = mempool_update_msg.tx_hash;
                        let mempool_entry = mempool_guard.txs.entry(tx_hash).or_insert( MempoolTx{ tx_hash,  source : mempool_update_msg.source(), ..MempoolTx::default()});
                        if let Some(logs) = &mempool_update_msg.mempool_tx.logs {
                            if mempool_entry.logs.is_none() {
                                mempool_entry.logs = Some(logs.clone());
                                run_async!(broadcaster.send(MempoolEvents::MempoolLogUpdate {tx_hash } ));
                            }
                        }
                        if let Some(state_update) = &mempool_update_msg.mempool_tx.state_update {
                            if mempool_entry.state_update.is_none() {
                                mempool_entry.state_update = Some(state_update.clone());
                                run_async!(broadcaster.send(MempoolEvents::MempoolStateUpdate{ tx_hash }));
                            }
                        }
                        if let Some(tx) = &mempool_update_msg.mempool_tx.tx {
                            if mempool_entry.tx.is_none() {
                                mempool_entry.tx = Some(tx.clone());
                                if let Some(cur_gas_price) = current_gas_price {
                                    if let Some(tx_gas_price) = if tx.max_fee_per_gas.is_some() {tx.max_fee_per_gas} else{ tx.gas_price } {
                                        if tx.gas > 30000 && tx_gas_price >= cur_gas_price && mempool_guard.is_valid_tx(tx) {
                                            run_async!(broadcaster.send(MempoolEvents::MempoolActualTxUpdate {tx_hash }));
                                        }
                                    }
                                }
                                run_async!(broadcaster.send(MempoolEvents::MempoolTxUpdate {tx_hash }));
                            }
                        }
                        drop(mempool_guard);
                    }
                    Err(e) => {
                        error!("mempool_update_rx : {}",e);
                        break Err(eyre!("MEMPOOL_UPDATE_RX_CLOSED"))

                    }
                }

            },
            msg = market_events_rx.recv() => {
                match msg {
                    Ok(market_event_msg) => {
                        match market_event_msg {
                            MarketEvents::GasUpdate{next_block_base_fee}  => {
                                current_gas_price = Some(next_block_base_fee);
                            }
                            MarketEvents::BlockTxUpdate{ block_number, block_hash } => {
                                let mempool_len = mempool.read().await.len();
                                debug!("Mempool len {}", mempool_len);

                                match market_history.read().await.get_block_by_hash(&block_hash) {
                                    Some(block) => {

                                        let mut mempool_write_guard = mempool.write().await;
                                        if let BlockTransactions::Full(txs) = block.transactions {
                                            for tx in txs.iter() {
                                                mempool_write_guard
                                                    .set_mined(tx.hash, block_number)
                                                    .set_nonce(tx.from, tx.nonce);
                                            }

                                        }
                                        drop(mempool_write_guard);


                                        let next_gas_price = chain_parameters.calc_next_block_base_fee(block.header.gas_used, block.header.gas_limit, block.header.base_fee_per_gas.unwrap_or_default());

                                        let mempool_read_guard = mempool.read().await;
                                        let ok_txes = mempool_read_guard.filter_ok_by_gas_price(next_gas_price);
                                        debug!("Mempool gas update {} {}", next_gas_price, ok_txes.len());
                                        for mempool_tx in ok_txes {
                                            let tx  = mempool_tx.tx.clone().unwrap();
                                            if tx.gas  < 50000 {
                                                continue
                                            }
                                            if mempool_read_guard.is_valid_tx(&tx) {
                                                let tx_hash = tx.hash;
                                                trace!("new tx ok {:?}", tx_hash);
                                                run_async!(broadcaster.send(MempoolEvents::MempoolActualTxUpdate { tx_hash }));
                                            } else{
                                               trace!("new tx gas change tx not valid {:?}", tx.hash);
                                            }
                                        }
                                        drop(mempool_read_guard);

                                        match last_cleaning_block {
                                            Some(bn)=>{
                                                if block_number - bn > 20 {
                                                    let mut mempool_write_guard = mempool.write().await;
                                                    info!("Start mempool cleaning started. len : {}", mempool_write_guard.len());
                                                    mempool_write_guard.clean_txs( block_number - 50, Utc::now() - Duration::minutes(20) );
                                                    last_cleaning_block = Some(block_number);
                                                    info!("Start mempool cleaning finished len : {}", mempool_write_guard.len());
                                                    drop(mempool_write_guard)
                                                }
                                            }
                                            None=>{
                                                last_cleaning_block = Some(block_number)
                                            }
                                        }

                                    }
                                    None => {
                                        error!("Block not found");
                                    }

                                }
                            }
                            _=>{}
                        }
                    }
                    Err(e) => {
                        error!("market_events_rx : {}",e);
                        break Err(eyre!("MARKET_EVENTS_RX_CLOSED"))
                    }
                }
            }
        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct MempoolActor {
    chain_parameters: ChainParameters,
    #[accessor]
    mempool: Option<SharedState<Mempool>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory>>,
    #[consumer]
    mempool_update_rx: Option<Broadcaster<MessageMempoolDataUpdate>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
    #[producer]
    mempool_events_tx: Option<Broadcaster<MempoolEvents>>,
}

impl MempoolActor {
    pub fn new(chain_parameters: ChainParameters) -> MempoolActor {
        MempoolActor {
            chain_parameters,
            mempool: None,
            block_history: None,
            mempool_update_rx: None,
            market_events_rx: None,
            mempool_events_tx: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain) -> MempoolActor {
        Self {
            mempool: Some(bc.mempool()),
            block_history: Some(bc.block_history()),
            mempool_update_rx: Some(bc.new_mempool_tx_channel()),
            market_events_rx: Some(bc.market_events_channel()),
            mempool_events_tx: Some(bc.mempool_events_channel()),
            ..self
        }
    }
}


impl Actor for MempoolActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_mempool_worker(
            self.chain_parameters.clone(),
            self.mempool.clone().unwrap(),
            self.block_history.clone().unwrap(),
            self.mempool_update_rx.clone().unwrap(),
            self.market_events_rx.clone().unwrap(),
            self.mempool_events_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "MempoolActor"
    }
}
