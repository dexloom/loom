use alloy_primitives::BlockNumber;
use alloy_rpc_types::BlockTransactions;
use chrono::{Duration, Utc};
use eyre::eyre;
use log::{debug, error, info, trace};
use tokio::sync::broadcast::error::RecvError;

use defi_blockchain::Blockchain;
use defi_events::{MempoolEvents, MessageBlock, MessageBlockHeader, MessageMempoolDataUpdate};
use defi_types::{ChainParameters, Mempool, MempoolTx};
use loom_actors::{run_async, subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

pub async fn new_mempool_worker(
    chain_parameters: ChainParameters,
    mempool: SharedState<Mempool>,
    mempool_update_rx: Broadcaster<MessageMempoolDataUpdate>,
    block_header_rx: Broadcaster<MessageBlockHeader>,
    block_with_txs_rx: Broadcaster<MessageBlock>,
    broadcaster: Broadcaster<MempoolEvents>,
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

                    current_gas_price = block_header.header.base_fee_per_gas.map(|x| x as u128);
                    let block_number = block_header.header.number;

                    let mempool_len = mempool.read().await.len();
                    debug!("Mempool len {}", mempool_len);


                    let mempool_read_guard = mempool.read().await;
                    let next_base_fee = chain_parameters.calc_next_block_base_fee_from_header(&block_header.header);

                    let ok_txes = mempool_read_guard.filter_ok_by_gas_price(next_base_fee as u128);
                    debug!("Mempool gas update {} {}", next_base_fee, ok_txes.len());
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

                },
                msg = block_with_txs_rx.recv() => {
                    let block_with_txs = match msg {
                        Ok(block_with_txs) => block_with_txs.inner,
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
                    if let BlockTransactions::Full(txs) = block_with_txs.transactions {
                        for tx in txs.iter() {
                            mempool_write_guard
                                .set_mined(tx.hash, block_with_txs.header.number)
                                .set_nonce(tx.from, tx.nonce);
                        }

                    }
                    drop(mempool_write_guard);
            }
        }
    }
}

#[derive(Accessor, Consumer, Producer, Default)]
pub struct MempoolActor {
    chain_parameters: ChainParameters,
    #[accessor]
    mempool: Option<SharedState<Mempool>>,
    #[consumer]
    mempool_update_rx: Option<Broadcaster<MessageMempoolDataUpdate>>,
    #[consumer]
    block_header_rx: Option<Broadcaster<MessageBlockHeader>>,
    #[consumer]
    block_with_txs_rx: Option<Broadcaster<MessageBlock>>,
    #[producer]
    mempool_events_tx: Option<Broadcaster<MempoolEvents>>,
}

impl MempoolActor {
    pub fn new() -> MempoolActor {
        MempoolActor::default()
    }

    pub fn on_bc(self, bc: &Blockchain) -> MempoolActor {
        Self {
            chain_parameters: bc.chain_parameters(),
            mempool: Some(bc.mempool()),
            mempool_update_rx: Some(bc.new_mempool_tx_channel()),
            block_header_rx: Some(bc.new_block_headers_channel()),
            block_with_txs_rx: Some(bc.new_block_with_tx_channel()),
            mempool_events_tx: Some(bc.mempool_events_channel()),
        }
    }
}

impl Actor for MempoolActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_mempool_worker(
            self.chain_parameters.clone(),
            self.mempool.clone().unwrap(),
            self.mempool_update_rx.clone().unwrap(),
            self.block_header_rx.clone().unwrap(),
            self.block_with_txs_rx.clone().unwrap(),
            self.mempool_events_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "MempoolActor"
    }
}
