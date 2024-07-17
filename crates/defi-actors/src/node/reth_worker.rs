use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use alloy_eips::BlockNumHash;
use alloy_network::Network;
use alloy_primitives::{Address, BlockHash, B256};
use alloy_provider::Provider;
use alloy_rpc_types::{Block, Header, Log};
use alloy_rpc_types_trace::geth::AccountState;
use alloy_transport::Transport;
use chrono::Utc;
use futures::StreamExt;
use log::{debug, error, info, trace};
use reth_chainspec::ChainSpecBuilder;
use reth_db::open_db_read_only;
use reth_primitives::{BlockHashOrNumber, BlockWithSenders};
use reth_provider::providers::StaticFileProvider;
use reth_provider::{AccountExtReader, BlockReader, ProviderFactory, ReceiptProvider, StateProvider, StorageReader, TransactionVariant};

use defi_events::{BlockLogs, BlockStateUpdate};
use loom_actors::{ActorResult, Broadcaster, WorkerResult};
use loom_utils::reth_types::append_all_matching_block_logs;

pub async fn reth_node_worker<P, T, N>(
    client: P,
    db_path: String,
    new_block_headers_channel: Option<Broadcaster<Header>>,
    new_block_with_tx_channel: Option<Broadcaster<Block>>,
    new_block_logs_channel: Option<Broadcaster<BlockLogs>>,
    new_block_state_update_channel: Option<Broadcaster<BlockStateUpdate>>,
) -> WorkerResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    info!("Starting node block hash worker");

    let sub = client.subscribe_blocks().await?;
    let mut stream = sub.into_stream();

    let mut block_processed: HashMap<BlockHash, chrono::DateTime<Utc>> = HashMap::new();

    let db_path = Path::new(&db_path);
    let db = open_db_read_only(db_path, Default::default())?;
    let spec = ChainSpecBuilder::mainnet().build();

    let factory = ProviderFactory::new(db, spec.into(), StaticFileProvider::read_only(db_path.join("static_files"))?);

    loop {
        tokio::select! {
            block_msg = stream.next() => {
                if let Some(block) = block_msg {
                    let block : Block = block;

                    if let Some(block_number) = block.header.number{

                        if let Some(block_hash) = block.header.hash  {
                            info!("Block hash received: {:?}" , block_hash);

                            let db_provider = factory.provider()?;
                            let state_provider = factory.latest()?;

                            let mut block_with_senders : Option<BlockWithSenders> = None;

                            if let std::collections::hash_map::Entry::Vacant(e) = block_processed.entry(block_hash) {
                                e.insert(Utc::now());

                                if let Some(block_headers_channel) = &new_block_headers_channel {
                                    if let Err(e) = block_headers_channel.send(block.header.clone()).await {
                                        error!("Block header broadcaster error {}", e);
                                    }
                                };
                                if let Some(block_with_tx_channel) = &new_block_with_tx_channel {
                                    //match provider.block(BlockHashOrNumber::Hash(block_hash)) {
                                    match db_provider.block_with_senders(BlockHashOrNumber::Hash(block_hash), TransactionVariant::WithHash) {

                                        Ok(block_with_senders_reth )=>{
                                            block_with_senders.clone_from(&block_with_senders_reth);

                                            if let Some(block_with_senders_reth) = block_with_senders_reth {
                                                debug!("block_with_senders_reth : txs {}", block_with_senders_reth.body.len());

                                                //convert RETH->RPC
                                                let block_with_senders_rpc = reth_rpc_types_compat::block::from_block_with_tx_hashes(block_with_senders_reth, block.header.total_difficulty.unwrap_or_default(), block.header.hash);
                                                //broadcast
                                                match block_with_tx_channel.send(block_with_senders_rpc).await {
                                                     Err(e) => {error!("Block header broadcaster error {}", e)}
                                                     _=>{
                                                        trace!("Block header sent");
                                                    }
                                                }
                                            }else{
                                                error!("block_with_senders_is None {block_number} {block_hash}");
                                            }
                                        }
                                        Err(e)=>{
                                            error!("block_with_senders error : {}", e);
                                            block_with_senders = None;
                                        }
                                    }
                                };

                                if let Some(block_logs_channel) = &new_block_logs_channel {
                                    match db_provider.receipts_by_block(BlockHashOrNumber::Hash(block_hash)) {
                                        Ok(block_receipts_reth)=>{

                                            if let Some(block_receipts_reth) = block_receipts_reth {
                                                if let Some(block_body_indexes) = db_provider.block_body_indices(block_number)? {

                                                    let mut logs : Vec<Log> = Vec::new();
                                                    if let Some(block_with_senders) = block_with_senders {
                                                        match append_all_matching_block_logs(&mut logs,  BlockNumHash::new(block_number, block_hash), block_receipts_reth, false, block_body_indexes, block_with_senders) {
                                                            Ok(_)=>{
                                                                trace!("logs {block_number} {block_hash} : {logs:?}");

                                                                let logs_update = BlockLogs {
                                                                    block_hash,
                                                                    logs
                                                                };

                                                                match block_logs_channel.send(logs_update).await {
                                                                     Err(e) => {error!("Block header broadcaster error {}", e)}
                                                                     _=>{
                                                                        trace!("Logs update sent")
                                                                     }
                                                                }
                                                            }
                                                            Err(e)=>{
                                                                error!("append_all_matching_block_logs error : {}", e);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Err(e)=>{
                                            error!("receipts_by_block error: {}", e)
                                        }
                                    }
                                }


                                if let Some(block_state_update_channel) = &new_block_state_update_channel {
                                    let changed_accounts = db_provider.changed_accounts_with_range(block_number..=block_number)?;


                                    let changed_storage = db_provider.changed_storages_with_range(block_number..=block_number)?;

                                    let storage_update : BTreeMap<Address, BTreeMap<B256, B256> >= changed_storage.into_iter().map(|(acc, cells)| {
                                        let cells : BTreeMap<B256, B256> = cells.into_iter().filter_map(|cell|
                                            match state_provider.storage(acc, cell){
                                                Ok(Some(x))=>{
                                                    Some( (cell, B256::from(x)  ))
                                                }
                                                _=>None
                                            }
                                        ).collect();

                                        (acc, cells)
                                    }).collect();

                                    // TODO : Check this code
                                    trace!("changed storage {block_number} {block_hash} : {storage_update:?}");

                                    //let state_update_map : HashMap<Address, AccountState> = HashMap::new();

                                    let  accounts = db_provider.basic_accounts(changed_accounts)?;

                                    let mut account_btree : BTreeMap<Address, AccountState> = accounts.into_iter().map(|(address, account)|{
                                        let account = account.unwrap_or_default();
                                        let account_code = if account.has_bytecode() {
                                             state_provider.bytecode_by_hash(account.bytecode_hash.unwrap_or_default()).ok().unwrap_or_default()
                                        }else{
                                            None
                                        };


                                        (address, AccountState{
                                            balance: Some(account.balance),
                                            code: account_code.map(|c|c.bytes()),
                                            nonce: Some(account.nonce),
                                            storage : Default::default()  })
                                    }).collect();

                                    for (account, storage_update) in storage_update.into_iter(){
                                        account_btree.entry(account).or_default().storage = storage_update;
                                    }

                                    debug!("StateUpdate created {block_number} {block_hash} : len {}", account_btree.len());


                                    let state_update = BlockStateUpdate {
                                        block_hash,
                                        state_update: vec![account_btree],
                                    };


                                    match block_state_update_channel.send(state_update).await {
                                         Err(e) => {error!("Block header broadcaster error {}", e)}
                                         _=>{
                                            trace!("State update sent")
                                        }
                                    }
                                };
                            } else {
                                error!("No block hash")
                            }

                            drop(db_provider);

                        }
                    }
                }
            }
        }
    }
}

pub async fn reth_node_worker_starter<P, T, N>(
    client: P,
    db_path: String,
    new_block_headers_channel: Option<Broadcaster<Header>>,
    new_block_with_tx_channel: Option<Broadcaster<Block>>,
    new_block_logs_channel: Option<Broadcaster<BlockLogs>>,
    new_block_state_update_channel: Option<Broadcaster<BlockStateUpdate>>,
) -> ActorResult
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    let handler = tokio::task::spawn(reth_node_worker(
        client,
        db_path.clone(),
        new_block_headers_channel,
        new_block_with_tx_channel,
        new_block_logs_channel,
        new_block_state_update_channel,
    ));
    Ok(vec![handler])
}
