use std::sync::Arc;

use alloy_primitives::{Address, BlockHash, BlockNumber};
use alloy_rpc_types::{Block, Header};
use eyre::Result;
use log::{debug, error, info, trace};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_blockchain::Blockchain;
use defi_entities::{BlockHistory, LatestBlock, MarketState};
use defi_events::{BlockLogs, BlockStateUpdate, MarketEvents};
use defi_types::ChainParameters;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_revm_db::LoomInMemoryDB;

#[allow(clippy::too_many_arguments)]
pub async fn new_block_history_worker(
    chain_parameters: ChainParameters,
    latest_block: SharedState<LatestBlock>,
    market_state: SharedState<MarketState>,
    block_history: SharedState<BlockHistory>,
    block_header_update_rx: Broadcaster<Header>,
    block_update_rx: Broadcaster<Block>,
    log_update_rx: Broadcaster<BlockLogs>,
    state_update_rx: Broadcaster<BlockStateUpdate>,
    sender: Broadcaster<MarketEvents>,
) -> WorkerResult {
    let mut block_header_update_rx: Receiver<Header> = block_header_update_rx.subscribe().await;
    let mut block_update_rx: Receiver<Block> = block_update_rx.subscribe().await;
    let mut log_update_rx: Receiver<BlockLogs> = log_update_rx.subscribe().await;
    let mut state_update_rx: Receiver<BlockStateUpdate> = state_update_rx.subscribe().await;

    loop {
        tokio::select! {
            msg = block_header_update_rx.recv() => {
                debug!("Block Header Update");
                let block_update : Result<Header, RecvError>  = msg;
                match block_update {
                    Ok(block_header)=>{
                        let block_hash : BlockHash = block_header.hash.unwrap_or_default();
                        let block_number : BlockNumber = block_header.number.unwrap_or_default();
                        let timestamp : u64 = block_header.timestamp;
                        let base_fee: u128 = block_header.base_fee_per_gas.unwrap_or_default();


                        let next_base_fee : u128 = chain_parameters.calc_next_block_base_fee(block_header.gas_used, block_header.gas_limit, block_header.base_fee_per_gas.unwrap_or_default());

                        match block_history.write().await.add_block_header(block_header.clone()) {
                            Ok(_) => {
                                latest_block.write().await.update(block_number, block_hash, Some(block_header.clone()), None, None, None );
                                sender.send(MarketEvents::BlockHeaderUpdate{
                                    block_number,
                                    block_hash,
                                    timestamp,
                                    base_fee,
                                    next_base_fee}).await.unwrap();
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

                        let block_hash : BlockHash = block.header.hash.unwrap_or_default();
                        let block_number : BlockNumber = block.header.number.unwrap_or_default();


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

#[derive(Accessor, Consumer, Producer)]
pub struct BlockHistoryActor {
    chain_parameters: ChainParameters,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[accessor]
    block_history: Option<SharedState<BlockHistory>>,
    #[consumer]
    block_header_update_rx: Option<Broadcaster<Header>>,
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
        Self { chain_parameters: ChainParameters::ethereum(), ..Self::default() }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            chain_parameters: bc.chain_parameters(),
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

impl Default for BlockHistoryActor {
    fn default() -> Self {
        BlockHistoryActor {
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
}

impl Actor for BlockHistoryActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(new_block_history_worker(
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
