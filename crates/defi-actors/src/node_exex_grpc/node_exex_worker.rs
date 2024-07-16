use std::collections::HashMap;
use std::future::IntoFuture;
use std::sync::Arc;

use alloy_eips::BlockNumHash;
use alloy_primitives::{Address, U256};
use alloy_rpc_types::{Block, BlockTransactionsKind, Header, Transaction};
use chrono::Utc;
use example_exex_remote::ExExClient;
use example_exex_remote::proto::{SubscribeRequest, Transaction as ProtoTransaction};
use example_exex_remote::proto::remote_ex_ex_client::RemoteExExClient;
use eyre::Report;
use futures::{FutureExt, pin_mut, Stream, StreamExt};
use log::{error, info};
use reth_exex::ExExNotification;
use reth_primitives::TransactionSigned;
use reth_provider::Chain;
use revm::db::{BundleAccount, StorageWithOriginalValues};
use revm::db::states::StorageSlot;
use tokio::select;

use defi_events::{
    BlockLogs, BlockStateUpdate, Message, MessageMempoolDataUpdate, NodeMempoolDataUpdate,
};
use defi_types::{GethStateUpdate, MempoolTx};
use loom_actors::{Broadcaster, WorkerResult};
use loom_utils::reth_types::append_all_matching_block_logs_sealed;

async fn process_chain_task(
    chain: Arc<Chain>,
    block_header_channel: Broadcaster<Header>,
    block_with_tx_channel: Broadcaster<Block>,
    logs_channel: Broadcaster<BlockLogs>,
    state_update_channel: Broadcaster<BlockStateUpdate>,
) -> eyre::Result<()> {
    for sealed_header in chain.headers() {
        let header = reth_rpc_types_compat::block::from_primitive_with_hash(sealed_header);
        if let Err(e) = block_header_channel.send(header).await {
            error!("block_header_channel.send error: {}", e)
        }
    }

    for (sealed_block, receipts) in chain.blocks_and_receipts() {
        let number = sealed_block.number;
        let hash = sealed_block.hash();

        let block_hash_num = BlockNumHash { number, hash };

        info!(
            "Processing block block_number={} block_hash={}",
            block_hash_num.number, block_hash_num.hash
        );
        match reth_rpc_types_compat::block::from_block(
            sealed_block.clone().unseal(),
            sealed_block.difficulty,
            BlockTransactionsKind::Full,
            Some(sealed_block.hash()),
        ) {
            Ok(block) => {
                if let Err(e) = block_with_tx_channel.send(block).await {
                    error!("block_with_tx_channel.send error : {}", e)
                }
            }
            Err(e) => {
                error!("from_block error : {}", e)
            }
        }

        let mut logs: Vec<alloy_rpc_types::Log> = Vec::new();

        let receipts = receipts.iter().filter_map(|r| r.clone()).collect();

        append_all_matching_block_logs_sealed(
            &mut logs,
            block_hash_num.clone(),
            receipts,
            false,
            &sealed_block,
        )?;

        let log_update = BlockLogs {
            block_hash: sealed_block.hash(),
            logs,
        };

        if let Err(e) = logs_channel.send(log_update).await {
            error!("logs_channel.send error : {}", e)
        }

        if let Some(execution_outcome) = chain.execution_outcome_at_block(block_hash_num.number) {
            let mut state_update = GethStateUpdate::new();

            let state_ref: &HashMap<Address, BundleAccount> = execution_outcome.bundle.state();

            for (address, accounts) in state_ref.iter() {
                let mut account_state = state_update.entry(*address).or_default();
                if let Some(account_info) = accounts.info.clone() {
                    account_state.code = account_info.code.map(|c| c.bytecode().clone());
                    account_state.balance = Some(account_info.balance);
                    account_state.nonce = Some(account_info.nonce);
                }

                let storage: &StorageWithOriginalValues = &accounts.storage;

                for (key, storage_slot) in storage.iter() {
                    let (key, storage_slot): (&U256, &StorageSlot) = (key, storage_slot);
                    account_state
                        .storage
                        .insert((*key).into(), storage_slot.present_value.into());
                }
            }

            let block_state_update = BlockStateUpdate {
                block_hash: block_hash_num.hash,
                state_update: vec![state_update],
            };

            if let Err(e) = state_update_channel.send(block_state_update).await {
                error!("state_update_channel.send error : {}", e)
            }
        }
    }

    Ok(())
}


fn get_current_chain(notification: ExExNotification) -> Option<Arc<Chain>> {
    match notification {
        ExExNotification::ChainCommitted { new } => {
            info!("Received commit commited_hash={:?}", new.range());
            Some(new)
        }
        ExExNotification::ChainReorged { old, new } => {
            info!("Received reorg from_chain={:?} to_chain={:?}", old.range(), new.range());
            Some(new)
        }
        ExExNotification::ChainReverted { old } => {
            info!("Received revert reverted_chain={:?}", old.range());
            None
        }
    }
}

pub async fn node_exex_grpc_worker(
    url: Option<String>,
    block_header_channel: Broadcaster<Header>,
    block_with_tx_channel: Broadcaster<Block>,
    logs_channel: Broadcaster<BlockLogs>,
    state_update_channel: Broadcaster<BlockStateUpdate>,
    mempool_channel: Broadcaster<MessageMempoolDataUpdate>,
) -> WorkerResult {
    let mut client =
        example_exex_remote::ExExClient::connect(url.unwrap_or("http://[::1]:10000".to_string())).await?;


    let stream_header = client.subscribe_header().await?;
    pin_mut!(stream_header);

    let stream_block = client.subscribe_block().await?;
    pin_mut!(stream_block);

    let stream_logs = client.subscribe_logs().await?;
    pin_mut!(stream_logs);

    let stream_state = client.subscribe_stata_update().await?;
    pin_mut!(stream_state);

    let stream_tx = client.subscribe_mempool_tx().await?;
    pin_mut!(stream_tx);


    loop {
        select! {
            /*notification = stream_exex.next() => {
                if let Some(notification) = notification {
                    if let Some(chain) = get_current_chain(notification){
                        tokio::task::spawn(process_chain_task(
                            chain,
                            block_header_channel.clone(),
                            block_with_tx_channel.clone(),
                            logs_channel.clone(),
                            state_update_channel.clone()
                        ));
                    }
                }
            },

             */
            header = stream_header.next() => {
                if let Some(header) = header {
                    if let Err(e) = block_header_channel.send(header).await {
                        error!("block_header_channel.send error : {}", e)
                    }
                }
            }

            block = stream_block.next() => {
                if let Some(block) = block {
                    if let Err(e) = block_with_tx_channel.send(block).await {
                        error!("block_with_tx_channel.send error : {}", e)
                    }
                }
            }

            logs = stream_logs.next() => {
                if let Some((block_hash, logs)) = logs {
                    let block_logs = BlockLogs {block_hash, logs};
                    if let Err(e) = logs_channel.send(block_logs).await {
                        error!("block_with_tx_channel.send error : {}", e)
                    }
                }
            }

            state_update = stream_state.next() => {
                if let Some((block_hash, state_update)) = state_update {
                    let block_state_update = BlockStateUpdate{
                        block_hash,
                        state_update : vec![state_update],
                    };
                    if let Err(e) = state_update_channel.send(block_state_update).await {
                        error!("block_with_tx_channel.send error : {}", e)
                    }
                }
            }
            pending_tx = stream_tx.next() => {
                if let Some(tx) = pending_tx {
                    let tx_hash = tx.hash;

                    let mempool_tx = MempoolTx{
                        source: "exex".to_string(),
                        tx_hash,
                        time: Utc::now(),
                        tx: Some(tx),
                        logs: None,
                        mined: None,
                        failed: None,
                        state_update: None,
                        pre_state: None,
                    };
                    let data_update = NodeMempoolDataUpdate{ tx_hash, mempool_tx};

                    if let Err(e) = mempool_channel.send(Message::new_with_source(data_update, "exex".to_string())).await {
                        error!("mempool_channel.send error : {}", e)
                    }

                }
            }
        }
    }
}
