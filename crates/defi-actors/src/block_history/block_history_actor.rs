use std::sync::Arc;

use alloy_primitives::{Address, BlockHash, BlockNumber};
use alloy_rpc_types::Block;
use defi_blockchain::Blockchain;
use defi_entities::{BlockHistory, LatestBlock, MarketState};
use defi_events::{BlockLogs, BlockStateUpdate, MarketEvents, MessageBlockHeader};
use eyre::Result;
use log::{debug, error, info, trace};
use loom_actors::{subscribe, Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_revm_db::LoomInMemoryDB;
use tokio::sync::broadcast::error::RecvError;

#[allow(clippy::too_many_arguments)]
pub async fn new_block_history_worker(
    latest_block: SharedState<LatestBlock>,
    market_state: SharedState<MarketState>,
    block_history: SharedState<BlockHistory>,
    block_header_update_rx: Broadcaster<MessageBlockHeader>,
    block_update_rx: Broadcaster<Block>,
    log_update_rx: Broadcaster<BlockLogs>,
    state_update_rx: Broadcaster<BlockStateUpdate>,
    sender: Broadcaster<MarketEvents>,
) -> WorkerResult {
    subscribe!(block_header_update_rx);
    subscribe!(block_update_rx);
    subscribe!(log_update_rx);
    subscribe!(state_update_rx);

    loop {
        tokio::select! {
            msg = block_header_update_rx.recv() => {
                debug!("Block Header Update");
                let block_update : Result<MessageBlockHeader, RecvError>  = msg;
                match block_update {
                    Ok(block_header)=>{
                        let next_base_fee = block_header.inner.next_block_base_fee;
                        let block_header = block_header.inner.header;
                        let block_hash : BlockHash = block_header.hash;
                        let block_number : BlockNumber = block_header.number;
                        let timestamp : u64 = block_header.timestamp;
                        let base_fee: u128 = block_header.base_fee_per_gas.unwrap_or_default();

                        match block_history.write().await.add_block_header(block_header.clone()) {
                            Ok(_) => {
                                latest_block.write().await.update(block_number, block_hash, Some(block_header.clone()), None, None, None );
                                sender.send(MarketEvents::BlockHeaderUpdate{
                                    block_number,
                                    block_hash,
                                    timestamp,
                                    base_fee,
                                    next_base_fee}).await?;
                            }
                            Err(e)=>{
                                error!("block_header_update add_block error {} {} {} ", e, block_number, block_hash);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }
            }

            msg = block_update_rx.recv() => {
                debug!("Block Update");
                let block_update : Result<Block, RecvError>  = msg;
                match block_update {
                    Ok(block)=>{

                        let block_hash : BlockHash = block.header.hash;
                        let block_number : BlockNumber = block.header.number;


                        match block_history.write().await.add_block(block.clone()) {
                            Ok(_) => {
                                latest_block.write().await.update(block_number, block_hash, None, Some(block.clone()), None, None );
                                sender.send(MarketEvents::BlockTxUpdate{ block_number, block_hash}).await.unwrap();
                            }
                            Err(e)=>{
                                error!("block_update add_block error {} {} {} ", e, block_number, block_hash);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("block_update error {}", e)
                    }
                }
            }
            msg = log_update_rx.recv() => {
                debug!("Log update");

                let log_update : Result<BlockLogs, RecvError>  = msg;
                match log_update {
                    Ok(msg) =>{
                        let block_hash : BlockHash = msg.block_hash;
                        match block_history.write().await.add_logs(block_hash, msg.logs.clone()) {
                            Ok(_) => {
                                let (latest_number, latest_hash) = latest_block.read().await.number_and_hash();
                                if latest_hash == block_hash {
                                    latest_block.write().await.update(latest_number, block_hash, None, None, Some(msg.logs), None );
                                    sender.send(MarketEvents::BlockLogsUpdate{ block_number: latest_number,  block_hash } ).await.unwrap();
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
                debug!("Block State update");
                let state_update_msg : Result<BlockStateUpdate, RecvError> = msg;
                match state_update_msg {
                    Ok(msg) => {
                        let block_hash : BlockHash = msg.block_hash;
                        let (latest_number, _) = latest_block.read().await.number_and_hash();

                        latest_block.write().await.update(latest_number, block_hash, None, None, None, Some(msg.state_update.clone()) );

                        let new_market_state_db = market_state.read().await.state_db.clone();

                        let add_state_diff_result= block_history.write().await.add_state_diff(block_hash, new_market_state_db, msg.state_update.clone());

                        match add_state_diff_result {
                            Ok(_) => {
                                //todo : state diff latest block update
                                //latest_block.write().await.update(block_number, None, None, logs.clone(), None );
                                let block_history_len = block_history.read().await.len();
                                debug!("Block History len :{}", block_history_len);

                                let mut new_market_state_db = market_state.read().await.state_db.clone();
                                {
                                    let market_state_read_guard= market_state.read().await;
                                    let accounts_len = market_state_read_guard.accounts_len();
                                    let accounts_db_len = market_state_read_guard.accounts_db_len();
                                    let storage_len = market_state_read_guard.storage_len();
                                    let storage_db_len = market_state_read_guard.storage_db_len();
                                    trace!("Market state len accounts {}/{} storage {}/{}  ", accounts_len, accounts_db_len, storage_len, storage_db_len);
                                }


                                //new_market_state_db.apply_geth_update_vec(msg.state_update.clone());
                                //let merged_db = new_market_state_db.update_cells();
                                //new_market_state_db = LoomInMemoryDB::new(Arc::new(merged_db));


                                for state_diff in msg.state_update.iter(){
                                    for (address, account_state) in state_diff.iter() {
                                        let address : Address = *address;
                                        if let Some(balance) = account_state.balance {
                                            if market_state.read().await.is_account(&address)  {
                                                match new_market_state_db.load_account(address) {
                                                    Ok(x) => {
                                                        x.info.balance = balance;
                                                        //trace!("Balance updated {:#20x} {}", address, balance );
                                                    }
                                                    _=>{
                                                        trace!("Balance updated for {:#20x} not found", address );
                                                    }
                                                };
                                            }
                                        }

                                        if let Some(nonce) = account_state.nonce {
                                            if market_state.read().await.is_account(&address)  {
                                                match new_market_state_db.load_account(address) {
                                                    Ok(x) => {
                                                        x.info.nonce = nonce;
                                                        trace!("Nonce updated {:#20x} {}", address, nonce );
                                                    }
                                                    _=>{
                                                        trace!("Nonce updated for {:#20x} not found", address );
                                                    }
                                                };
                                            }
                                        }

                                        for (slot, value) in account_state.storage.iter() {
                                            if market_state.read().await.is_force_insert(&address ) {
                                                trace!("Force slot updated {:#20x} {} {}", address, slot, value);
                                                if let Err(e) = new_market_state_db.insert_account_storage(address, (*slot).into(), (*value).into()) {
                                                    error!("{}", e)
                                                }
                                            }else if market_state.read().await.is_slot(&address, &(*slot).into() ) {
                                                trace!("Slot updated {:#20x} {} {}", address, slot, value);
                                                if let Err(e) = new_market_state_db.insert_account_storage(address, (*slot).into(), (*value).into()) {
                                                    error!("{}", e)
                                                }
                                            }
                                        }
                                    }
                                }

                                let market_state_clone= market_state.clone();
                                info!("market state updated ok records : update len: {} accounts: {} contracts: {}", msg.state_update.len(), new_market_state_db.accounts.len(),  new_market_state_db.contracts.len()  );
                                market_state.write().await.state_db = new_market_state_db.clone();

                                sender.send(MarketEvents::BlockStateUpdate{ block_hash} ).await.unwrap();

                                // TODO : Fix
                                //Merging DB in background and update market state
                                tokio::task::spawn( async move{
                                    let merged_db = LoomInMemoryDB::new( Arc::new(new_market_state_db.merge()));
                                    market_state_clone.write().await.state_db = merged_db;
                                });

                            }
                            Err(e)=>{
                                error!("block_state_update add_block error {} {}", e, block_hash);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("block state update message error : {}", e);
                    }
                }
            }
        }
    }
}

#[derive(Accessor, Consumer, Producer, Default)]
pub struct BlockHistoryActor {
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

impl BlockHistoryActor {
    pub fn new() -> Self {
        Self::default()
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
        }
    }
}

impl Actor for BlockHistoryActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::Builder::new().name(self.name()).spawn(new_block_history_worker(
            self.latest_block.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.block_history.clone().unwrap(),
            self.block_header_update_rx.clone().unwrap(),
            self.block_update_rx.clone().unwrap(),
            self.log_update_rx.clone().unwrap(),
            self.state_update_rx.clone().unwrap(),
            self.market_events_tx.clone().unwrap(),
        ))?;
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "BlockHistoryActor"
    }
}
