use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use alloy_primitives::BlockHash;
use alloy_provider::Provider;
use alloy_rpc_types::{Block, Header, Log};
use eyre::{ErrReport, OptionExt, Result};
use revm::InMemoryDB;
use tokio::sync::RwLock;

use defi_types::GethStateUpdateVec;

use crate::MarketState;

#[derive(Clone, Debug, Default)]
pub struct BlockHistoryEntry
{
    pub header: Option<Header>,
    pub block: Option<Block>,
    pub logs: Option<Vec<Log>>,
    pub state_update: Option<GethStateUpdateVec>,
    pub state_db: Option<InMemoryDB>,
}

impl BlockHistoryEntry
{
    fn new(header: Option<Header>,
           block: Option<Block>,
           logs: Option<Vec<Log>>,
           state_update: Option<GethStateUpdateVec>,
           state_db: Option<InMemoryDB>) -> BlockHistoryEntry {
        BlockHistoryEntry {
            header,
            block,
            logs,
            state_update,
            state_db,
        }
    }
}


#[derive(Debug, Clone)]
pub struct BlockHistory
{
    depth: usize,
    pub latest_block_number: u64,
    block_entries: HashMap<BlockHash, BlockHistoryEntry>,
    block_numbers: HashMap<u64, BlockHash>,
}


impl BlockHistory
{
    pub fn new(depth: usize) -> BlockHistory {
        BlockHistory {
            depth,
            latest_block_number: 0,
            block_entries: HashMap::new(),
            block_numbers: HashMap::new(),
        }
    }


    pub async fn fetch<P>(client: P, current_state: Arc<RwLock<MarketState>>, depth: usize) -> Result<BlockHistory>
        where P: Provider + 'static
    {

        //let market_guard = current_market.read().await;

        let latest_block_number = client.get_block_number().await?;

        let block = client.get_block_by_number(latest_block_number.into(), true).await?.unwrap();

        let block_hash = block.header.hash.ok_or_eyre("NO_BLOCK_HASH")?;

        let market_state_guard = current_state.read().await;

        let block_entry = BlockHistoryEntry::new(None, None, None, None, Some(market_state_guard.state_db.clone()));

        let mut block_entries: HashMap<BlockHash, BlockHistoryEntry> = HashMap::new();
        block_entries.insert(block_hash, block_entry);

        let mut block_numbers: HashMap<u64, BlockHash> = HashMap::new();
        block_numbers.insert(latest_block_number, block_hash);

        Ok(BlockHistory {
            depth,
            latest_block_number,
            block_entries,
            block_numbers,
        })
    }

    pub fn len(&self) -> usize {
        self.block_entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.block_entries.is_empty()
    }

    fn process_block_number_with_hash(&mut self, block_number: u64, block_hash: BlockHash) -> &mut BlockHistoryEntry {
        //todo: process reorg
        if self.latest_block_number < block_number {
            self.latest_block_number = block_number;
            self.block_numbers.insert(block_number, block_hash);
        }
        self.block_numbers.retain(|&key, _| key > (block_number - self.depth as u64));
        let actual_hashes: Vec<BlockHash> = self.block_numbers.values().cloned().collect();
        self.block_entries.retain(|key, _| actual_hashes.contains(key));

        self.block_entries.entry(block_hash).or_default()
    }

    fn process_block_hash(&mut self, block_hash: BlockHash) -> &mut BlockHistoryEntry {
        self.block_entries.entry(block_hash).or_default()
    }

    pub fn add_block_header(&mut self, block_header: Header) -> Result<()> {
        let block_hash = block_header.hash.unwrap();
        let block_number = block_header.number.unwrap();

        let market_history_entry = self.process_block_number_with_hash(block_number, block_hash);

        if market_history_entry.block.is_some() {
            return Err(ErrReport::msg("BLOCK_IS_ALREADY_PROCESSED"));
        }

        market_history_entry.header = Some(block_header);

        if block_number == self.latest_block_number + 1 {
            self.latest_block_number = block_number;
        }

        Ok(())
    }

    pub fn add_block(&mut self, block: Block) -> Result<()> {
        let block_hash = block.header.hash.unwrap();
        let block_number = block.header.number.unwrap();

        let market_history_entry = self.process_block_number_with_hash(block_number, block_hash);

        if market_history_entry.block.is_some() {
            return Err(ErrReport::msg("BLOCK_IS_ALREADY_PROCESSED"));
        }

        market_history_entry.block = Some(block);

        if block_number == self.latest_block_number + 1 {
            self.latest_block_number = block_number;
        }

        Ok(())
    }

    pub fn add_state_diff(&mut self, block_hash: BlockHash, state_db: InMemoryDB, state_diff: GethStateUpdateVec) -> Result<()> {
        let market_history_entry = self.process_block_hash(block_hash);

        if market_history_entry.state_db.is_none() {
            market_history_entry.state_db = Some(state_db);
            market_history_entry.state_update = Some(state_diff);

            Ok(())
        } else {
            Err(ErrReport::msg("BLOCK_STATE_IS_ALREADY_PROCESSED"))
        }
    }

    pub fn add_logs(&mut self, block_hash: BlockHash, logs: Vec<Log>) -> Result<()> {
        let market_history_entry = self.process_block_hash(block_hash);

        if market_history_entry.logs.is_none() {
            market_history_entry.logs = Some(logs);
            Ok(())
        } else {
            Err(ErrReport::msg("BLOCK_LOGS_ARE_ALREADY_PROCESSED"))
        }
    }


    pub fn get_market_history_entry(&self, block_hash: &BlockHash) -> Option<&BlockHistoryEntry> {
        self.block_entries.get(block_hash)
    }

    pub fn get_block_by_hash(&self, block_hash: &BlockHash) -> Option<Block> {
        self.block_entries.get(block_hash).and_then(|entry| entry.block.clone())
    }
}