use std::collections::{BTreeMap, HashMap, HashSet};

use alloy_primitives::{Address, BlockHash, BlockNumber, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{BlockId, BlockNumberOrTag};
use alloy_rpc_types_trace::geth::AccountState;
use eyre::Result;
use revm::db::{AccountState as DbAccountState, Database};
use revm::primitives::bitvec::macros::internal::funty::Fundamental;
use revm::primitives::{AccountInfo, Bytecode, KECCAK_EMPTY};
use revm::InMemoryDB;
use tracing::{debug, error, trace};

use defi_types::GethStateUpdate;
use loom_revm_db::LoomInMemoryDB;

#[derive(Clone)]
pub struct MarketState {
    pub block_number: BlockNumber,
    pub block_hash: BlockHash,
    pub state_db: LoomInMemoryDB,
    force_insert_accounts: HashMap<Address, bool>,
    pub read_only_cells: HashMap<Address, HashSet<U256>>,
}

impl MarketState {
    pub fn new(db: LoomInMemoryDB) -> MarketState {
        MarketState {
            block_number: Default::default(),
            block_hash: Default::default(),
            state_db: db,
            force_insert_accounts: HashMap::new(),
            read_only_cells: HashMap::new(),
        }
    }

    pub fn hash(&self) -> BlockHash {
        self.block_hash
    }
    pub fn number(&self) -> BlockNumber {
        self.block_number
    }

    pub fn number_and_hash(&self) -> (BlockNumber, BlockHash) {
        (self.block_number, self.block_hash)
    }

    pub fn accounts_len(&self) -> usize {
        self.state_db.accounts.len()
    }

    pub fn accounts_db_len(&self) -> usize {
        self.state_db.db.accounts.len()
    }

    pub fn storage_len(&self) -> usize {
        let mut ret = 0;
        for (_, a) in self.state_db.accounts.iter() {
            ret += a.storage.len()
        }
        ret
    }

    pub fn storage_db_len(&self) -> usize {
        let mut ret = 0;
        for (_, a) in self.state_db.db.accounts.iter() {
            ret += a.storage.len()
        }
        ret
    }

    pub fn is_force_insert(&self, address: &Address) -> bool {
        self.force_insert_accounts.contains_key(address)
    }

    pub fn add_force_insert(&mut self, address: Address) {
        self.force_insert_accounts.insert(address, true);
    }

    pub fn is_account(&self, address: &Address) -> bool {
        self.state_db.accounts.contains_key(address) || self.state_db.db.accounts.contains_key(address)
    }

    pub fn is_slot(&self, address: &Address, slot: &U256) -> bool {
        if let Some(account) = self.state_db.accounts.get(address) {
            account.storage.contains_key(slot)
        } else if let Some(account) = self.state_db.db.accounts.get(address) {
            account.storage.contains_key(slot)
        } else {
            false
        }
    }

    pub fn apply_account_info_btree(&mut self, address: &Address, account_updated_state: &AccountState, insert: bool, only_new: bool) {
        let account = self.state_db.load_account(*address);

        let Ok(account) = account;
        if insert
            || ((account.account_state == DbAccountState::NotExisting || account.account_state == DbAccountState::None) && only_new)
            || (!only_new && (account.account_state == DbAccountState::Touched || account.account_state == DbAccountState::StorageCleared))
        {
            let code: Option<Bytecode> = match &account_updated_state.code {
                Some(c) => {
                    if c.len() < 2 {
                        account.info.code.clone()
                    } else {
                        Some(Bytecode::new_raw(c.clone()))
                    }
                }
                None => account.info.code.clone(),
            };

            trace!(
                "apply_account_info {address}.  code len: {} storage len: {}",
                code.clone().map_or(0, |x| x.len()),
                account.storage.len()
            );

            let account_info = AccountInfo {
                balance: account_updated_state.balance.unwrap_or_default(),
                nonce: account_updated_state.nonce.unwrap_or_default().as_u64(),
                code_hash: if code.is_some() { KECCAK_EMPTY } else { Default::default() },
                code,
            };

            self.state_db.insert_account_info(*address, account_info);
        } else {
            trace!("apply_account_info exists {address}. storage len: {}", account.storage.len(),);
        }
        let account = self.state_db.load_account(*address).unwrap();
        account.account_state = DbAccountState::Touched;
        trace!(
            "after apply_account_info account: {address} state: {:?} storage len: {} code len : {}",
            account.account_state,
            account.storage.len(),
            account.info.code.clone().map_or(0, |c| c.len())
        );
    }

    pub fn apply_account_storage(&mut self, address: &Address, acc_state: &AccountState, insert: bool, only_new: bool) {
        if insert {
            for (slot, value) in acc_state.storage.iter() {
                trace!("Inserting storage {address:?} slot : {slot:?} value : {value:?}");
                let _ = self.state_db.insert_account_storage(*address, (*slot).into(), (*value).into());
            }
        } else {
            let account = self.state_db.load_account(*address).cloned().unwrap();
            for (slot, value) in acc_state.storage.iter() {
                let is_slot = account.storage.contains_key::<U256>(&(*slot).into());
                let _ = self.state_db.insert_account_storage(*address, (*slot).into(), (*value).into());
                trace!("Inserting storage {address:?} slot : {slot:?} value : {value:?}");
            }
        }
    }

    pub fn apply_state_update(&mut self, update_vec: &Vec<BTreeMap<Address, AccountState>>, insert: bool, only_new: bool) -> &mut Self {
        for update_record in update_vec {
            for (address, acc_state) in update_record {
                trace!(
                    "updating {address} insert: {insert} only_new: {only_new} storage len {} code: {}",
                    acc_state.storage.len(),
                    acc_state.code.is_some()
                );
                self.apply_account_info_btree(address, acc_state, insert, only_new);
                self.apply_account_storage(address, acc_state, insert, only_new);
            }
        }
        self
    }

    pub fn merge_db(&mut self, other: &InMemoryDB) {
        for (address, account) in other.accounts.iter() {
            if !self.is_account(address) {
                debug!("inserting account info {address}");
                self.state_db.insert_account_info(*address, account.info.clone())
            }
            for (cell, value) in &account.storage {
                if !self.is_slot(address, cell) || self.state_db.storage(*address, *cell).unwrap_or(U256::ZERO) != *value {
                    debug!("inserting cell {address} {cell} {value}");
                    if let Err(e) = self.state_db.insert_account_storage(*address, *cell, *value) {
                        error!("{}", e)
                    }
                }
            }
        }
    }

    pub fn update_account_storage(&mut self, account: Address, slot: U256, value: U256) -> &mut Self {
        if self.is_slot(&account, &slot) {
            let _ = self.state_db.insert_account_storage(account, slot, value);
        };

        self
    }

    pub fn add_state(&mut self, state: &GethStateUpdate) {
        for (address, account_state) in state.iter() {
            let hex_code = account_state.code.as_ref().map(|code_bytes| Bytecode::new_raw(code_bytes.clone()));

            let balance: U256 = account_state.balance.unwrap_or_default();

            let nonce = account_state.nonce.unwrap_or_default();

            trace!("Address {:#20x} Code : {}", address, hex_code.is_some());

            let account_info = AccountInfo {
                balance,
                nonce,
                code_hash: if hex_code.is_some() { KECCAK_EMPTY } else { Default::default() },
                code: hex_code,
            };

            self.state_db.insert_account_info(*address, account_info);
            for (slot, value) in account_state.storage.iter() {
                self.state_db.insert_account_storage(*address, (*slot).into(), (*value).into()).unwrap();
                trace!("Contract {} Storage {} = {}", address, slot, value);
            }
        }

        //debug!("Added state : {}", state.len());
    }

    pub async fn fetch_state<P: Provider + 'static>(&mut self, account: Address, client: P) {
        //let acc : Address = account.0.into();

        let Ok(account_info) = self.state_db.load_account(account);
        if let Ok(value) = client.get_balance(account).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await {
            if value != account_info.info.balance {
                trace!("Updating balance {} {} -> {}", account.to_checksum(None), account_info.info.balance, value);
                account_info.info.balance = value;
            }
        }

        for (cell, v) in account_info.storage.iter_mut() {
            if let Ok(value) = client.get_storage_at(account, *cell).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await {
                if value != *v {
                    trace!("Updating storage {} {} {} -> {}", account.to_checksum(None), cell, v, value);
                    *v = value;
                }
            }
        }
    }

    pub async fn fetch_all_states<P: Provider + Clone + 'static>(&mut self, client: P) -> Result<()> {
        let addresses: Vec<Address> = self.state_db.accounts.keys().copied().collect();
        for account in addresses {
            let acc: Address = account;
            self.fetch_state(acc, client.clone()).await;
        }
        Ok(())
    }

    pub fn disable_cell(&mut self, address: Address, cell: U256) {
        self.read_only_cells.entry(address).or_default().insert(cell);
    }

    pub fn disable_cell_vec(&mut self, address: Address, cells: Vec<U256>) {
        for cell in cells {
            self.disable_cell(address, cell)
        }
    }

    pub fn is_read_only_cell(&self, address: &Address, cell: &U256) -> bool {
        match self.read_only_cells.get(address) {
            Some(hashset) => hashset.contains(cell),
            _ => false,
        }
    }
}
