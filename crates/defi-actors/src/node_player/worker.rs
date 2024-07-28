use crate::node_player::mempool::process_mempool_task;
use alloy_eips::BlockId;
use alloy_network::Ethereum;
use alloy_primitives::BlockNumber;
use alloy_provider::Provider;
use alloy_rpc_types::{Block, BlockTransactions, BlockTransactionsKind, Filter, Header};
use chrono::{DateTime, Utc};
use debug_provider::{DebugProviderExt, HttpCachedTransport};
use defi_entities::MarketState;
use defi_events::{BlockLogs, BlockStateUpdate};
use defi_types::{debug_trace_block, Mempool};
use log::{debug, error};
use loom_actors::{Broadcaster, SharedState, WorkerResult};
use loom_revm_db::LoomInMemoryDB;
use std::ops::RangeInclusive;
use std::sync::Arc;
use std::time::Duration;

#[allow(clippy::too_many_arguments)]
pub async fn node_player_worker<P>(
    provider: P,
    start_block: BlockNumber,
    end_block: BlockNumber,
    mempool: Option<SharedState<Mempool>>,
    market_state: Option<SharedState<MarketState>>,
    new_block_headers_channel: Option<Broadcaster<Header>>,
    new_block_with_tx_channel: Option<Broadcaster<Block>>,
    new_block_logs_channel: Option<Broadcaster<BlockLogs>>,
    new_block_state_update_channel: Option<Broadcaster<BlockStateUpdate>>,
) -> WorkerResult
where
    P: Provider<HttpCachedTransport, Ethereum> + DebugProviderExt<HttpCachedTransport, Ethereum> + Send + Sync + Clone + 'static,
{
    for _ in RangeInclusive::new(start_block, end_block) {
        let curblock_number = provider.client().transport().fetch_next_block().await?;
        let block = provider.get_block_by_number(curblock_number.into(), false).await?;

        if let Some(mempool) = mempool.clone() {
            let mut mempool_guard = mempool.write().await;
            for tx_hash in mempool_guard.txs.clone().keys() {
                if mempool_guard.is_mined(tx_hash) {
                    mempool_guard.remove_tx(&tx_hash);
                } else {
                    mempool_guard.set_mined(*tx_hash, curblock_number);
                }
            }

            //mempool_guard.clean_txs(curblock_number - 1, DateTime::<Utc>::MIN_UTC);
            debug!("Mempool cleaned");
        }

        if let Some(block) = block {
            let curblock_hash = block.header.hash.unwrap_or_default();

            // Processing mempool tx to update state
            if let Some(mempool) = mempool.clone() {
                if let Some(market_state) = market_state.clone() {
                    if let Err(e) = process_mempool_task(mempool, market_state, block.header.clone()).await {
                        error!("process_mempool_task : {e}");
                    }
                };
            };

            if let Some(block_headers_channel) = &new_block_headers_channel {
                if let Err(e) = block_headers_channel.send(block.header).await {
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
                                    guard
                                        .txs
                                        .values()
                                        .filter(|x| x.state_update.is_some() && x.logs.is_some())
                                        .flat_map(|x| x.tx.clone())
                                        .collect()
                                } else {
                                    vec![]
                                }
                            } else {
                                vec![]
                            };

                            //let mut txs = Vec::new();

                            if txs.is_empty() {
                                if let Err(e) = block_with_tx_channel.send(block).await {
                                    error!("new_block_with_tx_channel.send error: {e}");
                                }
                            } else {
                                if let Some(block_txs) = block.transactions.as_transactions() {
                                    txs.extend(block_txs.iter().cloned());
                                    let mut updated_block = block;

                                    updated_block.transactions = BlockTransactions::Full(txs);
                                    if let Err(e) = block_with_tx_channel.send(updated_block).await {
                                        error!("new_block_with_tx_channel.send updated block error: {e}");
                                    }
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
                        guard.txs.values().flat_map(|x| x.logs.clone().unwrap_or_default()).collect()
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
                        let logs_update = BlockLogs { block_hash: curblock_hash, logs };
                        if let Err(e) = block_logs_channel.send(logs_update).await {
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
                        let mut mempool_guard = mempool.write().await;
                        if !mempool_guard.is_empty() {
                            let mut marker_state_guard = market_state.write().await;
                            for mempool_tx in mempool_guard.txs.values() {
                                if let Some(state_update) = &mempool_tx.state_update {
                                    marker_state_guard.state_db.apply_geth_update(state_update.clone());
                                }
                            }
                            marker_state_guard.state_db = LoomInMemoryDB::new(Arc::new(marker_state_guard.state_db.merge()));
                        }
                    }
                }

                match debug_trace_block(provider.clone(), BlockId::Hash(curblock_hash.into()), true).await {
                    Ok((_, post)) => {
                        if let Err(e) =
                            block_state_update_channel.send(BlockStateUpdate { block_hash: curblock_hash, state_update: post }).await
                        {
                            error!("new_block_state_update_channel error: {e}");
                        }
                    }
                    Err(e) => {
                        error!("debug_trace_block error : {e}")
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    Ok("Node block player worker finished".to_string())
}
