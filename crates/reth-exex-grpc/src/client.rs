use std::collections::BTreeMap;

use alloy_primitives::{map::HashMap, Address, U256};
use alloy_rpc_types::{
    serde_helpers::WithOtherFields,
    {Block, BlockTransactions, BlockTransactionsKind},
};
use alloy_rpc_types_trace::geth::AccountState;
use async_stream::stream;
use eyre::{eyre, Result};
use reth::primitives::{SealedHeader, TransactionSigned};
use reth::revm::db::states::StorageSlot;
use reth::revm::db::{BundleAccount, StorageWithOriginalValues};
use reth::rpc::eth::EthTxBuilder;
use reth_exex::ExExNotification;
use reth_primitives::SealedBlockWithSenders;
use reth_tracing::tracing::error;
use tokio_stream::Stream;
use tonic::transport::Channel;

use crate::helpers::append_all_matching_block_logs_sealed;
use crate::proto::remote_ex_ex_client::RemoteExExClient;
use crate::proto::SubscribeRequest;

#[derive(Debug, Clone)]
pub struct ExExClient {
    client: RemoteExExClient<Channel>,
}

impl ExExClient {
    pub async fn connect(url: String) -> eyre::Result<ExExClient> {
        let client = RemoteExExClient::connect(url).await?.max_encoding_message_size(usize::MAX).max_decoding_message_size(usize::MAX);

        Ok(ExExClient { client })
    }

    pub async fn subscribe_mempool_tx(&self) -> Result<impl Stream<Item = WithOtherFields<alloy_rpc_types::eth::Transaction>> + '_> {
        let stream = self.client.clone().subscribe_mempool_tx(SubscribeRequest {}).await;
        let mut stream = match stream {
            Ok(stream) => stream.into_inner(),
            Err(e) => {
                error!(error=?e, "subscribe header");
                return Err(eyre!("ERROR"));
            }
        };

        Ok(stream! {
            loop {
                match stream.message().await {
                    Ok(Some(transaction_proto)) => {
                        if let Ok(transaction) = TransactionSigned::try_from(&transaction_proto){
                            if let Some(transaction) = transaction.into_ecrecovered() {
                                let transaction = reth_rpc_types_compat::transaction::from_recovered::<EthTxBuilder>(transaction, &EthTxBuilder);
                                yield transaction;
                            }
                        }
                    }
                    Ok(None) => break, // Stream has ended
                    Err(err) => {
                        eprintln!("Error receiving mempooltx.message: {:?}", err);
                        break;
                    }
                }
            }
        })
    }

    pub async fn subscribe_header(&self) -> Result<impl Stream<Item = alloy_rpc_types::Header> + '_> {
        let stream = self.client.clone().subscribe_header(SubscribeRequest {}).await;

        let mut stream = match stream {
            Ok(stream) => stream.into_inner(),
            Err(e) => {
                error!(error=?e, "subscribe header");
                return Err(eyre!("ERROR"));
            }
        };
        Ok(stream! {
            loop {
                match stream.message().await {
                    Ok(Some(sealed_header)) => {
                        if let Ok(header)  = sealed_header.try_into() {
                            let header = reth_rpc_types_compat::block::from_primitive_with_hash(header);
                            yield header;
                        }
                    },
                    Ok(None) => {
                        break;

                    } // Stream has ended
                    Err(err) => {
                        eprintln!("Error receiving header.message: {:?}", err);
                        break;
                    }
                }
            }
        })
    }

    pub async fn subscribe_block(&self) -> Result<impl Stream<Item = alloy_rpc_types::Block>> {
        let stream = self.client.clone().subscribe_block(SubscribeRequest {}).await;

        let mut stream = match stream {
            Ok(stream) => stream.into_inner(),
            Err(e) => {
                error!(error=?e, "subscribe header");
                return Err(eyre!("ERROR"));
            }
        };

        Ok(stream! {
            loop {
                match stream.message().await {
                    Ok(Some(block_msg)) => {
                        if let Ok(sealed_block)  = SealedBlockWithSenders::try_from(&block_msg) {
                            let diff = sealed_block.difficulty;
                            let hash = sealed_block.hash();

                            if let Ok(block) = reth_rpc_types_compat::block::from_block::<EthTxBuilder>(
                                sealed_block.unseal(),
                                diff,
                                BlockTransactionsKind::Full,
                                Some(hash),
                                &EthTxBuilder)
                            {

                                let block_eth : Block = Block {
                                    header: block.header,
                                    uncles: block.uncles,
                                    transactions: BlockTransactions::Full(block.transactions.into_transactions().map(|t| t.inner).collect()),
                                    size: block.size,
                                    withdrawals: block.withdrawals
                                };


                                yield block_eth
                            }
                        }
                    },
                    Ok(None) => break, // Stream has ended
                    Err(err) => {
                        eprintln!("Error receiving block.message: {:?}", err);
                        break;
                    }
                }
            }
        })
    }
    pub async fn subscribe_logs(&self) -> Result<impl Stream<Item = (alloy_rpc_types::Header, Vec<alloy_rpc_types::Log>)>> {
        let stream = self.client.clone().subscribe_receipts(SubscribeRequest {}).await;

        let mut stream = match stream {
            Ok(stream) => stream.into_inner(),
            Err(e) => {
                error!(error=?e, "subscribe receipts");
                return Err(eyre!("ERROR"));
            }
        };
        Ok(stream! {
            loop {
                match stream.message().await {
                    Ok(Some(notification)) => {
                        if let Some(receipts) = notification.receipts {
                            if let Some(sealed_block) = notification.block {
                                if let Ok((block_hash, block_header, logvec)) = append_all_matching_block_logs_sealed(
                                    receipts,
                                    false,
                                    sealed_block,
                                ){
                                    let header : alloy_rpc_types::Header = reth_rpc_types_compat::block::from_primitive_with_hash(SealedHeader::new(block_header, block_hash));
                                    yield (header, logvec);
                                }
                            }
                        }

                    },
                    Ok(None) => break, // Stream has ended
                    Err(err) => {
                        eprintln!("Error receiving logs.message: {:?}", err);
                        break;
                    }
                }
            }
        })
    }

    pub async fn subscribe_stata_update(&self) -> Result<impl Stream<Item = (alloy_rpc_types::Header, BTreeMap<Address, AccountState>)>> {
        let stream = self.client.clone().subscribe_state_update(SubscribeRequest {}).await;

        let mut stream = match stream {
            Ok(stream) => stream.into_inner(),
            Err(e) => {
                error!(error=?e, "subscribe receipts");
                return Err(eyre!("ERROR"));
            }
        };
        Ok(stream! {
            loop {
                match stream.message().await {
                    Ok(Some(state_update)) => {
                        if let Some(sealed_header) = state_update.sealed_header {
                            if let Ok(header)  = sealed_header.try_into() {
                                let header = reth_rpc_types_compat::block::from_primitive_with_hash(header);

                                if let Some(bundle_proto) = state_update.bundle {

                                    if let Ok(bundle_state) = reth::revm::db::BundleState::try_from(&bundle_proto){
                                        let mut state_update : BTreeMap<Address, AccountState> = BTreeMap::new();

                                        let state_ref: &HashMap<Address, BundleAccount> = &bundle_state.state;

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
                                                account_state
                                                    .storage
                                                    .insert((*key).into(), storage_slot.present_value.into());
                                            }
                                        }
                                        yield (header, state_update);
                                    }
                                }
                            }
                        }
                    },
                    Ok(None) => break, // Stream has ended
                    Err(err) => {
                        eprintln!("Error receiving state_update.message: {:?}", err);
                        break;
                    }
                }
            }
        })
    }

    pub async fn subscribe_exex(&self) -> Result<impl Stream<Item = ExExNotification> + '_> {
        let stream = self.client.clone().subscribe_ex_ex(SubscribeRequest {}).await;

        let mut stream = match stream {
            Ok(stream) => stream.into_inner(),
            Err(e) => {
                error!(error=?e, "subscribe exex");
                return Err(eyre!("ERROR"));
            }
        };

        Ok(stream! {


            loop {
                match stream.message().await {
                    Ok(Some(notification)) => {
                            match ExExNotification::try_from(&notification) {
                                Ok(notification) => yield notification,
                                Err(err) => eprintln!("Error converting notification: {:?}", err),
                            }
                        },
                    Ok(None) => break, // Stream has ended
                    Err(err) => {
                        eprintln!("Error receiving exex.message: {:?}", err);
                        break;
                    }
                }
            }
        })
    }
}
