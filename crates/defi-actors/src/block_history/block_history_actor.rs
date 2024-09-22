use alloy_network::Ethereum;
use alloy_primitives::{BlockHash, BlockNumber};
use alloy_provider::Provider;
use alloy_rpc_types::Block;
use alloy_transport::Transport;
use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::{apply_state_update, BlockHistory, BlockHistoryManager, LatestBlock, MarketState};
use defi_events::{BlockLogs, BlockStateUpdate, MarketEvents, MessageBlockHeader};
use eyre::Result;
use log::{debug, error, info, trace};
use loom_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_revm_db::LoomInMemoryDB;
use std::borrow::BorrowMut;
use std::marker::PhantomData;
use std::ops::DerefMut;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

#[allow(clippy::too_many_arguments)]
pub async fn new_block_history_worker<P, T>(
    client: P,
    latest_block: SharedState<LatestBlock>,
    market_state: SharedState<MarketState>,
    block_history: SharedState<BlockHistory>,
    block_header_update_rx: Broadcaster<MessageBlockHeader>,
    block_update_rx: Broadcaster<Block>,
    log_update_rx: Broadcaster<BlockLogs>,
    state_update_rx: Broadcaster<BlockStateUpdate>,
    market_events_tx: Broadcaster<MarketEvents>,
) -> WorkerResult
where
    T: Transport + Clone + Send + Sync + 'static,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    subscribe!(block_header_update_rx);
    subscribe!(block_update_rx);
    subscribe!(log_update_rx);
    subscribe!(state_update_rx);

    let block_history_manager = BlockHistoryManager::new(client);

    loop {
        tokio::select! {
            msg = block_header_update_rx.recv() => {
                let block_update : Result<MessageBlockHeader, RecvError>  = msg;
                match block_update {
                    Ok(block_header)=>{

                        let next_base_fee = block_header.inner.next_block_base_fee;
                        let block_header = block_header.inner.header;
                        let block_hash : BlockHash = block_header.hash;
                        let block_number : BlockNumber = block_header.number;
                        let timestamp : u64 = block_header.timestamp;
                        let base_fee: u128 = block_header.base_fee_per_gas.unwrap_or_default();

                        debug!("Block Header Update {} {}", block_number, block_hash);

                        let mut block_history_guard = block_history.write().await;

                        match block_history_manager.set_chain_head(block_history_guard.borrow_mut(), block_header.clone()).await {
                            Ok(reorg_depth) => {
                                if reorg_depth > 0 {
                                    debug!("Re-org detected. Block {} Depth {} New hash {}", block_number, reorg_depth, block_hash);
                                }

                                latest_block.write().await.update(block_number, block_hash, Some(block_header.clone()), None, None, None );
                                if let Err(e) = market_events_tx.send(MarketEvents::BlockHeaderUpdate{
                                    block_number,
                                    block_hash,
                                    timestamp,
                                    base_fee,
                                    next_base_fee}).await
                                {
                                    error!("market_events_tx.send : {}", e);
                                }

                            }
                            Err(e)=>{
                                error!("block_history_manager.set_chain_head error at {} hash {} error : {} ", block_number, block_hash, e);
                            }
                        }
                        drop(block_history_guard);
                    }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }
            }

            msg = block_update_rx.recv() => {
                let block_update : Result<Block, RecvError>  = msg;
                match block_update {
                    Ok(block)=>{
                        let block_hash : BlockHash = block.header.hash;
                        let block_number : BlockNumber = block.header.number;
                        debug!("Block Update {} {}", block_number, block_hash);


                        match block_history.write().await.add_block(block.clone()) {
                            Ok(_) => {
                                latest_block.write().await.update(block_number, block_hash, None, Some(block.clone()), None, None );
                                if let Err(e) = market_events_tx.send(MarketEvents::BlockTxUpdate{ block_number, block_hash}).await {
                                    error!("market_events_tx.send : {}", e)
                                }
                            }
                            Err(e)=>{
                                error!("block_update add_block error at block {} with hash {} : {}", block_number, block_hash, e);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }
            }
            msg = log_update_rx.recv() => {
                let log_update : Result<BlockLogs, RecvError>  = msg;
                match log_update {
                    Ok(msg) =>{
                        let block_hash : BlockHash = msg.block_hash;
                        debug!("Log update {}", block_hash);


                        match block_history.write().await.add_logs(block_hash, msg.logs.clone()) {
                            Ok(_) => {
                                let (latest_number, latest_hash) = latest_block.read().await.number_and_hash();
                                if latest_hash == block_hash {
                                    latest_block.write().await.update(latest_number, block_hash, None, None, Some(msg.logs), None );
                                    market_events_tx.send(MarketEvents::BlockLogsUpdate{ block_number: latest_number,  block_hash } ).await.unwrap();
                                }
                            }
                            Err(e)=>{
                                error!("block_log_update add_block error {} {}", e, block_hash);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }

            }
            msg = state_update_rx.recv() => {
                // todo(Make getting market state from previous block)
                let state_update_msg : Result<BlockStateUpdate, RecvError> = msg;
                match state_update_msg {
                    Ok(msg) => {
                        let msg_block_hash : BlockHash = msg.block_hash;
                        debug!("Block State update {}", msg_block_hash);


                        let latest_block_guard = latest_block.read().await;
                        let (latest_block_number, latest_block_hash) = latest_block_guard.number_and_hash();
                        let latest_block_parent_hash = latest_block_guard.parent_hash().unwrap_or_default();
                        drop(latest_block_guard);

                        if latest_block_hash != msg_block_hash {
                            error!("State update for block that is not latest {} need {}", msg_block_hash, latest_block_hash);
                            if let Err(e) = block_history.write().await.add_state_diff(msg_block_hash, None, msg.state_update.clone()) {
                                error!("block_history.add_state_diff {}", e)
                            }

                        } else{
                            latest_block.write().await.update(latest_block_number, msg_block_hash, None, None, None, Some(msg.state_update.clone()) );

                            let market_state_guard= market_state.read().await;

                            let new_market_state_db = if market_state_guard.block_hash.is_zero() || market_state_guard.block_hash == latest_block_parent_hash {
                                let db = market_state.read().await.state_db.clone();
                                apply_state_update(db, msg.state_update.clone(), &market_state_guard)
                            }else{
                                let mut block_history = block_history.write().await;
                                block_history_manager.apply_state_update_on_parent_db(block_history.deref_mut(), &market_state_guard, msg.block_hash ).await?
                            };

                            drop(market_state_guard);


                            let add_state_diff_result= block_history.write().await.add_state_diff(msg_block_hash, Some(new_market_state_db.clone()), msg.state_update.clone());

                            match add_state_diff_result {
                                Ok(_) => {
                                    //todo : state diff latest block update
                                    let block_history = block_history.read().await;
                                    debug!("Block History len :{}", block_history.len());

                                    let mut market_state_guard= market_state.write().await;

                                    let accounts_len = market_state_guard.accounts_len();
                                    let accounts_db_len = market_state_guard.accounts_db_len();
                                    let storage_len = market_state_guard.storage_len();
                                    let storage_db_len = market_state_guard.storage_db_len();
                                    trace!("Market state len accounts {}/{} storage {}/{}  ", accounts_len, accounts_db_len, storage_len, storage_db_len);

                                    market_state_guard.state_db = new_market_state_db.clone();
                                    market_state_guard.block_hash = msg_block_hash;
                                    market_state_guard.block_number = latest_block_number;
                                    drop(market_state_guard);

                                    info!("market state updated ok records : update len: {} accounts: {} contracts: {}", msg.state_update.len(), new_market_state_db.accounts.len(),  new_market_state_db.contracts.len()  );

                                    market_events_tx.send(MarketEvents::BlockStateUpdate{ block_hash : msg_block_hash} ).await.unwrap();

                                    //Merging DB in background and update market state
                                    let market_state_clone= market_state.clone();

                                    tokio::task::spawn( async move{
                                        let mut market_state_guard = market_state_clone.write().await;
                                        market_state_guard.state_db = LoomInMemoryDB::new( Arc::new(new_market_state_db.merge()));
                                    });
                                }
                                Err(e)=>{
                                    error!("block_state_update add_block error {} {}", e, msg_block_hash);
                                }
                            }
                        }
                    }
                    Err(e)=>{
                        error!("block state update message error : {}", e);
                    }
                }
                debug!("Block State update finished");

            }
        }
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct BlockHistoryActor<P, T> {
    client: P,
    _t: PhantomData<T>,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory>>,
    #[consumer]
    block_header_update_rx: Option<Broadcaster<MessageBlockHeader>>,
    #[consumer]
    block_update_rx: Option<Broadcaster<Block>>,
    #[consumer]
    log_update_rx: Option<Broadcaster<BlockLogs>>,
    #[consumer]
    state_update_rx: Option<Broadcaster<BlockStateUpdate>>,
    #[producer]
    market_events_tx: Option<Broadcaster<MarketEvents>>,
}

impl<P, T> BlockHistoryActor<P, T>
where
    T: Transport + Sync + Send + Clone + 'static,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Sync + Send + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self {
            client,
            _t: PhantomData,
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

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            latest_block: Some(bc.latest_block()),
            market_state: Some(bc.market_state()),
            block_history: Some(bc.block_history()),
            block_header_update_rx: Some(bc.new_block_headers_channel()),
            block_update_rx: Some(bc.new_block_with_tx_channel()),
            log_update_rx: Some(bc.new_block_logs_channel()),
            state_update_rx: Some(bc.new_block_state_update_channel()),
            market_events_tx: Some(bc.market_events_channel()),
            ..self
        }
    }
}

impl<P, T> Actor for BlockHistoryActor<P, T>
where
    T: Transport + Sync + Send + Clone + 'static,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Sync + Send + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_block_history_worker(
            self.client.clone(),
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
