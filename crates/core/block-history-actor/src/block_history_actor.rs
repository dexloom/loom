use alloy_network::Ethereum;
use alloy_primitives::{BlockHash, BlockNumber};
use alloy_provider::Provider;
use alloy_rpc_types::Header;
use eyre::{eyre, Result};
use loom_core_actors::{run_sync, subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_evm_db::DatabaseLoomExt;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::ChainParameters;
use loom_types_entities::{BlockHistory, BlockHistoryManager, BlockHistoryState, LatestBlock, MarketState};
use loom_types_events::{MarketEvents, MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate};
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::borrow::BorrowMut;
use std::ops::DerefMut;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info, trace, warn};

pub async fn set_chain_head<P, DB>(
    block_history_manager: &BlockHistoryManager<P, DB>,
    block_history: &mut BlockHistory<DB>,
    latest_block: &mut LatestBlock,
    market_events_tx: Broadcaster<MarketEvents>,
    header: Header,
    chain_parameters: &ChainParameters,
) -> Result<(bool, usize)>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
    DB: Clone,
{
    let block_number = header.number;
    let block_hash = header.hash;

    debug!(%block_number, %block_hash, "set_chain_head block_number");

    match block_history_manager.set_chain_head(block_history, header.clone()).await {
        Ok((is_new_block, reorg_depth)) => {
            if reorg_depth > 0 {
                debug!("Re-org detected. Block {} Depth {} New hash {}", block_number, reorg_depth, block_hash);
            }

            if is_new_block {
                let base_fee = header.base_fee_per_gas.unwrap_or_default();
                let next_base_fee = chain_parameters.calc_next_block_base_fee(header.gas_used, header.gas_limit, base_fee);

                let timestamp: u64 = header.timestamp;

                latest_block.update(block_number, block_hash, Some(header), None, None, None);

                if let Err(e) =
                    market_events_tx.send(MarketEvents::BlockHeaderUpdate { block_number, block_hash, timestamp, base_fee, next_base_fee })
                {
                    error!("market_events_tx.send : {}", e);
                }
            }

            Ok((is_new_block, reorg_depth))
        }
        Err(e) => {
            error!("block_history_manager.set_chain_head error at {} hash {} error : {} ", block_number, block_hash, e);
            Err(eyre!("CANNOT_SET_CHAIN_HEAD"))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn new_block_history_worker<P, DB>(
    client: P,
    chain_parameters: ChainParameters,
    latest_block: SharedState<LatestBlock>,
    market_state: SharedState<MarketState<DB>>,
    block_history: SharedState<BlockHistory<DB>>,
    block_header_update_rx: Broadcaster<MessageBlockHeader>,
    block_update_rx: Broadcaster<MessageBlock>,
    log_update_rx: Broadcaster<MessageBlockLogs>,
    state_update_rx: Broadcaster<MessageBlockStateUpdate>,
    market_events_tx: Broadcaster<MarketEvents>,
) -> WorkerResult
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
    DB: BlockHistoryState + DatabaseRef + DatabaseCommit + DatabaseLoomExt + Send + Sync + Clone + 'static,
{
    subscribe!(block_header_update_rx);
    subscribe!(block_update_rx);
    subscribe!(log_update_rx);
    subscribe!(state_update_rx);

    debug!("new_block_history_worker started");

    let block_history_manager = BlockHistoryManager::new(client);

    loop {
        tokio::select! {
            msg = block_header_update_rx.recv() => {
                let block_update : Result<MessageBlockHeader, RecvError>  = msg;
                match block_update {
                    Ok(block_header)=>{
                        let mut block_history_guard = block_history.write().await;
                        let mut latest_block_guard = latest_block.write().await;

                        debug!("Block Header, Update {} {}", block_header.header.number, block_header.header.hash_slow());


                        set_chain_head(
                            &block_history_manager,
                            block_history_guard.borrow_mut(),
                            latest_block_guard.borrow_mut(),
                            market_events_tx.clone(),
                            block_header.inner.header,
                            &chain_parameters
                        ).await?;
                    }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }
            }

            msg = block_update_rx.recv() => {
                let block_update : Result<MessageBlock, RecvError>  = msg;
                match block_update {
                    Ok(block)=>{
                        let block = block.inner.block;
                        let block_header : Header = block.header.clone();
                        let block_hash : BlockHash = block_header.hash;
                        let block_number : BlockNumber = block_header.number;

                        debug!("Block Update {} {}", block_number, block_header.hash);

                        let mut block_history_guard = block_history.write().await;
                        let mut latest_block_guard = latest_block.write().await;

                        match set_chain_head(
                            &block_history_manager,
                            block_history_guard.borrow_mut(),
                            latest_block_guard.borrow_mut(),
                            market_events_tx.clone(),
                            block_header,
                            &chain_parameters
                        ).await
                            {
                                Ok(_)=>{
                                    match block_history_guard.add_block(block.clone()) {
                                        Ok(_)=>{
                                            if block_hash == latest_block_guard.block_hash {
                                                latest_block_guard.update(block_number, block_hash, None, Some(block.clone()), None, None );

                                                if let Err(e) = market_events_tx.send(MarketEvents::BlockTxUpdate{ block_number, block_hash}) {
                                                    error!("market_events_tx.send : {}", e)
                                                }
                                            }
                                        }
                                        Err(e)=>{
                                            error!("block_update add_block error at block {} with hash {} : {}", block_number, block_hash, e);
                                        }
                                    }
                                }
                                Err(e)=>{
                                    error!("{}", e);
                                }

                            }
                        }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }
            }
            msg = log_update_rx.recv() => {
                let log_update : Result<MessageBlockLogs, RecvError>  = msg;
                match log_update {
                    Ok(msg) =>{
                        let blocklogs = msg.inner;
                        let block_header : Header = blocklogs.block_header.clone();
                        let block_hash : BlockHash = block_header.hash;
                        let block_number : BlockNumber = block_header.number;

                        debug!("Block Logs Update {} {}", block_number, block_header.hash);

                        let mut block_history_guard = block_history.write().await;
                        let mut latest_block_guard = latest_block.write().await;

                        match set_chain_head(
                            &block_history_manager,
                            block_history_guard.borrow_mut(),
                            latest_block_guard.borrow_mut(),
                            market_events_tx.clone(),
                            block_header,
                            &chain_parameters
                        ).await
                        {
                            Ok(_)=>{
                                match block_history_guard.add_logs(block_hash,blocklogs.logs.clone()) {
                                    Ok(_)=>{
                                        if block_hash == latest_block_guard.block_hash {
                                            latest_block_guard.update(block_number, block_hash, None, None,Some(blocklogs.logs), None );

                                            if let Err(e) = market_events_tx.send(MarketEvents::BlockLogsUpdate { block_number, block_hash}) {
                                                error!("market_events_tx.send : {}", e)
                                            }
                                        }
                                    }
                                    Err(e)=>{
                                        error!("block_logs_update add_logs error at block {} with hash {} : {}", block_number, block_hash, e);
                                    }
                                }
                            }
                            Err(e)=>{
                                error!("block_logs_update {}", e);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }

            }
            msg = state_update_rx.recv() => {

                let state_update_msg : Result<MessageBlockStateUpdate, RecvError> = msg;

                let msg = match state_update_msg {
                    Ok(message_block_state_update) => message_block_state_update,
                    Err(e) => {
                        error!("state_update_rx.recv error {}", e);
                        continue
                    }
                };

                let msg = msg.inner;
                let msg_block_header = msg.block_header;
                let msg_block_number : BlockNumber = msg_block_header.number;
                let msg_block_hash : BlockHash = msg_block_header.hash;
                debug!("Block State update {} {}", msg_block_number, msg_block_hash);


                let mut block_history_guard = block_history.write().await;
                let mut latest_block_guard = latest_block.write().await;
                let mut market_state_guard = market_state.write().await;


                if let Err(e) = set_chain_head(&block_history_manager, block_history_guard.borrow_mut(),
                    latest_block_guard.borrow_mut(),market_events_tx.clone(), msg_block_header, &chain_parameters).await {
                    error!("set_chain_head : {}", e);
                    continue
                }

                let (latest_block_number, latest_block_hash) = latest_block_guard.number_and_hash();
                let latest_block_parent_hash = latest_block_guard.parent_hash().unwrap_or_default();

                if latest_block_hash != msg_block_hash {
                    warn!(%msg_block_number, %msg_block_hash, %latest_block_number, %latest_block_hash, "State update for block that is not latest.");
                    if let Err(err) = block_history_guard.add_state_diff(msg_block_hash,  msg.state_update.clone()) {
                        error!(%err, %msg_block_number, %msg_block_hash, "Error during add_state_diff.");
                    }
                } else{
                    latest_block_guard.update(msg_block_number, msg_block_hash, None, None, None, Some(msg.state_update.clone()) );

                    let new_market_state_db = if market_state_guard.block_hash.is_zero() || market_state_guard.block_hash == latest_block_parent_hash {
                         market_state_guard.state_db.clone()
                    } else {
                        match block_history_manager.apply_state_update_on_parent_db(block_history_guard.deref_mut(), &market_state_guard.config, msg_block_hash ).await {
                            Ok(db) => db,
                            Err(err) => {
                                error!(%err, %msg_block_number, %msg_block_hash, "Error during apply_state_update_on_parent_db.");
                                continue
                            }
                        }
                    };


                    if let Err(err) = block_history_guard.add_state_diff(msg_block_hash, msg.state_update.clone()) {
                        error!(%err, %msg_block_number, %msg_block_hash, "Error during block_history.add_state_diff.");
                        continue
                    }

                    let block_history_entry = block_history_guard.get_block_history_entry(&msg_block_hash);

                    let Some(block_history_entry) = block_history_entry else { continue };

                    let updated_db = new_market_state_db.apply_update(block_history_entry, &market_state_guard.config);

                    if let Err(err) = block_history_guard.add_db(msg_block_hash, updated_db.clone()) {
                        error!(%err, %msg_block_number, %msg_block_hash, "Error during block_history.add_db.");
                        continue
                    }

                    debug!("Block History len: {}", block_history_guard.len());

                    let accounts_len = market_state_guard.state_db.accounts_len();
                    let contracts_len = market_state_guard.state_db.contracts_len();
                    let storage_len = market_state_guard.state_db.storage_len();

                    trace!("Market state len accounts {} contracts {} storage {}", accounts_len, contracts_len, storage_len);

                    info!("market state updated ok records : update len: {} accounts: {} contracts: {} storage: {}", msg.state_update.len(),
                         updated_db.accounts_len(), updated_db.contracts_len() , updated_db.storage_len() );

                    market_state_guard.state_db = updated_db.clone();
                    market_state_guard.block_hash = msg_block_hash;
                    market_state_guard.block_number = latest_block_number;


                    run_sync!(market_events_tx.send(MarketEvents::BlockStateUpdate{ block_hash : msg_block_hash} ));


                    #[cfg(not(debug_assertions))]
                    {
                        // Merging DB in background and update market state
                        let market_state_clone = market_state.clone();

                        tokio::task::spawn( async move{
                            let merged_db = updated_db.maintain();
                            let mut market_state_guard = market_state_clone.write().await;
                            market_state_guard.state_db = merged_db;
                            debug!("Merged DB stored in MarketState at block {}", msg_block_number)
                        });
                    }

                    #[cfg(debug_assertions)]
                    {

                        market_state_guard.state_db = updated_db.maintain();

                        let accounts = market_state_guard.state_db.accounts_len();

                        let storage = market_state_guard.state_db.storage_len();
                        let contracts = market_state_guard.state_db.contracts_len();

                        trace!(accounts, storage, contracts, "Merging finished. Market state len" );

                    }



                }

            }
        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct BlockHistoryActor<P, DB> {
    client: P,
    chain_parameters: ChainParameters,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[accessor]
    market_state: Option<SharedState<MarketState<DB>>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory<DB>>>,
    #[consumer]
    block_header_update_rx: Option<Broadcaster<MessageBlockHeader>>,
    #[consumer]
    block_update_rx: Option<Broadcaster<MessageBlock>>,
    #[consumer]
    log_update_rx: Option<Broadcaster<MessageBlockLogs>>,
    #[consumer]
    state_update_rx: Option<Broadcaster<MessageBlockStateUpdate>>,
    #[producer]
    market_events_tx: Option<Broadcaster<MarketEvents>>,
}

impl<P, DB> BlockHistoryActor<P, DB>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Sync + Send + Clone + 'static,
    DB: DatabaseRef + BlockHistoryState + DatabaseLoomExt + DatabaseCommit + Database + Send + Sync + Clone + Default + 'static,
{
    pub fn new(client: P) -> Self {
        Self {
            client,
            chain_parameters: ChainParameters::ethereum(),
            latest_block: None,
            market_state: None,
            block_history: None,
            block_header_update_rx: None,
            block_update_rx: None,
            log_update_rx: None,
            state_update_rx: None,
            market_events_tx: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain, state: &BlockchainState<DB>) -> Self {
        Self {
            chain_parameters: bc.chain_parameters(),
            latest_block: Some(bc.latest_block()),
            block_header_update_rx: Some(bc.new_block_headers_channel()),
            block_update_rx: Some(bc.new_block_with_tx_channel()),
            log_update_rx: Some(bc.new_block_logs_channel()),
            state_update_rx: Some(bc.new_block_state_update_channel()),
            market_events_tx: Some(bc.market_events_channel()),
            market_state: Some(state.market_state()),
            block_history: Some(state.block_history()),
            ..self
        }
    }
}

impl<P, DB> Actor for BlockHistoryActor<P, DB>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Sync + Send + Clone + 'static,
    DB: BlockHistoryState + DatabaseRef + DatabaseCommit + DatabaseLoomExt + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_block_history_worker(
            self.client.clone(),
            self.chain_parameters.clone(),
            self.latest_block.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.block_history.clone().unwrap(),
            self.block_header_update_rx.clone().unwrap(),
            self.block_update_rx.clone().unwrap(),
            self.log_update_rx.clone().unwrap(),
            self.state_update_rx.clone().unwrap(),
            self.market_events_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "BlockHistoryActor"
    }
}
