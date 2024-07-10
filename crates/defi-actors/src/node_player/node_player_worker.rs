use std::ops::RangeInclusive;

use alloy_eips::BlockId;
use alloy_network::{Ethereum, Network};
use alloy_primitives::BlockNumber;
use alloy_provider::Provider;
use alloy_rpc_types::{Block, BlockTransactionsKind, Filter, Header};
use log::error;

use debug_provider::{DebugProviderExt, HttpCachedTransport};
use defi_events::{BlockLogs, BlockStateUpdate};
use defi_types::debug_trace_block;
use loom_actors::{Broadcaster, WorkerResult};

pub async fn node_player_worker<P>(
    provider: P,
    start_block: BlockNumber,
    end_block: BlockNumber,
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

        if let Some(block) = block {
            let curblock_hash = block.header.hash.unwrap_or_default();
            if let Some(block_headers_channel) = &new_block_headers_channel {
                if let Err(e) = block_headers_channel.send(block.header).await {
                    error!("new_block_headers_channel.send error: {e}");
                }
            }
            if let Some(block_with_tx_channel) = &new_block_with_tx_channel {
                match provider.get_block_by_hash(curblock_hash, BlockTransactionsKind::Full).await {
                    Ok(block) => {
                        if let Some(block) = block {
                            if let Err(e) = block_with_tx_channel.send(block).await {
                                error!("new_block_with_tx_channel.send error: {e}");
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
                match provider.get_logs(&filter).await {
                    Ok(logs) => {
                        let logs_update = BlockLogs {
                            block_hash: curblock_hash,
                            logs,
                        };
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
                match debug_trace_block(provider.clone(), BlockId::Hash(curblock_hash.into()), true).await {
                    Ok((_, post)) => {
                        if let Err(e) = block_state_update_channel.send(BlockStateUpdate { block_hash: curblock_hash, state_update: post }).await {
                            error!("new_block_state_update_channel error: {e}");
                        }
                    }
                    Err(e) => {
                        error!("debug_trace_block error : {e}")
                    }
                }
            }
        }
    }


    Ok("DONE".to_string())
}


