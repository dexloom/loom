use alloy_eips::BlockNumHash;
use alloy_network::primitives::{BlockTransactions, BlockTransactionsKind};
use alloy_primitives::map::HashMap;
use alloy_primitives::{Address, TxHash, U256};
use alloy_rpc_types::Block;
use futures::TryStreamExt;
use loom_core_actors::Broadcaster;
use loom_core_blockchain::Blockchain;
use loom_evm_utils::reth_types::append_all_matching_block_logs_sealed;
use loom_node_actor_config::NodeBlockActorConfig;
use loom_types_blockchain::{GethStateUpdate, MempoolTx};
use loom_types_events::{
    BlockHeader, BlockLogs, BlockStateUpdate, Message, MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate,
    MessageMempoolDataUpdate, NodeMempoolDataUpdate,
};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_provider::Chain;
use reth_rpc::eth::EthTxBuilder;
use reth_transaction_pool::TransactionPool;
use revm::db::states::StorageSlot;
use revm::db::{BundleAccount, StorageWithOriginalValues};
use revm::DatabaseRef;
use std::sync::Arc;
use tokio::select;
use tracing::{debug, error, info};

async fn process_chain(
    chain: Arc<Chain>,
    block_header_channel: Broadcaster<MessageBlockHeader>,
    block_with_tx_channel: Broadcaster<MessageBlock>,
    logs_channel: Broadcaster<MessageBlockLogs>,
    state_update_channel: Broadcaster<MessageBlockStateUpdate>,
    config: &NodeBlockActorConfig,
) -> eyre::Result<()> {
    if config.block_header {
        for sealed_header in chain.headers() {
            let header = reth_rpc_types_compat::block::from_primitive_with_hash(sealed_header);
            if let Err(e) = block_header_channel.send(MessageBlockHeader::new_with_time(BlockHeader::new(header))).await {
                error!(error=?e.to_string(), "block_header_channel.send")
            }
        }
    }

    for (sealed_block, receipts) in chain.blocks_and_receipts() {
        let number = sealed_block.number;
        let hash = sealed_block.hash();

        let block_hash_num = BlockNumHash { number, hash };

        // Block with tx
        if config.block_with_tx {
            info!(block_number=?block_hash_num.number, block_hash=?block_hash_num.hash, "Processing block");
            match reth_rpc_types_compat::block::from_block::<EthTxBuilder>(
                sealed_block.clone().unseal(),
                sealed_block.difficulty,
                BlockTransactionsKind::Full,
                Some(sealed_block.hash()),
                &EthTxBuilder,
            ) {
                Ok(block) => {
                    let block: Block = Block {
                        transactions: BlockTransactions::Full(block.transactions.into_transactions().map(|t| t.inner).collect()),
                        header: block.header,
                        uncles: block.uncles,
                        size: block.size,
                        withdrawals: block.withdrawals,
                    };

                    if let Err(e) = block_with_tx_channel.send(Message::new_with_time(block)).await {
                        error!(error=?e.to_string(), "block_with_tx_channel.send")
                    }
                }
                Err(e) => {
                    error!(error = ?e, "from_block")
                }
            }
        }

        // Block logs
        if config.block_logs {
            let mut logs: Vec<alloy_rpc_types::Log> = Vec::new();

            let receipts = receipts.iter().filter_map(|r| r.clone()).collect();

            append_all_matching_block_logs_sealed(&mut logs, block_hash_num, receipts, false, sealed_block)?;

            let block_header = reth_rpc_types_compat::block::from_primitive_with_hash(sealed_block.header.clone());

            let log_update = BlockLogs { block_header: block_header.clone(), logs };

            if let Err(e) = logs_channel.send(Message::new_with_time(log_update)).await {
                error!(error=?e.to_string(), "logs_channel.send")
            }
        }

        // Block state update
        if config.block_state_update {
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
                let block_header = reth_rpc_types_compat::block::from_primitive_with_hash(sealed_block.header.clone());

                let block_state_update = BlockStateUpdate { block_header: block_header.clone(), state_update: vec![state_update] };

                if let Err(e) = state_update_channel.send(Message::new_with_time(block_state_update)).await {
                    error!(error=?e.to_string(), "block_with_tx_channel.send")
                }
            }
        }
    }

    Ok(())
}

pub async fn loom_exex<Node: FullNodeComponents, DB: DatabaseRef + Send + Sync + Clone + 'static>(
    mut ctx: ExExContext<Node>,
    bc: Blockchain<DB>,
    config: NodeBlockActorConfig,
) -> eyre::Result<()> {
    info!("Loom ExEx is started");

    while let Some(exex_notification) = ctx.notifications.try_next().await? {
        match &exex_notification {
            ExExNotification::ChainCommitted { new } => {
                info!(committed_chain = ?new.range(), "Received commit");
                if let Err(e) = process_chain(
                    new.clone(),
                    bc.new_block_headers_channel(),
                    bc.new_block_with_tx_channel(),
                    bc.new_block_logs_channel(),
                    bc.new_block_state_update_channel(),
                    &config,
                )
                .await
                {
                    error!(error=?e, "process_chain");
                }
            }
            ExExNotification::ChainReorged { old, new } => {
                // revert to block before the reorg
                info!(from_chain = ?old.range(), to_chain = ?new.range(), "Received reorg");
                if let Err(e) = process_chain(
                    new.clone(),
                    bc.new_block_headers_channel(),
                    bc.new_block_with_tx_channel(),
                    bc.new_block_logs_channel(),
                    bc.new_block_state_update_channel(),
                    &config,
                )
                .await
                {
                    error!(error=?e, "process_chain");
                }
            }
            ExExNotification::ChainReverted { old } => {
                info!(reverted_chain = ?old.range(), "Received revert");
            }
        };
        if let Some(committed_chain) = exex_notification.committed_chain() {
            ctx.events.send(ExExEvent::FinishedHeight(committed_chain.tip().num_hash()))?;
        }
    }

    info!("Loom ExEx is finished");
    Ok(())
}

pub async fn mempool_worker<Pool, DB>(mempool: Pool, bc: Blockchain<DB>) -> eyre::Result<()>
where
    Pool: TransactionPool + Clone + 'static,
    DB: DatabaseRef + Send + Sync + Clone + 'static,
{
    info!("Mempool worker started");
    let mut tx_listener = mempool.new_transactions_listener();

    let mempool_tx = bc.new_mempool_tx_channel();

    loop {
        select! {
            tx_notification = tx_listener.recv() => {
                if let Some(tx_notification) = tx_notification {
                    let recovered_tx = tx_notification.transaction.to_recovered_transaction();
                    let tx_hash: TxHash = recovered_tx.hash;
                    let tx : alloy_rpc_types_eth::Transaction = reth_rpc_types_compat::transaction::from_recovered::<EthTxBuilder>(recovered_tx,&EthTxBuilder).inner;
                    let update_msg: MessageMempoolDataUpdate = MessageMempoolDataUpdate::new_with_source(NodeMempoolDataUpdate { tx_hash, mempool_tx: MempoolTx { tx: Some(tx), ..MempoolTx::default() } }, "exex".to_string());
                    if let Err(e) =  mempool_tx.send(update_msg).await {
                        error!(error=?e.to_string(), "mempool_tx.send");
                    }else{
                        debug!(hash = ?tx_notification.transaction.hash(), "Received pool tx");
                    }
                }
            }
        }
    }
}
