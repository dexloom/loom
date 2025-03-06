use crate::mempool::replayer_mempool_task;
use alloy_eips::BlockId;
use alloy_network::Ethereum;
use alloy_primitives::BlockNumber;
use alloy_provider::Provider;
use alloy_rpc_types::{BlockTransactions, BlockTransactionsKind, Filter};
use loom_core_actors::{Broadcaster, SharedState, WorkerResult};
use loom_evm_db::DatabaseLoomExt;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::{debug_trace_block, Mempool};
use loom_types_entities::MarketState;
use loom_types_events::{
    BlockHeader, BlockLogs, BlockStateUpdate, BlockUpdate, Message, MessageBlock, MessageBlockHeader, MessageBlockLogs,
    MessageBlockStateUpdate,
};
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::fmt::Debug;
use std::ops::RangeInclusive;
use std::time::Duration;
use tracing::{debug, error};

#[allow(clippy::too_many_arguments)]
pub async fn node_player_worker<P, DB>(
    provider: P,
    start_block: BlockNumber,
    end_block: BlockNumber,
    mempool: Option<SharedState<Mempool>>,
    market_state: Option<SharedState<MarketState<DB>>>,
    new_block_headers_channel: Option<Broadcaster<MessageBlockHeader>>,
    new_block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    new_block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    new_block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
) -> WorkerResult
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
    DB: Database + DatabaseRef + DatabaseCommit + Send + Sync + Clone + DatabaseLoomExt + 'static,
    <DB as DatabaseRef>::Error: Debug,
{
    for curblock_number in RangeInclusive::new(start_block, end_block) {
        //let curblock_number = provider.client().transport().fetch_next_block().await?;
        let block = provider.get_block_by_number(curblock_number.into(), BlockTransactionsKind::Hashes).await?;

        if let Some(block) = block {
            let block_header = block.header.clone();
            let curblock_hash = block.header.hash;

            if let Some(mempool) = mempool.clone() {
                let mut mempool_guard = mempool.write().await;
                for tx_hash in mempool_guard.txs.clone().keys() {
                    if mempool_guard.is_mined(tx_hash) {
                        //mempool_guard.remove_tx(tx_hash);
                    } else {
                        mempool_guard.set_mined(*tx_hash, curblock_number);
                    }
                }

                //mempool_guard.clean_txs(curblock_number - 1, DateTime::<Utc>::MIN_UTC);
                debug!("Mempool cleaned");
            }

            // Processing mempool tx to update state
            if let Some(mempool) = mempool.clone() {
                if let Some(market_state) = market_state.clone() {
                    if let Err(e) = replayer_mempool_task(mempool, market_state, block.header.clone()).await {
                        error!("process_mempool_task : {e}");
                    }
                };
            };

            if let Some(block_headers_channel) = &new_block_headers_channel {
                if let Err(e) = block_headers_channel.send(Message::new_with_time(BlockHeader::new(block.header))) {
                    error!("new_block_headers_channel.send error: {e}");
                }
            }
            if let Some(block_with_tx_channel) = &new_block_with_tx_channel {
                match provider.get_block_by_hash(curblock_hash, BlockTransactionsKind::Full).await {
                    Ok(block) => {
                        if let Some(block) = block {
                            let mut txs = if let Some(mempool) = mempool.clone() {
                                let guard = mempool.read().await;

                                if !guard.is_empty() {
                                    guard.filter_on_block(curblock_number).into_iter().flat_map(|x| x.tx.clone()).collect()
                                } else {
                                    vec![]
                                }
                            } else {
                                vec![]
                            };

                            if txs.is_empty() {
                                let block_update = BlockUpdate { block };
                                if let Err(e) = block_with_tx_channel.send(Message::new_with_time(block_update)) {
                                    error!("new_block_with_tx_channel.send error: {e}");
                                }
                            } else if let Some(block_txs) = block.transactions.as_transactions() {
                                txs.extend(block_txs.iter().cloned());
                                let mut block = block;

                                block.transactions = BlockTransactions::Full(txs);
                                let block_update = BlockUpdate { block };
                                if let Err(e) = block_with_tx_channel.send(Message::new_with_time(block_update)) {
                                    error!("new_block_with_tx_channel.send updated block error: {e}");
                                }
                            }
                        } else {
                            error!("Block is empty")
                        }
                    }
                    Err(e) => {
                        error!("get_logs error: {e}")
                    }
                }
            }

            if let Some(block_logs_channel) = &new_block_logs_channel {
                let filter = Filter::new().at_block_hash(curblock_hash);

                let mut logs = if let Some(mempool) = mempool.clone() {
                    let guard = mempool.read().await;

                    if !guard.is_empty() {
                        guard.filter_on_block(curblock_number).into_iter().flat_map(|x| x.logs.clone().unwrap_or_default()).collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                match provider.get_logs(&filter).await {
                    Ok(block_logs) => {
                        debug!("Mempool logs : {}", logs.len());
                        logs.extend(block_logs);
                        let logs_update = BlockLogs { block_header: block_header.clone(), logs };
                        if let Err(e) = block_logs_channel.send(Message::new_with_time(logs_update)) {
                            error!("new_block_logs_channel.send error: {e}");
                        }
                    }
                    Err(e) => {
                        error!("get_logs error: {e}")
                    }
                }
            }

            if let Some(block_state_update_channel) = &new_block_state_update_channel {
                if let Some(mempool) = mempool.clone() {
                    if let Some(market_state) = market_state.clone() {
                        let mempool_guard = mempool.read().await;
                        let txes = mempool_guard.filter_on_block(curblock_number);

                        if !txes.is_empty() {
                            let mut marker_state_guard = market_state.write().await;
                            for mempool_tx in txes {
                                if let Some(state_update) = &mempool_tx.state_update {
                                    marker_state_guard.apply_geth_update(state_update.clone());
                                }
                            }
                            marker_state_guard.state_db = marker_state_guard.state_db.clone().maintain();
                        }
                    }
                }

                match debug_trace_block(provider.clone(), BlockId::Hash(curblock_hash.into()), true).await {
                    Ok((_, post)) => {
                        if let Err(e) =
                            block_state_update_channel.send(Message::new_with_time(BlockStateUpdate { block_header, state_update: post }))
                        {
                            error!("new_block_state_update_channel error: {e}");
                        }
                    }
                    Err(e) => {
                        error!("debug_trace_block error : {e}")
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    Ok("Node block player worker finished".to_string())
}
