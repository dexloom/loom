use alloy_consensus::transaction::Transaction;
use alloy_network::{Ethereum, TransactionResponse};
use alloy_primitives::{Address, TxHash, U256};
use alloy_provider::Provider;
use eyre::{eyre, Result};
use influxdb::{Timestamp, WriteQuery};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tracing::{error, info};

use loom_core_blockchain::Blockchain;
use loom_evm_utils::NWETH;
use loom_types_entities::{LatestBlock, Swap, Token};

use loom_core_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_types_blockchain::debug_trace_transaction;
use loom_types_events::{MarketEvents, MessageTxCompose, TxComposeMessageType};

#[derive(Clone, Debug)]
struct TxToCheck {
    block: u64,
    token_in: Token,
    profit: U256,
    cost: U256,
    tips: U256,
    swap: Swap,
}

async fn calc_coinbase_diff<P: Provider<Ethereum> + 'static>(client: P, tx_hash: TxHash, coinbase: Address) -> Result<U256> {
    let (pre, post) = debug_trace_transaction(client, tx_hash, true).await?;

    let coinbase_pre = pre.get(&coinbase).ok_or(eyre!("COINBASE_NOT_FOUND_IN_PRE"))?;
    let coinbase_post = post.get(&coinbase).ok_or(eyre!("COINBASE_NOT_FOUND_IN_POST"))?;

    let balance_diff = coinbase_post.balance.unwrap_or_default().checked_sub(coinbase_pre.balance.unwrap_or_default()).unwrap_or_default();
    info!("Stuffing tx mined MF tx: {:?} sent to coinbase: {}", tx_hash, NWETH::to_float(balance_diff));

    Ok(balance_diff)
}

pub async fn stuffing_tx_monitor_worker<P: Provider<Ethereum> + Clone + 'static>(
    client: P,
    latest_block: SharedState<LatestBlock>,
    tx_compose_channel_rx: Broadcaster<MessageTxCompose>,
    market_events_rx: Broadcaster<MarketEvents>,
    influxdb_write_channel_tx: Broadcaster<WriteQuery>,
) -> WorkerResult {
    let mut tx_compose_channel_rx: Receiver<MessageTxCompose> = tx_compose_channel_rx.subscribe();
    let mut market_events_rx: Receiver<MarketEvents> = market_events_rx.subscribe();

    let mut txs_to_check: HashMap<TxHash, TxToCheck> = HashMap::new();

    loop {
        tokio::select! {
            msg = market_events_rx.recv() => {
                let market_event_msg : Result<MarketEvents, RecvError> = msg;
                match market_event_msg {
                    Ok(market_event)=>{
                        if let MarketEvents::BlockTxUpdate{ block_number,..} = market_event {
                            let coinbase =  latest_block.read().await.coinbase().unwrap_or_default();
                            if let Some(txs) = latest_block.read().await.txs().cloned() {
                                for (idx, tx) in txs.iter().enumerate() {
                                    let tx_hash = tx.tx_hash();
                                    if let Some(tx_to_check) = txs_to_check.get(&tx_hash).cloned(){
                                        info!("Stuffing tx found mined {:?} block: {} -> {} idx: {} profit: {} tips: {} token: {} to: {:?} {}", tx.tx_hash(), tx_to_check.block, block_number, idx, NWETH::to_float(tx_to_check.profit), NWETH::to_float(tx_to_check.tips), tx_to_check.token_in.get_symbol(), tx.to().unwrap_or_default(), tx_to_check.swap );
                                        if idx < txs.len() - 1 {
                                            let others_tx = &txs[idx+1];
                                            let others_tx_hash = others_tx.tx_hash();
                                            let client_clone = client.clone();
                                            let influx_channel_clone = influxdb_write_channel_tx.clone();
                                            info!("Stuffing tx mined {:?} MF tx: {:?} to: {:?}", tx.tx_hash(), others_tx.tx_hash(), others_tx.to().unwrap_or_default() );
                                            tokio::task::spawn( async move {
                                                if let Ok(coinbase_diff)  = calc_coinbase_diff(client_clone, others_tx_hash, coinbase).await {
                                                    let start_time_utc =   chrono::Utc::now();
                                                    let bribe = NWETH::to_float(tx_to_check.tips);
                                                    let others_bribe = NWETH::to_float(coinbase_diff);
                                                    let cost = NWETH::to_float(tx_to_check.cost);

                                                    let write_query = WriteQuery::new(Timestamp::from(start_time_utc), "stuffing_mined")
                                                        .add_field("our_bribe", bribe)
                                                        .add_field("our_cost", cost)
                                                        .add_field("others_bribe", others_bribe)
                                                        .add_tag("tx_block", tx_to_check.block)
                                                        .add_tag("block", block_number)
                                                        .add_tag("block_idx", idx as u64)
                                                        .add_tag("stuffing_tx", tx_hash.to_string())
                                                        .add_tag("other_tx", others_tx_hash.to_string());
                                                    if let Err(e) = influx_channel_clone.send(write_query) {
                                                       error!("Failed to send block latency to influxdb: {:?}", e);
                                                    }
                                                };
                                            });
                                        }
                                        txs_to_check.remove::<TxHash>(&tx.tx_hash());
                                    }
                                }
                            }
                            info!("Stuffing txs to check : {} at block {}", txs_to_check.len(), block_number);


                            let start_time_utc =   chrono::Utc::now();

                            let write_query = WriteQuery::new(Timestamp::from(start_time_utc), "stuffing_waiting").add_field("value", txs_to_check.len() as u64).add_tag("block", block_number);
                            if let Err(e) = influxdb_write_channel_tx.send(write_query) {
                               error!("Failed to send block latency to influxdb: {:?}", e);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("market_event_rx error : {e}")
                    }
                }
            },

            msg = tx_compose_channel_rx.recv() => {
                let tx_compose_update : Result<MessageTxCompose, RecvError>  = msg;
                match tx_compose_update {
                    Ok(tx_compose_msg)=>{
                        if let TxComposeMessageType::Sign(tx_compose_data) = tx_compose_msg.inner {
                            for stuffing_tx_hash in tx_compose_data.stuffing_txs_hashes.iter() {
                                let Some(swap) = & tx_compose_data.swap else {continue};

                                let token_in = swap.get_first_token().map_or(
                                    Arc::new(Token::new(Address::repeat_byte(0x11))), |x| x.clone()
                                );

                                let cost = U256::from(tx_compose_data.next_block_base_fee + tx_compose_data.priority_gas_fee) * U256::from(tx_compose_data.gas);

                                let entry = txs_to_check.entry(*stuffing_tx_hash).or_insert(
                                        TxToCheck{
                                                block : tx_compose_data.next_block_number,
                                                token_in : token_in.as_ref().clone(),
                                                profit : U256::ZERO,
                                                tips : U256::ZERO,
                                                swap : swap.clone(),
                                                cost,
                                        }
                                );
                                let profit = swap.abs_profit();
                                let profit = token_in.calc_eth_value(profit).unwrap_or_default();

                                if entry.profit < profit {
                                    entry.token_in = token_in.as_ref().clone();
                                    entry.profit = profit;
                                    entry.tips = tx_compose_data.tips.unwrap_or_default();

                                    entry.swap = swap.clone();
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

#[derive(Accessor, Consumer, Producer)]
pub struct StuffingTxMonitorActor<P> {
    client: P,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[consumer]
    tx_compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
    #[producer]
    influxdb_write_channel_tx: Option<Broadcaster<WriteQuery>>,
}

impl<P: Provider<Ethereum> + Send + Sync + Clone + 'static> StuffingTxMonitorActor<P> {
    pub fn new(client: P) -> Self {
        StuffingTxMonitorActor {
            client,
            latest_block: None,
            tx_compose_channel_rx: None,
            market_events_rx: None,
            influxdb_write_channel_tx: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            latest_block: Some(bc.latest_block()),
            tx_compose_channel_rx: Some(bc.tx_compose_channel()),
            market_events_rx: Some(bc.market_events_channel()),
            influxdb_write_channel_tx: Some(bc.influxdb_write_channel()),
            ..self
        }
    }
}

impl<P> Actor for StuffingTxMonitorActor<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(stuffing_tx_monitor_worker(
            self.client.clone(),
            self.latest_block.clone().unwrap(),
            self.tx_compose_channel_rx.clone().unwrap(),
            self.market_events_rx.clone().unwrap(),
            self.influxdb_write_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "StuffingTxMonitorActor"
    }
}
