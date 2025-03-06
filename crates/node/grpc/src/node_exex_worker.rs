use std::sync::Arc;

use alloy_eips::BlockNumHash;
use alloy_primitives::{map::HashMap, Address, U256};
use alloy_rpc_types::{Block, BlockTransactionsKind, Header, Transaction};
use chrono::Utc;
use futures::{pin_mut, StreamExt};
use reth_exex::ExExNotification;
use reth_provider::Chain;
use reth_rpc::eth::EthTxBuilder;
use revm::db::states::StorageSlot;
use revm::db::{BundleAccount, StorageWithOriginalValues};
use tokio::select;
use tracing::{error, info};

use loom_core_actors::{Broadcaster, WorkerResult};
use loom_evm_utils::reth_types::append_all_matching_block_logs_sealed;
use loom_node_grpc_exex_proto::ExExClient;
use loom_types_blockchain::{GethStateUpdate, MempoolTx};
use loom_types_events::{
    BlockHeader, BlockLogs, BlockStateUpdate, BlockUpdate, Message, MessageBlock, MessageBlockHeader, MessageBlockLogs,
    MessageBlockStateUpdate, MessageMempoolDataUpdate, NodeMempoolDataUpdate,
};

#[allow(dead_code)]
async fn process_chain_task(
    chain: Arc<Chain>,
    block_header_channel: Broadcaster<Header>,
    block_with_tx_channel: Broadcaster<Block<Transaction>>,
    logs_channel: Broadcaster<BlockLogs>,
    state_update_channel: Broadcaster<BlockStateUpdate>,
) -> eyre::Result<()> {
    for sealed_header in chain.headers() {
        //let Ok(sealed_header) = TryInto::<reth::primitives::SealedHeader>::try_into(sealed_header.header().clone()) else { continue };
        //let Ok(sealed_header) = TryInto::<Sealed>

        let header = sealed_header.header().clone();

        let header = alloy_rpc_types::Header {
            hash: sealed_header.hash(),
            total_difficulty: Some(sealed_header.difficulty),
            size: Some(U256::from(sealed_header.size())),
            inner: header,
        };

        if let Err(e) = block_header_channel.send(header) {
            error!("block_header_channel.send error: {}", e)
        }
    }

    let eth_tx_builder = EthTxBuilder::default();

    for (sealed_block, receipts) in chain.blocks_and_receipts() {
        let number = sealed_block.number;
        let hash = sealed_block.hash();

        let block_hash_num = BlockNumHash { number, hash };
        let block_consensus_header = sealed_block.header().clone();

        info!("Processing block block_number={} block_hash={}", block_hash_num.number, block_hash_num.hash);
        match reth_rpc_types_compat::block::from_block(sealed_block.clone(), BlockTransactionsKind::Full, &eth_tx_builder) {
            Ok(block) => {
                if let Err(e) = block_with_tx_channel.send(block) {
                    error!("block_with_tx_channel.send error : {}", e)
                }
            }
            Err(e) => {
                error!("from_block error : {}", e)
            }
        }

        let mut logs: Vec<alloy_rpc_types::Log> = Vec::new();

        let receipts = receipts.clone();

        append_all_matching_block_logs_sealed(&mut logs, block_hash_num, receipts, false, sealed_block)?;

        let block_header = Header {
            hash: block_consensus_header.hash_slow(),
            total_difficulty: Some(block_consensus_header.difficulty),
            size: Some(U256::from(block_consensus_header.size())),
            inner: block_consensus_header.clone(),
        };

        let log_update = BlockLogs { block_header: block_header.clone(), logs };

        if let Err(e) = logs_channel.send(log_update) {
            error!("logs_channel.send error : {}", e)
        }

        if let Some(execution_outcome) = chain.execution_outcome_at_block(block_hash_num.number) {
            let mut state_update = GethStateUpdate::new();

            let state_ref: &HashMap<Address, BundleAccount> = execution_outcome.bundle.state();

            for (address, accounts) in state_ref.iter() {
                let account_state = state_update.entry(*address).or_default();
                if let Some(account_info) = accounts.info.clone() {
                    account_state.code = account_info.code.map(|c| c.bytecode().clone());
                    account_state.balance = Some(account_info.balance);
                    account_state.nonce = Some(account_info.nonce);
                }

                let storage: &StorageWithOriginalValues = &accounts.storage;

                for (key, storage_slot) in storage.iter() {
                    let (key, storage_slot): (&U256, &StorageSlot) = (key, storage_slot);
                    account_state.storage.insert((*key).into(), storage_slot.present_value.into());
                }
            }

            let block_state_update = BlockStateUpdate { block_header, state_update: vec![state_update] };

            if let Err(e) = state_update_channel.send(block_state_update) {
                error!("state_update_channel.send error : {}", e)
            }
        }
    }

    Ok(())
}

#[allow(dead_code)]
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
    block_header_channel: Broadcaster<MessageBlockHeader>,
    block_with_tx_channel: Broadcaster<MessageBlock>,
    logs_channel: Broadcaster<MessageBlockLogs>,
    state_update_channel: Broadcaster<MessageBlockStateUpdate>,
    mempool_channel: Broadcaster<MessageMempoolDataUpdate>,
) -> WorkerResult {
    let client = ExExClient::connect(url.unwrap_or("http://[::1]:10000".to_string())).await?;

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

            header = stream_header.next() => {
                if let Some(header) = header {
                    if let Err(e) = block_header_channel.send(
                        MessageBlockHeader::new_with_time(BlockHeader::new( header)))
                    {
                        error!("block_header_channel.send error : {}", e)
                    }
                }
            }

            block = stream_block.next() => {
                if let Some(block) = block {
                    if let Err(e) = block_with_tx_channel.send(
                        Message::new_with_time( BlockUpdate{block})
                    ) {
                        error!("block_with_tx_channel.send error : {}", e)
                    }
                }
            }

            logs = stream_logs.next() => {
                if let Some((block_header, logs)) = logs {
                    let block_logs = BlockLogs {block_header, logs};
                    if let Err(e) = logs_channel.send(
                        Message::new_with_time(block_logs)
                    ) {
                        error!("block_with_tx_channel.send error : {}", e)
                    }
                }
            }

            state_update = stream_state.next() => {
                if let Some((block_header, state_update)) = state_update {
                    let block_state_update = BlockStateUpdate{
                        block_header,
                        state_update : vec![state_update],
                    };
                    if let Err(e) = state_update_channel.send(
                        Message::new_with_time(block_state_update)
                    ) {
                        error!("block_with_tx_channel.send error : {}", e)
                    }
                }
            }
            pending_tx = stream_tx.next() => {
                if let Some(tx) = pending_tx {
                    let tx_hash = *tx.inner.tx_hash();

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

                    if let Err(e) = mempool_channel.send(Message::new_with_source(data_update, "exex".to_string())) {
                        error!("mempool_channel.send error : {}", e)
                    }

                }
            }
        }
    }
}
