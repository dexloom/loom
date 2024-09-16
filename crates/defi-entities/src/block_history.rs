use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::{BlockResponse, Ethereum};
use alloy_primitives::{Address, BlockHash, BlockNumber};
use alloy_provider::Provider;
use alloy_rpc_types::{Block, BlockId, BlockTransactionsKind, Filter, Header, Log};
use alloy_transport::Transport;
use debug_provider::DebugProviderExt;
use defi_types::{debug_trace_block, GethStateUpdateVec};
use eyre::{eyre, ErrReport, OptionExt, Result};
use log::{error, trace, warn};
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

    pub fn hash(&self) -> Option<BlockHash> {
        self.header.as_ref().map(|h| h.hash)
    }

    pub fn parent_hash(&self) -> Option<BlockHash> {
        self.header.as_ref().map(|h| h.parent_hash)
    }

    pub fn number(&self) -> Option<BlockNumber> {
        self.header.as_ref().map(|h| h.number)
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

    fn get_or_insert_entry_mut(&mut self, block_hash: BlockHash) -> &mut BlockHistoryEntry {
        self.block_entries.entry(block_hash).or_default()
    }

    fn set_entry(&mut self, entry: BlockHistoryEntry) {
        if let Some(block_hash) = entry.hash() {
            if let Some(block_number) = entry.number() {
                self.block_numbers.insert(block_number, block_hash);
                self.block_entries.insert(block_hash, entry);
            }
        }
    }

    fn check_reorg_at_block(&mut self, block_number: BlockNumber) -> Option<BlockHash> {
        if let Some(block_hash) = self.get_block_hash_for_block_number(block_number) {
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

        if market_history_entry.header.is_none() {
            market_history_entry.header = Some(block.header.clone());
        }

        market_history_entry.block = Some(block);

        if block_number == self.latest_block_number + 1 {
            self.latest_block_number = block_number;
        }

        Ok(())
    }

    pub fn add_state_diff(&mut self, block_hash: BlockHash, state_db: LoomInMemoryDB, state_diff: GethStateUpdateVec) -> Result<()> {
        let market_history_entry = self.get_or_insert_entry_mut(block_hash);

        if market_history_entry.state_db.is_none() {
            market_history_entry.state_db = Some(state_db);
            market_history_entry.state_update = Some(state_diff);

            Ok(())
        } else {
            Err(ErrReport::msg("BLOCK_STATE_IS_ALREADY_PROCESSED"))
        }
    }

    pub fn add_logs(&mut self, block_hash: BlockHash, logs: Vec<Log>) -> Result<()> {
        let market_history_entry = self.get_or_insert_entry_mut(block_hash);

        if market_history_entry.logs.is_none() {
            market_history_entry.logs = Some(logs);
            Ok(())
        } else {
            Err(ErrReport::msg("BLOCK_LOGS_ARE_ALREADY_PROCESSED"))
        }
    }

    pub fn get_entry(&self, block_hash: &BlockHash) -> Option<&BlockHistoryEntry> {
        self.block_entries.get(block_hash)
    }

    pub fn get_block_by_hash(&self, block_hash: &BlockHash) -> Option<Block> {
        self.block_entries.get(block_hash).and_then(|entry| entry.block.clone())
    }

    pub fn get_block_hash_for_block_number(&self, block_number: BlockNumber) -> Option<BlockHash> {
        self.block_numbers.get(&block_number).cloned()
    }
}

pub struct BlockHistoryManager<P, T> {
    client: P,
    _t: PhantomData<T>,
}

impl<P, T> BlockHistoryManager<P, T>
where
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
    T: Transport + Clone + Send + Sync + 'static,
{
    pub async fn init(&self, current_state: Arc<RwLock<MarketState>>, depth: usize) -> Result<BlockHistory>
    where
        T: Transport + Clone,
        P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        let latest_block_number = self.client.get_block_number().await?;

        let block = self.client.get_block_by_number(latest_block_number.into(), true).await?;
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

    pub fn new(client: P) -> Self {
        Self { client, _t: PhantomData }
    }

    pub async fn fetch_entry_by_hash(&self, block_history: &mut BlockHistory, block_hash: BlockHash) -> Result<BlockHistoryEntry> {
        let block = self.client.get_block_by_hash(block_hash, BlockTransactionsKind::Full).await?;
        if let Some(block) = block {
            let header = block.header().clone();

            let filter = Filter::new().at_block_hash(block_hash);

            let logs = self.client.get_logs(&filter).await?;

            let (_, state_update) = debug_trace_block(self.client.clone(), BlockId::Hash(block_hash.into()), true).await?;

            let block_entry = BlockHistoryEntry::new(Some(header), Some(block), Some(logs), Some(state_update), None);
            Ok(block_entry)
        } else {
            Err(eyre!("BLOCK_IS_EMPTY"))
        }
    }

    pub async fn add_entry_by_hash(&self, block_history: &mut BlockHistory, block_hash: BlockHash) -> Result<()>
    where
        T: Transport + Clone,
        P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        let entry = match block_history.get_entry(&block_hash) {
            Some(entry) => entry,
            None => &self.fetch_entry_by_hash(block_history, block_hash).await?,
        };
        //let parent_entry = self.get_or_fetch_entry();
        Ok(())
    }

    pub async fn fetch_entry_data(&self, entry: &mut BlockHistoryEntry) -> Result<()>
    where
        T: Transport + Clone,
        P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        if let Some(header) = &entry.header {
            if entry.logs.is_none() {
                let filter = Filter::new().at_block_hash(header.hash);
                let logs = self.client.get_logs(&filter).await?;
                entry.logs = Some(logs);
            }

            if entry.block.is_none() {
                let block = self.client.get_block_by_hash(header.hash, BlockTransactionsKind::Full).await?;
                if let Some(block) = block {
                    entry.block = Some(block);
                }
            }

            if entry.state_update.is_none() {
                if let Ok((_, state_update)) = debug_trace_block(self.client.clone(), BlockId::Hash(header.hash.into()), true).await {
                    entry.state_update = Some(state_update);
                } else {
                    error!("ERROR_FETCHING_STATE_UPDATE")
                }
            }
        }
        if entry.is_fetched() {
            Ok(())
        } else {
            Err(eyre!("BLOCK_DATA_NOT_FETCHED"))
        }
    }
    pub async fn get_or_fetch_entry(&self, block_history: &mut BlockHistory, block_hash: BlockHash) -> Result<BlockHistoryEntry> {
        if let Some(entry) = block_history.get_entry(&block_hash) {
            Ok(entry.clone())
        } else {
            Ok(self.fetch_entry_by_hash(block_history, block_hash).await?)
        }
    }

    pub async fn get_or_fetch_parent_db(
        &self,
        block_history: &mut BlockHistory,
        market_state: &MarketState,
        parent_hash: BlockHash,
    ) -> Result<LoomInMemoryDB> {
        let mut parent_entry = self.get_or_fetch_entry(block_history, parent_hash).await?;

        match &parent_entry.state_db {
            Some(db) => Ok(db.clone()),
            None => {
                let db = self
                    .get_or_fetch_parent_db(block_history, market_state, parent_entry.hash().ok_or_eyre("NO_PARENT_BLOCK_HASH")?)
                    .await?;
                let db = Self::apply_state_update(db, parent_entry.state_update.clone().unwrap_or_default(), market_state);
                parent_entry.state_db = Some(db.clone());
                block_history.set_entry(parent_entry);
                Ok(db)
            }
        }
    }

    pub fn apply_state_update(db: LoomInMemoryDB, state_update: GethStateUpdateVec, market_state: &MarketState) -> LoomInMemoryDB {
        let mut db = db;
        for state_diff in state_update.into_iter() {
            for (address, account_state) in state_diff.into_iter() {
                let address: Address = address;
                if let Some(balance) = account_state.balance {
                    if market_state.is_account(&address) {
                        match db.load_account(address) {
                            Ok(x) => {
                                x.info.balance = balance;
                                //trace!("Balance updated {:#20x} {}", address, balance );
                            }
                            _ => {
                                trace!("Balance updated for {:#20x} not found", address);
                            }
                        };
                    }
                }

                if let Some(nonce) = account_state.nonce {
                    if market_state.is_account(&address) {
                        match db.load_account(address) {
                            Ok(x) => {
                                x.info.nonce = nonce;
                                trace!("Nonce updated {:#20x} {}", address, nonce);
                            }
                            _ => {
                                trace!("Nonce updated for {:#20x} not found", address);
                            }
                        };
                    }
                }

                for (slot, value) in account_state.storage.iter() {
                    if market_state.is_force_insert(&address) {
                        trace!("Force slot updated {:#20x} {} {}", address, slot, value);
                        if let Err(e) = db.insert_account_storage(address, (*slot).into(), (*value).into()) {
                            error!("{}", e)
                        }
                    } else if market_state.is_slot(&address, &(*slot).into()) {
                        trace!("Slot updated {:#20x} {} {}", address, slot, value);
                        if let Err(e) = db.insert_account_storage(address, (*slot).into(), (*value).into()) {
                            error!("{}", e)
                        }
                    }
                }
            }
        }
        db
    }

    pub async fn apply_state_update_on_parent_db(
        &self,
        block_history: &mut BlockHistory,
        market_state: &MarketState,
        block_hash: BlockHash,
    ) -> Result<LoomInMemoryDB> {
        let mut entry = block_history.get_or_insert_entry_mut(block_hash);
        if !entry.is_fetched() {
            self.fetch_entry_data(&mut entry).await?;
        }

        let entry = entry.clone();

        let parent_db = self.get_or_fetch_parent_db(block_history, market_state, entry.parent_hash().ok_or_eyre("NO_PARENT_HASH")?).await?;

        let db = Self::apply_state_update(parent_db, entry.state_update.clone().unwrap_or_default(), market_state);

        Ok(db)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy_node_bindings::Anvil;
    use alloy_primitives::{Address, B256, U256};
    use alloy_provider::ext::AnvilApi;
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::ClientBuilder;
    use alloy_rpc_types::BlockNumberOrTag;
    use alloy_rpc_types_trace::geth::AccountState;
    use debug_provider::AnvilProviderExt;
    use defi_types::GethStateUpdate;

    fn create_next_header(parent: &Header, child_id: u64) -> Header {
        let number = parent.number + 1;
        let hash: BlockHash = (U256::try_from(parent.hash).unwrap() * U256::from(256) + U256::from(number + child_id)).try_into().unwrap();
        let number = parent.number + 1;

        Header { hash, parent_hash: parent.hash, number, ..Default::default() }
    }

    fn account_state_with_nonce_and_balance(nonce: u64, balance: U256) -> AccountState {
        AccountState { balance: Some(balance), code: None, nonce: Some(nonce), storage: Default::default() }
    }
    fn account_state_add_storage(account_state: AccountState, key: B256, value: B256) -> AccountState {
        let mut account_state = account_state;
        account_state.storage.insert(key, value);
        account_state
    }

    fn geth_state_update_add_account(update: GethStateUpdate, address: Address, account_state: AccountState) -> GethStateUpdate {
        let mut update = update;
        update.insert(address, account_state);
        update
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

    #[tokio::test]
    async fn test_with_anvil() -> Result<()> {
        let anvil = Anvil::new().try_spawn()?;
        let client_anvil = ClientBuilder::default().http(anvil.endpoint_url()).boxed();

        let provider = ProviderBuilder::new().on_client(client_anvil);

        provider.anvil_set_auto_mine(false).await?;

        let block_number_0 = provider.get_block_number().await?;

        let block_0 = provider.get_block_by_number(BlockNumberOrTag::Latest, true).await?.unwrap();

        let market_state = Arc::new(RwLock::new(MarketState::new(LoomInMemoryDB::default())));

        let block_history_manager = BlockHistoryManager::new(provider.clone());

        let mut block_history = block_history_manager.init(market_state.clone(), 10).await?;

        let snap = provider.anvil_snapshot().await?;

        provider.anvil_mine(Some(U256::from(1)), None).await?;

        let block_number_2 = provider.get_block_number().await?;
        let block_2 = provider.get_block_by_number(BlockNumberOrTag::Latest, true).await?.unwrap();

        assert_eq!(block_number_2, block_number_0 + 1);
        assert_eq!(block_2.header.parent_hash, block_0.header.hash);

        block_history.add_block_header(block_2.header.clone())?;

        let mut entry_2 = block_history_manager.get_or_fetch_entry(&mut block_history, block_2.header.hash).await?;
        block_history_manager.fetch_entry_data(&mut entry_2).await;
        entry_2.state_update = Some(vec![geth_state_update_add_account(
            GethStateUpdate::default(),
            Address::repeat_byte(1),
            account_state_with_nonce_and_balance(1, U256::from(2)),
        )]);

        assert_eq!(entry_2.is_fetched(), true);

        provider.revert(snap.to()).await?;
        let block_number_2 = provider.get_block_number().await?;
        let block_2 = provider.get_block_by_number(BlockNumberOrTag::Latest, true).await?.unwrap();

        assert_eq!(block_number_2, block_number_0);
        assert_eq!(block_2.header.hash, block_0.header.hash);

        Ok(())
    }
}
