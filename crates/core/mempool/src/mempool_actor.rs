use alloy_primitives::BlockNumber;
use chrono::{Duration, Utc};
use eyre::eyre;
use influxdb::{Timestamp, WriteQuery};
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info, trace};

use loom_core_actors::{run_sync, subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::Blockchain;
use loom_types_blockchain::{ChainParameters, Mempool, MempoolTx};
use loom_types_blockchain::{LoomBlock, LoomDataTypes, LoomDataTypesEthereum, LoomHeader, LoomTx};
use loom_types_events::{MempoolEvents, MessageBlock, MessageBlockHeader, MessageMempoolDataUpdate};

pub async fn new_mempool_worker<LDT: LoomDataTypes>(
    chain_parameters: ChainParameters,
    mempool: SharedState<Mempool<LDT>>,
    mempool_update_rx: Broadcaster<MessageMempoolDataUpdate<LDT>>,
    block_header_rx: Broadcaster<MessageBlockHeader<LDT>>,
    block_with_txs_rx: Broadcaster<MessageBlock<LDT>>,
    broadcaster: Broadcaster<MempoolEvents<LDT>>,
    influxdb_write_channel_tx: Broadcaster<WriteQuery>,
) -> WorkerResult {
    subscribe!(mempool_update_rx);
    subscribe!(block_header_rx);
    subscribe!(block_with_txs_rx);

    let mut current_gas_price: Option<u128> = None;
    let mut last_cleaning_block: Option<BlockNumber> = None;

    loop {
        tokio::select! {
            msg = mempool_update_rx.recv() => {
                let mempool_update_msg = match msg {
                    Ok(mempool_update_msg) => mempool_update_msg,
                    Err(e) => {
                        match e {
                            RecvError::Closed => {
                                error!("Mempool update channel closed");
                                break Err(eyre!("MEMPOOL_UPDATE_RX_CLOSED"))
                            }
                            RecvError::Lagged(lag) => {
                                error!("Mempool update channel lagged by {} messages", lag);
                                continue;
                            }
                        }
                    }
                };

                let mut mempool_guard = mempool.write().await;
                let tx_hash = mempool_update_msg.tx_hash;
                let mempool_entry = mempool_guard.txs.entry(tx_hash).or_insert( MempoolTx::<LDT>{ tx_hash,  source : mempool_update_msg.source(), ..MempoolTx::default()});
                if let Some(logs) = &mempool_update_msg.mempool_tx.logs {
                    if mempool_entry.logs.is_none() {
                        mempool_entry.logs = Some(logs.clone());
                        run_sync!(broadcaster.send(MempoolEvents::MempoolLogUpdate {tx_hash } ));
                    }
                }
                if let Some(state_update) = &mempool_update_msg.mempool_tx.state_update {
                    if mempool_entry.state_update.is_none() {
                        mempool_entry.state_update = Some(state_update.clone());
                        run_sync!(broadcaster.send(MempoolEvents::MempoolStateUpdate{ tx_hash }));
                    }
                }
                if let Some(tx) = &mempool_update_msg.mempool_tx.tx {
                    if mempool_entry.tx.is_none() {
                        mempool_entry.tx = Some(tx.clone());
                        if let Some(cur_gas_price) = current_gas_price {
                            if tx.gas_limit() > 30000 && tx.gas_price() >= cur_gas_price && mempool_guard.is_valid_tx(tx) {
                                run_sync!(broadcaster.send(MempoolEvents::MempoolActualTxUpdate {tx_hash }));
                            }
                        }
                        run_sync!(broadcaster.send(MempoolEvents::MempoolTxUpdate {tx_hash }));
                    }
                }
                drop(mempool_guard);
            },
            msg = block_header_rx.recv() => {
                let block_header = match msg {
                    Ok(message_block_header) => {message_block_header.inner}
                     Err(e) => {
                        match e {
                            RecvError::Closed => {
                                error!("Block header channel closed");
                                break Err(eyre!("BLOCK_HEADER_RX_CLOSED"))
                            }
                            RecvError::Lagged(lag) => {
                                error!("Block header channel lagged by {} messages", lag);
                                continue;
                            }
                        }
                    }
                };

                current_gas_price = block_header.header.base_fee();
                let block_number = block_header.header.number();

                let mempool_len = mempool.read().await.len();
                debug!("Mempool len {}", mempool_len);


                let mempool_read_guard = mempool.read().await;
                let next_base_fee =  block_header.header.next_base_fee(&chain_parameters);

                let ok_txes = mempool_read_guard.filter_ok_by_gas_price(next_base_fee as u128);
                debug!("Mempool gas update {} {}", next_base_fee, ok_txes.len());
                for mempool_tx in ok_txes {
                    let tx = mempool_tx.tx.clone().unwrap();
                    if tx.gas_limit()  < 50000 {
                        continue
                    }
                    if mempool_read_guard.is_valid_tx(&tx) {
                        let tx_hash = tx.tx_hash();
                        trace!("new tx ok {:?}", tx_hash);
                        run_sync!(broadcaster.send(MempoolEvents::MempoolActualTxUpdate { tx_hash }));
                    } else{
                       trace!("new tx gas change tx not valid {:?}", tx.tx_hash());
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

            },
            msg = block_with_txs_rx.recv() => {
                let block_with_txs = match msg {
                    Ok(block_with_txs) => block_with_txs.inner.block,
                    Err(e) => {
                        match e {
                            RecvError::Closed => {
                                error!("Block with txs channel closed");
                                break Err(eyre!("BLOCK_WITH_TXS_RX_CLOSED"))
                            }
                            RecvError::Lagged(lag) => {
                                error!("Block with txs channel lagged by {} messages", lag);
                                continue;
                            }
                        }
                    }
                };
                let mut mempool_write_guard = mempool.write().await;

                let mut mempool_tx_counter=0;
                let tx_count = block_with_txs.transactions().len();
                let mempool_size = mempool_write_guard.len();

                for tx in block_with_txs.transactions() {

                    if mempool_write_guard.is_tx(&tx.tx_hash()) {
                        mempool_tx_counter += 1;
                    }

                    mempool_write_guard
                        .set_mined(tx.tx_hash(), block_with_txs.number())
                        .set_nonce(tx.from(), tx.nonce());
                }
                let start_time_utc =   chrono::Utc::now();
                let write_query = WriteQuery::new(Timestamp::from(start_time_utc), "mempool")
                    .add_tag("block", block_with_txs.number())
                    .add_field("tx_count_block", tx_count as u64)
                    .add_field("tx_count_found", mempool_tx_counter)
                    .add_field("tx_mempool_size", mempool_size as u64);

                if let Err(e) = influxdb_write_channel_tx.send(write_query) {
                       error!("Failed to send mempool stat to influxdb: {:?}", e);
                }

                drop(mempool_write_guard);
            }
        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct MempoolActor<LDT: LoomDataTypes + 'static = LoomDataTypesEthereum> {
    chain_parameters: ChainParameters,
    #[accessor]
    mempool: Option<SharedState<Mempool<LDT>>>,
    #[consumer]
    mempool_update_rx: Option<Broadcaster<MessageMempoolDataUpdate<LDT>>>,
    #[consumer]
    block_header_rx: Option<Broadcaster<MessageBlockHeader<LDT>>>,
    #[consumer]
    block_with_txs_rx: Option<Broadcaster<MessageBlock<LDT>>>,
    #[producer]
    mempool_events_tx: Option<Broadcaster<MempoolEvents<LDT>>>,
    #[producer]
    influxdb_write_channel_tx: Option<Broadcaster<WriteQuery>>,
}

impl<LDT: LoomDataTypes> Default for MempoolActor<LDT> {
    fn default() -> Self {
        Self {
            chain_parameters: ChainParameters::ethereum(),
            mempool: None,
            mempool_update_rx: None,
            mempool_events_tx: None,
            block_header_rx: None,
            block_with_txs_rx: None,
            influxdb_write_channel_tx: None,
        }
    }
}

impl<LDT: LoomDataTypes> MempoolActor<LDT> {
    pub fn new() -> MempoolActor<LDT> {
        MempoolActor::default()
    }

    pub fn on_bc(self, bc: &Blockchain<LDT>) -> MempoolActor<LDT> {
        Self {
            chain_parameters: bc.chain_parameters(),
            mempool: Some(bc.mempool()),
            mempool_update_rx: Some(bc.new_mempool_tx_channel()),
            block_header_rx: Some(bc.new_block_headers_channel()),
            block_with_txs_rx: Some(bc.new_block_with_tx_channel()),
            mempool_events_tx: Some(bc.mempool_events_channel()),
            influxdb_write_channel_tx: Some(bc.influxdb_write_channel()),
        }
    }
}

impl<LDT: LoomDataTypes> Actor for MempoolActor<LDT> {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_mempool_worker(
            self.chain_parameters.clone(),
            self.mempool.clone().unwrap(),
            self.mempool_update_rx.clone().unwrap(),
            self.block_header_rx.clone().unwrap(),
            self.block_with_txs_rx.clone().unwrap(),
            self.mempool_events_tx.clone().unwrap(),
            self.influxdb_write_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "MempoolActor"
    }
}
