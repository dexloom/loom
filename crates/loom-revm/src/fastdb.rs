use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use alloy_primitives::U256;
use alloy_rpc_types_trace::geth::AccountState;
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

#[derive(Clone, Default)]
pub struct FastDB {
    in_memory_db: InMemoryDB,
    update: BTreeMap<Address, AccountState>,
}

impl FastDB {
    pub fn empty() -> FastDB {
        FastDB {
            in_memory_db: InMemoryDB::new(EmptyDB::new()),
            update: BTreeMap::new(),
        }
    }

    pub fn with_db(self, in_memory_db: InMemoryDB) -> FastDB {
        FastDB {
            in_memory_db,
            ..self
        }
    }

    pub fn with_update(self, update: BTreeMap<Address, AccountState>) -> FastDB {
        FastDB {
            update,
            ..self
        }
    }

    pub fn with_update_vec(self, update_vec: Vec<BTreeMap<Address, AccountState>>) -> FastDB {
        let mut update: BTreeMap<Address, AccountState> = BTreeMap::new();

        for update_entry in update_vec {
            for (account, update_state) in update_entry {
                let mut account_state = update.entry(account).or_default();
                if update_state.code.is_some() {
                    account_state.code = update_state.code
                }
                if update_state.balance.is_some() {
                    account_state.balance = update_state.balance
                }
                if update_state.nonce.is_some() {
                    account_state.nonce = update_state.nonce
                }
                for (slot, value) in update_state.storage {
                    account_state.storage.insert(slot, value);
                }
            }
        }

        FastDB {
            update,
            ..self
        }
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

#[cfg(test)]
mod tests {
    use alloy_primitives::Bytes;
    use rand::{Rng, thread_rng};
    use revm::db::DbAccount;
    use revm::primitives::KECCAK_EMPTY;

    use super::*;

    fn generate_account(mem_size: usize) -> DbAccount {
        let mut rng = thread_rng();
        let mut storage: HashMap<U256, U256> = HashMap::new();
        for _j in 0..mem_size {
            storage.insert(rng.gen::<U256>(), rng.gen::<U256>());
        }

        let code = rng.gen::<U256>();

        let info = AccountInfo::new(U256::ZERO, 0, KECCAK_EMPTY, Bytecode::new_raw(Bytes::from(code.to_be_bytes_vec())));

        let acc = DbAccount {
            info,
            account_state: DbAccountState::Touched,
            storage,
        };
        acc
    }

    fn generate_accounts(acc_size: usize, mem_size: usize) -> Vec<DbAccount> {
        let mut rng = thread_rng();
        let mut ret: Vec<DbAccount> = Vec::new();
        for _i in 0..acc_size {
            ret.push(generate_account(mem_size));
        }
        ret
    }

    #[test]
    fn test_speed() {
        let mut db0 = CacheDB::new(EmptyDB::new());

        let n = 100000;
        let n2 = 1000;

        let accs = generate_accounts(n, 100);
        let addr: Vec<Address> = (0..n).map(|x| Address::random()).collect();


        for a in 0..n {
            db0.accounts.insert(addr[a], accs[a].clone());
        }

        let accs2 = generate_accounts(n2, 100);
        let start_time = chrono::Local::now();

        for (i, a) in accs2.into_iter().enumerate() {
            db0.accounts.insert(addr[i], a);
        }


        println!("Write {n2} {}", chrono::Local::now() - start_time);

        let start_time = chrono::Local::now();

        for i in 0..n2 {
            let acc = db0.load_account(addr[i]).unwrap();
        }

        println!("Read {n2} {}", chrono::Local::now() - start_time);

        let mut db1 = FastInMemoryDb::with_ext_db(Arc::new(db0));

        let start_time = chrono::Local::now();

        for i in 0..n2 {
            let acc = db1.load_account(addr[i]).unwrap();
        }

        println!("Read known {n2} {}", chrono::Local::now() - start_time);

        let start_time = chrono::Local::now();

        for i in n2..n2 + n2 {
            let acc = db1.load_account(addr[i]).unwrap();
        }

        println!("Read unknown {n2} {}", chrono::Local::now() - start_time);
    }

    #[test]
    fn db_read() {
        let acc = generate_account(100);
        let addr = Address::random();
        let mut db0 = CacheDB::new(EmptyDB::new());
        db0.accounts.insert(addr, acc.clone());
        let mut db1 = FastInMemoryDb::with_ext_db(Arc::new(db0));
        let acc_ref = db1.basic_ref(addr).unwrap();
        println!("{:?}", acc_ref);
        let acc_ref = db1.basic_ref(Address::random()).unwrap();
        println!("{:?}", acc_ref);
    }
}
