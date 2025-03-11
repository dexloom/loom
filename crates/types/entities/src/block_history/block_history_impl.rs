use crate::block_history::block_history_state::BlockHistoryState;
use crate::market_state::MarketStateConfig;
use alloy_consensus::{BlockHeader, TxEnvelope};
use alloy_json_rpc::RpcRecv;
use alloy_network::{BlockResponse, Ethereum, Network};
use alloy_primitives::{BlockHash, BlockNumber, FixedBytes};
use alloy_provider::Provider;
use alloy_rpc_types::{Block, BlockId, BlockTransactionsKind, Filter, Header, Log};
use eyre::{eyre, ErrReport, OptionExt, Result};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::{
    debug_trace_block, GethStateUpdate, GethStateUpdateVec, LoomBlock, LoomDataTypes, LoomDataTypesEVM, LoomDataTypesEthereum, LoomHeader,
};
use serde::de::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::IntoFuture;
use std::marker::PhantomData;
use tracing::{debug, error};

#[derive(Clone, Debug)]
pub struct BlockHistoryEntry<LDT: LoomDataTypes> {
    pub header: LDT::Header,
    pub block: Option<LDT::Block>,
    pub logs: Option<Vec<LDT::Log>>,
    pub state_update: Option<Vec<LDT::StateUpdate>>,
}

impl<LDT: LoomDataTypes> Default for BlockHistoryEntry<LDT> {
    fn default() -> Self {
        Self { ..Default::default() }
    }
}

impl<LDT> BlockHistoryEntry<LDT>
where
    LDT: LoomDataTypes,
{
    pub fn new(
        header: LDT::Header,
        block: Option<LDT::Block>,
        logs: Option<Vec<LDT::Log>>,
        state_update: Option<Vec<LDT::StateUpdate>>,
    ) -> BlockHistoryEntry<LDT> {
        BlockHistoryEntry { header, block, logs, state_update }
    }

    pub fn is_fetched(&self) -> bool {
        self.state_update.is_some() && self.block.is_some() && self.logs.is_some()
    }

    pub fn hash(&self) -> LDT::BlockHash {
        self.header.get_hash()
    }

    pub fn parent_hash(&self) -> LDT::BlockHash {
        self.header.get_parent_hash()
    }

    pub fn number(&self) -> BlockNumber {
        self.header.get_number()
    }

    pub fn timestamp(&self) -> u64 {
        self.header.get_timestamp()
    }
}

#[derive(Debug, Clone)]
pub struct BlockHistory<S, LDT: LoomDataTypes> {
    depth: usize,
    pub latest_block_number: u64,
    block_states: HashMap<LDT::BlockHash, S>,
    block_entries: HashMap<LDT::BlockHash, BlockHistoryEntry<LDT>>,
    block_numbers: HashMap<u64, LDT::BlockHash>,
}

impl<S, LDT> BlockHistory<S, LDT>
where
    LDT: LoomDataTypes,
    S: BlockHistoryState<LDT>,
{
    pub fn new(depth: usize) -> BlockHistory<S, LDT> {
        BlockHistory::<S, LDT> {
            depth,
            latest_block_number: 0,
            block_states: Default::default(),
            block_entries: Default::default(),
            block_numbers: HashMap::new(),
        }
    }

    pub fn add_db(&mut self, block_hash: LDT::BlockHash, state: S) -> Result<()> {
        self.block_states.insert(block_hash, state);
        Ok(())
    }
}

impl<S, LDT: LoomDataTypes> BlockHistory<S, LDT> {
    pub fn len(&self) -> usize {
        self.block_entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.block_entries.is_empty()
    }

    fn get_or_insert_entry_with_header(&mut self, header: LDT::Header) -> &mut BlockHistoryEntry<LDT> {
        let block_number = header.get_number();
        let block_hash = header.get_hash();

        //todo: process reorg
        if self.latest_block_number <= block_number {
            self.latest_block_number = block_number;
            self.block_numbers.insert(block_number, block_hash);
        }

        if block_number > self.depth as u64 {
            self.block_numbers.retain(|&key, _| key > (block_number - self.depth as u64));
            let actual_hashes: Vec<LDT::BlockHash> = self.block_numbers.values().cloned().collect();
            self.block_entries.retain(|key, _| actual_hashes.contains(key));
            self.block_states.retain(|key, _| actual_hashes.contains(key));
        }

        self.block_entries.entry(block_hash).or_insert(BlockHistoryEntry::new(header, None, None, None))
    }

    fn get_or_insert_entry_mut(&mut self, block_hash: LDT::BlockHash) -> &mut BlockHistoryEntry<LDT> {
        self.block_entries.entry(block_hash).or_default()
    }

    fn set_entry(&mut self, entry: BlockHistoryEntry<LDT>) {
        self.block_numbers.insert(entry.number(), entry.hash());
        self.block_entries.insert(entry.hash(), entry);
    }

    fn check_reorg_at_block(&mut self, block_number: BlockNumber) -> Option<BlockHash> {
        if let Some(block_hash) = self.get_block_hash_for_block_number(block_number) {
            None
        } else {
            None
        }
    }

    pub fn add_block_header(&mut self, block_header: LDT::Header) -> Result<bool> {
        let block_hash = block_header.get_hash();
        let block_number = block_header.get_number();
        let mut is_new = false;

        if !self.contains_block(&block_hash) {
            let market_history_entry = self.get_or_insert_entry_with_header(block_header.clone());
            let parent_block_hash = block_header.get_parent_hash();

            if block_number >= self.latest_block_number {
                is_new = true;
            }

            if block_number == self.latest_block_number + 1 {
                self.latest_block_number = block_number;
            }
            Ok(is_new)
        } else {
            if let Some(market_history_entry) = self.get_block_history_entry(&block_hash) {
                debug!(
                    "Block header is already processed: {} block : {} state_update : {} logs : {}",
                    market_history_entry.hash(),
                    market_history_entry.block.is_some(),
                    market_history_entry.state_update.is_some(),
                    market_history_entry.logs.is_some(),
                );
            }
            Err(eyre!("BLOCK_IS_ALREADY_PROCESSED"))
        }
    }

    pub fn add_block(&mut self, block: LDT::Block) -> Result<()> {
        let block_hash = block.get_header().get_hash();
        let block_number = block.get_header().get_number();

        let market_history_entry = self.get_or_insert_entry_with_header(block.get_header());

        if market_history_entry.block.is_some() {
            debug!(
                "Block is already processed: {} block : {} state_update : {} logs : {}",
                market_history_entry.hash(),
                market_history_entry.block.is_some(),
                market_history_entry.state_update.is_some(),
                market_history_entry.logs.is_some(),
            );

            return Err(ErrReport::msg("BLOCK_IS_ALREADY_PROCESSED"));
        }

        market_history_entry.block = Some(block);

        Ok(())
    }

    pub fn add_state_diff(&mut self, block_hash: LDT::BlockHash, state_diff: Vec<LDT::StateUpdate>) -> Result<()> {
        let market_history_entry = self.get_or_insert_entry_mut(block_hash);

        if market_history_entry.state_update.is_none() {
            market_history_entry.state_update = Some(state_diff);
            Ok(())
        } else {
            debug!(
                "Block state is already processed: {} block : {} state_update : {} logs : {}",
                market_history_entry.hash(),
                market_history_entry.block.is_some(),
                market_history_entry.state_update.is_some(),
                market_history_entry.logs.is_some(),
            );
            Err(ErrReport::msg("BLOCK_STATE_IS_ALREADY_PROCESSED"))
        }
    }

    pub fn add_logs(&mut self, block_hash: LDT::BlockHash, logs: Vec<LDT::Log>) -> Result<()> {
        let market_history_entry = self.get_or_insert_entry_mut(block_hash);

        if market_history_entry.logs.is_none() {
            market_history_entry.logs = Some(logs);
            Ok(())
        } else {
            debug!(
                "Block log is already processed : {} block : {} state_update : {} logs : {}",
                market_history_entry.hash(),
                market_history_entry.block.is_some(),
                market_history_entry.state_update.is_some(),
                market_history_entry.logs.is_some(),
            );
            Err(ErrReport::msg("BLOCK_LOGS_ARE_ALREADY_PROCESSED"))
        }
    }

    pub fn get_block_history_entry(&self, block_hash: &LDT::BlockHash) -> Option<&BlockHistoryEntry<LDT>> {
        self.block_entries.get(block_hash)
    }

    pub fn get_block_state(&self, block_hash: &LDT::BlockHash) -> Option<&S> {
        self.block_states.get(block_hash)
    }

    pub fn get_entry_mut(&mut self, block_hash: &LDT::BlockHash) -> Option<&mut BlockHistoryEntry<LDT>> {
        self.block_entries.get_mut(block_hash)
    }

    pub fn get_block_by_hash(&self, block_hash: &LDT::BlockHash) -> Option<LDT::Block> {
        self.block_entries.get(block_hash).and_then(|entry| entry.block.clone())
    }

    pub fn get_block_hash_for_block_number(&self, block_number: BlockNumber) -> Option<LDT::BlockHash> {
        self.block_numbers.get(&block_number).cloned()
    }

    pub fn get_first_block_number(&self) -> Option<BlockNumber> {
        self.block_entries.values().map(|x| x.header.get_number()).min()
    }

    pub fn contains_block(&self, block_hash: &LDT::BlockHash) -> bool {
        self.block_entries.contains_key(block_hash)
    }
}

pub struct BlockHistoryManager<P, N, DB, LDT> {
    client: P,
    _td: PhantomData<(N, DB, LDT)>,
}
//
// impl<P, S, LDT> BlockHistoryManager<P, Ethereum, S, LDT>
// where
//     P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
//     S: BlockHistoryState<LDT> + Clone,
//     LDT: LoomDataTypesEVM,
// {
//     pub async fn fetch_entry_data(&self, entry: &mut BlockHistoryEntry<LDT>) -> Result<()> {
//         if entry.logs.is_none() {
//             let filter = Filter::new().at_block_hash(entry.hash());
//             let logs = self.client.get_logs(&filter).await?;
//             entry.logs = Some(logs);
//         }
//
//         if entry.block.is_none() {
//             let block = self.client.get_block_by_hash(entry.hash()).full().await?;
//             if let Some(block) = block {
//                 entry.block = Some(block);
//             }
//         }
//
//         if entry.state_update.is_none() {
//             if let Ok((_, state_update)) = debug_trace_block(self.client.clone(), BlockId::Hash(entry.hash().into()), true).await {
//                 entry.state_update = Some(state_update);
//             } else {
//                 error!("ERROR_FETCHING_STATE_UPDATE");
//                 entry.state_update = Some(vec![]);
//             }
//         }
//
//         if entry.is_fetched() {
//             Ok(())
//         } else {
//             Err(eyre!("BLOCK_DATA_NOT_FETCHED"))
//         }
//     }
// }

impl<'de, P, N, S, LDT> BlockHistoryManager<P, N, S, LDT>
where
    N: Network<BlockResponse = LDT::Block>,
    P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    S: BlockHistoryState<LDT> + Clone,
    LDT: LoomDataTypesEVM,
    LDT::Block: BlockResponse + RpcRecv,
{
    pub fn init(&self, current_state: S, depth: usize, block: LDT::Block) -> BlockHistory<S, LDT>
    where
        P: Provider<N> + Send + Sync + Clone + 'static,
    {
        let latest_block_number = <alloy_rpc_types::Header as LoomHeader<LDT>>::get_number(&block.get_header());
        let block_hash = <alloy_rpc_types::Header as LoomHeader<LDT>>::get_hash(&block.get_header());

        let block_entry = BlockHistoryEntry::new(block.get_header().clone(), Some(block), None, None);

        let mut block_entries: HashMap<LDT::BlockHash, BlockHistoryEntry<LDT>> = HashMap::new();
        let mut block_numbers: HashMap<u64, LDT::BlockHash> = HashMap::new();
        let mut block_states: HashMap<LDT::BlockHash, S> = HashMap::new();

        block_numbers.insert(latest_block_number, block_hash);
        block_entries.insert(block_hash, block_entry);
        block_states.insert(block_hash, current_state);

        BlockHistory { depth, latest_block_number, block_states, block_entries, block_numbers }
    }

    pub fn new(client: P) -> Self {
        Self { client, _td: PhantomData }
    }

    pub async fn fetch_entry_data(&self, entry: &mut BlockHistoryEntry<LDT>) -> Result<()> {
        if entry.logs.is_none() {
            let filter = Filter::new().at_block_hash(entry.hash());
            let logs = self.client.get_logs(&filter).await?;
            entry.logs = Some(logs);
        }

        if entry.block.is_none() {
            let block = self.client.get_block_by_hash(entry.hash()).await?;
            if let Some(block) = block {
                entry.block = Some(block);
            }
        }

        if entry.state_update.is_none() {
            if let Ok((_, state_update)) = debug_trace_block(self.client.clone(), BlockId::Hash(entry.hash().into()), true).await {
                entry.state_update = Some(state_update);
            } else {
                error!("ERROR_FETCHING_STATE_UPDATE");
                entry.state_update = Some(vec![]);
            }
        }

        if entry.is_fetched() {
            Ok(())
        } else {
            Err(eyre!("BLOCK_DATA_NOT_FETCHED"))
        }
    }
    pub async fn get_or_fetch_entry_cloned(
        &self,
        block_history: &mut BlockHistory<S, LDT>,
        block_hash: BlockHash,
    ) -> Result<BlockHistoryEntry<LDT>> {
        if let Some(entry) = block_history.get_block_history_entry(&block_hash) {
            Ok(entry.clone())
        } else {
            let entry = self.fetch_entry_by_hash(block_hash).await?;
            block_history.set_entry(entry.clone());
            Ok(entry)
        }
    }

    pub async fn get_parent_state(
        &self,
        block_history: &mut BlockHistory<S, LDT>,
        market_state_config: &MarketStateConfig,
        parent_hash: BlockHash,
    ) -> Result<S> {
        let mut parent_hash = parent_hash;
        let mut parent_db: Option<S> = None;
        let mut missed_blocks: Vec<BlockHash> = vec![];
        let first_block_number = block_history.get_first_block_number();

        loop {
            let parent_entry = block_history.get_block_history_entry(&parent_hash).ok_or_eyre("PARENT_ENTRY_NOT_FOUND")?;
            if let Some(first_block_number) = first_block_number {
                if parent_entry.number() < first_block_number {
                    break;
                }
                match block_history.block_states.get(&parent_hash) {
                    Some(db) => {
                        parent_db = Some(db.clone());
                        break;
                    }
                    None => {
                        missed_blocks.push(parent_entry.hash());
                        parent_hash = parent_entry.parent_hash();
                    }
                }
            }
        }

        match parent_db {
            Some(db) => {
                let mut db = db;
                missed_blocks.reverse();
                for missed_block_hash in missed_blocks.into_iter() {
                    let missed_entry = block_history.get_entry_mut(&missed_block_hash).ok_or_eyre("ENTRY_NOT_FOUND")?;
                    db = db.apply_update(missed_entry, market_state_config);
                    block_history.block_states.insert(missed_block_hash, db.clone());
                }
                Ok(db)
            }
            None => Err(eyre!("PARENT_DB_NOT_FOUND")),
        }
    }

    pub async fn apply_state_update_on_parent_db(
        &self,
        block_history: &mut BlockHistory<S, LDT>,
        market_state_config: &MarketStateConfig,
        block_hash: LDT::BlockHash,
    ) -> Result<S> {
        let mut entry = block_history.get_or_insert_entry_mut(block_hash).clone();
        if !entry.is_fetched() {
            self.fetch_entry_data(&mut entry).await?;
        }

        let parent_db = self.get_parent_state(block_history, market_state_config, entry.parent_hash()).await?;

        let db = parent_db.apply_update(&entry, market_state_config);

        Ok(db)
    }

    pub async fn set_chain_head(&self, block_history: &mut BlockHistory<S, LDT>, header: LDT::Header) -> Result<(bool, usize)> {
        let mut reorg_depth = 0;
        let mut is_new_block = false;
        let mut is_new_block = false;
        let parent_hash = LoomHeader::<LDT>::get_parent_hash(&header);
        let first_block_number = block_history.get_first_block_number();

        if let Ok(is_new) = block_history.add_block_header(header) {
            is_new_block = is_new;
            if let Some(min_block) = first_block_number {
                let mut parent_block_hash: LDT::BlockHash = parent_hash;

                if is_new {
                    loop {
                        match block_history.get_block_history_entry(&parent_block_hash).cloned() {
                            Some(entry) => {
                                if block_history.get_block_hash_for_block_number(entry.number()).unwrap_or_default() == entry.hash() {
                                    break;
                                } else {
                                    block_history.block_numbers.insert(entry.number(), entry.hash());
                                    reorg_depth += 1;
                                    parent_block_hash = entry.parent_hash();
                                }
                            }
                            None => {
                                let entry = self.fetch_entry_by_hash(parent_block_hash).await?;
                                if entry.number() < min_block {
                                    break;
                                }
                                block_history.set_entry(entry.clone());
                                parent_block_hash = entry.parent_hash();
                                reorg_depth += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok((is_new_block, reorg_depth))
    }

    pub async fn fetch_entry_by_hash(&self, block_hash: LDT::BlockHash) -> Result<BlockHistoryEntry<LDT>> {
        let block = self.client.get_block_by_hash(block_hash).full().await?;
        if let Some(block) = block {
            let header = block.get_header().clone();

            let filter = Filter::new().at_block_hash(block_hash);

            let logs = self.client.get_logs(&filter).await?;

            let state_update = match debug_trace_block(self.client.clone(), BlockId::Hash(block_hash.into()), true).await {
                Ok((_, state_update)) => state_update,
                Err(_) => {
                    vec![]
                }
            };

            let block_entry = BlockHistoryEntry::new(header, Some(block), Some(logs), Some(state_update));
            Ok(block_entry)
        } else {
            Err(eyre!("BLOCK_IS_EMPTY"))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::MarketState;
    use alloy_node_bindings::Anvil;
    use alloy_primitives::{Address, U256};
    use alloy_provider::ext::AnvilApi;
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::ClientBuilder;
    use alloy_rpc_types::BlockNumberOrTag;
    use loom_evm_db::LoomDBType;
    use loom_evm_utils::geth_state_update::*;
    use loom_node_debug_provider::AnvilProviderExt;
    use loom_types_blockchain::{GethStateUpdate, LoomDataTypesEthereum};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn create_next_header(parent: &Header, child_id: u64) -> Header {
        let number = parent.number + 1;
        let hash: BlockHash = (U256::try_from(parent.hash).unwrap() * U256::from(256) + U256::from(number + child_id)).try_into().unwrap();
        let number = parent.number + 1;

        let consensus_header = alloy_consensus::Header { parent_hash: parent.hash, number, ..Default::default() };

        Header { hash, inner: consensus_header, total_difficulty: None, size: None }
    }

    fn create_header(number: BlockNumber, hash: BlockHash) -> Header {
        let consensus_header = alloy_consensus::Header { number, ..Default::default() };

        Header { hash, inner: consensus_header, total_difficulty: None, size: None }
    }

    #[test]
    fn test_add_block_header() {
        let mut block_history = BlockHistory::<LoomDBType, LoomDataTypesEthereum>::new(10);

        let header_1_0 = create_header(1, U256::from(1).into());
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
        let mut block_history = BlockHistory::<LoomDBType, LoomDataTypesEthereum>::new(10);

        let header_1_0 = create_header(1, U256::from(1).into());
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
        let mut block_history = BlockHistory::<LoomDBType, LoomDataTypesEthereum>::new(10);

        let header_1_0 = create_header(1, U256::from(1).into());
        let header_2_0 = create_next_header(&header_1_0, 0);
        let header_2_1 = create_next_header(&header_1_0, 1);
        let header_3_0 = create_next_header(&header_2_0, 0);
        let header_3_1 = create_next_header(&header_2_1, 0);
        let header_4_1 = create_next_header(&header_3_1, 0);

        block_history.add_block_header(header_1_0).unwrap();
        block_history.add_block_header(header_2_0).unwrap();
        block_history.add_block_header(header_3_0).unwrap();
        block_history.add_block_header(header_2_1).unwrap();
        block_history.add_block_header(header_3_1.clone()).unwrap();
        block_history.add_block_header(header_4_1.clone()).unwrap();

        assert_eq!(block_history.block_entries.len(), 6);
        assert_eq!(block_history.latest_block_number, 4);
        assert_eq!(block_history.block_numbers[&3], header_3_1.hash);
        assert_eq!(block_history.block_numbers[&4], header_4_1.hash);
    }

    #[tokio::test]
    async fn test_with_anvil() -> Result<()> {
        let anvil = Anvil::new().try_spawn()?;
        let client_anvil = ClientBuilder::default().http(anvil.endpoint_url());

        let provider = ProviderBuilder::new().disable_recommended_fillers().on_client(client_anvil);

        provider.anvil_set_auto_mine(false).await?;

        let block_number_0 = provider.get_block_number().await?;

        let block_0 = provider.get_block_by_number(BlockNumberOrTag::Latest).full().await?.unwrap();

        let market_state = Arc::new(RwLock::new(MarketState::new(LoomDBType::default())));

        let block_history_manager = BlockHistoryManager::new(provider.clone());

        let mut block_history = block_history_manager.init(LoomDBType::default(), 10, block_0.clone());

        let snap = provider.anvil_snapshot().await?;

        provider.anvil_mine(Some(1), None).await?;

        let block_number_2 = provider.get_block_number().await?;
        let block_2 = provider.get_block_by_number(BlockNumberOrTag::Latest).full().await?.unwrap();

        assert_eq!(block_number_2, block_number_0 + 1);
        assert_eq!(block_2.header.parent_hash, block_0.header.hash);

        block_history.add_block_header(block_2.header.clone())?;

        let mut entry_2 = block_history_manager.get_or_fetch_entry_cloned(&mut block_history, block_2.header.hash).await?;
        block_history_manager.fetch_entry_data(&mut entry_2).await;
        entry_2.state_update = Some(vec![geth_state_update_add_account(
            GethStateUpdate::default(),
            Address::repeat_byte(1),
            account_state_with_nonce_and_balance(1, U256::from(2)),
        )]);

        assert_eq!(entry_2.is_fetched(), true);

        provider.revert(snap.to()).await?;
        let block_number_2 = provider.get_block_number().await?;
        let block_2 = provider.get_block_by_number(BlockNumberOrTag::Latest).full().await?.unwrap();

        assert_eq!(block_number_2, block_number_0);
        assert_eq!(block_2.header.hash, block_0.header.hash);

        Ok(())
    }
}
