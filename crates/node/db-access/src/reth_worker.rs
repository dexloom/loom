use alloy_eips::{BlockHashOrNumber, BlockNumHash};
use alloy_network::Ethereum;
use alloy_primitives::{Address, BlockHash, B256};
use alloy_provider::Provider;
use alloy_rpc_types::{Block, BlockTransactions, Log};
use alloy_rpc_types_trace::geth::AccountState;
use chrono::Utc;
use futures::StreamExt;
use reth_chainspec::ChainSpecBuilder;
use reth_db::mdbx::DatabaseArguments;
use reth_db::{open_db_read_only, ClientVersion, DatabaseEnv};
use reth_node_ethereum::EthereumNode;
use reth_node_types::NodeTypesWithDBAdapter;
use reth_primitives::{Block as RethBlock, RecoveredBlock};
use reth_provider::providers::StaticFileProvider;
use reth_provider::BlockBodyIndicesProvider;
use reth_provider::{AccountExtReader, BlockReader, ProviderFactory, ReceiptProvider, StateProvider, StorageReader, TransactionVariant};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, trace};

use loom_core_actors::{Actor, ActorResult, Broadcaster, Producer, WorkerResult};
use loom_core_actors_macros::Producer;
use loom_core_blockchain::Blockchain;
use loom_evm_utils::reth_types::append_all_matching_block_logs;
use loom_node_actor_config::NodeBlockActorConfig;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_events::{
    BlockHeader, BlockLogs, BlockStateUpdate, BlockUpdate, Message, MessageBlock, MessageBlockHeader, MessageBlockLogs,
    MessageBlockStateUpdate,
};

pub async fn reth_node_worker<P>(
    client: P,
    db_path: String,
    new_block_headers_channel: Option<Broadcaster<MessageBlockHeader>>,
    new_block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    new_block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    new_block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
) -> WorkerResult
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    info!("Starting node block hash worker");

    let sub = client.subscribe_blocks().await?;
    let mut stream = sub.into_stream();

    let mut block_processed: HashMap<BlockHash, chrono::DateTime<Utc>> = HashMap::new();

    let db_path = Path::new(&db_path);
    let db = Arc::new(open_db_read_only(db_path.join("db").as_path(), DatabaseArguments::new(ClientVersion::default()))?);
    let spec = Arc::new(ChainSpecBuilder::mainnet().build());
    let factory = ProviderFactory::<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>::new(
        db.clone(),
        spec.clone(),
        StaticFileProvider::read_only(db_path.join("static_files"), true)?,
    );

    loop {
        tokio::select! {
        block_msg = stream.next() => {
            let Some(block_header) = block_msg else {
                    continue
            };
            let block_number = block_header.number;
            let block_hash = block_header.hash;
                    info!("Block hash received: {:?}" , block_hash);

                    let db_provider = factory.provider()?;
                    let state_provider = factory.latest()?;

                    let mut block_with_senders : Option<RecoveredBlock<RethBlock>> = None;

                    if let std::collections::hash_map::Entry::Vacant(e) = block_processed.entry(block_hash) {
                        e.insert(Utc::now());

                        if block_processed.len() > 10 {
                            let oldest_hash = block_processed.keys()
                              .min_by_key(|&&hash| block_processed[&hash])
                              .cloned();

                           if let Some(oldest_hash) = oldest_hash {
                                    block_processed.remove(&oldest_hash);
                                }
                    }

                        if let Some(block_headers_channel) = &new_block_headers_channel {
                            if let Err(e) = block_headers_channel.send(MessageBlockHeader::new_with_time(BlockHeader::new( block_header.clone()))) {
                                error!("Block header broadcaster error {}", e);
                            }
                        };
                        if let Some(block_with_tx_channel) = &new_block_with_tx_channel {
                            //match provider.block(BlockHashOrNumber::Hash(block_hash)) {
                            match db_provider.block_with_senders(BlockHashOrNumber::Hash(block_hash), TransactionVariant::WithHash) {

                                Ok(block_with_senders_reth )=>{
                                    block_with_senders.clone_from(&block_with_senders_reth);

                                    if let Some(block_with_senders_reth) = block_with_senders_reth {
                                        debug!("block_with_senders_reth : txs {}", block_with_senders_reth.body().transactions.len());

                                        //convert RETH->RPCx
                                        let block_with_senders_rpc = reth_rpc_types_compat::block::from_block_with_tx_hashes(block_with_senders_reth);

                                        let txs = BlockTransactions::Full(block_with_senders_rpc.transactions.clone().into_transactions().collect());
                                        // remove OtherFields
                                        let block_with_senders_rpc : Block = Block{
                                            transactions: txs,
                                            header: block_with_senders_rpc.header,
                                            uncles: block_with_senders_rpc.uncles,
                                            withdrawals : block_with_senders_rpc.withdrawals,
                                        };

                                        //broadcast
                                        match block_with_tx_channel.send( Message::new_with_time(BlockUpdate{ block : block_with_senders_rpc})) {
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
                                                            block_header : block_header.clone(),
                                                            logs
                                                        };

                                                        match block_logs_channel.send(Message::new_with_time(  logs_update)) {
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
                                let new_cells : BTreeMap<B256, B256> = cells.iter().filter_map(|cell|
                                    match state_provider.storage(acc, *cell){
                                        Ok(Some(x))=>{
                                            Some( (*cell, B256::from(x)  ))
                                        }
                                        _=>None
                                    }
                                ).collect();

                                (acc, new_cells)
                            }).collect();

                            // TODO : Check this code
                            trace!("changed storage {block_number} {block_hash} : {storage_update:?}");

                            //let state_update_map : HashMap<Address, AccountState> = HashMap::new();

                            let  accounts = db_provider.basic_accounts(changed_accounts)?;

                            let mut account_btree : BTreeMap<Address, AccountState> = accounts.into_iter().map(|(address, account)|{
                                let account = account.unwrap_or_default();
                                let account_code = if account.has_bytecode() {
                                     state_provider.bytecode_by_hash(&account.bytecode_hash.unwrap_or_default()).ok().unwrap_or_default()
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
                                block_header : block_header.clone(),
                                state_update: vec![account_btree],
                            };


                            match block_state_update_channel.send( Message::new_with_time(state_update)) {
                                 Err(e) => {error!("Block header broadcaster error {}", e)}
                                 _=>{
                                    trace!("State update sent")
                                }
                            }
                        };
                    }
                    drop(db_provider);


            }
        }
    }
}

pub fn reth_node_worker_starter<P>(
    client: P,
    db_path: String,
    new_block_headers_channel: Option<Broadcaster<MessageBlockHeader>>,
    new_block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    new_block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    new_block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
) -> ActorResult
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
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

// When using this actor make sure to set the persistence threshold to zero when reth is started
#[derive(Producer)]
pub struct RethDbAccessBlockActor<P> {
    client: P,
    config: NodeBlockActorConfig,
    reth_db_path: String,
    #[producer]
    block_header_channel: Option<Broadcaster<MessageBlockHeader>>,
    #[producer]
    block_with_tx_channel: Option<Broadcaster<MessageBlock>>,
    #[producer]
    block_logs_channel: Option<Broadcaster<MessageBlockLogs>>,
    #[producer]
    block_state_update_channel: Option<Broadcaster<MessageBlockStateUpdate>>,
}

impl<P> RethDbAccessBlockActor<P>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
{
    fn name(&self) -> &'static str {
        "NodeBlockActor"
    }

    pub fn new(client: P, config: NodeBlockActorConfig, reth_db_path: String) -> RethDbAccessBlockActor<P> {
        RethDbAccessBlockActor {
            client,
            config,
            reth_db_path,
            block_header_channel: None,
            block_with_tx_channel: None,
            block_logs_channel: None,
            block_state_update_channel: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            block_header_channel: if self.config.block_header { Some(bc.new_block_headers_channel()) } else { None },
            block_with_tx_channel: if self.config.block_with_tx { Some(bc.new_block_with_tx_channel()) } else { None },
            block_logs_channel: if self.config.block_logs { Some(bc.new_block_logs_channel()) } else { None },
            block_state_update_channel: if self.config.block_state_update { Some(bc.new_block_state_update_channel()) } else { None },
            ..self
        }
    }
}

impl<P> Actor for RethDbAccessBlockActor<P>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        reth_node_worker_starter(
            self.client.clone(),
            self.reth_db_path.clone(),
            self.block_header_channel.clone(),
            self.block_with_tx_channel.clone(),
            self.block_logs_channel.clone(),
            self.block_state_update_channel.clone(),
        )
    }
    fn name(&self) -> &'static str {
        self.name()
    }
}

#[cfg(test)]
mod test {
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::WsConnect;
    use alloy_rpc_types::Header;
    use tokio::select;
    use tracing::{debug, error, info};

    use crate::reth_worker::RethDbAccessBlockActor;
    use eyre::Result;
    use loom_core_actors::{Actor, Broadcaster, Producer};
    use loom_node_actor_config::NodeBlockActorConfig;
    use loom_types_events::{MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate};

    #[tokio::test]
    #[ignore]
    async fn revm_worker_test() -> Result<()> {
        let _ = env_logger::builder().format_timestamp_millis().try_init();

        info!("Creating channels");
        let new_block_headers_channel: Broadcaster<MessageBlockHeader> = Broadcaster::new(10);
        let new_block_with_tx_channel: Broadcaster<MessageBlock> = Broadcaster::new(10);
        let new_block_state_update_channel: Broadcaster<MessageBlockStateUpdate> = Broadcaster::new(10);
        let new_block_logs_channel: Broadcaster<MessageBlockLogs> = Broadcaster::new(10);

        let node_url = std::env::var("MAINNET_WS")?;

        let ws_connect = WsConnect::new(node_url);
        //let client = ClientBuilder::default().ws(ws_connect).await?;
        //let client = ProviderBuilder::new().on_client(client).;

        let client = ProviderBuilder::new().disable_recommended_fillers().on_ws(ws_connect).await?;

        let db_path = std::env::var("RETH_DB_PATH")?;

        let mut node_block_actor = RethDbAccessBlockActor::new(client.clone(), NodeBlockActorConfig::all_enabled(), db_path);
        match node_block_actor
            .produce(new_block_headers_channel.clone())
            .produce(new_block_with_tx_channel.clone())
            .produce(new_block_logs_channel.clone())
            .produce(new_block_state_update_channel.clone())
            .start()
        {
            Err(e) => {
                error!("{}", e)
            }
            _ => {
                info!("Node actor started successfully")
            }
        }

        let mut new_block_rx = new_block_headers_channel.subscribe();
        let mut new_block_with_tx_rx = new_block_with_tx_channel.subscribe();
        let mut new_block_logs_rx = new_block_logs_channel.subscribe();
        let mut new_block_state_update_rx = new_block_state_update_channel.subscribe();

        for i in 1..10 {
            select! {
                msg_fut = new_block_rx.recv() => {
                    let msg : Header = msg_fut?.inner.header;
                    debug!("Block header received : {:?}", msg);
                }
                msg_fut = new_block_with_tx_rx.recv() => {
                    let msg : MessageBlock = msg_fut?;
                    debug!("Block withtx received : {:?}", msg);
                }
                msg_fut = new_block_logs_rx.recv() => {
                    let msg : MessageBlockLogs = msg_fut?;
                    debug!("Block logs received : {:?}", msg);
                }
                msg_fut = new_block_state_update_rx.recv() => {
                    let msg : MessageBlockStateUpdate = msg_fut?;
                    debug!("Block state update received : {:?}", msg);
                }

            }
            println!("{i}")
        }
        Ok(())
    }
}
