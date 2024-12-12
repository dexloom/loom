use std::vec::Vec;

use alloy::primitives::map::{Entry, HashMap};
use alloy::primitives::BlockNumber;
use alloy::{
    consensus::constants::KECCAK_EMPTY,
    primitives::{Address, Log, B256, U256},
};
use revm::db::AccountState;
use revm::primitives::{Account, AccountInfo, Bytecode};
use revm::{Database, DatabaseCommit, DatabaseRef};

use crate::fast_hasher::SimpleBuildHasher;

/// A [Database] implementation that stores all state changes in memory.
///
/// This implementation wraps a [DatabaseRef] that is used to load data ([AccountInfo]).
///
/// Accounts and code are stored in two separate maps, the `accounts` map maps addresses to [FastDbAccount],
/// whereas contracts are identified by their code hash, and are stored in the `contracts` map.
/// The [FastDbAccount] holds the code hash of the contract, which is used to look up the contract in the `contracts` map.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FastCacheDB<ExtDB> {
    /// Account info where None means it is not existing. Not existing state is needed for Pre TANGERINE forks.
    /// `code` is always `None`, and bytecode can be found in `contracts`.
    pub accounts: HashMap<Address, FastDbAccount>,
    /// Tracks all contracts by their code hash.
    pub contracts: HashMap<B256, Bytecode, SimpleBuildHasher>,
    /// All logs that were committed via [DatabaseCommit::commit].
    pub logs: Vec<Log>,
    /// All cached block hashes from the [DatabaseRef].
    pub block_hashes: HashMap<BlockNumber, B256>,
    /// The underlying database ([DatabaseRef]) that is used to load data.
    ///
    /// Note: this is read-only, data is never written to this database.
    pub db: ExtDB,
}

impl<ExtDB: Default + DatabaseRef> Default for FastCacheDB<ExtDB> {
    fn default() -> Self {
        Self::new(ExtDB::default())
    }
}

impl<ExtDB> FastCacheDB<ExtDB> {
    pub fn new(db: ExtDB) -> Self
    where
        ExtDB: DatabaseRef,
    {
        let mut contracts = HashMap::with_hasher(SimpleBuildHasher::default());
        contracts.insert(KECCAK_EMPTY, Bytecode::default());
        contracts.insert(B256::ZERO, Bytecode::default());
        Self { accounts: HashMap::default(), contracts, logs: Vec::default(), block_hashes: HashMap::default(), db }
    }

    /// Inserts the account's code into the cache.
    ///
    /// Accounts objects and code are stored separately in the cache, this will take the code from the account and instead map it to the code hash.
    ///
    /// Note: This will not insert into the underlying external database.
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
}

impl<ExtDB: DatabaseRef> FastCacheDB<ExtDB> {
    /// Returns the account for the given address.
    ///
    /// If the account was not found in the cache, it will be loaded from the underlying database.
    pub fn load_account(&mut self, address: Address) -> Result<&mut FastDbAccount, ExtDB::Error> {
        let db = &self.db;
        match self.accounts.entry(address) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => Ok(entry.insert(
                db.basic_ref(address)?
                    .map(|info| FastDbAccount { info, ..Default::default() })
                    .unwrap_or_else(FastDbAccount::new_not_existing),
            )),
        }
    }

    /// insert account storage without overriding account info
    pub fn insert_account_storage(&mut self, address: Address, slot: U256, value: U256) -> Result<(), ExtDB::Error> {
        let account = self.load_account(address)?;
        account.storage.insert(slot, value);
        Ok(())
    }

    /// replace account storage without overriding account info
    pub fn replace_account_storage(&mut self, address: Address, storage: HashMap<U256, U256>) -> Result<(), ExtDB::Error> {
        let account = self.load_account(address)?;
        account.account_state = AccountState::StorageCleared;
        account.storage = storage.into_iter().collect();
        Ok(())
    }
}

impl<ExtDB> DatabaseCommit for FastCacheDB<ExtDB> {
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        for (address, mut account) in changes {
            if !account.is_touched() {
                continue;
            }
            if account.is_selfdestructed() {
                let db_account = self.accounts.entry(address).or_default();
                db_account.storage.clear();
                db_account.account_state = AccountState::NotExisting;
                db_account.info = AccountInfo::default();
                continue;
            }
            let is_newly_created = account.is_created();
            self.insert_contract(&mut account.info);

            let db_account = self.accounts.entry(address).or_default();
            db_account.info = account.info;

            db_account.account_state = if is_newly_created {
                db_account.storage.clear();
                AccountState::StorageCleared
            } else if db_account.account_state.is_storage_cleared() {
                // Preserve old account state if it already exists
                AccountState::StorageCleared
            } else {
                AccountState::Touched
            };
            db_account.storage.extend(account.storage.into_iter().map(|(key, value)| (key, value.present_value())));
        }
    }
}

impl<ExtDB: DatabaseRef> Database for FastCacheDB<ExtDB> {
    type Error = ExtDB::Error;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let basic = match self.accounts.entry(address) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(
                self.db
                    .basic_ref(address)?
                    .map(|info| FastDbAccount { info, ..Default::default() })
                    .unwrap_or_else(FastDbAccount::new_not_existing),
            ),
        };
        Ok(basic.info())
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        match self.contracts.entry(code_hash) {
            Entry::Occupied(entry) => Ok(entry.get().clone()),
            Entry::Vacant(entry) => {
                // if you return code bytes when basic fn is called this function is not needed.
                Ok(entry.insert(self.db.code_by_hash_ref(code_hash)?).clone())
            }
        }
    }

    /// Get the value in an account's storage slot.
    ///
    /// It is assumed that account is already loaded.
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        match self.accounts.entry(address) {
            Entry::Occupied(mut acc_entry) => {
                let acc_entry = acc_entry.get_mut();
                match acc_entry.storage.entry(index) {
                    Entry::Occupied(entry) => Ok(*entry.get()),
                    Entry::Vacant(entry) => {
                        if matches!(acc_entry.account_state, AccountState::StorageCleared | AccountState::NotExisting) {
                            Ok(U256::ZERO)
                        } else {
                            let slot = self.db.storage_ref(address, index)?;
                            entry.insert(slot);
                            Ok(slot)
                        }
                    }
                }
            }
            Entry::Vacant(acc_entry) => {
                // acc needs to be loaded for us to access slots.
                let info = self.db.basic_ref(address)?;
                let (account, value) = if info.is_some() {
                    let value = self.db.storage_ref(address, index)?;
                    let mut account: FastDbAccount = info.into();
                    account.storage.insert(index, value);
                    (account, value)
                } else {
                    (info.into(), U256::ZERO)
                };
                acc_entry.insert(account);
                Ok(value)
            }
        }
    }

    fn block_hash(&mut self, number: BlockNumber) -> Result<B256, Self::Error> {
        match self.block_hashes.entry(number) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let hash = self.db.block_hash_ref(number)?;
                entry.insert(hash);
                Ok(hash)
            }
        }
    }
}

impl<ExtDB: DatabaseRef> DatabaseRef for FastCacheDB<ExtDB> {
    type Error = ExtDB::Error;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        match self.accounts.get(&address) {
            Some(acc) => Ok(acc.info()),
            None => self.db.basic_ref(address),
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        match self.contracts.get(&code_hash) {
            Some(entry) => Ok(entry.clone()),
            None => self.db.code_by_hash_ref(code_hash),
        }
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        match self.accounts.get(&address) {
            Some(acc_entry) => match acc_entry.storage.get(&index) {
                Some(entry) => Ok(*entry),
                None => {
                    if matches!(acc_entry.account_state, AccountState::StorageCleared | AccountState::NotExisting) {
                        Ok(U256::ZERO)
                    } else {
                        self.db.storage_ref(address, index)
                    }
                }
            },
            None => self.db.storage_ref(address, index),
        }
    }

    fn block_hash_ref(&self, number: BlockNumber) -> Result<B256, Self::Error> {
        match self.block_hashes.get(&number) {
            Some(entry) => Ok(*entry),
            None => self.db.block_hash_ref(number),
        }
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FastDbAccount {
    pub info: AccountInfo,
    /// If account is selfdestructed or newly created, storage will be cleared.
    pub account_state: AccountState,
    /// storage slots
    pub storage: HashMap<U256, U256, SimpleBuildHasher>,
}

impl FastDbAccount {
    pub fn new_not_existing() -> Self {
        Self { account_state: AccountState::NotExisting, ..Default::default() }
    }

    pub fn info(&self) -> Option<AccountInfo> {
        if matches!(self.account_state, AccountState::NotExisting) {
            None
        } else {
            Some(self.info.clone())
        }
    }
}

impl From<Option<AccountInfo>> for FastDbAccount {
    fn from(from: Option<AccountInfo>) -> Self {
        from.map(Self::from).unwrap_or_else(Self::new_not_existing)
    }
}

impl From<AccountInfo> for FastDbAccount {
    fn from(info: AccountInfo) -> Self {
        Self { info, account_state: AccountState::None, ..Default::default() }
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::map::HashMap;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::FastCacheDB;
    use crate::in_memory_db::LoomInMemoryDB;
    use alloy::primitives::{Bytes, B256};
    use alloy::rpc::types::trace::geth::AccountState as GethAccountState;
    use revm::db::EmptyDB;
    use revm::primitives::{db::Database, AccountInfo, Address, Bytecode, I256, KECCAK_EMPTY, U256};
    use revm::DatabaseRef;

    #[test]
    fn test_insert_account_storage() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let mut init_state = FastCacheDB::new(EmptyDB::default());
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let (key, value) = (U256::from(123), U256::from(456));
        let mut new_state = FastCacheDB::new(init_state);
        new_state.insert_account_storage(account, key, value).unwrap();

        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce);
        assert_eq!(new_state.storage(account, key), Ok(value));
    }

    #[test]
    fn test_insert_account_storage_inherited() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let mut init_state = FastCacheDB::new(EmptyDB::default());
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let (key, value) = (U256::from(123), U256::from(456));
        let mut new_state = FastCacheDB::new(init_state);
        new_state.insert_account_storage(account, key, value).unwrap();

        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce);
        assert_eq!(new_state.storage(account, key), Ok(value));
    }

    #[test]
    fn test_replace_account_storage() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let mut init_state = FastCacheDB::new(EmptyDB::default());
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let (key0, value0) = (U256::from(123), U256::from(456));
        let (key1, value1) = (U256::from(789), U256::from(999));
        init_state.insert_account_storage(account, key0, value0).unwrap();

        let mut new_state = LoomInMemoryDB::new(Arc::new(init_state));
        assert_eq!(new_state.accounts.len(), 0);
        let mut hm: HashMap<U256, U256> = Default::default();
        hm.insert(key1, value1);

        new_state.replace_account_storage(account, hm).unwrap();

        let mut new_state = new_state.merge();

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
        let mut init_state = FastCacheDB::new(EmptyDB::new());
        init_state.insert_account_info(account, AccountInfo { nonce, code: Some(code.clone()), ..Default::default() });

        let (key0, value0) = (U256::from(123), U256::from(456));
        let (key1, value1) = (U256::from(789), U256::from(999));
        init_state.insert_account_storage(account, key0, value0).unwrap();
        init_state.insert_account_storage(account, key1, value1).unwrap();

        let mut new_state = LoomInMemoryDB::new(Arc::new(init_state));
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
        assert_eq!(new_state.storage_ref(account, key0), Ok(U256::from(333)));
        assert_eq!(new_state.storage_ref(account, key1), Ok(value1));
        assert_eq!(new_state.accounts.len(), 1);

        let mut new_state = new_state.merge();

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
        let mut init_state = FastCacheDB::new(EmptyDB::default());
        init_state.insert_account_info(account, AccountInfo { nonce, code: Some(code.clone()), ..Default::default() });

        let (key0, value0) = (U256::from(123), U256::from(456));
        let (key1, value1) = (U256::from(789), U256::from(999));
        let (key2, value2) = (U256::from(999), U256::from(111));
        init_state.insert_account_storage(account, key0, value0).unwrap();
        init_state.insert_account_storage(account, key1, value1).unwrap();

        let mut new_state = LoomInMemoryDB::new(Arc::new(init_state));
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

        let mut new_state = new_state.merge();

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
        let mut init_state = FastCacheDB::new(EmptyDB::default());
        init_state.insert_account_info(account, AccountInfo { nonce, code: Some(code.clone()), ..Default::default() });

        let (key0, value0) = (U256::from(123), U256::from(456));
        let (key1, value1) = (U256::from(789), U256::from(999));
        let (key2, value2) = (U256::from(999), U256::from(111));
        init_state.insert_account_storage(account, key0, value0).unwrap();
        init_state.insert_account_storage(account, key1, value1).unwrap();

        let mut new_state = LoomInMemoryDB::new(Arc::new(init_state));
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

        let mut new_state = new_state.update_cells();

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
        let mut init_state = FastCacheDB::new(EmptyDB::default());
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let serialized = serde_json::to_string(&init_state).unwrap();
        let deserialized: FastCacheDB<EmptyDB> = serde_json::from_str(&serialized).unwrap();

        assert!(deserialized.accounts.contains_key(&account));
        assert_eq!(deserialized.accounts.get(&account).unwrap().info.nonce, nonce);
    }
}
