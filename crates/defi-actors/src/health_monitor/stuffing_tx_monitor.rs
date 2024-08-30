use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::{Address, TxHash, U256};
use alloy_provider::Provider;
use eyre::{eyre, Result};
use log::{error, info};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_blockchain::Blockchain;
use defi_entities::{LatestBlock, Swap, Token};
use loom_utils::NWETH;

use defi_events::{MarketEvents, MessageTxCompose, TxCompose};
use defi_types::debug_trace_transaction;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

#[derive(Clone, Debug)]
struct TxToCheck {
    block: u64,
    token_in: Token,
    profit: U256,
    tips: U256,
    swap: Swap,
}

async fn check_mf_tx<P: Provider + 'static>(client: P, tx_hash: TxHash, coinbase: Address) -> Result<()> {
    let (pre, post) = debug_trace_transaction(client, tx_hash, true).await?;

    let coinbase_pre = pre.get(&coinbase).ok_or(eyre!("COINBASE_NOT_FOUND_IN_PRE"))?;
    let coinbase_post = post.get(&coinbase).ok_or(eyre!("COINBASE_NOT_FOUND_IN_POST"))?;

    let balance_diff = coinbase_post.balance.unwrap_or_default().checked_sub(coinbase_pre.balance.unwrap_or_default()).unwrap_or_default();
    info!("Stuffing tx mined MF tx: {:?} sent to coinbase: {}", tx_hash, NWETH::to_float(balance_diff));

    Ok(())
}

pub async fn stuffing_tx_monitor_worker<P: Provider + Clone + 'static>(
    client: P,
    latest_block: SharedState<LatestBlock>,
    tx_compose_channel_rx: Broadcaster<MessageTxCompose>,
    market_events_rx: Broadcaster<MarketEvents>,
) -> WorkerResult {
    let mut tx_compose_channel_rx: Receiver<MessageTxCompose> = tx_compose_channel_rx.subscribe().await;
    let mut market_events_rx: Receiver<MarketEvents> = market_events_rx.subscribe().await;

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
                                    let tx_hash = tx.hash;
                                    if let Some(tx_to_check) = txs_to_check.get(&tx_hash).cloned(){
                                        info!("Stuffing tx found mined {:?} block: {} -> {} idx: {} profit: {} tips: {} token: {} to: {:?} {}", tx.hash, tx_to_check.block, block_number, idx, NWETH::to_float(tx_to_check.profit), NWETH::to_float(tx_to_check.tips), tx_to_check.token_in.get_symbol(), tx.to.unwrap_or_default(), tx_to_check.swap );
                                        if idx < txs.len() - 1 {
                                            let mf_tx = &txs[idx+1];
                                            info!("Stuffing tx mined {:?} MF tx: {:?} to: {:?}", tx.hash, mf_tx.hash, mf_tx.to.unwrap_or_default() );
                                            tokio::task::spawn(
                                                check_mf_tx(client.clone(), mf_tx.hash, coinbase)
                                            );
                                        }
                                        txs_to_check.remove::<TxHash>(&tx.hash);
                                    }
                                }
                            }
                            info!("Stuffing txs to check : {} at block {}", txs_to_check.len(), block_number)
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
                        if let TxCompose::Broadcast(broadcast_data) = tx_compose_msg.inner {
                            for stuffing_tx_hash in broadcast_data.stuffing_txs_hashes.iter() {

                                let token_in = broadcast_data.swap.get_first_token().map_or(
                                    Arc::new(Token::new(Address::repeat_byte(0x11))), |x| x.clone()
                                );

                                let entry = txs_to_check.entry(*stuffing_tx_hash).or_insert(
                                        TxToCheck{
                                                block : broadcast_data.block,
                                                token_in : token_in.as_ref().clone(),
                                                profit : U256::ZERO,
                                                tips : U256::ZERO,
                                                swap : broadcast_data.swap.clone(),
                                        }
                                );
                                let profit = broadcast_data.swap.abs_profit();
                                let profit = token_in.calc_eth_value(profit).unwrap_or_default();

                                if entry.profit < profit {
                                    entry.token_in = token_in.as_ref().clone();
                                    entry.profit = profit;
                                    entry.tips = broadcast_data.tips.unwrap_or_default();
                                    entry.swap = broadcast_data.swap.clone()
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
pub struct StuffingTxMonitorActor<P> {
    client: P,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[consumer]
    tx_compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[consumer]
    market_events_rx: Option<Broadcaster<MarketEvents>>,
}

impl<P: Provider + Send + Sync + Clone + 'static> StuffingTxMonitorActor<P> {
    pub fn new(client: P) -> Self {
        StuffingTxMonitorActor { client, latest_block: None, tx_compose_channel_rx: None, market_events_rx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            latest_block: Some(bc.latest_block()),
            tx_compose_channel_rx: Some(bc.compose_channel()),
            market_events_rx: Some(bc.market_events_channel()),
            ..self
        }
    }
}

impl<P> Actor for StuffingTxMonitorActor<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(stuffing_tx_monitor_worker(
            self.client.clone(),
            self.latest_block.clone().unwrap(),
            self.tx_compose_channel_rx.clone().unwrap(),
            self.market_events_rx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "StuffingTxMonitorActor"
    }
}
