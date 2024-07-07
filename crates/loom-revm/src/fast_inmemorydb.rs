use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

use alloy::{
    primitives::U256,
    rpc::types::trace::geth::AccountState,
};
use log::trace;
use revm::{Database, DatabaseCommit, DatabaseRef, InMemoryDB};
use revm::db::{AccountState as DbAccountState, CacheDB, EmptyDB};
use revm::precompile::{Address, B256};
use revm::primitives::{Account, AccountInfo, Bytecode, KECCAK_EMPTY};
use revm::primitives::bitvec::macros::internal::funty::Fundamental;

#[derive(Default, Clone)]
pub struct FastInMemoryDb<ExtDB>(CacheDB<ExtDB>);


impl<ExtDB> FastInMemoryDb<ExtDB>
where
    ExtDB: DatabaseRef<Error=Infallible> + Send + Sync + Clone + 'static,
{
    pub fn with_ext_db(ext_db: ExtDB) -> Self {
        FastInMemoryDb(CacheDB::new(ext_db))
    }

    pub fn with_ext_db_and_update(in_memory_db: ExtDB, update: Vec<BTreeMap<Address, AccountState>>) -> Self {
        let mut update_db = FastInMemoryDb(CacheDB::new(in_memory_db));
        update_db.apply_state_update(update, false, false);
        update_db
    }

    pub fn apply_account_info_btree(&mut self, address: Address, account_updated_state: AccountState, insert: bool, only_new: bool) {
        let account = self.load_account(address);

        if let Ok(account) = account {
            if insert
                || ((account.account_state == DbAccountState::NotExisting || account.account_state == DbAccountState::None) && only_new)
                || (!only_new && (account.account_state == DbAccountState::Touched || account.account_state == DbAccountState::StorageCleared))
            {
                account.account_state = DbAccountState::Touched;
                let code: Option<Bytecode> = match account_updated_state.code {
                    Some(c) => {
                        if c.len() < 2 {
                            account.info.code.clone()
                        } else {
                            Some(
                                Bytecode::new_raw(
                                    c
                                )
                            )
                        }
                    }
                    None => {
                        account.info.code.clone()
                    }
                };

                //trace!("apply_account_info {address}.  code len: {} storage len: {}", code.clone().map_or(0, |x| x.len()), account.storage.len()  );

                let account_info = AccountInfo {
                    balance: account_updated_state.balance.unwrap_or_default(),
                    nonce: account_updated_state.nonce.unwrap_or_default().as_u64(),
                    code_hash: if code.is_some() { KECCAK_EMPTY } else { Default::default() },
                    code,
                };


                self.insert_account_info(address, account_info);
                //trace!("after apply_account_info account: {address} state: {:?} storage len: {} code len : {}", account.account_state, account.storage.len(), account.info.code.clone().map_or(0, |c| c.len())  );
            } else {
                trace!("apply_account_info exists {address}. storage len: {}", account.storage.len(),   );
            }
        }
    }


    pub fn apply_account_storage(&mut self, address: Address, acc_state: AccountState, insert: bool, only_new: bool) {
        if insert {
            for (slot, value) in acc_state.storage.iter() {
                trace!("Inserting storage {address:?} slot : {slot:?} value : {value:?}");
                let _ = self.insert_account_storage(address, (*slot).into(), (*value).into());
            }
        } else {
            let account = self.load_account(address).cloned().unwrap();
            for (slot, value) in acc_state.storage.into_iter() {
                let is_slot = account.storage.contains_key::<U256>(&slot.into());
                if is_slot && !only_new {
                    let _ = self.insert_account_storage(address, slot.into(), value.into());
                    trace!("Inserting storage {address:?} slot : {slot:?} value : {value:?}");
                } else if !is_slot && only_new {
                    let _ = self.insert_account_storage(address, slot.into(), value.into());
                    trace!("Inserting storage {address:?} slot : {slot:?} value : {value:?}");
                }
            }
        }
    }

    pub fn apply_state_update(&mut self, update_vec: Vec<BTreeMap<Address, AccountState>>, insert: bool, only_new: bool) -> &mut Self {
        for update_record in update_vec {
            for (address, acc_state) in update_record {
                trace!("updating {address} insert: {insert} only_new: {only_new} storage len {} code: {}", acc_state.storage.len(), acc_state.code.is_some()  );

                let acc_state_no_storage = AccountState {
                    storage: BTreeMap::new(),
                    ..acc_state.clone()
                };
                self.apply_account_info_btree(address, acc_state_no_storage, insert, only_new);

                self.apply_account_storage(address, acc_state, insert, only_new);
            }
        }
        self
    }
}


impl<ExtDB> Deref for FastInMemoryDb<ExtDB>
where
    ExtDB: DatabaseRef + Send + Sync + Clone + 'static,
{
    type Target = CacheDB<ExtDB>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<ExtDB> DerefMut for FastInMemoryDb<ExtDB>
where
    ExtDB: DatabaseRef + Send + Sync + Clone + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}


impl<ExtDB> Database for FastInMemoryDb<ExtDB>
where
    ExtDB: DatabaseRef<Error=Infallible> + Send + Sync + Clone + 'static,
{
    type Error = Infallible;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.basic(address)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0.code_by_hash(code_hash)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.0.storage(address, index)
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.0.block_hash(number)
    }
}

impl<ExtDB> DatabaseRef for FastInMemoryDb<ExtDB>
where
    ExtDB: DatabaseRef<Error=Infallible> + Send + Sync + Clone + 'static,
{
    type Error = Infallible;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.basic_ref(address)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0.code_by_hash_ref(code_hash)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.0.storage_ref(address, index)
    }

    fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
        self.0.block_hash_ref(number)
    }
}

impl<ExtDB> DatabaseCommit for FastInMemoryDb<ExtDB>
where
    ExtDB: DatabaseRef + Send + Sync + Clone + 'static,
{
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        self.0.commit(changes)
    }
}

