use crate::alloydb::AlloyDB;
use crate::fast_cache_db::FastDbAccount;
use crate::fast_hasher::SimpleBuildHasher;
use crate::loom_db_helper::LoomDBHelper;
use crate::DatabaseLoomExt;
use alloy::consensus::constants::KECCAK_EMPTY;
use alloy::eips::BlockNumberOrTag;
use alloy::primitives::map::HashMap;
use alloy::primitives::{Address, BlockNumber, Log, B256, U256};
use alloy::providers::{Network, Provider, ProviderBuilder};
use alloy::rpc::client::ClientBuilder;
use alloy::rpc::types::trace::geth::AccountState as GethAccountState;
use alloy::transports::Transport;
use eyre::{ErrReport, OptionExt, Result};
use revm::db::{AccountState as DBAccountState, EmptyDBTyped};
use revm::primitives::{Account, AccountInfo, Bytecode};
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::collections::hash_map::Entry;
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tracing::{error, trace};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone)]
pub struct LoomDB
where
    Self: Sized + Send + Sync,
{
    pub accounts: HashMap<Address, FastDbAccount>,
    pub contracts: HashMap<B256, Bytecode, SimpleBuildHasher>,
    pub logs: Vec<Log>,
    pub block_hashes: HashMap<BlockNumber, B256>,
    pub read_only_db: Option<Arc<LoomDB>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub ext_db: Option<Arc<dyn DatabaseRef<Error = ErrReport> + Send + Sync>>,
}

impl Debug for LoomDB {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoomDB").field("accounts", &self.accounts).finish()
    }
}

impl Default for LoomDB {
    fn default() -> Self {
        LoomDB::new().with_ext_db(EmptyDBTyped::<ErrReport>::new())
    }
}

#[allow(dead_code)]
impl LoomDB {
    pub fn empty() -> Self {
        Self::default()
    }
    pub fn new() -> Self {
        let mut contracts = HashMap::with_hasher(SimpleBuildHasher::default());
        contracts.insert(KECCAK_EMPTY, Bytecode::default());
        contracts.insert(B256::ZERO, Bytecode::default());

        Self {
            read_only_db: None,
            ext_db: None,
            accounts: Default::default(),
            contracts,
            logs: Default::default(),
            block_hashes: Default::default(),
        }
    }

    pub fn is_rw_ro_account(&self, address: &Address) -> bool {
        self.accounts.contains_key(address) || if let Some(db) = &self.read_only_db { db.accounts.contains_key(address) } else { false }
    }

    pub fn is_rw_ro_slot(&self, address: &Address, slot: &U256) -> bool {
        let is_rw_slot = if let Some(account) = self.accounts.get(address) { account.storage.contains_key(slot) } else { false };

        if is_rw_slot {
            true
        } else if let Some(read_only_db) = &self.read_only_db {
            if let Some(account) = read_only_db.accounts.get(address) {
                account.storage.contains_key(slot)
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn read_only_db(&self) -> Self {
        self.read_only_db.clone().map_or(LoomDB::empty(), |a| a.as_ref().clone())
    }
    pub fn rw_contracts_len(&self) -> usize {
        self.contracts.len()
    }
    pub fn rw_accounts_len(&self) -> usize {
        self.accounts.len()
    }

    pub fn ro_contracts_len(&self) -> usize {
        self.read_only_db.as_ref().map_or(0, |db| db.contracts_len())
    }

    pub fn ro_accounts_len(&self) -> usize {
        self.read_only_db.as_ref().map_or(0, |db| db.accounts_len())
    }

    pub fn rw_storage_len(&self) -> usize {
        self.accounts.values().map(|a| a.storage.len()).sum()
    }
    pub fn ro_storage_len(&self) -> usize {
        self.read_only_db.as_ref().map_or(0, |db| db.accounts.values().map(|a| a.storage.len()).sum())
    }

    pub fn with_ext_db<ExtDB>(self, ext_db: ExtDB) -> Self
    where
        ExtDB: DatabaseRef<Error = ErrReport> + Send + Sync + 'static,
        Self: Sized,
    {
        let ext_db = Arc::new(ext_db) as Arc<dyn DatabaseRef<Error = ErrReport> + Send + Sync>;
        Self { ext_db: Some(ext_db), ..self }
    }

    pub fn without_ext_db(self) -> Self
    where
        Self: Sized,
    {
        Self { ext_db: None, ..self }
    }

    pub fn with_ro_db(self, db: Option<LoomDB>) -> Self {
        Self { read_only_db: db.map(Arc::new), ..self }
    }

    pub fn new_with_ext_db<ExtDB>(db: LoomDB, ext_db: ExtDB) -> Self
    where
        ExtDB: DatabaseRef<Error = ErrReport> + Send + Sync + 'static,
        Self: Sized,
    {
        Self::new().with_ro_db(Some(db)).with_ext_db(ext_db)
    }

    // Returns the account for the given address.
    ///
    /// If the account was not found in the cache, it will be loaded from the underlying database.
    pub fn load_ro_rw_ext_account(&mut self, address: Address) -> Result<&mut FastDbAccount> {
        match self.accounts.entry(address) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => Ok(entry.insert(
                LoomDBHelper::get_or_fetch_basic(&self.read_only_db, &self.ext_db, address)
                    .unwrap_or_default()
                    .map(|info| FastDbAccount { info, ..Default::default() })
                    .unwrap_or_else(FastDbAccount::new_not_existing),
            )),
        }
    }

    // Returns the account for the given address.
    ///
    /// If the account was not found in the cache, it will be loaded from the underlying database.
    pub fn load_ro_rw_account(&mut self, address: Address) -> Result<&mut FastDbAccount> {
        match self.accounts.entry(address) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => Ok(entry.insert(
                LoomDBHelper::get_basic(&self.read_only_db, address)
                    .unwrap_or_default()
                    .map(|info| FastDbAccount { info, ..Default::default() })
                    .unwrap_or_else(FastDbAccount::new_not_existing),
            )),
        }
    }

    pub fn new_with_ro_db_and_provider<P, N>(read_only_db: Option<LoomDB>, client: P) -> Result<Self>
    where
        N: Network,
        P: Provider<N> + 'static,
        Self: Sized,
    {
        let box_transport = client.client().transport().clone().boxed();

        let rpc_client = ClientBuilder::default().transport(box_transport, true);

        let provider = ProviderBuilder::new().disable_recommended_fillers().on_client(rpc_client);

        let ext_db = AlloyDB::new(provider, BlockNumberOrTag::Latest.into());

        let ext_db = ext_db.ok_or_eyre("EXT_DB_NOT_CREATED")?;

        Ok(Self::new().with_ro_db(read_only_db).with_ext_db(ext_db))
    }

    pub fn insert_contract(&mut self, account: &mut AccountInfo) {
        if let Some(code) = &account.code {
            if !code.is_empty() {
                if account.code_hash == KECCAK_EMPTY {
                    account.code_hash = code.hash_slow();
                }
                self.contracts.entry(account.code_hash).or_insert_with(|| code.clone());
            }
        }
        if account.code_hash == B256::ZERO {
            account.code_hash = KECCAK_EMPTY;
        }
    }

    /// Insert account info but not override storage
    pub fn insert_account_info(&mut self, address: Address, mut info: AccountInfo) {
        self.insert_contract(&mut info);
        self.accounts.entry(address).or_default().info = info;
    }

    /// insert account storage without overriding account info
    pub fn insert_account_storage(&mut self, address: Address, slot: U256, value: U256) -> Result<()> {
        let account = self.load_account(address)?;
        account.storage.insert(slot, value);
        Ok(())
    }

    /// replace account storage without overriding account info
    pub fn replace_account_storage(&mut self, address: Address, storage: HashMap<U256, U256>) -> Result<()> {
        let account = self.load_account(address)?;
        account.account_state = DBAccountState::StorageCleared;
        account.storage = storage.into_iter().collect();
        Ok(())
    }

    pub fn merge_all(self) -> LoomDB {
        let mut read_only_db = self.read_only_db.unwrap_or_default().as_ref().clone();

        for (k, v) in self.block_hashes.iter() {
            read_only_db.block_hashes.insert(*k, *v);
        }

        for (k, v) in self.contracts.iter() {
            read_only_db.contracts.insert(*k, v.clone());
        }
        read_only_db.logs.clone_from(&self.logs);

        for (address, account) in self.accounts.iter() {
            let mut info = account.info.clone();
            read_only_db.insert_contract(&mut info);

            let entry = read_only_db.accounts.entry(*address).or_default();
            entry.info = info;
            for (k, v) in account.storage.iter() {
                entry.storage.insert(*k, *v);
            }
        }

        let read_only_db = Some(Arc::new(read_only_db));

        LoomDB { read_only_db, ext_db: self.ext_db, ..Default::default() }
    }

    pub fn merge_accounts(self) -> LoomDB {
        let read_only_db = if let Some(read_only_db) = self.read_only_db {
            let mut read_only_db_clone = (*read_only_db).clone();

            for (k, v) in self.block_hashes.iter() {
                read_only_db_clone.block_hashes.insert(*k, *v);
            }
            for (k, v) in self.contracts.iter() {
                read_only_db_clone.contracts.entry(*k).and_modify(|k| k.clone_from(v));
            }
            read_only_db_clone.logs.clone_from(&self.logs);

            for (address, account) in self.accounts.iter() {
                read_only_db_clone.accounts.entry(*address).and_modify(|db_account| {
                    let info = account.info.clone();
                    db_account.info = info;
                    for (k, v) in account.storage.iter() {
                        db_account.storage.insert(*k, *v);
                    }
                    db_account.account_state = DBAccountState::Touched
                });
            }
            Some(Arc::new(read_only_db_clone))
        } else {
            None
        };

        LoomDB { read_only_db, ext_db: self.ext_db, ..Default::default() }
    }

    pub fn merge_cells(self) -> LoomDB {
        let read_only_db = if let Some(read_only_db) = self.read_only_db {
            let mut read_only_db_clone = (*read_only_db).clone();

            for (k, v) in self.block_hashes.iter() {
                read_only_db_clone.block_hashes.insert(*k, *v);
            }
            for (k, v) in self.contracts.iter() {
                read_only_db_clone.contracts.entry(*k).and_modify(|k| k.clone_from(v));
            }
            read_only_db_clone.logs.clone_from(&self.logs);

            for (address, account) in self.accounts.iter() {
                read_only_db_clone.accounts.entry(*address).and_modify(|db_account| {
                    let info = account.info.clone();
                    db_account.info = info;
                    for (k, v) in account.storage.iter() {
                        db_account.storage.entry(*k).and_modify(|cv| cv.clone_from(v));
                    }
                    db_account.account_state = DBAccountState::Touched
                });
            }
            Some(Arc::new(read_only_db_clone))
        } else {
            None
        };

        LoomDB { read_only_db, ext_db: self.ext_db, ..Default::default() }
    }

    pub fn apply_geth_update(&mut self, update: BTreeMap<Address, GethAccountState>) {
        for (addr, acc_state) in update {
            trace!("apply_geth_update {} is code {} storage_len {} ", addr, acc_state.code.is_some(), acc_state.storage.len());

            for (k, v) in acc_state.storage.iter() {
                if let Err(e) = self.insert_account_storage(addr, (*k).into(), (*v).into()) {
                    error!("apply_geth_update :{}", e);
                }
            }
            if let Ok(account) = self.load_account(addr) {
                if let Some(code) = acc_state.code.clone() {
                    let bytecode = Bytecode::new_raw(code);
                    account.info.code_hash = bytecode.hash_slow();
                    account.info.code = Some(bytecode);
                }
                if let Some(nonce) = acc_state.nonce {
                    //trace!("nonce : {} -> {}", account.info.nonce, nonce);
                    account.info.nonce = nonce;
                }
                if let Some(balance) = acc_state.balance {
                    account.info.balance = balance;
                }
                account.account_state = DBAccountState::Touched;
            }
        }
    }

    pub fn apply_geth_update_vec(&mut self, update: Vec<BTreeMap<Address, GethAccountState>>) {
        for entry in update.into_iter() {
            self.apply_geth_update(entry);
        }
    }

    pub fn apply_account_info_btree(
        &mut self,
        address: &Address,
        account_updated_state: &alloy::rpc::types::trace::geth::AccountState,
        insert: bool,
        only_new: bool,
    ) {
        let account = self.load_cached_account(*address);

        if let Ok(account) = account {
            if insert
                || ((account.account_state == DBAccountState::NotExisting || account.account_state == DBAccountState::None) && only_new)
                || (!only_new
                    && (account.account_state == DBAccountState::Touched || account.account_state == DBAccountState::StorageCleared))
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
                    nonce: account_updated_state.nonce.unwrap_or_default(),
                    code_hash: if code.is_some() { KECCAK_EMPTY } else { Default::default() },
                    code,
                };

                self.insert_account_info(*address, account_info);
            } else {
                trace!("apply_account_info exists {address}. storage len: {}", account.storage.len(),);
            }
        }

        if let Ok(account) = self.load_cached_account(*address) {
            account.account_state = DBAccountState::Touched;
            trace!(
                "after apply_account_info account: {address} state: {:?} storage len: {} code len : {}",
                account.account_state,
                account.storage.len(),
                account.info.code.clone().map_or(0, |c| c.len())
            );
        } else {
            trace!(%address, "account not found after apply");
        }
    }

    pub fn apply_account_storage(&mut self, address: &Address, acc_state: &GethAccountState, insert: bool, only_new: bool) {
        if insert {
            if let Ok(account) = self.load_cached_account(*address) {
                for (slot, value) in acc_state.storage.iter() {
                    trace!(%address, ?slot, ?value, "Inserting storage");
                    account.storage.insert((*slot).into(), (*value).into());
                }
            }
        } else if self.is_account(address) {
            let slots_to_insert: Vec<_> = acc_state
                .storage
                .iter()
                .filter_map(|(slot, value)| {
                    let is_slot = self.is_slot(address, &(*slot).into());
                    if is_slot || only_new {
                        Some(((*slot).into(), (*value).into()))
                    } else {
                        None
                    }
                })
                .collect();

            if let Ok(account) = self.load_cached_account(*address) {
                for (slot, value) in slots_to_insert {
                    account.storage.insert(slot, value);
                    trace!(%address, ?slot, ?value, "Inserting storage");
                }
            }
        }
    }

    pub fn apply_geth_state_update(
        &mut self,
        update_vec: &Vec<BTreeMap<Address, GethAccountState>>,
        insert: bool,
        only_new: bool,
    ) -> &mut Self {
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
}

impl DatabaseLoomExt for LoomDB {
    fn with_ext_db(&mut self, arc_db: impl DatabaseRef<Error = ErrReport> + Send + Sync + 'static) {
        self.ext_db = Some(Arc::new(arc_db))
    }

    fn is_account(&self, address: &Address) -> bool {
        self.is_rw_ro_account(address)
    }

    fn is_slot(&self, address: &Address, slot: &U256) -> bool {
        self.is_rw_ro_slot(address, slot)
    }

    fn contracts_len(&self) -> usize {
        self.rw_contracts_len() + self.ro_contracts_len()
    }

    fn accounts_len(&self) -> usize {
        self.rw_accounts_len() + self.ro_contracts_len()
    }

    fn storage_len(&self) -> usize {
        self.rw_storage_len() + self.ro_storage_len()
    }

    fn load_account(&mut self, address: Address) -> Result<&mut FastDbAccount> {
        self.load_ro_rw_ext_account(address)
    }

    fn load_cached_account(&mut self, address: Address) -> Result<&mut FastDbAccount> {
        self.load_ro_rw_account(address)
    }

    fn insert_contract(&mut self, account: &mut AccountInfo) {
        self.insert_contract(account)
    }

    fn insert_account_info(&mut self, address: Address, info: AccountInfo) {
        self.insert_account_info(address, info)
    }

    fn insert_account_storage(&mut self, address: Address, slot: U256, value: U256) -> Result<()> {
        self.insert_account_storage(address, slot, value)
    }

    fn replace_account_storage(&mut self, address: Address, storage: HashMap<U256, U256>) -> Result<()> {
        self.replace_account_storage(address, storage)
    }

    fn maintain(self) -> Self {
        self.merge_all()
    }
}

impl DatabaseRef for LoomDB {
    type Error = eyre::ErrReport;
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        trace!(%address, "basic_ref");
        let result = match address {
            Address::ZERO => Ok(Some(AccountInfo::default())),
            _ => match self.accounts.get(&address) {
                Some(acc) => {
                    trace!(%address, "account found");
                    Ok(acc.info())
                }
                None => Ok(LoomDBHelper::get_or_fetch_basic(&self.read_only_db, &self.ext_db, address).unwrap_or_default()),
            },
        };

        result
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        match self.contracts.get(&code_hash) {
            Some(entry) => Ok(entry.clone()),
            None => LoomDBHelper::get_code_by_hash(&self.read_only_db, code_hash),
        }
    }

    fn storage_ref(&self, address: Address, slot: U256) -> Result<U256, Self::Error> {
        trace!(%address, ?slot, "storage_ref");

        match self.accounts.get(&address) {
            Some(acc_entry) => match acc_entry.storage.get(&slot) {
                Some(entry) => {
                    trace!(%address, ?slot, %entry,  "storage_ref");
                    Ok(*entry)
                }
                None => {
                    if matches!(acc_entry.account_state, DBAccountState::StorageCleared | DBAccountState::NotExisting) {
                        trace!(%address, ?slot, state=?acc_entry.account_state, "storage_ref ZERO");
                        Ok(U256::ZERO)
                    } else {
                        LoomDBHelper::get_or_fetch_storage(&self.read_only_db, &self.ext_db, address, slot)
                    }
                }
            },
            None => LoomDBHelper::get_or_fetch_storage(&self.read_only_db, &self.ext_db, address, slot),
        }
    }

    fn block_hash_ref(&self, number: BlockNumber) -> Result<B256, Self::Error> {
        match self.block_hashes.get(&number) {
            Some(entry) => Ok(*entry),
            None => LoomDBHelper::get_or_fetch_block_hash(&self.read_only_db, &self.ext_db, number),
        }
    }
}

impl Database for LoomDB {
    type Error = ErrReport;

    fn basic(&mut self, address: Address) -> std::result::Result<Option<AccountInfo>, Self::Error> {
        trace!(%address, "basic");

        let basic = match self.accounts.entry(address) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(
                LoomDBHelper::get_or_fetch_basic(&self.read_only_db, &self.ext_db, address)
                    .unwrap_or_default()
                    .map(|info| FastDbAccount { info, ..Default::default() })
                    .unwrap_or_else(FastDbAccount::new_not_existing),
            ),
        };
        Ok(basic.info())
    }

    fn code_by_hash(&mut self, code_hash: B256) -> std::result::Result<Bytecode, Self::Error> {
        match self.contracts.entry(code_hash) {
            Entry::Occupied(entry) => Ok(entry.get().clone()),
            Entry::Vacant(entry) => {
                // if you return code bytes when basic fn is called this function is not needed.
                Ok(entry.insert(LoomDBHelper::get_code_by_hash(&self.read_only_db, code_hash)?).clone())
            }
        }
    }

    /// Get the value in an account's storage slot.
    ///
    /// It is assumed that account is already loaded.
    fn storage(&mut self, address: Address, slot: U256) -> std::result::Result<U256, Self::Error> {
        trace!(%address, ?slot, "storage");

        match self.accounts.entry(address) {
            Entry::Occupied(mut acc_entry) => {
                let acc_entry = acc_entry.get_mut();
                match acc_entry.storage.entry(slot) {
                    Entry::Occupied(entry) => Ok(*entry.get()),
                    Entry::Vacant(entry) => {
                        if matches!(acc_entry.account_state, DBAccountState::StorageCleared | DBAccountState::NotExisting) {
                            Ok(U256::ZERO)
                        } else {
                            let slot = LoomDBHelper::get_or_fetch_storage(&self.read_only_db, &self.ext_db, address, slot)?;
                            entry.insert(slot);
                            Ok(slot)
                        }
                    }
                }
            }
            Entry::Vacant(acc_entry) => {
                let info = LoomDBHelper::get_or_fetch_basic(&self.read_only_db, &self.ext_db, address)?;
                let (account, value) = if info.is_some() {
                    let value = LoomDBHelper::get_or_fetch_storage(&self.read_only_db, &self.ext_db, address, slot)?;
                    let mut account: FastDbAccount = info.into();
                    account.storage.insert(slot, value);
                    (account, value)
                } else {
                    (info.into(), U256::ZERO)
                };
                acc_entry.insert(account);
                Ok(value)
            }
        }
    }

    fn block_hash(&mut self, number: BlockNumber) -> std::result::Result<B256, Self::Error> {
        match self.block_hashes.entry(number) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let hash = LoomDBHelper::get_or_fetch_block_hash(&self.read_only_db, &self.ext_db, number)?;
                entry.insert(hash);
                Ok(hash)
            }
        }
    }
}

impl DatabaseCommit for LoomDB {
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        for (address, mut account) in changes {
            if !account.is_touched() {
                continue;
            }
            if account.is_selfdestructed() {
                let db_account = self.accounts.entry(address).or_default();
                db_account.storage.clear();
                db_account.account_state = DBAccountState::NotExisting;
                db_account.info = AccountInfo::default();
                continue;
            }
            let is_newly_created = account.is_created();
            self.insert_contract(&mut account.info);

            let db_account = self.accounts.entry(address).or_default();
            db_account.info = account.info;

            db_account.account_state = if is_newly_created {
                db_account.storage.clear();
                DBAccountState::StorageCleared
            } else if db_account.account_state.is_storage_cleared() {
                // Preserve old account state if it already exists
                DBAccountState::StorageCleared
            } else {
                DBAccountState::Touched
            };
            db_account.storage.extend(account.storage.into_iter().map(|(key, value)| (key, value.present_value())));
        }
    }
}

#[cfg(test)]
mod test {
    use super::GethAccountState;
    use crate::alloydb::AlloyDB;
    use crate::loom_db::LoomDB;
    use alloy::eips::BlockNumberOrTag;
    use alloy::primitives::map::HashMap;
    use alloy::primitives::{Address, Bytes, B256, I256, U256};
    use alloy::providers::{Provider, ProviderBuilder};
    use eyre::ErrReport;
    use revm::db::EmptyDBTyped;
    use revm::primitives::{AccountInfo, Bytecode, KECCAK_EMPTY};
    use revm::{Database, DatabaseRef};
    use std::collections::BTreeMap;

    #[test]
    fn test_new_with_provider() {
        let db = LoomDB::new();
        let provider = ProviderBuilder::new().on_anvil_with_wallet();

        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async move {
            let test_addr = Address::parse_checksummed("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", None).unwrap();

            let balance = provider.get_balance(test_addr).await?;

            let db = LoomDB::new_with_ro_db_and_provider(Some(db), provider.clone()).unwrap();

            let info = db.basic_ref(test_addr).unwrap().unwrap();

            assert_eq!(info.balance, U256::from(10000000000000000000000u128));
            assert_eq!(info.balance, balance);
            Ok::<(), ErrReport>(())
        })
        .unwrap();
    }

    #[test]
    fn test_new_with_ext_db() {
        let db = LoomDB::new();
        let provider = ProviderBuilder::new().on_anvil_with_wallet();

        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async move {
            let test_addr = Address::parse_checksummed("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", None).unwrap();

            let balance = provider.get_balance(test_addr).await?;

            let ext_db = AlloyDB::new(provider.clone(), BlockNumberOrTag::Latest.into()).unwrap();

            let db = LoomDB::new_with_ext_db(db, ext_db);

            let info = db.basic_ref(test_addr).unwrap().unwrap();
            assert_eq!(info.balance, U256::from(10000000000000000000000u128));
            assert_eq!(info.balance, balance);

            Ok::<(), ErrReport>(())
        })
        .unwrap();
    }

    #[test]
    fn test_insert_account_storage() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let mut init_state = LoomDB::new();
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let (key, value) = (U256::from(123), U256::from(456));
        let mut new_state = LoomDB::new().with_ro_db(Some(init_state));

        new_state.insert_account_storage(account, key, value).unwrap();

        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce);
        assert_eq!(new_state.storage(account, key).unwrap(), value);
    }

    #[test]
    fn test_insert_account_storage_inherited() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let mut init_state = LoomDB::new();
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let (key, value) = (U256::from(123), U256::from(456));
        let mut new_state = LoomDB::new().with_ro_db(Some(init_state));
        new_state.insert_account_storage(account, key, value).unwrap();

        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce);
        assert_eq!(new_state.storage(account, key).unwrap(), value);
    }

    #[test]
    fn test_replace_account_storage() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let mut init_state = LoomDB::new();
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let (key0, value0) = (U256::from(123), U256::from(456));
        let (key1, value1) = (U256::from(789), U256::from(999));
        init_state.insert_account_storage(account, key0, value0).unwrap();

        let mut new_state = LoomDB::new().with_ro_db(Some(init_state));
        assert_eq!(new_state.accounts.len(), 0);
        let mut hm: HashMap<U256, U256> = Default::default();
        hm.insert(key1, value1);

        new_state.replace_account_storage(account, hm).unwrap();

        let mut new_state = new_state.merge_all();

        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce);
        assert_eq!(new_state.storage(account, key0).unwrap(), value0);
        assert_eq!(new_state.storage(account, key1).unwrap(), value1);
        assert_eq!(new_state.accounts.len(), 1);
    }

    #[test]
    fn test_apply_geth_update() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let code = Bytecode::new_raw(Bytes::from(vec![1, 2, 3]));
        let mut init_state = LoomDB::new();
        init_state.insert_account_info(account, AccountInfo { nonce, code: Some(code.clone()), ..Default::default() });

        let (key0, value0) = (U256::from(123), U256::from(456));
        let (key1, value1) = (U256::from(789), U256::from(999));
        init_state.insert_account_storage(account, key0, value0).unwrap();
        init_state.insert_account_storage(account, key1, value1).unwrap();

        let mut new_state = LoomDB::new().with_ro_db(Some(init_state));
        assert_eq!(new_state.accounts.len(), 0);

        let update_record = GethAccountState {
            balance: None,
            code: Some(Bytes::from(vec![1, 2, 3])),
            nonce: Some(nonce + 1),
            storage: [(B256::from(I256::try_from(123).unwrap()), B256::from(I256::try_from(333).unwrap()))].into(),
        };

        let update: BTreeMap<Address, GethAccountState> = [(account, update_record)].into();

        new_state.apply_geth_update(update);

        assert_eq!(new_state.basic(account).unwrap().unwrap().code, Some(code.clone()));
        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce + 1);
        assert_eq!(new_state.storage_ref(account, key0).unwrap(), U256::from(333));
        assert_eq!(new_state.storage_ref(account, key1).unwrap(), value1);
        assert_eq!(new_state.accounts.len(), 1);

        let mut new_state = new_state.merge_all();

        assert_eq!(new_state.basic(account).unwrap().unwrap().code, Some(code.clone()));
        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce + 1);
        assert_eq!(new_state.storage_ref(account, key0).unwrap(), U256::from(333));
        assert_eq!(new_state.storage_ref(account, key1).unwrap(), value1);
        assert_eq!(new_state.accounts.len(), 1);
    }

    #[test]
    fn test_merge() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let code = Bytecode::new_raw(Bytes::from(vec![1, 2, 3]));
        let mut init_state = LoomDB::new();
        init_state.insert_account_info(account, AccountInfo { nonce, code: Some(code.clone()), ..Default::default() });

        let (key0, value0) = (U256::from(123), U256::from(456));
        let (key1, value1) = (U256::from(789), U256::from(999));
        let (key2, value2) = (U256::from(999), U256::from(111));
        init_state.insert_account_storage(account, key0, value0).unwrap();
        init_state.insert_account_storage(account, key1, value1).unwrap();

        let mut new_state = LoomDB::new().with_ro_db(Some(init_state));
        assert_eq!(new_state.accounts.len(), 0);

        new_state.insert_account_info(
            account,
            AccountInfo {
                balance: U256::ZERO,
                code: Some(Bytecode::new_raw(Bytes::from(vec![1, 2, 2]))),
                nonce: nonce + 1,
                code_hash: KECCAK_EMPTY,
            },
        );

        new_state.insert_account_storage(account, key0, U256::from(333)).unwrap();
        new_state.insert_account_storage(account, key2, value2).unwrap();

        let mut new_state = new_state.merge_all();

        assert_eq!(new_state.basic(account).unwrap().unwrap().code, Some(Bytecode::new_raw(Bytes::from(vec![1, 2, 2]))));
        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce + 1);
        assert_eq!(new_state.storage_ref(account, key0).unwrap(), U256::from(333));
        assert_eq!(new_state.storage_ref(account, key1).unwrap(), value1);
        assert_eq!(new_state.storage_ref(account, key2).unwrap(), value2);
        assert_eq!(new_state.accounts.len(), 1);
    }

    #[test]
    fn test_update_cell() {
        let account = Address::with_last_byte(42);
        let account2 = Address::with_last_byte(43);
        let nonce = 42;
        let code = Bytecode::new_raw(Bytes::from(vec![1, 2, 3]));
        let mut init_state = LoomDB::new();
        init_state.insert_account_info(account, AccountInfo { nonce, code: Some(code.clone()), ..Default::default() });

        let (key0, value0) = (U256::from(123), U256::from(456));
        let (key1, value1) = (U256::from(789), U256::from(999));
        let (key2, value2) = (U256::from(999), U256::from(111));
        init_state.insert_account_storage(account, key0, value0).unwrap();
        init_state.insert_account_storage(account, key1, value1).unwrap();

        let mut new_state = LoomDB::new().with_ro_db(Some(init_state)).with_ext_db(EmptyDBTyped::<ErrReport>::new());
        assert_eq!(new_state.accounts.len(), 0);

        new_state.insert_account_info(
            account,
            AccountInfo {
                balance: U256::ZERO,
                code: Some(Bytecode::new_raw(Bytes::from(vec![1, 2, 2]))),
                nonce: nonce + 1,
                code_hash: KECCAK_EMPTY,
            },
        );

        new_state.insert_account_info(
            account2,
            AccountInfo {
                balance: U256::ZERO,
                code: Some(Bytecode::new_raw(Bytes::from(vec![1, 2, 2]))),
                nonce: nonce + 1,
                code_hash: KECCAK_EMPTY,
            },
        );

        new_state.insert_account_storage(account, key0, U256::from(333)).unwrap();
        new_state.insert_account_storage(account, key2, value2).unwrap();

        let mut new_state = new_state.merge_cells();

        assert_eq!(new_state.basic(account).unwrap().unwrap().code, Some(Bytecode::new_raw(Bytes::from(vec![1, 2, 2]))));
        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce + 1);
        assert_eq!(new_state.storage_ref(account, key0).unwrap(), U256::from(333));
        assert_eq!(new_state.storage_ref(account, key1).unwrap(), value1);
        assert_eq!(new_state.storage_ref(account, key2).unwrap(), U256::ZERO);
        assert_eq!(new_state.storage_ref(account2, key0).unwrap(), U256::ZERO);
        assert_eq!(new_state.accounts.len(), 1);
        assert_eq!(new_state.basic(account2).unwrap(), None);
        assert_eq!(new_state.accounts.len(), 2);
        assert_eq!(new_state.basic(account2).unwrap(), None);
        assert_eq!(new_state.accounts.len(), 2);
    }

    #[cfg(feature = "serde-json")]
    #[test]
    fn test_serialize_deserialize_cachedb() {
        let account = Address::with_last_byte(69);
        let nonce = 420;
        let mut init_state = LoomDB::new();
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let serialized = serde_json::to_string(&init_state).unwrap();
        let deserialized: LoomDB = serde_json::from_str(&serialized).unwrap();

        assert!(deserialized.accounts.contains_key(&account));
        assert_eq!(deserialized.accounts.get(&account).unwrap().info.nonce, nonce);
    }
}
