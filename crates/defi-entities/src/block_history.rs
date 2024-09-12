use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use alloy_network::{BlockResponse, Ethereum, Network};
use alloy_primitives::{BlockHash, BlockNumber};
use alloy_provider::Provider;
use alloy_rpc_types::{Block, BlockId, BlockTransactionsKind, Filter, Header, Log};
use alloy_transport::Transport;
use debug_provider::DebugProviderExt;
use defi_types::{debug_trace_block, GethStateUpdateVec};
use eyre::{eyre, ErrReport, Result};
use log::warn;
use loom_revm_db::LoomInMemoryDB;
use tokio::sync::RwLock;

use crate::MarketState;

#[derive(Clone, Debug, Default)]
pub struct BlockHistoryEntry {
    pub header: Option<Header>,
    pub block: Option<Block>,
    pub logs: Option<Vec<Log>>,
    pub state_update: Option<GethStateUpdateVec>,
    pub state_db: Option<LoomInMemoryDB>,
}

impl BlockHistoryEntry {
    pub fn new(
        header: Option<Header>,
        block: Option<Block>,
        logs: Option<Vec<Log>>,
        state_update: Option<GethStateUpdateVec>,
        state_db: Option<LoomInMemoryDB>,
    ) -> BlockHistoryEntry {
        BlockHistoryEntry { header, block, logs, state_update, state_db }
    }

    pub fn is_complete(&self) -> bool {
        self.state_db.is_some() && self.is_fetched()
    }

    pub fn is_fetched(&self) -> bool {
        self.state_update.is_some() && self.header.is_some() && self.block.is_some() && self.logs.is_some()
    }

    pub async fn fetch<P, T>(&mut self, client: P) -> Result<()>
    where
        T: Transport + Clone,
        P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        if let Some(header) = &self.header {
            if self.logs.is_none() {
                let filter = Filter::new().at_block_hash(header.hash);
                let logs = client.get_logs(&filter).await?;
                self.logs = Some(logs);
            }

            if self.block.is_none() {
                let block = client.get_block_by_hash(header.hash, BlockTransactionsKind::Full).await?;
                if let Some(block) = block {
                    self.block = Some(block);
                }
            }

            if self.state_update.is_none() {
                let (_, state_update) = debug_trace_block(client.clone(), BlockId::Hash(header.hash.into()), true).await?;
                self.state_update = Some(state_update);
            }
        }
        if self.is_fetched() {
            Ok(())
        } else {
            Err(eyre!("BLOCK_DATA_NOT_FETCHED"))
        }
    }

    pub async fn add_parent_entry(&mut self, parent: &BlockHistoryEntry) {
        if let Some(mut parent_db) = parent.state_db.clone() {
            if let Some(parent_state_update) = parent.state_update.clone() {
                // Update only current cells
                parent_db.apply_geth_update_vec(parent_state_update);
                self.state_db = Some(parent_db);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlockHistory {
    depth: usize,
    pub latest_block_number: u64,
    block_entries: HashMap<BlockHash, BlockHistoryEntry>,
    block_numbers: HashMap<u64, BlockHash>,
}

impl BlockHistory {
    pub fn new(depth: usize) -> BlockHistory {
        BlockHistory { depth, latest_block_number: 0, block_entries: HashMap::new(), block_numbers: HashMap::new() }
    }

    pub async fn init<P, T, N>(client: P, current_state: Arc<RwLock<MarketState>>, depth: usize) -> Result<BlockHistory>
    where
        N: Network,
        T: Transport + Clone,
        P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        let latest_block_number = client.get_block_number().await?;

        let block = client.get_block_by_number(latest_block_number.into(), true).await?;
        if let Some(block) = block {
            let market_state_guard = current_state.read().await;

            let block_entry = BlockHistoryEntry::new(None, None, None, None, Some(market_state_guard.state_db.clone()));

            let mut block_entries: HashMap<BlockHash, BlockHistoryEntry> = HashMap::new();
            block_entries.insert(block.header.hash, block_entry);

            let mut block_numbers: HashMap<u64, BlockHash> = HashMap::new();
            block_numbers.insert(latest_block_number, block.header.hash);

            Ok(BlockHistory { depth, latest_block_number, block_entries, block_numbers })
        } else {
            Err(eyre!("BLOCK_IS_EMPTY"))
        }
    }

    pub async fn fetch_entry<P, T>(client: P, block_hash: BlockHash) -> Result<BlockHistoryEntry>
    where
        T: Transport + Clone,
        P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        let block = client.get_block_by_hash(block_hash, BlockTransactionsKind::Full).await?;
        if let Some(block) = block {
            let header = block.header().clone();

            let filter = Filter::new().at_block_hash(block_hash);

            let logs = client.get_logs(&filter).await?;

            let (_, state_update) = debug_trace_block(client.clone(), BlockId::Hash(block_hash.into()), true).await?;

            let block_entry = BlockHistoryEntry::new(Some(header), Some(block), Some(logs), Some(state_update), None);
            Ok(block_entry)
        } else {
            Err(eyre!("BLOCK_IS_EMPTY"))
        }
    }

    pub async fn add_entry_by_hash<P, T>(&mut self, client: P, block_hash: BlockHash) -> Result<()>
    where
        T: Transport + Clone,
        P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        let entry = match self.get_entry(&block_hash) {
            Some(entry) => entry,
            None => Self::fetch_entry(client, block_hash).await?,
        };
        let parent_entry = self.get_or_fetch_entry();
        Ok(())
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

        if block_number > self.depth as u64 {
            self.block_numbers.retain(|&key, _| key > (block_number - self.depth as u64));
            let actual_hashes: Vec<BlockHash> = self.block_numbers.values().cloned().collect();
            self.block_entries.retain(|key, _| actual_hashes.contains(key));
        }

        self.block_entries.entry(block_hash).or_default()
    }

    fn get_or_insert_entry(&mut self, block_hash: BlockHash) -> &mut BlockHistoryEntry {
        self.block_entries.entry(block_hash).or_default()
    }

    fn check_reorg_at_block(&mut self, block_number: BlockNumber) -> Option<BlockHash> {
        if let Some(block_hash) = self.get_block_hash_for_block_number(block_number) {
            let entry = self.get_or_insert_entry();

            None
        } else {
            None
        }
    }

    fn process_reorg(&mut self) -> Option<BlockNumber> {
        let current_block_number = self.latest_block_number;
        let current_block_hash = self.block_numbers.get(&current_block_number);
        let prev_block_hash = self.block_numbers.get(&(current_block_number - 1)).unwrap_or_default().clone();

        if let Some(current_block_hash) = current_block_hash {
            if let Some(current_block_entry) = self.block_entries.get(current_block_hash) {
                if let Some(current_block_header) = current_block_entry.header.clone() {
                    if current_block_header.parent_hash != prev_block_hash {
                        // reorg detected
                    }
                }
            }
        }
        None
    }

    pub fn add_block_header(&mut self, block_header: Header) -> Result<()> {
        let block_hash = block_header.hash;
        let block_number = block_header.number;

        let market_history_entry = self.process_block_number_with_hash(block_number, block_hash);

        if market_history_entry.header.is_some() {
            warn!(
                "Block is already processed header: {} block : {} state_update : {} logs : {}",
                market_history_entry.header.is_some(),
                market_history_entry.block.is_some(),
                market_history_entry.state_update.is_some(),
                market_history_entry.logs.is_some(),
            );
            return Err(ErrReport::msg("BLOCK_IS_ALREADY_PROCESSED"));
        }

        market_history_entry.header = Some(block_header);

        if block_number == self.latest_block_number + 1 {
            self.latest_block_number = block_number;
        }

        Ok(())
    }

    pub fn add_block(&mut self, block: Block) -> Result<()> {
        let block_hash = block.header.hash;
        let block_number = block.header.number;

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

    pub fn add_state_diff(&mut self, block_hash: BlockHash, state_db: LoomInMemoryDB, state_diff: GethStateUpdateVec) -> Result<()> {
        let market_history_entry = self.get_or_insert_entry(block_hash);

        if market_history_entry.state_db.is_none() {
            market_history_entry.state_db = Some(state_db);
            market_history_entry.state_update = Some(state_diff);

            Ok(())
        } else {
            Err(ErrReport::msg("BLOCK_STATE_IS_ALREADY_PROCESSED"))
        }
    }

    pub fn add_logs(&mut self, block_hash: BlockHash, logs: Vec<Log>) -> Result<()> {
        let market_history_entry = self.get_or_insert_entry(block_hash);

        if market_history_entry.logs.is_none() {
            market_history_entry.logs = Some(logs);
            Ok(())
        } else {
            Err(ErrReport::msg("BLOCK_LOGS_ARE_ALREADY_PROCESSED"))
        }
    }

    pub fn get_or_fetch_entry<P, T>(&mut self, client: P, block_hash: BlockHash) -> Result<&BlockHistoryEntry>
    where
        T: Transport + Clone,
        P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        if let Some(entry) = self.get_entry(&block_hash) {
            Ok(entry)
        } else {
            Ok(Self::fetch_entry(client, block_hash)?)
        }
    }

    pub fn get_entry(&self, block_hash: &BlockHash) -> Option<&BlockHistoryEntry> {
        self.block_entries.get(block_hash)
    }

    pub fn get_block_by_hash(&self, block_hash: &BlockHash) -> Option<Block> {
        self.block_entries.get(block_hash).and_then(|entry| entry.block.clone())
    }

    pub fn get_block_hash_for_block_number(&self, block_number: BlockNumber) -> Option<BlockHash> {
        *self.block_numbers.get(&block_number)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy_primitives::U256;

    fn create_next_header(parent: &Header, child_id: u64) -> Header {
        let number = parent.number + 1;
        let hash: BlockHash = (U256::try_from(parent.hash).unwrap() * U256::from(256) + U256::from(number + child_id)).try_into().unwrap();
        let number = parent.number + 1;

        Header { hash, parent_hash: parent.hash, number, ..Default::default() }
    }

    #[test]
    fn test_add_block_header() {
        let mut block_history = BlockHistory::new(10);

        let header_1_0 = Header { number: 1, hash: U256::from(1).into(), ..Default::default() };
        let header_2_0 = create_next_header(&header_1_0, 0);
        let header_3_0 = create_next_header(&header_2_0, 0);

        block_history.add_block_header(header_1_0).unwrap();
        block_history.add_block_header(header_2_0).unwrap();
        block_history.add_block_header(header_3_0).unwrap();

        assert_eq!(block_history.block_entries.len(), 3);
        assert_eq!(block_history.latest_block_number, 3);
        assert_eq!(block_history.block_numbers[&3], BlockHash::from(U256::from(0x010203)));
    }

    #[test]
    fn test_add_missed_header() {
        let mut block_history = BlockHistory::new(10);

        let header_1_0 = Header { number: 1, hash: U256::from(1).into(), ..Default::default() };
        let header_2_0 = create_next_header(&header_1_0, 0);
        let header_2_1 = create_next_header(&header_1_0, 1);
        let header_3_0 = create_next_header(&header_2_0, 0);

        block_history.add_block_header(header_1_0).unwrap();
        block_history.add_block_header(header_2_0).unwrap();
        block_history.add_block_header(header_3_0).unwrap();
        block_history.add_block_header(header_2_1).unwrap();

        assert_eq!(block_history.block_entries.len(), 4);
        assert_eq!(block_history.latest_block_number, 3);
        assert_eq!(block_history.block_numbers[&3], BlockHash::from(U256::from(0x010203)));
    }

    #[test]
    fn test_add_reorged_header() {
        let mut block_history = BlockHistory::new(10);

        let header_1_0 = Header { number: 1, hash: U256::from(1).into(), ..Default::default() };
        let header_2_0 = create_next_header(&header_1_0, 0);
        let header_2_1 = create_next_header(&header_1_0, 1);
        let header_3_0 = create_next_header(&header_2_0, 0);
        let header_3_1 = create_next_header(&header_2_1, 0);
        let header_4_1 = create_next_header(&header_3_1, 0);

        block_history.add_block_header(header_1_0).unwrap();
        block_history.add_block_header(header_2_0).unwrap();
        block_history.add_block_header(header_3_0).unwrap();
        block_history.add_block_header(header_2_1).unwrap();
        //block_history.add_block_header(header_3_1).unwrap();
        block_history.add_block_header(header_4_1).unwrap();

        assert_eq!(block_history.block_entries.len(), 5);
        assert_eq!(block_history.latest_block_number, 4);
        assert_eq!(block_history.block_numbers[&3], BlockHash::from(U256::from(0x010304)));
        assert_eq!(block_history.block_numbers[&4], BlockHash::from(U256::from(0x01030404)));
    }
}
